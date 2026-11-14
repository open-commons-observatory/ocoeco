//! What fits here: from a blank on a typed line, what may come next - the language asked the other way.
//! A lens says what comes into focus; asked from a blank, the same declaration says what may enter. An
//! entry's fields are what its lens declares; a field held to a set offers the set; a field that is a link
//! offers the rows that exist, as links; the id offers the rows that a change would be made to; a field of
//! free words offers nothing, and says so. Nothing here is written apart from the language: it is read.

use crate::entries::{entry, fields};
use crate::join::rows;
use crate::mark::Mark;
use std::path::Path;

/// A command of the program, as its declaration says: its name, what it is, its flags (the long name,
/// what it is, whether it takes a value) and its positionals' names, in order.
#[derive(Debug, Clone, Default)]
pub struct Command {
    pub name: String,
    pub is: String,
    pub flags: Vec<(String, String, bool)>,
    pub positionals: Vec<String>,
}

/// One thing that may come next: the word, and what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fit {
    pub word: String,
    pub is: String,
}

fn fit(word: impl Into<String>, is: impl Into<String>) -> Fit {
    Fit {
        word: word.into(),
        is: is.into(),
    }
}

/// What a row is, in a few words: the first of its fields that says something.
fn about(row: &serde_json::Value) -> String {
    let said = [
        "title", "what", "decision", "term", "says", "question", "name", "text", "practice",
    ]
    .iter()
    .find_map(|f| {
        row.get(*f)
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
    })
    .unwrap_or_default();
    let mut short: String = said.chars().take(70).collect();
    if said.chars().count() > 70 {
        short.push_str("...");
    }
    short
}

/// What may come next after `words` - what is typed after `ocoeco`, the last being the word being typed
/// (empty at a blank) - standing in the place at `dir`. `commands` are the program's own, with what each
/// is. Only what begins as the word being typed begins is offered.
pub fn after(words: &[String], dir: &Path, commands: &[Command]) -> Vec<Fit> {
    let typing = words.last().cloned().unwrap_or_default();
    let offered = match words {
        [] | [_] => commands
            .iter()
            .map(|c| fit(c.name.clone(), c.is.clone()))
            .collect(),
        [glimpse, _] if glimpse == "glimpse" => crate::lenses::lenses()
            .iter()
            .flat_map(|l| {
                l.glimpse
                    .iter()
                    .map(|e| fit(e.entry.clone(), e.says.clone()))
            })
            .collect(),
        [glimpse, name, rest @ ..] if glimpse == "glimpse" => match entry(name) {
            // a record of work, said in its own words, and the flags it takes
            None => commands
                .iter()
                .find(|c| c.name == "glimpse")
                .map(|c| command_fits(c, &words[1..], dir))
                .unwrap_or_default(),
            Some((lens, e)) => {
                let declared = fields(&lens, &e.table);
                let before = rest
                    .len()
                    .checked_sub(2)
                    .and_then(|at| rest.get(at))
                    .cloned()
                    .unwrap_or_default();
                match before.strip_prefix("--") {
                    // after a field's name: what that field may hold
                    Some(field) if field == "id" || e.asks.iter().any(|a| a == field) => {
                        values(field, &e.table, &declared, &lens, dir, &typing)
                    }
                    Some("dir") => Vec::new(),
                    // otherwise: the fields not yet given
                    _ => {
                        let given: Vec<&str> =
                            rest.iter().filter_map(|w| w.strip_prefix("--")).collect();
                        std::iter::once(("id".to_string(), true, Vec::new()))
                            .chain(e.asks.iter().map(|a| {
                                declared
                                    .iter()
                                    .find(|(n, _, _)| n == a)
                                    .cloned()
                                    .unwrap_or((a.clone(), false, Vec::new()))
                            }))
                            .filter(|(name, _, _)| !given.contains(&name.as_str()))
                            .map(|(name, needed, allowed)| {
                                let is = match (name.as_str(), needed, allowed.is_empty()) {
                                    ("id", _, _) => {
                                        "which one - a new id, or one that exists, to change it"
                                            .to_string()
                                    }
                                    (_, _, false) => format!("one of: {}", allowed.join(", ")),
                                    (_, true, true) => "needed for a new one".to_string(),
                                    _ => String::new(),
                                };
                                fit(format!("--{name}"), is)
                            })
                            .collect()
                    }
                }
            }
        },
        // any other command: what its declaration says it takes
        [name, rest @ ..] => commands
            .iter()
            .find(|c| &c.name == name)
            .map(|c| command_fits(c, rest, dir))
            .unwrap_or_default(),
    };
    offered
        .into_iter()
        .filter(|f| f.word.starts_with(typing.as_str()))
        .collect()
}

/// What may come next for a command: after a flag that takes a value, what that value may be; otherwise
/// what its next positional may be, and the flags not yet given.
fn command_fits(c: &Command, rest: &[String], dir: &Path) -> Vec<Fit> {
    let before = &rest[..rest.len().saturating_sub(1)];
    let typing = rest.last().cloned().unwrap_or_default();
    let takes = |flag: &str| c.flags.iter().any(|(f, _, value)| f == flag && *value);
    if let Some(flag) = before
        .last()
        .and_then(|w| w.strip_prefix("--"))
        .filter(|f| takes(f))
    {
        return value_named(flag, dir);
    }
    // the positionals given so far: words that are neither a flag nor what a flag was given
    let (mut given, mut skip) = (0, false);
    for word in before {
        if std::mem::take(&mut skip) {
            continue;
        }
        match word.strip_prefix("--") {
            Some(flag) => skip = takes(flag),
            None => given += 1,
        }
    }
    let mut offered = Vec::new();
    if !typing.starts_with('-') {
        if let Some(positional) = c.positionals.get(given) {
            offered.extend(value_named(positional, dir));
        }
    }
    let used: Vec<&str> = before.iter().filter_map(|w| w.strip_prefix("--")).collect();
    offered.extend(
        c.flags
            .iter()
            .filter(|(f, _, _)| !used.contains(&f.as_str()))
            .map(|(f, is, _)| fit(format!("--{f}"), is.clone())),
    );
    offered
}

/// What a value of this name may be, where the language or the program says: a lens in view, a surface, a
/// table this place keeps. Of any other, nothing is known here.
fn value_named(name: &str, dir: &Path) -> Vec<Fit> {
    match name {
        "lens" => crate::lenses::with(&crate::board::in_view(dir).0)
            .unwrap_or_default()
            .into_iter()
            .map(|l| fit(l.lens.id, l.lens.says))
            .collect(),
        "surface" => crate::dome::SURFACES.iter().map(|s| fit(*s, "")).collect(),
        "table" => std::fs::read_to_string(dir.join("oco.toml"))
            .ok()
            .and_then(|t| Mark::parse(&t).ok())
            .map(|m| m.exports.into_keys().map(|t| fit(t, "")).collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// What a field of an entry may hold, standing in the place at `dir`. A link may point into this place or
/// into another star in view - every place kept beside it - and each stroke narrows: the star, then its
/// kind of row, then the row.
fn values(
    field: &str,
    table: &str,
    declared: &[(String, bool, Vec<String>)],
    lens: &crate::lenses::Lens,
    dir: &Path,
    typing: &str,
) -> Vec<Fit> {
    let read = |d: &Path| {
        std::fs::read_to_string(d.join("oco.toml"))
            .ok()
            .and_then(|t| Mark::parse(&t).ok())
    };
    let mark = read(dir);
    let kept = |t: &str| mark.as_ref().map(|m| rows(dir, m, t)).unwrap_or_default();
    // the id: the rows a change would be made to - or a new one
    if field == "id" {
        return kept(table)
            .iter()
            .filter_map(|r| Some(fit(r.get("id")?.as_str()?, about(r))))
            .collect();
    }
    // a set: one of these
    if let Some((_, _, allowed)) = declared
        .iter()
        .find(|(n, _, allowed)| n == field && !allowed.is_empty())
    {
        return allowed.iter().map(|a| fit(a.clone(), "")).collect();
    }
    // a link: the rows that exist where it may point
    let relation = crate::lenses::relations()
        .into_iter()
        .chain(lens.relation.iter().cloned())
        .find(|r| r.from == format!("{table}.{field}"));
    let Some(relation) = relation else {
        return vec![fit("", "your own words")];
    };
    let anywhere = relation.to.iter().any(|t| t == "*");
    let kinds = |m: &Mark| -> Vec<String> {
        if anywhere {
            m.exports.keys().cloned().collect()
        } else {
            relation.to.clone()
        }
    };
    // the other stars in view: every place kept beside this one, as the lab keeps them
    let here = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let others: Vec<(String, std::path::PathBuf, Mark)> = here
        .parent()
        .and_then(Path::parent)
        .map(crate::board::projects)
        .unwrap_or_default()
        .into_iter()
        .filter(|d| std::fs::canonicalize(d).ok().as_deref() != Some(here.as_path()))
        .filter_map(|d| read(&d).map(|m| (m.project.id.clone(), d, m)))
        .collect();
    let rows_of = |prefix: &str, d: &Path, m: &Mark, t: &str| -> Vec<Fit> {
        rows(d, m, t)
            .into_iter()
            .filter_map(|r| {
                Some(fit(
                    format!("{prefix}{t}:{}", r.get("id")?.as_str()?),
                    about(&r),
                ))
            })
            .collect()
    };
    // typed into another star: its kinds of row, then its rows
    if let Some((star, d, m)) = others
        .iter()
        .find(|(id, _, _)| typing.starts_with(&format!("{id}/")))
    {
        let prefix = format!("{star}/");
        let rest = &typing[prefix.len()..];
        return match rest.split_once(':') {
            None => kinds(m)
                .iter()
                .filter(|t| !rows(d, m, t).is_empty())
                .map(|t| {
                    fit(
                        format!("{prefix}{t}:"),
                        format!("{} rows", rows(d, m, t).len()),
                    )
                })
                .collect(),
            Some((t, _)) => rows_of(&prefix, d, m, t),
        };
    }
    // the stars themselves, to go into
    let stars: Vec<Fit> = others
        .iter()
        .map(|(id, _, m)| fit(format!("{id}/"), m.project.name.clone()))
        .collect();
    if typing.contains('/') {
        return stars;
    }
    // this place: where it may point into many kinds, the kind is chosen first, then the row; a link into
    // one kind of row is written as the id alone, as the language writes it - with a kind named, kind:id
    let tables = mark
        .as_ref()
        .map(kinds)
        .unwrap_or_else(|| relation.to.clone());
    let local: Vec<Fit> = match typing.split_once(':') {
        None if anywhere || tables.len() > 1 => tables
            .iter()
            .map(|t| fit(format!("{t}:"), format!("{} rows", kept(t).len())))
            .filter(|f| f.is != "0 rows")
            .collect(),
        None => tables
            .iter()
            .flat_map(|t| {
                kept(t)
                    .into_iter()
                    .filter_map(|r| Some(fit(r.get("id")?.as_str()?.to_string(), about(&r))))
            })
            .collect(),
        Some((t, _)) => mark
            .as_ref()
            .map(|m| rows_of("", dir, m, t))
            .unwrap_or_default(),
    };
    if typing.contains(':') {
        local
    } else {
        local.into_iter().chain(stars).collect()
    }
}

/// The words of a typed line up to where the cursor stands, as a shell gives them (`COMP_LINE`,
/// `COMP_POINT`): the program's own name left out, and an empty last word where the cursor is at a blank.
pub fn typed_line(line: &str, point: usize) -> Vec<String> {
    let upto: String = line.chars().take(point).collect();
    let mut words: Vec<String> = upto
        .split_whitespace()
        .skip(1)
        .map(str::to_string)
        .collect();
    if upto.ends_with(char::is_whitespace) || words.is_empty() {
        words.push(String::new());
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn from_a_blank_what_may_come_next_is_read_off_the_language_and_the_rows_that_exist() {
        // a place kept as the lab keeps places, owner/repository under one root, so that a star beside it is in view
        let d = std::env::temp_dir()
            .join(format!("ocoeco-fits-{}", std::process::id()))
            .join("o")
            .join("a");
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nbacklog = \"data/backlog.json\"\nmilestones = \"data/milestones.json\"\ndecisions = \"data/decisions.json\"\n").unwrap();
        std::fs::write(
            d.join("data/backlog.json"),
            "{\"id\":\"a1\",\"title\":\"A thing to do\"}\n",
        )
        .unwrap();
        std::fs::write(
            d.join("data/milestones.json"),
            "{\"id\":\"m1\",\"title\":\"Reached\"}\n",
        )
        .unwrap();
        std::fs::write(d.join("data/decisions.json"), "").unwrap();
        let words = |f: Vec<Fit>| f.into_iter().map(|f| f.word).collect::<Vec<_>>();
        let commands = [
            Command {
                name: "glimpse".into(),
                is: "Record".into(),
                flags: vec![("kind".into(), "What kind".into(), true)],
                positionals: vec!["summary".into()],
            },
            Command {
                name: "focus".into(),
                is: "Look".into(),
                flags: vec![
                    ("surface".into(), "The surface".into(), true),
                    ("beam".into(), "Its beam".into(), false),
                ],
                positionals: vec!["lens".into()],
            },
        ];
        // a command of the program: its positional, from the language - and its flags, from its declaration
        let focus = words(after(&line(&["focus", ""]), &d, &commands));
        assert!(
            focus.contains(&"work".to_string()) && focus.contains(&"--surface".to_string()),
            "{focus:?}"
        );
        assert_eq!(
            words(after(
                &line(&["focus", "work", "--surface", "s"]),
                &d,
                &commands
            )),
            ["screen"]
        );
        assert_eq!(
            words(after(&line(&["focus", "work", "--b"]), &d, &commands)),
            ["--beam"]
        );
        assert_eq!(
            words(after(
                &line(&["focus", "work", "--beam", "--surface", "text", ""]),
                &d,
                &commands
            )),
            Vec::<String>::new(),
            "all given"
        );
        assert_eq!(
            words(after(&line(&["gl"]), &d, &commands)),
            ["glimpse"],
            "a command, by what is typed"
        );
        assert!(
            words(after(&line(&["glimpse", ""]), &d, &commands)).contains(&"item".to_string()),
            "the entries"
        );
        let fields = after(
            &line(&["glimpse", "item", "--status", "open", ""]),
            &d,
            &commands,
        );
        assert!(
            fields.iter().any(|f| f.word == "--title")
                && !fields.iter().any(|f| f.word == "--status"),
            "the fields not yet given"
        );
        assert!(
            fields
                .iter()
                .all(|f| f.word != "--since" && f.word != "--driver"),
            "what the computer supplies is not asked"
        );
        assert_eq!(
            words(after(
                &line(&["glimpse", "item", "--status", "n"]),
                &d,
                &commands
            )),
            ["next"],
            "one of a set"
        );
        assert_eq!(
            words(after(
                &line(&["glimpse", "item", "--id", ""]),
                &d,
                &commands
            )),
            ["a1"],
            "the rows a change would be made to"
        );
        // a link that may point into two kinds: the kind first - and only kinds that hold rows - then the row
        assert_eq!(
            words(after(
                &line(&["glimpse", "item", "--serves", ""]),
                &d,
                &commands
            )),
            ["milestones:"]
        );
        let rows = after(
            &line(&["glimpse", "item", "--serves", "milestones:"]),
            &d,
            &commands,
        );
        assert_eq!(
            (rows[0].word.as_str(), rows[0].is.as_str()),
            ("milestones:m1", "Reached")
        );
        assert_eq!(
            words(after(
                &line(&["glimpse", "item", "--title", ""]),
                &d,
                &commands
            )),
            [""],
            "free words: nothing to offer"
        );
        // a link into another star in view: the star, then its kind of row, then the row
        let other = d.parent().unwrap().join("other-star");
        std::fs::create_dir_all(other.join("data")).unwrap();
        std::fs::write(other.join("oco.toml"), "schema = 1\n[project]\nid = \"o/other\"\nname = \"Other: a star beside\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\ndecisions = \"data/decisions.json\"\n").unwrap();
        std::fs::write(
            other.join("data/decisions.json"),
            "{\"id\":\"d9\",\"decision\":\"Over there\"}\n",
        )
        .unwrap();
        assert!(
            words(after(
                &line(&["glimpse", "decision", "--refines", ""]),
                &d,
                &commands
            ))
            .contains(&"o/other/".to_string()),
            "the stars in view"
        );
        assert_eq!(
            words(after(
                &line(&["glimpse", "decision", "--refines", "o/other/"]),
                &d,
                &commands
            )),
            ["o/other/decisions:"]
        );
        assert_eq!(
            words(after(
                &line(&["glimpse", "decision", "--refines", "o/other/decisions:d"]),
                &d,
                &commands
            )),
            ["o/other/decisions:d9"]
        );
        // a link into one kind of row: the id alone, as the language writes it
        std::fs::write(
            d.join("data/decisions.json"),
            "{\"id\":\"d1\",\"decision\":\"The first\"}\n",
        )
        .unwrap();
        assert_eq!(
            words(after(
                &line(&["glimpse", "decision", "--refines", "d"]),
                &d,
                &commands
            )),
            ["d1"]
        );
        assert_eq!(
            typed_line("ocoeco glimpse item --st", 24),
            line(&["glimpse", "item", "--st"])
        );
        assert_eq!(typed_line("ocoeco glimpse ", 15), line(&["glimpse", ""]));
    }
}
