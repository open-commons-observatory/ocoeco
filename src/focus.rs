//! Focus: a lens brings data into view. A lens's focus is declared as data (lenses/*.toml): parts, each
//! a query over the lens's tables and a line saying how one answer is laid out. The data of every place
//! asked for is gathered - each row with its star - and the queries are run by tad's SQL engine. Nothing
//! is made: the same data, brought into focus; and what is seen is laid out as plain text - the first
//! of the ways a focus may be laid out.

use crate::join::rows;
use crate::lenses::{lenses, Lens};
use crate::mark::Mark;
use anyhow::{bail, Context, Result};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use tad::sql::SqlEngine;

/// Gathers the lens's tables from the places at `dirs` into `into`, each row with its star and seq -
/// its place in the order gathered - and returns the declaration of what was gathered.
fn gather(lens: &Lens, dirs: &[PathBuf], into: &Path) -> Result<tad::Declaration> {
    std::fs::create_dir_all(into)?;
    let mut tables = toml::Table::new();
    for (table, declared) in &lens.tables {
        let (mut text, mut seq) = (String::new(), 0u64);
        for dir in dirs {
            let Ok(mark) = std::fs::read_to_string(dir.join("oco.toml"))
                .map_err(anyhow::Error::from)
                .and_then(|t| Mark::parse(&t))
            else {
                continue;
            };
            let star = mark
                .project
                .id
                .rsplit('/')
                .next()
                .unwrap_or(&mark.project.id)
                .to_string();
            for mut row in rows(dir, &mark, table) {
                let Some(fields) = row.as_object_mut() else {
                    continue;
                };
                seq += 1;
                fields.insert("star".into(), star.clone().into());
                fields.insert("seq".into(), seq.into());
                text.push_str(&serde_json::to_string(&row)?);
                text.push('\n');
            }
        }
        std::fs::write(into.join(format!("{table}.json")), text)?;
        let mut declared = declared.clone();
        if let Some(fields) = declared.get_mut("fields").and_then(|f| f.as_array_mut()) {
            for (name, kind) in [("star", "string"), ("seq", "integer")] {
                let mut field = toml::Table::new();
                field.insert("name".into(), name.into());
                field.insert("type".into(), kind.into());
                fields.push(toml::Value::Table(field));
            }
        }
        tables.insert(table.clone(), declared);
    }
    let mut whole = toml::Table::new();
    whole.insert("tables".into(), toml::Value::Table(tables));
    toml::Value::Table(whole)
        .try_into()
        .context("the lens's tables are a tad declaration")
}

/// The answers to one query: its rows.
fn answers(engine: &dyn SqlEngine, load: &str, sql: &str) -> Result<Vec<Map<String, Value>>> {
    let printed = engine.run(
        &format!("{load}{};\n", sql.trim().trim_end_matches(';')),
        "json",
    )?;
    if printed.trim().is_empty() {
        return Ok(Vec::new());
    }
    let rows: Value =
        serde_json::from_str(printed.trim()).context("the engine's answer is not rows")?;
    Ok(rows
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| r.as_object().cloned())
        .collect())
}

/// A line with each {name} replaced by that answer's value; what the answer does not hold stays as written.
pub fn laid_out(line: &str, answer: &Map<String, Value>) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        let name = &rest[open + 1..open + close];
        match answer.get(name) {
            Some(Value::String(s)) => out.push_str(s),
            Some(Value::Null) => {}
            Some(other) => out.push_str(&other.to_string()),
            None => out.push_str(&rest[open..=open + close]),
        }
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}

/// What the lens `lens` brings into focus of the places at `dirs`, as of `today` (YYYY-MM-DD).
pub fn focus(lens: &str, dirs: &[PathBuf], today: &str, engine: &dyn SqlEngine) -> Result<String> {
    let all = lenses();
    let Some(found) = all.iter().find(|l| l.lens.id == lens) else {
        bail!(
            "no lens is called {lens}: there are {}",
            all.iter()
                .map(|l| l.lens.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    };
    if found.focus.is_empty() {
        bail!("the lens {lens} has no focus yet");
    }
    static CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let scratch = std::env::temp_dir().join(format!(
        "ocoeco-focus-{}-{}",
        std::process::id(),
        CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let brought = (|| -> Result<String> {
        let load = tad::sql::load_script(&scratch, &gather(found, dirs, &scratch)?);
        let mut text = String::new();
        for part in &found.focus {
            let rows = answers(engine, &load, &part.sql.replace("{today}", today))?;
            if rows.is_empty() && !part.always {
                continue;
            }
            if let Some(title) = &part.title {
                text.push_str(&title.replace("{count}", &rows.len().to_string()));
                text.push('\n');
            }
            for row in &rows {
                text.push_str(&laid_out(&part.line, row));
                text.push('\n');
            }
        }
        Ok(text)
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    brought
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An engine that answers from what it is asked: one row for work in progress, none waiting.
    struct StandIn;
    impl SqlEngine for StandIn {
        fn run(&self, script: &str, _format: &str) -> Result<String> {
            assert!(
                script.contains("CREATE TABLE \"backlog\"") && script.contains("\"star\" VARCHAR"),
                "the gathered rows carry their star"
            );
            Ok(
                if script.contains("status = 'in-progress' ORDER BY 1, seq") {
                    r#"[{"driver":"claude","star":"r","id":"a","title":"A","next":" - next: x","age":null}]"#.into()
                } else if script.contains("count(DISTINCT star)") {
                    r#"[{"open":2,"parked":0,"stars":1}]"#.into()
                } else {
                    "[]".into()
                },
            )
        }
    }

    #[test]
    fn a_line_is_laid_out_from_an_answer() {
        let answer: Map<String, Value> =
            serde_json::from_str(r#"{"a":"one","n":3,"gone":null}"#).unwrap();
        assert_eq!(
            laid_out("{a} and {n}{gone}; {unknown} stays", &answer),
            "one and 3; {unknown} stays"
        );
    }

    #[test]
    fn a_focus_is_its_parts_and_an_empty_part_is_left_out() {
        let d = std::env::temp_dir().join(format!("ocoeco-focus-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nbacklog = \"data/backlog.json\"\n").unwrap();
        std::fs::write(
            d.join("data/backlog.json"),
            "{\"id\":\"a\",\"title\":\"A\",\"status\":\"in-progress\"}\n",
        )
        .unwrap();
        let text = focus("work", &[d], "2026-11-12", &StandIn).unwrap();
        assert_eq!(text, "in progress (1)\n  claude: r/a - A - next: x\nopen 2 and parked 0, across 1 stars\n");
        assert!(focus("no-such-lens", &[], "2026-11-12", &StandIn)
            .unwrap_err()
            .to_string()
            .contains("there are knowledge, looking-back"));
    }
}
