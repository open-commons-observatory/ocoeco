//! The window, where there is no terminal to move in. One stands somewhere and is shown what is there, a
//! mark before everything one can stand on; one says a mark to stand on what it marks, a lens by its name
//! to look through it from where one stands, an entry by its name to make one from there, back, or up -
//! words that were just shown, never a flag to remember. What is shown is what the lenses bring into
//! focus - around the thing stood on, or what the place shows first - laid out by a surface a lens says;
//! what was marked is kept beside the trail, so that the mark said next names what was shown. What a
//! window shows is decided by lenses; this carries what is said, and what an entry is given.

use crate::lenses::Lens;
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};
use tad::sql::SqlEngine;

/// The surface the window is laid out on where there is no terminal: said by a lens.
const SURFACE: &str = "as-window";

/// The lens that says what an entry asks and the row it would keep.
const ENTERING: &str = "entering";

/// The entry said - by its name, or as the thing a mark marked - with the lens that declares it.
fn the_entry<'a>(said: &str, all: &'a [Lens]) -> Option<(&'a Lens, &'a crate::lenses::Entry)> {
    match said.rsplit_once("/lens_entries:") {
        Some((_, of)) => {
            let (lens, name) = of.split_once('/')?;
            let lens = all.iter().find(|l| l.lens.id == lens)?;
            Some((lens, lens.glimpse.iter().find(|e| e.entry == name)?))
        }
        None => all
            .iter()
            .find_map(|l| l.glimpse.iter().find(|e| e.entry == said).map(|e| (l, e))),
    }
}

/// What was given for an entry, kept as the lens of entering says: the lens is given the words, and says
/// each field and value of the row and the place it is kept in - or what is lacking; the entry itself then
/// makes the row, as it is made anywhere. Returns the thing entered, and what to say of it.
fn kept_as_the_lens_says(
    said: &str,
    given: &str,
    all: &[Lens],
    dirs: &[PathBuf],
    today: &str,
    engine: &dyn SqlEngine,
) -> Result<(String, String)> {
    let Some((lens, declared)) = the_entry(said, all) else {
        bail!("{said} is no entry a lens in view declares");
    };
    let name = declared.entry.as_str();
    let known: Vec<String> = crate::entries::fields(lens, &declared.table)
        .into_iter()
        .map(|(field, _, _)| field)
        .chain(["in".to_string()])
        .collect();
    // the lens is given the words for this one look, and says the row
    crate::focus::given_is(given_as_fields(given, &known));
    let says = crate::focus::brought(&format!("{ENTERING}@{said}"), dirs, today, engine);
    crate::focus::given_is(Vec::new());
    let says = says?;
    let rows = crate::focus::every_answer(&says);
    let words = |row: &serde_json::Map<String, serde_json::Value>, key: &str| {
        row.get(key).and_then(|v| v.as_str()).map(str::to_string)
    };
    let lacking: Vec<String> = rows
        .iter()
        .copied()
        .filter_map(|row| words(row, "lacking"))
        .collect();
    if !lacking.is_empty() {
        bail!("the {name} is not entered: {}", lacking.join("; "));
    }
    let keeps: Vec<(String, String)> = rows
        .iter()
        .copied()
        .filter_map(|row| Some((words(row, "keep_field")?, words(row, "keep_value")?)))
        .collect();
    let Some(id) = keeps
        .iter()
        .find(|(field, _)| field == "id")
        .map(|(_, id)| id.clone())
    else {
        bail!("the lens of entering names no row to keep: nothing was given that {name} asks");
    };
    let Some((place, dir)) = rows
        .iter()
        .copied()
        .find_map(|row| words(row, "kept_in"))
        .and_then(|place| place_called(&place, dirs))
    else {
        bail!("the lens of entering says no place in view to keep the {name} in: stand on what it is to say something of, or give in: PLACE");
    };
    let mut asked: Vec<String> = vec![
        "glimpse".into(),
        name.into(),
        "--id".into(),
        id.clone(),
        "--dir".into(),
        dir.display().to_string(),
    ];
    for (field, value) in keeps.iter().filter(|(field, _)| field != "id") {
        asked.push(format!("--{field}"));
        asked.push(value.clone());
    }
    // made by the entry itself, as it is made anywhere - standing in the place it is kept in
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
    let called = id
        .trim_start_matches(|c: char| c.is_ascii_digit() || c == '-')
        .replace('-', " ");
    Ok((
        format!("{place}/{}:{id}", declared.table),
        format!(
            "entered: {place} | {} {name}: {called}\n",
            if name.starts_with(['a', 'e', 'i', 'o', 'u']) {
                "an"
            } else {
                "a"
            }
        ),
    ))
}

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
    let mut dirs = crate::board::in_view(dir).0;
    let mut through: Option<String> = None;
    let mut entered = String::new();
    // an entry is said as the lens of entering says it - by its name, or by its mark where it is marked.
    // One does not stand on an entry: here stays where one stands
    let all = crate::lenses::with(&dirs)?;
    let mut an_entry_said: Option<String> = None;
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
            if thing.contains("/lens_entries:") {
                an_entry_said = Some(thing.clone());
            } else {
                crate::trail::step(who, thing);
            }
        }
        word if all.iter().any(|l| l.lens.id == word) => {
            through = Some(word.to_string());
        }
        word if the_entry(word, &all).is_some() => {
            an_entry_said = Some(word.to_string());
        }
        // a thing by its name - as it was shown, or as it is known: one stands on it, as on what a mark
        // marks; and an entry named so is said
        word if word.contains(':') => {
            let Some(thing) = crate::focus::Named::of(&dirs)?
                .names_of(word)
                .into_iter()
                .next()
            else {
                bail!("{word} is the name of no one thing in view: ocoeco, alone, shows where you stand and what can be said from there");
            };
            if thing.contains("/lens_entries:") {
                an_entry_said = Some(thing);
            } else {
                crate::trail::step(who, &thing);
            }
        }
        word => bail!(
            "{word} is no mark, no lens in view, no entry a lens in view declares and no thing by its name: ocoeco, alone, shows where you stand and what can be said from there"
        ),
    }
    // an entry said: with nothing given, what it asks is shown - through the lens of entering, from the
    // entry; with what it asks given, it is kept as that lens says, and one stands on what was made
    let mut asks: Option<String> = None;
    if let Some(entry) = an_entry_said {
        let given = given();
        if given.trim().is_empty() {
            asks = Some(entry);
        } else {
            // what is given may be for more than one entry: each is kept as the lens of entering says, one
            // after the other, and one stands on what was made last. Where one of them is not entered, what
            // came before it stays entered, and what was not is said
            for (at, (entry, given)) in given_entry_by_entry(&entry, &given, &all)
                .into_iter()
                .enumerate()
            {
                match kept_as_the_lens_says(&entry, &given, &all, &dirs, today, engine) {
                    Ok((thing, says)) => {
                        crate::trail::step(who, &thing);
                        entered.push_str(&says);
                        // a first step keeps the trail as a place of its own: what is in view is read
                        // again, so that here and back name what was just made in this same saying
                        dirs = crate::board::in_view(dir).0;
                        // what was entered may be a part of the language itself: it is read again, so
                        // that what is said and shown next is by the language as it now is
                        crate::lenses::as_kept_among(&dirs);
                    }
                    Err(e) if at == 0 => return Err(e),
                    Err(e) => {
                        entered.push_str(&format!("not entered: {e:#}\n"));
                        break;
                    }
                }
            }
        }
    }
    // what is looked through: the lens named, from where one stands if it looks from a thing; else what
    // is around the thing stood on; else what the place shows first
    let stands = crate::trail::stands(who);
    let looked = match (&asks, &through, &stands) {
        (Some(entry), _, _) => format!("{ENTERING}@{entry}"),
        (None, Some(lens), Some(thing)) if crate::focus::looks_from(lens, thing, &dirs) => {
            format!("{lens}@{thing}")
        }
        (None, Some(lens), _) => lens.clone(),
        (None, None, Some(thing)) => format!("around@{thing}"),
        (None, None, None) => {
            crate::lenses::whole_said_at(dir).unwrap_or_else(|| "fulldome".to_string())
        }
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
    // one thing marked once, by whichever of its names it was said first
    marked.sort();
    marked.dedup_by_key(|(mark, _)| *mark);
    let things: Vec<String> = marked.into_iter().map(|(_, thing)| thing).collect();
    crate::trail::mark(who, &things);
    let laid = crate::focus::brought(&format!("{SURFACE}@{looked}"), &dirs, today, engine)?;
    Ok(format!(
        "{entered}{}",
        crate::dome::surface("lines", 80)?.lay_out(&laid)
    ))
}

/// What was given, entry by entry: first for the entry said; then, after each line that says `and` and the
/// name of an entry in view, for that one.
fn given_entry_by_entry(said: &str, given: &str, all: &[Lens]) -> Vec<(String, String)> {
    let mut acts: Vec<(String, String)> = vec![(said.to_string(), String::new())];
    for line in given.lines() {
        let another = line
            .strip_prefix("and ")
            .map(str::trim)
            .filter(|name| !name.contains(' ') && the_entry(name, all).is_some());
        match another {
            Some(entry) => acts.push((entry.to_string(), String::new())),
            None => {
                if let Some((_, words)) = acts.last_mut() {
                    words.push_str(line);
                    words.push('\n');
                }
            }
        }
    }
    acts
}

/// What was given for an entry: each FIELD: words - a line that names no field of it goes on with the
/// field before, on a line of its own. What is given keeps its lines: a query stays the lines it was.
fn given_as_fields(given: &str, known: &[String]) -> Vec<(String, String)> {
    let mut fields: Vec<(String, String)> = Vec::new();
    for line in given.lines() {
        // a field is named at the start of a line, by its name and a colon - with its words after a space,
        // or none: a colon within the words, or doubled as a query writes it, is words
        let named = line.split_once(':').filter(|(field, words)| {
            known.iter().any(|k| k == field.trim()) && (words.is_empty() || words.starts_with(' '))
        });
        match named {
            Some((field, words)) => {
                fields.push((field.trim().to_string(), words.trim().to_string()))
            }
            None => {
                if let Some((_, words)) = fields.last_mut().filter(|_| !line.trim().is_empty()) {
                    if !words.is_empty() {
                        words.push('\n');
                    }
                    words.push_str(line.trim_end());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_given_may_be_for_more_than_one_entry() {
        let all = crate::lenses::lenses();
        assert_eq!(
            given_entry_by_entry(
                "sign",
                "what: one thing\nkind: lacking\nand connection\nfrom: here\nhow: led to\nand so on, which is words\nand nothing-so-called\n",
                &all
            ),
            [
                (
                    "sign".to_string(),
                    "what: one thing\nkind: lacking\n".to_string()
                ),
                (
                    "connection".to_string(),
                    "from: here\nhow: led to\nand so on, which is words\nand nothing-so-called\n"
                        .to_string()
                ),
            ],
            "after a line that says and, and the name of an entry, what follows is for that one; any other line is words"
        );
    }

    #[test]
    fn what_is_given_for_an_entry_is_read_field_by_field() {
        let known = ["saw".to_string(), "learned".to_string(), "in".to_string()];
        assert_eq!(
            given_as_fields(
                "saw: one thing\n  and more of it\n\nlearned: this: that\nsaw::not a field\nin: o/r\n",
                &known
            ),
            [
                ("saw".to_string(), "one thing\n  and more of it".to_string()),
                (
                    "learned".to_string(),
                    "this: that\nsaw::not a field".to_string()
                ),
                ("in".to_string(), "o/r".to_string()),
            ],
            "a line that names no field goes on with the field before, on a line of its own; a colon in the words is words"
        );
    }

    #[test]
    fn what_an_entry_would_keep_is_said_by_the_lens_of_entering() {
        let d = std::env::temp_dir().join(format!("ocoeco-entering-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("lenses")).unwrap();
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(
            d.join("oco.toml"),
            "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nnotes = \"data/notes.json\"\n",
        )
        .unwrap();
        std::fs::write(
            d.join("data/notes.json"),
            "{\"id\":\"2026-01-02-what-is-already-kept\",\"says\":\"What is already kept\"}\n",
        )
        .unwrap();
        // a place's own lens: a table whose rows are named by the day, an entry that makes one, and a
        // field of it that points at things
        std::fs::write(
            d.join("lenses/notes.toml"),
            "[lens]\nid = \"notes\"\nsays = \"Notes, and the entry that makes one.\"\n\n[tables.notes]\nkey = [\"id\"]\nfields = [\n  { name = \"id\", type = \"string\", required = true, pattern = \"^[0-9]{4}-[0-9]{2}-[0-9]{2}-[a-z0-9-]+$\" },\n  { name = \"says\", type = \"string\", required = true },\n  { name = \"about\", type = \"string\" },\n]\n\n[[relation]]\nfrom = \"notes.about\"\nto = [\"notes\"]\n\n[[glimpse]]\nentry = \"note\"\nsays = \"A note: what it says, and what it is about.\"\ntable = \"notes\"\nasks = [\"says\", \"about\"]\n\n[[focus]]\ntitle = \"notes ({count})\"\nsql = \"SELECT id, says FROM notes ORDER BY id\"\nline = \"{id} - {says}\"\n",
        )
        .unwrap();
        // held where there is an engine to ask: where there is none - the check on GitHub - it steps aside
        let Ok(engine) = tad::sql::DuckDb::find() else {
            eprintln!("no engine here: what the lens of entering says is held where there is one");
            return;
        };
        let dirs = [d.clone()];
        let says = |given: &[(&str, &str)]| -> Vec<serde_json::Map<String, serde_json::Value>> {
            crate::focus::given_is(
                given
                    .iter()
                    .map(|(field, words)| (field.to_string(), words.to_string()))
                    .collect(),
            );
            let parts =
                crate::focus::brought("entering@note", &dirs, "2026-01-02", &engine).unwrap();
            crate::focus::given_is(Vec::new());
            crate::focus::every_answer(&parts)
                .into_iter()
                .cloned()
                .collect()
        };
        let of = |rows: &[serde_json::Map<String, serde_json::Value>], key: &str| -> Vec<String> {
            rows.iter()
                .filter_map(|row| row.get(key).and_then(|v| v.as_str()).map(str::to_string))
                .collect()
        };
        // nothing given: what it asks, what its fields point at, and how it is said
        let asked = says(&[]);
        assert!(of(&asked, "keep_field").is_empty() && of(&asked, "lacking").is_empty());
        assert!(
            of(&asked, "how")
                .iter()
                .any(|how| how.contains("points at notes"))
                && of(&asked, "how")
                    .iter()
                    .any(|how| how.starts_with("- say note with what it asks")),
            "{asked:?}"
        );
        // given what it asks: the row - named by its first words, dated as its table names its rows, each
        // field as given - kept in the place that brings the entry
        let row = says(&[
            (
                "says",
                "A first thing, said plainly - and more words than seven are",
            ),
            ("about", "notes:2026-01-02-what-is-already-kept"),
        ]);
        assert_eq!(of(&row, "keep_field"), ["id", "says", "about"], "{row:?}");
        assert_eq!(
            of(&row, "keep_value"),
            [
                "2026-01-02-a-first-thing-said-plainly-and-more",
                "A first thing, said plainly - and more words than seven are",
                "notes:2026-01-02-what-is-already-kept"
            ]
        );
        assert_eq!(of(&row, "kept_in"), ["o/r", "o/r", "o/r"], "{row:?}");
        assert!(of(&row, "lacking").is_empty(), "{row:?}");
        // a name already kept is not taken by first words - given, it names the row to change
        let twice = says(&[("says", "What is already kept")]);
        assert!(
            of(&twice, "lacking")
                .iter()
                .any(|l| l.contains("already keeps one called 2026-01-02-what-is-already-kept")),
            "{twice:?}"
        );
        let changed = says(&[
            ("id", "2026-01-02-what-is-already-kept"),
            ("says", "Said anew"),
        ]);
        assert!(of(&changed, "lacking").is_empty(), "{changed:?}");
        assert_eq!(
            of(&changed, "keep_value"),
            ["2026-01-02-what-is-already-kept", "Said anew"]
        );
        // and with nothing to name it by, nothing is kept
        let nameless = says(&[("about", "notes:2026-01-02-what-is-already-kept")]);
        assert!(
            of(&nameless, "lacking")
                .iter()
                .any(|l| l.contains("nothing to name it by")),
            "{nameless:?}"
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
