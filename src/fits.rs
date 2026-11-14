//! What fits here: from a blank on a typed line, what may come next - the language asked the other way.
//! A lens says what comes into focus; asked from a blank, the same declaration says what may enter. An
//! entry's fields are what its lens declares; a field held to a set offers the set; a field that is a link
//! offers the rows that exist, as links; the id offers the rows that a change would be made to; a field of
//! free words offers nothing, and says so. Nothing here is written apart from the language: it is read.

use crate::entries::{entry, fields};
use crate::join::rows;
use crate::mark::Mark;
use std::path::Path;

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
pub fn after(words: &[String], dir: &Path, commands: &[(String, String)]) -> Vec<Fit> {
    let typing = words.last().cloned().unwrap_or_default();
    let offered = match words {
        [] | [_] => commands
            .iter()
            .map(|(c, is)| fit(c.clone(), is.clone()))
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
            None => Vec::new(),
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
        _ => Vec::new(),
    };
    offered
        .into_iter()
        .filter(|f| f.word.starts_with(typing.as_str()))
        .collect()
}

/// What a field of an entry may hold, standing in the place at `dir`.
fn values(
    field: &str,
    table: &str,
    declared: &[(String, bool, Vec<String>)],
    lens: &crate::lenses::Lens,
    dir: &Path,
    typing: &str,
) -> Vec<Fit> {
    let mark = std::fs::read_to_string(dir.join("oco.toml"))
        .ok()
        .and_then(|t| Mark::parse(&t).ok());
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
    let tables: Vec<String> = match (&mark, anywhere) {
        (Some(m), true) => m.exports.keys().cloned().collect(),
        _ => relation.to.clone(),
    };
    // where it may point into many kinds, the kind is chosen first, then the row
    match typing.split_once(':') {
        None if anywhere || tables.len() > 1 => tables
            .iter()
            .map(|t| fit(format!("{t}:"), format!("{} rows", kept(t).len())))
            .filter(|f| f.is != "0 rows")
            .collect(),
        _ => {
            let wanted = typing.split_once(':').map(|(t, _)| t.to_string());
            tables
                .iter()
                .filter(|t| wanted.as_deref().is_none_or(|w| w == t.as_str()))
                .flat_map(|t| {
                    kept(t).into_iter().filter_map(move |r| {
                        Some(fit(format!("{t}:{}", r.get("id")?.as_str()?), about(&r)))
                    })
                })
                .collect()
        }
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
        let d = std::env::temp_dir().join(format!("ocoeco-fits-{}", std::process::id()));
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
            ("glimpse".to_string(), "Record".to_string()),
            ("focus".to_string(), "Look".to_string()),
        ];
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
        assert_eq!(
            typed_line("ocoeco glimpse item --st", 24),
            line(&["glimpse", "item", "--st"])
        );
        assert_eq!(typed_line("ocoeco glimpse ", 15), line(&["glimpse", ""]));
    }
}
