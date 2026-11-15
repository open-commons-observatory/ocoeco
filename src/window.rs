//! The window, where there is no terminal to move in. One stands somewhere and is shown what is there, a
//! mark before every line about a thing; one says a mark to stand on what it marks, a lens by its name to
//! look through it from where one stands, back, or up - words that were just shown, never a flag to
//! remember. What is shown is what the lenses bring into focus - around the thing stood on, or what the
//! place shows first - laid out by a surface a lens says; what was marked is kept beside the trail, so
//! that the mark said next names what was shown. Nothing of what a window shows is decided here.

use anyhow::{bail, Result};
use std::path::Path;
use tad::sql::SqlEngine;

/// The surface the window is laid out on where there is no terminal: said by a lens.
const SURFACE: &str = "as-window";

/// What `who` says in the window, standing where they stand among what is in view from `dir` - and what
/// is then shown. Nothing said: where one stands. A mark: one stands on what it marked. A lens by its
/// name: it is looked through, from where one stands if it looks from a thing. Back: where one stood
/// before. Up: one stands on nothing, and is shown what the place shows first.
pub fn said(
    words: &[String],
    dir: &Path,
    who: &str,
    today: &str,
    engine: &dyn SqlEngine,
) -> Result<String> {
    let dirs = crate::board::in_view(dir).0;
    let mut through: Option<String> = None;
    match words.first().map(String::as_str).unwrap_or_default() {
        "" => {}
        "up" => crate::trail::leave(who),
        "back" => match crate::trail::before(who) {
            Some(thing) => crate::trail::step(who, &thing),
            None => bail!("there is no step before this one to go back to"),
        },
        mark if mark.chars().all(|c| c.is_ascii_digit()) => {
            let marked = crate::trail::marked(who);
            let Some(thing) = mark
                .parse::<usize>()
                .ok()
                .and_then(|n| n.checked_sub(1))
                .and_then(|at| marked.get(at))
            else {
                bail!("nothing is marked {mark} in what was last shown: ocoeco, alone, shows where you stand");
            };
            crate::trail::step(who, thing);
        }
        lens => {
            if !crate::lenses::with(&dirs)?
                .iter()
                .any(|l| l.lens.id == lens)
            {
                bail!("{lens} is no mark, and no lens in view is called so: ocoeco, alone, shows where you stand and what can be said from there");
            }
            through = Some(lens.to_string());
        }
    }
    // what is looked through: the lens named, from where one stands if it looks from a thing; else what
    // is around the thing stood on; else what the place shows first
    let stands = crate::trail::stands(who);
    let looked = match (&through, &stands) {
        (Some(lens), Some(thing)) if crate::focus::stands_somewhere(lens, &dirs) => {
            format!("{lens}@{thing}")
        }
        (Some(lens), _) => lens.clone(),
        (None, Some(thing)) => format!("around@{thing}"),
        (None, None) => crate::lenses::whole_said_at(dir).unwrap_or_else(|| "fulldome".to_string()),
    };
    // one look: what the lens brings is brought once, for its marks and for the surface that lays it out
    let _one_look = crate::relations::one_look();
    let parts = crate::focus::brought(&looked, &dirs, today, engine)?;
    let named = crate::focus::Named::of(&dirs)?;
    let mut marked: Vec<(u64, String)> = crate::focus::as_plane(&parts, &named)
        .iter()
        .filter_map(|row| {
            Some((
                row.get("mark")?.as_u64()?,
                row.get("thing")?.as_str()?.to_string(),
            ))
        })
        .collect();
    marked.sort();
    marked.dedup();
    let things: Vec<String> = marked.into_iter().map(|(_, thing)| thing).collect();
    crate::trail::mark(who, &things);
    let laid = crate::focus::brought(&format!("{SURFACE}@{looked}"), &dirs, today, engine)?;
    Ok(crate::dome::surface("lines", 80)?.lay_out(&laid))
}
