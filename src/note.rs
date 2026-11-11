//! Recording, as a tool (SPEC.md section 11): add or update one row of a project's own table - a
//! backlog item, a decision, a concept, a working, a log entry - checked against the shared format,
//! so nobody hand-writes a table. Only the row it touches is rewritten; every other line keeps its
//! exact text, so a change reads as one line.
use crate::mark::Mark;
use anyhow::{bail, Context, Result};
use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
use serde_json::Value;
use std::path::Path;

/// A row with its fields in the order they were written.
#[derive(Debug, Clone, PartialEq)]
pub struct Row(pub Vec<(String, Value)>);

impl<'de> Deserialize<'de> for Row {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct Fields;
        impl<'de> Visitor<'de> for Fields {
            type Value = Row;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> std::result::Result<Row, A::Error> {
                let mut v = Vec::new();
                while let Some((k, x)) = m.next_entry::<String, Value>()? {
                    v.push((k, x));
                }
                Ok(Row(v))
            }
        }
        d.deserialize_map(Fields)
    }
}

impl Row {
    pub(crate) fn get(&self, k: &str) -> Option<&Value> {
        self.0.iter().find(|(x, _)| x == k).map(|(_, v)| v)
    }
    pub(crate) fn set(&mut self, k: &str, v: Value) {
        match self.0.iter_mut().find(|(x, _)| x == k) {
            Some(e) => e.1 = v,
            None => self.0.push((k.to_string(), v)),
        }
    }
    pub(crate) fn text(&self, k: &str) -> String {
        match self.get(k) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Null) | None => String::new(),
            Some(v) => v.to_string(),
        }
    }
    /// One line of the table, compact, in the row's own field order.
    pub fn line(&self) -> String {
        let fields: Vec<String> = self
            .0
            .iter()
            .map(|(k, v)| format!("{}:{}", Value::String(k.clone()), v))
            .collect();
        format!("{{{}}}", fields.join(","))
    }
}

/// The shared format of a table: its fields in order, those that must not be empty, the values some
/// may take, and the field a new row is dated by.
struct Shape {
    table: &'static str,
    order: &'static [&'static str],
    required: &'static [&'static str],
    enums: &'static [(&'static str, &'static [&'static str])],
    dated: Option<&'static str>,
}

const SHAPES: &[Shape] = &[
    Shape {
        table: "backlog",
        order: &["id", "title", "status", "note", "opened"],
        required: &["title", "status"],
        enums: &[("status", &["open", "in-progress", "done"])],
        dated: Some("opened"),
    },
    Shape {
        table: "decisions",
        order: &["id", "date", "decision", "why", "rejected"],
        required: &["date", "decision", "why"],
        enums: &[],
        dated: Some("date"),
    },
    Shape {
        table: "concepts",
        order: &["id", "name", "summary", "why"],
        required: &["name", "summary"],
        enums: &[],
        dated: None,
    },
    Shape {
        table: "session_log",
        order: &[
            "id",
            "entry_date",
            "title",
            "done",
            "considered",
            "rejected",
        ],
        required: &["entry_date", "title", "done"],
        enums: &[],
        dated: Some("entry_date"),
    },
    Shape {
        table: "workings",
        order: &[
            "id", "what", "kind", "serves", "how", "why", "evidence", "uses",
        ],
        required: &["what", "kind", "serves", "how"],
        enums: &[("kind", &["tool", "check", "gatherer", "test", "practice"])],
        dated: None,
    },
    Shape {
        table: "idea",
        order: &["id", "ord", "kind", "heading", "text"],
        required: &["ord", "kind", "text"],
        enums: &[(
            "kind",
            &["lead", "problem", "idea", "picture", "limit", "start"],
        )],
        dated: None,
    },
    Shape {
        table: "vocabulary",
        order: &["id", "term", "means", "not", "see"],
        required: &["term", "means"],
        enums: &[],
        dated: None,
    },
    Shape {
        table: "handwork",
        order: &[
            "id",
            "action",
            "count",
            "first",
            "last",
            "status",
            "automated_by",
            "minutes",
        ],
        required: &["action", "count", "status"],
        enums: &[("status", &["manual", "automated"])],
        dated: Some("first"),
    },
    Shape {
        table: "sources",
        order: &["id", "title", "by", "year", "url", "takes"],
        required: &["title", "takes"],
        enums: &[],
        dated: None,
    },
    Shape {
        table: "milestones",
        order: &[
            "id",
            "subject",
            "ord",
            "title",
            "horizon",
            "readiness",
            "criteria",
            "why",
        ],
        required: &["subject", "ord", "title", "horizon", "readiness"],
        enums: &[("horizon", &["working", "perfection", "ongoing"])],
        dated: None,
    },
];

const NUMBERS: [&str; 8] = [
    "ord",
    "readiness",
    "order",
    "count",
    "minutes",
    "load",
    "effort",
    "year",
];

/// One change to a field: set it, or append to its text.
pub struct Change {
    pub field: String,
    pub value: String,
    pub append: bool,
}

/// FIELD=VALUE.
pub fn parse_change(s: &str, append: bool) -> Result<Change> {
    let (k, v) = s
        .split_once('=')
        .with_context(|| format!("'{s}' is not FIELD=VALUE"))?;
    if k.trim().is_empty() {
        bail!("'{s}' names no field");
    }
    Ok(Change {
        field: k.trim().to_string(),
        value: v.to_string(),
        append,
    })
}

fn cut(s: &str) -> String {
    let c: String = s.chars().take(40).collect();
    if s.chars().count() > 40 {
        c + "..."
    } else {
        c
    }
}

/// Adds or updates the row `id` of `table` in the repository at `dir`; returns what it did.
pub fn note(dir: &Path, table: &str, id: &str, changes: &[Change], today: &str) -> Result<String> {
    let mark = Mark::parse(
        &std::fs::read_to_string(dir.join("oco.toml"))
            .context("no oco.toml here: this repository has no mark")?,
    )?;
    let path = mark.exports.get(table).with_context(|| {
        format!("this repository does not export a {table} table (add it to [exports] in oco.toml)")
    })?;
    if path.contains('@') {
        bail!("{table} is kept on another branch ({path}): it is gathered there, not written here");
    }
    if id.trim().is_empty() {
        bail!("a row needs an id");
    }
    let file = dir.join(path);
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    // every row, with the exact text of its line so untouched rows are written back unchanged
    let mut rows: Vec<(Row, Option<String>)> = Vec::new();
    for l in text.lines().filter(|l| !l.trim().is_empty()) {
        let r: Row = serde_json::from_str(l)
            .with_context(|| format!("{}: a line is not a row: {}", file.display(), cut(l)))?;
        rows.push((r, Some(l.to_string())));
    }
    let sorted = rows
        .windows(2)
        .all(|w| w[0].0.text("id") <= w[1].0.text("id"));
    let shape = SHAPES.iter().find(|s| s.table == table);
    let pos = rows.iter().position(|(r, _)| r.text("id") == id);
    let added = pos.is_none();
    let mut row = match pos {
        Some(i) => rows[i].0.clone(),
        None => {
            let mut r = Row(vec![("id".into(), Value::String(id.into()))]);
            if let Some(f) = shape.and_then(|s| s.dated) {
                r.set(f, Value::String(today.into()));
            }
            r
        }
    };
    let mut said = Vec::new();
    for c in changes {
        if c.field == "id" {
            bail!("the id is given by --id, not changed by a field");
        }
        let old = row.text(&c.field);
        let new = if c.append && !old.is_empty() {
            format!("{old} {}", c.value)
        } else {
            c.value.clone()
        };
        let v = if NUMBERS.contains(&c.field.as_str()) {
            Value::from(
                new.trim()
                    .parse::<i64>()
                    .with_context(|| format!("{} must be a whole number, not '{new}'", c.field))?,
            )
        } else {
            Value::String(new.clone())
        };
        if !added && old != new {
            said.push(format!("{}: '{}' -> '{}'", c.field, cut(&old), cut(&new)));
        }
        row.set(&c.field, v);
    }
    if let Some(s) = shape {
        if added {
            // a new row's fields in the format's order, then any others
            let mut ordered: Vec<(String, Value)> = s
                .order
                .iter()
                .filter_map(|k| row.get(k).map(|v| (k.to_string(), v.clone())))
                .collect();
            ordered.extend(
                row.0
                    .iter()
                    .filter(|(k, _)| !s.order.contains(&k.as_str()))
                    .cloned(),
            );
            row = Row(ordered);
        }
        let missing: Vec<&str> = s
            .required
            .iter()
            .copied()
            .filter(|k| row.text(k).trim().is_empty())
            .collect();
        if !missing.is_empty() {
            bail!("{table}/{id} needs {}", missing.join(", "));
        }
        for (k, allowed) in s.enums {
            let v = row.text(k);
            if !allowed.contains(&v.as_str()) {
                bail!(
                    "{table}/{id}: {k} '{v}' is not one of: {}",
                    allowed.join(", ")
                );
            }
        }
    }
    match pos {
        Some(i) => rows[i] = (row, None),
        None => rows.push((row, None)),
    }
    if added && sorted {
        rows.sort_by_key(|(r, _)| r.text("id"));
    }
    if let Some(p) = file.parent() {
        std::fs::create_dir_all(p)?;
    }
    let out: String = rows
        .iter()
        .map(|(r, orig)| orig.clone().unwrap_or_else(|| r.line()) + "\n")
        .collect();
    std::fs::write(&file, out).with_context(|| format!("writing {}", file.display()))?;
    Ok(if added {
        format!("added {table}/{id}")
    } else if said.is_empty() {
        format!("{table}/{id}: unchanged")
    } else {
        format!("updated {table}/{id} - {}", said.join("; "))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ocoeco-note-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("oco")).unwrap();
        std::fs::write(
            d.join("oco.toml"),
            "schema = 1\n[project]\nid = \"o/r\"\nname = \"R\"\nkind = \"tool\"\nparent = \"open-commons-observatory\"\n\n[exports]\nbacklog = \"oco/backlog.json\"\ndecisions = \"oco/decisions.json\"\nactivity = \"oco/activity.json@oco-data\"\n",
        )
        .unwrap();
        d
    }
    fn set(s: &str) -> Change {
        parse_change(s, false).unwrap()
    }

    #[test]
    fn a_new_row_is_dated_ordered_and_sorted_in() {
        let d = repo("new");
        std::fs::write(d.join("oco/backlog.json"), "{\"id\":\"b\",\"title\":\"B\",\"status\":\"open\",\"note\":\"\",\"opened\":\"2026-01-01\"}\n").unwrap();
        let said = note(
            &d,
            "backlog",
            "a",
            &[set("note=Читать"), set("status=open"), set("title=A")],
            "2026-11-11",
        )
        .unwrap();
        assert_eq!(said, "added backlog/a");
        let t = std::fs::read_to_string(d.join("oco/backlog.json")).unwrap();
        assert_eq!(t.lines().next().unwrap(), "{\"id\":\"a\",\"title\":\"A\",\"status\":\"open\",\"note\":\"Читать\",\"opened\":\"2026-11-11\"}");
    }

    #[test]
    fn an_update_touches_one_line_and_keeps_its_field_order() {
        let d = repo("update");
        let other = "{\"id\": \"a\", \"title\": \"kept exactly\", \"status\": \"open\"}";
        std::fs::write(
            d.join("oco/backlog.json"),
            format!(
                "{other}\n{{\"note\":\"n\",\"id\":\"b\",\"title\":\"B\",\"status\":\"open\"}}\n"
            ),
        )
        .unwrap();
        let said = note(
            &d,
            "backlog",
            "b",
            &[set("status=done"), parse_change("note=more", true).unwrap()],
            "x",
        )
        .unwrap();
        assert!(
            said.contains("status: 'open' -> 'done'") && said.contains("'n' -> 'n more'"),
            "{said}"
        );
        let t = std::fs::read_to_string(d.join("oco/backlog.json")).unwrap();
        assert_eq!(
            t.lines().next().unwrap(),
            other,
            "an untouched row keeps its exact text"
        );
        assert_eq!(
            t.lines().nth(1).unwrap(),
            "{\"note\":\"n more\",\"id\":\"b\",\"title\":\"B\",\"status\":\"done\"}"
        );
    }

    #[test]
    fn it_refuses_what_the_format_does_not_allow() {
        let d = repo("refuse");
        assert!(note(
            &d,
            "backlog",
            "a",
            &[set("title=A"), set("status=maybe")],
            "x"
        )
        .unwrap_err()
        .to_string()
        .contains("not one of"));
        assert!(note(&d, "backlog", "a", &[set("status=open")], "x")
            .unwrap_err()
            .to_string()
            .contains("needs title"));
        assert!(note(&d, "decisions", "d", &[set("decision=D")], "x")
            .unwrap_err()
            .to_string()
            .contains("needs why"));
        assert!(note(&d, "concepts", "c", &[set("name=C")], "x")
            .unwrap_err()
            .to_string()
            .contains("does not export"));
        assert!(note(&d, "activity", "a", &[], "x")
            .unwrap_err()
            .to_string()
            .contains("another branch"));
        assert!(parse_change("nothing", false).is_err());
        assert!(
            !d.join("oco/backlog.json").exists(),
            "a refused change writes nothing"
        );
    }

    #[test]
    fn a_word_needs_its_meaning() {
        let d = repo("word");
        let t = std::fs::read_to_string(d.join("oco.toml"))
            .unwrap()
            .replace(
                "[exports]\n",
                "[exports]\nvocabulary = \"oco/vocabulary.json\"\n",
            );
        std::fs::write(d.join("oco.toml"), t).unwrap();
        assert!(note(&d, "vocabulary", "w", &[set("term=word")], "x")
            .unwrap_err()
            .to_string()
            .contains("needs means"));
        assert_eq!(
            note(
                &d,
                "vocabulary",
                "w",
                &[set("term=word"), set("means=what it says")],
                "x"
            )
            .unwrap(),
            "added vocabulary/w"
        );
    }
}
