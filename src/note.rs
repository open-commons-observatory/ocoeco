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

/// The language as the place at `dir` speaks it: the tables the lenses built into ocoeco define, and those
/// its own lenses define. None where the place's lenses cannot be read together; the shared language then
/// stands alone.
fn spoken_at(dir: &Path) -> Option<tad::Declaration> {
    let mut tables = crate::lenses::tables();
    for lens in crate::lenses::with(&[dir.to_path_buf()]).ok()? {
        for (name, declared) in lens.tables {
            tables.entry(name).or_insert(declared);
        }
    }
    let mut whole = toml::Table::new();
    whole.insert("tables".into(), toml::Value::Table(tables));
    toml::Value::Table(whole).try_into().ok()
}

/// ocoeco's shared language: the tables every star may keep - those the lenses define (language/*.toml),
/// together, as a tad declaration. Each table is a minimum; a star's rows may carry more fields.
pub fn language() -> &'static tad::Declaration {
    static LANGUAGE: std::sync::OnceLock<tad::Declaration> = std::sync::OnceLock::new();
    LANGUAGE.get_or_init(|| {
        let mut whole = toml::Table::new();
        whole.insert("tables".into(), toml::Value::Table(crate::lenses::tables()));
        toml::Value::Table(whole)
            .try_into()
            .expect("the lenses' tables are a tad declaration")
    })
}

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
    // a row is held to its table's declaration as the place speaks the language: the lenses built into
    // ocoeco, and those the place brings - as files or kept as rows
    let spoken = spoken_at(dir);
    let shape = spoken
        .as_ref()
        .and_then(|d| d.tables.get(table))
        .or_else(|| language().tables.get(table));
    let pos = rows.iter().position(|(r, _)| r.text("id") == id);
    let added = pos.is_none();
    let mut row = match pos {
        Some(i) => rows[i].0.clone(),
        None => {
            let mut r = Row(vec![("id".into(), Value::String(id.into()))]);
            if let Some(f) = shape.and_then(|s| s.dated.as_deref()) {
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
        let ty = shape
            .and_then(|s| s.fields.iter().find(|f| f.name == c.field))
            .map(|f| f.ty.as_str())
            .unwrap_or("string");
        let v = match ty {
            "integer" => Value::from(
                new.trim()
                    .parse::<i64>()
                    .with_context(|| format!("{} must be a whole number, not '{new}'", c.field))?,
            ),
            "number" => match new.trim().parse::<i64>() {
                Ok(n) => Value::from(n),
                Err(_) => Value::from(
                    new.trim()
                        .parse::<f64>()
                        .with_context(|| format!("{} must be a number, not '{new}'", c.field))?,
                ),
            },
            "boolean" => Value::Bool(match new.trim() {
                "true" | "yes" => true,
                "false" | "no" => false,
                other => bail!("{} must be true or false, not '{other}'", c.field),
            }),
            _ => Value::String(new.clone()),
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
                .fields
                .iter()
                .map(|f| f.name.as_str())
                .filter_map(|k| row.get(k).map(|v| (k.to_string(), v.clone())))
                .collect();
            ordered.extend(
                row.0
                    .iter()
                    .filter(|(k, _)| !s.fields.iter().any(|f| f.name == *k))
                    .cloned(),
            );
            row = Row(ordered);
        }
        let missing: Vec<&str> = s
            .fields
            .iter()
            .filter(|f| f.required && f.name != "id")
            .map(|f| f.name.as_str())
            .filter(|k| row.text(k).trim().is_empty())
            .collect();
        if !missing.is_empty() {
            bail!("{table}/{id} needs {}", missing.join(", "));
        }
        // everything else the shared language says about a row, checked by tad
        let fields: serde_json::Map<String, Value> = row.0.iter().cloned().collect();
        let problems = tad::check_row(s, &fields)?;
        if !problems.is_empty() {
            bail!("{table}/{id}: {}", problems.join("; "));
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

    #[test]
    fn the_shared_language_is_a_tad_declaration() {
        let l = language();
        // every table some lens defines, and no other
        assert_eq!(l.tables.len(), crate::lenses::tables().len());
        assert!(
            l.tables.values().all(|t| t.extra_fields && t.key == ["id"]),
            "each table a minimum, keyed by id"
        );
        assert_eq!(l.tables["backlog"].dated.as_deref(), Some("opened"));
    }
}

#[cfg(test)]
mod spoken_at_a_place {
    use super::*;

    #[test]
    fn a_row_is_held_to_the_declaration_of_the_lens_the_place_brings() {
        let d = std::env::temp_dir().join(format!("ocoeco-spoken-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("lenses")).unwrap();
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nquestions = \"data/questions.json\"\n").unwrap();
        std::fs::write(d.join("data/questions.json"), "").unwrap();
        std::fs::write(d.join("lenses/questions.toml"), "[lens]\nid = \"questions\"\nsays = \"What is asked.\"\n[tables.questions]\nkey = [\"id\"]\nfields = [{ name = \"id\", type = \"string\", required = true }, { name = \"asked\", type = \"integer\" }, { name = \"answered\", type = \"boolean\" }]\n").unwrap();
        let change = |field: &str, value: &str| {
            vec![Change {
                field: field.into(),
                value: value.into(),
                append: false,
            }]
        };
        // a whole number where the place's own lens declares one, and nothing else
        assert!(
            note(&d, "questions", "q1", &change("asked", "x"), "2026-11-14")
                .unwrap_err()
                .to_string()
                .contains("asked must be a whole number")
        );
        note(&d, "questions", "q1", &change("asked", "3"), "2026-11-14").unwrap();
        note(
            &d,
            "questions",
            "q1",
            &change("answered", "true"),
            "2026-11-14",
        )
        .unwrap();
        let row: serde_json::Value = serde_json::from_str(
            std::fs::read_to_string(d.join("data/questions.json"))
                .unwrap()
                .trim(),
        )
        .unwrap();
        assert_eq!(
            (row["asked"].as_i64(), row["answered"].as_bool()),
            (Some(3), Some(true)),
            "written as the kinds its own lens declares"
        );
    }
}
