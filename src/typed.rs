//! The typed line, read back. A surface has two directions: it lays a projection out, and it reads back
//! what someone gives there. This is the reading back of a line typed in a shell: what an entry may be
//! given is handed to argenv's reader as records - a field is an input, its allowed values are the
//! input's - and what was typed comes back as what was given for each, or as what could not be taken.
//!
//! The reader is argenv's own, as it is: it knows `--field value` and `--field=value`, says which word it
//! does not know and what was meant, and refuses a value outside a field's set with the set. Nothing of
//! how a line is read is written here; only what an entry is, said in the reader's terms.

use std::collections::BTreeMap;

use argenv_resolve::{lint, EnvSource, Finding, Invocation, Record};

/// No environment: an entry's fields are given on the line. (Who is driving comes from the environment,
/// and is the program's to supply - not a field a person gives.)
struct Nothing;

impl EnvSource for Nothing {
    fn get(&self, _name: &str) -> Option<String> {
        None
    }
    fn names(&self) -> Vec<String> {
        Vec::new()
    }
}

/// One thing that may be given on the line: its key, the values it is held to (none: any words), and
/// whether the line cannot do without it.
fn record(key: &str, kind: &str, allowed: &[String], required: bool, summary: String) -> Record {
    serde_json::from_value(serde_json::json!({
        "key": key, "type": kind, "allowed": allowed, "required": required, "summary": summary,
        "arg": { "long": key, "arity": 1, "value_name": key.to_uppercase() },
    }))
    .expect("a record is made of what a record holds")
}

/// What an entry may be given on a typed line: which one (its id), where (the place), each field of its
/// table - held to the field's set where it has one - and, for a field of free words, adding to it.
pub fn records(entry: &str, fields: &[(String, bool, Vec<String>)]) -> Vec<Record> {
    let mut model = vec![
        record("id", "string", &[], true, format!("which {entry}")),
        record(
            "dir",
            "string",
            &[],
            false,
            "the place it is entered into".to_string(),
        ),
    ];
    for (name, _, allowed) in fields.iter().filter(|(name, _, _)| name != "id") {
        let kind = if allowed.is_empty() { "string" } else { "enum" };
        model.push(record(
            name,
            kind,
            allowed,
            false,
            format!("{name}, of the {entry}"),
        ));
        if allowed.is_empty() {
            model.push(record(
                &format!("add-{name}"),
                "string",
                &[],
                false,
                format!("words added to {name}"),
            ));
        }
    }
    model
}

/// What the reader could not take of a typed line, in its own words - and whether a word was one it does
/// not know, so that whoever asked can say what it does know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotTaken {
    pub said: Vec<String>,
    pub unknown: bool,
}

/// A typed line read against what may be given: what was given for each key - or what the reader could
/// not take. The reader says each thing itself; nothing here restates it.
pub fn read(model: &[Record], words: &[String]) -> Result<BTreeMap<String, String>, NotTaken> {
    let read = Invocation {
        args: words,
        env: &Nothing,
    }
    .resolve(model);
    let findings = lint(model, &read);
    let unknown = findings
        .iter()
        .any(|f| matches!(f, Finding::UnknownArg { .. }));
    let mut said: Vec<String> = findings.iter().map(|finding| finding.to_string()).collect();
    // a word left over is neither a field nor what was given for one - the reader has no word for that;
    // unless a field was not known, for then what is left over is most likely what was given for it
    if !unknown {
        said.extend(
            read.positionals().iter().map(|word| {
                format!("{word:?} is not understood: a value is given as --FIELD VALUE")
            }),
        );
    }
    if said.is_empty() {
        Ok(read
            .values()
            .iter()
            .map(|(key, value)| (key.clone(), value.raw.clone()))
            .collect())
    } else {
        Err(NotTaken { said, unknown })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    fn item() -> Vec<Record> {
        records(
            "item",
            &[
                ("id".into(), true, vec![]),
                ("title".into(), true, vec![]),
                ("status".into(), true, vec!["open".into(), "next".into()]),
            ],
        )
    }

    #[test]
    fn a_typed_line_is_read_back_as_what_was_given() {
        let given = read(
            &item(),
            &line(&[
                "--id",
                "a",
                "--title",
                "A thing",
                "--status=next",
                "--add-title",
                " more",
            ]),
        )
        .unwrap();
        let got: Vec<(&str, &str)> = given
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                ("add-title", " more"),
                ("id", "a"),
                ("status", "next"),
                ("title", "A thing")
            ],
            "both ways of giving a value are read"
        );
        // a field held to a set cannot be added to: there is nothing to add words to
        assert!(!item().iter().any(|r| r.key == "add-status"));
    }

    #[test]
    fn what_cannot_be_taken_is_said_by_the_reader_itself() {
        let not = |words: &[&str]| read(&item(), &line(words)).unwrap_err();
        // a word with a letter missing is answered with what was meant; what was given for it is not a second thing
        assert_eq!(
            not(&["--id", "a", "--staus", "next"]),
            NotTaken {
                said: vec!["--staus is not a declared flag; did you mean --status?".into()],
                unknown: true
            }
        );
        assert!(
            not(&["--id", "a", "--status", "nxt"]).said[0]
                .contains("which is invalid: accepted: open, next"),
            "a value outside the set is refused with the set"
        );
        assert_eq!(
            not(&["--title", "A"]).said,
            ["id is required; supply it with --id"]
        );
        assert_eq!(
            not(&["--id", "a", "--title"]).said,
            ["--title needs a value"]
        );
        // the one thing said here: a word left over, for which the reader has no word
        assert_eq!(
            not(&["--id", "a", "stray"]),
            NotTaken {
                said: vec!["\"stray\" is not understood: a value is given as --FIELD VALUE".into()],
                unknown: false
            }
        );
    }
}
