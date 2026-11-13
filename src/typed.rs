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

/// A typed line read against what may be given: what was given for each key - or everything the reader
/// could not take, each as argenv found it.
pub fn read(model: &[Record], words: &[String]) -> Result<BTreeMap<String, String>, Vec<Finding>> {
    let read = Invocation {
        args: words,
        env: &Nothing,
    }
    .resolve(model);
    let mut findings = lint(model, &read);
    // a word that is neither a field nor a field's value is left over: it is not understood either -
    // unless a field was not known, for then what is left over is most likely what was given for it
    if !findings
        .iter()
        .any(|f| matches!(f, Finding::UnknownArg { .. }))
    {
        findings.extend(read.positionals().iter().map(|word| Finding::UnknownArg {
            arg: word.clone(),
            did_you_mean: None,
        }));
    }
    if findings.is_empty() {
        Ok(read
            .values()
            .iter()
            .map(|(key, value)| (key.clone(), value.raw.clone()))
            .collect())
    } else {
        Err(findings)
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
    fn what_cannot_be_taken_is_said_as_the_reader_found_it() {
        let found = |words: &[&str]| read(&item(), &line(words)).unwrap_err();
        let misspelt = found(&["--id", "a", "--staus", "next"]);
        assert!(
            matches!(&misspelt[0], Finding::UnknownArg { arg, did_you_mean: Some(meant) } if arg == "--staus" && meant == "--status"),
            "a word with a letter missing is answered with what was meant"
        );
        assert_eq!(
            misspelt.len(),
            1,
            "and what was given for it is not said to be a second thing not understood"
        );
        assert!(found(&["--id", "a", "--status", "nxt"]).iter().any(|f| matches!(f, Finding::InvalidValue { key, reason, .. } if key == "status" && reason.contains("open, next"))), "a value outside the set is refused with the set");
        assert!(found(&["--title", "A"])
            .iter()
            .any(|f| matches!(f, Finding::MissingRequired { key, .. } if key == "id")));
        assert!(found(&["--id", "a", "--title"])
            .iter()
            .any(|f| matches!(f, Finding::MissingValue { arg } if arg == "--title")));
        assert!(
            found(&["--id", "a", "stray"])
                .iter()
                .any(|f| matches!(f, Finding::UnknownArg { arg, .. } if arg == "stray")),
            "a word left over is not understood"
        );
    }
}
