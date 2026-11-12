//! Focus: a lens brings data into view. A lens's focus is declared as data (lenses/*.toml): parts, each
//! a query over the lens's tables and a line saying how one answer is laid out. The data of every place
//! asked for is gathered - each row with its star - and the queries are run by tad's SQL engine. Nothing
//! is made: the same data, brought into focus; and what is seen is laid out as plain text - the first
//! of the ways a focus may be laid out.

use crate::dome::Dome;
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
    // its own tables, and those it reads from other lenses: one lens defines a piece of data, many read it
    let all = crate::lenses::tables();
    let read: Vec<(String, toml::Value)> = lens
        .lens
        .reads
        .iter()
        .filter_map(|t| all.get(t).map(|d| (t.clone(), d.clone())))
        .collect();
    for (table, declared) in lens
        .tables
        .iter()
        .map(|(t, d)| (t.clone(), d.clone()))
        .chain(read)
    {
        let (table, declared) = (&table, &declared);
        let (mut text, mut seq) = (String::new(), 0u64);
        for dir in dirs {
            // what was observed is a place too: a folder of tables whose every row names its project
            if !dir.join("oco.toml").is_file() && dir.join("projects.json").is_file() {
                for mut fields in crate::harvest::read_rows(&dir.join(format!("{table}.json"))) {
                    let project = fields
                        .get("project")
                        .and_then(|p| p.as_str())
                        .unwrap_or_default()
                        .to_string();
                    seq += 1;
                    fields.insert(
                        "star".into(),
                        project.rsplit('/').next().unwrap_or(&project).into(),
                    );
                    fields.insert("seq".into(), seq.into());
                    text.push_str(&serde_json::to_string(&fields)?);
                    text.push('\n');
                }
                continue;
            }
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

/// One part of a lens, brought into focus: its heading, if it has one, and each answer laid out as a line.
/// How the parts are shown - indented, as a list, in a pane - is the surface's to say, not the lens's.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Brought {
    pub title: Option<String>,
    pub lines: Vec<String>,
    /// What a line means beyond its words, when the lens says so: a measure of a whole (so much, of so
    /// much), one for each line, or none. A lens adds meaning; how it looks is each surface's to say.
    pub measures: Vec<Option<(f64, f64)>>,
    /// What each line is about, so it can be known again another time though its words change (an age,
    /// a count): its star and id when the answer names them, otherwise the line itself.
    #[serde(default)]
    pub keys: Vec<String>,
}

/// What the lens `lens` brings into focus of the places at `dirs`, as of `today` (YYYY-MM-DD): its parts.
pub fn brought(
    lens: &str,
    dirs: &[PathBuf],
    today: &str,
    engine: &dyn SqlEngine,
) -> Result<Vec<Brought>> {
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
    let parts = (|| -> Result<Vec<Brought>> {
        let load = tad::sql::load_script(&scratch, &gather(found, dirs, &scratch)?);
        let mut parts = Vec::new();
        for part in &found.focus {
            let rows = answers(engine, &load, &part.sql.replace("{today}", today))?;
            if rows.is_empty() && !part.always {
                continue;
            }
            parts.push(Brought {
                title: part
                    .title
                    .as_ref()
                    .map(|t| t.replace("{count}", &rows.len().to_string())),
                lines: rows.iter().map(|row| laid_out(&part.line, row)).collect(),
                keys: rows
                    .iter()
                    .map(|row| {
                        match (
                            row.get("star").and_then(Value::as_str),
                            row.get("id").and_then(Value::as_str),
                        ) {
                            (Some(star), Some(id)) => format!("{star}/{id}"),
                            _ => laid_out(&part.line, row),
                        }
                    })
                    .collect(),
                measures: match &part.measure {
                    Some(column) => rows
                        .iter()
                        .map(|row| {
                            let so_much = match row.get(column) {
                                Some(Value::Number(n)) => n.as_f64(),
                                Some(Value::String(s)) => s.parse().ok(),
                                _ => None,
                            };
                            so_much.map(|v| (v, part.of.unwrap_or(1.0)))
                        })
                        .collect(),
                    None => Vec::new(),
                },
            });
        }
        Ok(parts)
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    parts
}

/// A heading without its count: "in progress (2)" is "in progress".
fn named(title: &str) -> &str {
    title.rsplit_once(" (").map_or(title, |(name, _)| name)
}

/// What is new in view since `before`: each line of a headed part that was not in that part before,
/// said under its heading; and what is no longer in view at all. A part with no heading - a total -
/// is not news, and a line whose words changed but which is about the same thing is not new.
pub fn news(before: &[Brought], now: &[Brought]) -> Vec<String> {
    let was_in = |name: &str| -> std::collections::BTreeSet<&String> {
        before
            .iter()
            .filter(|p| p.title.as_deref().map(named) == Some(name))
            .flat_map(|p| p.keys.iter())
            .collect()
    };
    let mut said = Vec::new();
    for part in now {
        let Some(title) = &part.title else { continue };
        let was = was_in(named(title));
        for (key, line) in part.keys.iter().zip(&part.lines) {
            if !was.contains(key) {
                said.push(format!("{}: {line}", named(title)));
            }
        }
    }
    let in_view: std::collections::BTreeSet<&String> = now
        .iter()
        .filter(|p| p.title.is_some())
        .flat_map(|p| p.keys.iter())
        .collect();
    let mut gone = std::collections::BTreeSet::new();
    for key in before
        .iter()
        .filter(|p| p.title.is_some())
        .flat_map(|p| p.keys.iter())
    {
        if !in_view.contains(key) && gone.insert(key) {
            said.push(format!("no longer in view: {key}"));
        }
    }
    said
}

/// What is new in view since the focus kept at `kept`, which then keeps this one. The first time
/// nothing is kept, so nothing is new: it only keeps.
pub fn since(kept: &Path, now: &[Brought]) -> Result<Vec<String>> {
    let before: Option<Vec<Brought>> = std::fs::read_to_string(kept)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    if let Some(dir) = kept.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(kept, serde_json::to_string(now)?)?;
    Ok(before.map(|before| news(&before, now)).unwrap_or_default())
}

/// What the lens brings into focus, laid out on the plain-text surface.
pub fn focus(lens: &str, dirs: &[PathBuf], today: &str, engine: &dyn SqlEngine) -> Result<String> {
    Ok(crate::dome::Text.lay_out(&brought(lens, dirs, today, engine)?))
}

/// Every lens with a focus, brought into focus over the place at `dir`: what fails to, and why.
pub fn every_lens_focuses(dir: &Path, engine: &dyn SqlEngine) -> Vec<String> {
    lenses()
        .iter()
        .filter(|l| !l.focus.is_empty())
        .filter_map(|l| {
            focus(&l.lens.id, &[dir.to_path_buf()], "2026-01-01", engine)
                .err()
                .map(|e| {
                    format!(
                        "the lens {} cannot come into focus here: {}",
                        l.lens.id,
                        e.to_string().lines().next().unwrap_or_default()
                    )
                })
        })
        .collect()
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
    fn what_is_new_in_view_is_said_once_and_a_changed_age_is_not_news() {
        let part = |title: &str, rows: &[(&str, &str)]| Brought {
            title: Some(title.into()),
            lines: rows.iter().map(|(_, l)| l.to_string()).collect(),
            keys: rows.iter().map(|(k, _)| k.to_string()).collect(),
            measures: vec![],
        };
        let total = |line: &str| Brought {
            title: None,
            lines: vec![line.into()],
            keys: vec![line.into()],
            measures: vec![],
        };
        let before = [
            part("in progress (1)", &[("r/a", "ann: r/a - A, 1 day(s)")]),
            part("next (2)", &[("r/b", "r/b - B"), ("r/c", "r/c - C")]),
            total("open 5"),
        ];
        let now = [
            part(
                "in progress (2)",
                &[("r/a", "ann: r/a - A, 2 day(s)"), ("r/b", "bob: r/b - B")],
            ),
            part("waiting (1)", &[("r/d", "r/d - D - waits on E")]),
            total("open 4"),
        ];
        assert_eq!(
            news(&before, &now),
            [
                "in progress: bob: r/b - B",
                "waiting: r/d - D - waits on E",
                "no longer in view: r/c"
            ]
        );
        assert!(news(&now, &now).is_empty());
        let kept =
            std::env::temp_dir().join(format!("ocoeco-since-{}/work.json", std::process::id()));
        let _ = std::fs::remove_file(&kept);
        assert!(
            since(&kept, &before).unwrap().is_empty(),
            "the first time only keeps"
        );
        assert_eq!(since(&kept, &now).unwrap().len(), 3);
        assert!(since(&kept, &now).unwrap().is_empty(), "said once");
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
        assert_eq!(
            text,
            "in progress (1)\n  claude: r/a - A - next: x\nopen 2 and parked 0, across 1 stars\n"
        );
        assert!(focus("no-such-lens", &[], "2026-11-12", &StandIn)
            .unwrap_err()
            .to_string()
            .contains("there are knowledge, looking-back"));
    }
}
