//! The window, where there is no terminal to move in. One stands somewhere and is shown what is there, a
//! mark before everything one can stand on; one says a mark to stand on what it marks, a lens by its name
//! to look through it from where one stands, an entry by its name to make one from there, back, or up -
//! words that were just shown, never a flag to remember. What is shown is what the lenses bring into
//! focus - around the thing stood on, or what the place shows first - laid out by a surface a lens says;
//! what was marked is kept beside the trail, so that the mark said next names what was shown. What a
//! window shows is decided by lenses; this carries what is said, and what an entry is given.

use anyhow::{bail, Result};
use std::path::{Path, PathBuf};
use tad::sql::SqlEngine;

/// The surface the window is laid out on where there is no terminal: said by a lens.
const SURFACE: &str = "as-window";

/// What `who` says in the window, standing where they stand among what is in view from `dir` - and what
/// is then shown. Nothing said: where one stands. A mark: one stands on what it marked. A lens by its
/// name: it is looked through, from where one stands if it looks from a thing. An entry by its name: what
/// it asks, from where one stands - or, with what it asks `given`, the entry is made, and one stands on
/// it. Back: where one stood before. Up: one stands on nothing, and is shown what the place shows first.
pub fn said(
    words: &[String],
    given: &dyn Fn() -> String,
    dir: &Path,
    who: &str,
    today: &str,
    engine: &dyn SqlEngine,
) -> Result<String> {
    let dirs = crate::board::in_view(dir).0;
    let mut through: Option<String> = None;
    let mut entered = String::new();
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
        word if crate::lenses::with(&dirs)?
            .iter()
            .any(|l| l.lens.id == word) =>
        {
            through = Some(word.to_string());
        }
        word => {
            match an_entry(
                word,
                &given(),
                crate::trail::stands(who).as_deref(),
                &crate::trail::marked(who),
                &dirs,
                today,
            )? {
                Came::Shown(asks) => return Ok(asks),
                Came::Made { thing, says } => {
                    crate::trail::step(who, &thing);
                    entered = says;
                }
            }
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
    Ok(format!(
        "{entered}{}",
        crate::dome::surface("lines", 80)?.lay_out(&laid)
    ))
}

/// What came of saying an entry's name: what it asks, shown - or the entry, made.
enum Came {
    Shown(String),
    Made { thing: String, says: String },
}

/// What was given for an entry: each FIELD: words - a line that names no field of it goes on with the
/// field before.
fn given_as_fields(given: &str, known: &[String]) -> Vec<(String, String)> {
    let mut fields: Vec<(String, String)> = Vec::new();
    for line in given.lines() {
        let named = line
            .split_once(':')
            .filter(|(field, _)| known.iter().any(|k| k == field.trim()));
        match named {
            Some((field, words)) => {
                fields.push((field.trim().to_string(), words.trim().to_string()))
            }
            None => {
                if let Some((_, words)) = fields.last_mut().filter(|_| !line.trim().is_empty()) {
                    if !words.is_empty() {
                        words.push(' ');
                    }
                    words.push_str(line.trim());
                }
            }
        }
    }
    fields
}

/// The place of that name among those in view - by its id or by its star - and the folder it is held in.
fn place_called(name: &str, dirs: &[PathBuf]) -> Option<(String, PathBuf)> {
    dirs.iter().find_map(|dir| {
        let text = std::fs::read_to_string(dir.join("oco.toml")).ok()?;
        let id = crate::mark::Mark::parse(&text).ok()?.project.id;
        (id == name || id.rsplit('/').next() == Some(name)).then(|| (id, dir.clone()))
    })
}

/// An entry by its name, said standing on a thing: with nothing given, what it asks, from there; with
/// what it asks given, it is made - by the entry itself, as it is made anywhere - where the thing stood
/// on is held, or in the place given. A field that points at things takes `here` for where one stands,
/// and a mark for what it marks in what was last shown.
fn an_entry(
    name: &str,
    given: &str,
    stands: Option<&str>,
    marked: &[String],
    dirs: &[PathBuf],
    today: &str,
) -> Result<Came> {
    let elsewhere = given.lines().find_map(|line| {
        line.strip_prefix("in:")
            .map(|place| place.trim().to_string())
    });
    let of_here = stands
        .and_then(|thing| thing.split_once(':'))
        .and_then(|(held, _)| held.rsplit_once('/'))
        .map(|(place, _)| place.to_string());
    let Some(wanted) = elsewhere.or(of_here) else {
        bail!("{name} is no mark and no lens in view - and standing on nothing, nothing is entered: stand on what it is to say something of");
    };
    let Some((place, dir)) = place_called(&wanted, dirs) else {
        bail!("no place in view is called {wanted}");
    };
    let Some((lens, entry)) = crate::entries::entry_in(name, &dir) else {
        bail!("{name} is no mark, no lens in view, and no entry that is made in {place}: ocoeco, alone, shows where you stand and what can be entered from there");
    };
    let all = crate::entries::fields(&lens, &entry.table);
    let known: Vec<String> = all
        .iter()
        .map(|(field, _, _)| field.clone())
        .chain(["in".to_string()])
        .collect();
    let fields = given_as_fields(given, &known);
    // which of its fields point at things: those a relation in view is declared from
    let pointing: Vec<String> = crate::lenses::with(dirs)?
        .iter()
        .flat_map(|l| l.relation.iter())
        .filter_map(|r| {
            r.from
                .split_once('.')
                .filter(|(table, _)| *table == entry.table)
                .map(|(_, field)| field.to_string())
        })
        .collect();
    if fields.iter().all(|(field, _)| field == "in") {
        let also: Vec<&str> = all
            .iter()
            .map(|(field, _, _)| field.as_str())
            .filter(|field| *field != "id" && !entry.asks.iter().any(|a| a == field))
            .collect();
        return Ok(Came::Shown(format!(
            "{name} - {}\n  it asks: {}\n  it may also carry: {}\n  of these, what points at things: {} - there, here names where you stand{}, and a mark what it marks\n  it would be kept in {place}\n- say {name} again with what it asks beneath it, each as FIELD: words on lines of its own; in: PLACE keeps it elsewhere, id: NAME names it\n",
            entry.says,
            entry.asks.join(", "),
            also.join(", "),
            pointing.join(", "),
            stands.map(|thing| format!(" ({thing})")).unwrap_or_default(),
        )));
    }
    let said = |field: &str| {
        fields
            .iter()
            .find(|(f, _)| f == field)
            .map(|(_, words)| words.clone())
    };
    // named by its first words where no name is given: never an id typed from memory
    let id = match said("id") {
        Some(id) => id,
        None => {
            let words = fields
                .iter()
                .find(|(field, _)| field != "in" && !pointing.contains(field))
                .map(|(_, words)| words.as_str())
                .unwrap_or_default();
            let slug = crate::manual::slug(
                &words
                    .split_whitespace()
                    .take(8)
                    .collect::<Vec<_>>()
                    .join(" "),
            );
            let dated = lens
                .tables
                .get(&entry.table)
                .and_then(|t| t.get("fields"))
                .and_then(|f| f.as_array())
                .into_iter()
                .flatten()
                .any(|f| {
                    f.get("name").and_then(|n| n.as_str()) == Some("id")
                        && f.get("pattern")
                            .and_then(|p| p.as_str())
                            .is_some_and(|p| p.starts_with("^[0-9]{4}"))
                });
            let id = if dated {
                format!("{today}-{slug}")
            } else {
                slug
            };
            if id.trim_matches('-').is_empty() {
                bail!("there is nothing to name this {name} by: give what it asks, or id: NAME");
            }
            if crate::routes::rows_by_place(&entry.table, std::slice::from_ref(&dir))
                .iter()
                .any(|(_, row)| row.get("id").and_then(|v| v.as_str()) == Some(id.as_str()))
            {
                bail!("{place} already keeps a {name} called {id}: give id: NAME to name this one - or id: {id}, to change that one");
            }
            id
        }
    };
    let mut asked: Vec<String> = vec![
        "glimpse".into(),
        name.into(),
        "--id".into(),
        id.clone(),
        "--dir".into(),
        dir.display().to_string(),
    ];
    for (field, words) in fields.iter().filter(|(f, _)| f != "in" && f != "id") {
        asked.push(format!("--{field}"));
        asked.push(if pointing.contains(field) {
            // in a field that points at things, here names where one stands, and a mark what it marks
            // in what was last shown: a thing is pointed at, never named from memory
            words
                .split_whitespace()
                .map(|word| {
                    let marks = word
                        .parse::<usize>()
                        .ok()
                        .and_then(|n| n.checked_sub(1))
                        .and_then(|at| marked.get(at));
                    match (word, stands, marks) {
                        ("here", Some(thing), _) => thing,
                        (_, _, Some(thing)) => thing.as_str(),
                        _ => word,
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            words.clone()
        });
    }
    // made by the entry itself, as it is made anywhere: asked, guarded and supplied the same
    // - and standing in the place it is kept in: an entry a place's own lens declares is known there
    let made = std::process::Command::new(std::env::current_exe()?)
        .args(&asked)
        .current_dir(&dir)
        .output()?;
    if !made.status.success() {
        let why = String::from_utf8_lossy(&made.stderr);
        bail!(
            "the {name} was not entered: {}",
            why.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join(" - ")
        );
    }
    let words = id
        .trim_start_matches(|c: char| c.is_ascii_digit() || c == '-')
        .replace('-', " ");
    Ok(Came::Made {
        thing: format!("{place}/{}:{id}", entry.table),
        says: format!("entered: {place} | a {name}: {words}\n"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_given_for_an_entry_is_read_field_by_field() {
        let known = ["saw".to_string(), "learned".to_string(), "in".to_string()];
        assert_eq!(
            given_as_fields(
                "saw: one thing\n  and more of it\n\nlearned: this: that\nin: o/r\n",
                &known
            ),
            [
                ("saw".to_string(), "one thing and more of it".to_string()),
                ("learned".to_string(), "this: that".to_string()),
                ("in".to_string(), "o/r".to_string()),
            ],
            "a line that names no field goes on with the field before; a colon in the words is words"
        );
    }
}
