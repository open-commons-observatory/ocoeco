//! The harvest: every confirmed project's exported tables, each row tagged with its project, merged
//! by table name into the observatory's own tad data - with a registry of the projects and a list of
//! problems (a broken mark, a table that could not be read), shown rather than lost.

use crate::discover::{Candidate, Source};
use anyhow::Result;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::Path;

/// A table's lines -> its rows, each tagged with the project; how many lines were not rows.
pub fn tag_rows(text: &str, project: &str) -> (Vec<Map<String, Value>>, usize) {
    let mut bad = 0;
    let rows = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| match serde_json::from_str::<Value>(l) {
            Ok(Value::Object(mut m)) => {
                m.insert("project".into(), json!(project));
                Some(m)
            }
            _ => {
                bad += 1;
                None
            }
        })
        .collect();
    (rows, bad)
}

/// The canonical form of a merged table: one JSON object per line, ordered by project, then id.
pub fn canonical(rows: &mut [Map<String, Value>]) -> String {
    let key = |m: &Map<String, Value>| {
        (
            m["project"].to_string(),
            m.get("id").map(Value::to_string).unwrap_or_default(),
        )
    };
    rows.sort_by_key(key);
    rows.iter()
        .map(|r| format!("{}\n", Value::Object(r.clone())))
        .collect()
}

pub struct Harvest {
    pub projects: Vec<Value>,
    pub tables: BTreeMap<String, Vec<Map<String, Value>>>,
    pub problems: Vec<Value>,
}

/// Reads every confirmed candidate's exported tables through `source`.
pub fn harvest(candidates: &[Candidate], source: &dyn Source) -> Harvest {
    let mut h = Harvest {
        projects: Vec::new(),
        tables: BTreeMap::new(),
        problems: Vec::new(),
    };
    for c in candidates {
        let Some(mark) = &c.mark else {
            h.problems
                .push(json!({"id": c.found.repo, "project": c.found.repo, "problem": c.problem}));
            continue;
        };
        let id = &mark.project.id;
        h.projects.push(
            json!({"id": id, "project": id, "name": mark.project.name, "title": mark.project.title,
            "order": mark.project.order, "kind": mark.project.kind,
            "parent": mark.project.parent, "link": c.found.url, "platform": c.found.platform,
            "tables": mark.exports.keys().cloned().collect::<Vec<_>>()}),
        );
        // a lens declares its family and views in lens.toml: one row of the lenses table
        if mark.project.kind == "lens" {
            match source.read(&c.found, "lens.toml") {
                Ok(Some(text)) => match toml::from_str::<toml::Value>(&text) {
                    Ok(v) => {
                        let mut row = Map::new();
                        row.insert("id".into(), json!(id));
                        row.insert("project".into(), json!(id));
                        row.insert("lens".into(), serde_json::to_value(v).unwrap_or(Value::Null));
                        h.tables.entry("lenses".into()).or_default().push(row);
                    }
                    Err(e) => h.problems.push(json!({"id": format!("{id}:lens"), "project": id, "problem": format!("lens.toml does not read: {e}")})),
                },
                Ok(None) => h.problems.push(json!({"id": format!("{id}:lens"), "project": id, "problem": "a lens without lens.toml"})),
                Err(e) => h.problems.push(json!({"id": format!("{id}:lens"), "project": id, "problem": format!("{e:#}")})),
            }
        }
        for (table, path) in &mark.exports {
            match source.read(&c.found, path) {
                Ok(Some(text)) => {
                    let (rows, bad) = tag_rows(&text, id);
                    if bad > 0 {
                        h.problems.push(json!({"id": format!("{id}:{table}"), "project": id, "problem": format!("{bad} lines of {path} are not rows")}));
                    }
                    h.tables.entry(table.clone()).or_default().extend(rows);
                }
                Ok(None) => h.problems.push(json!({"id": format!("{id}:{table}"), "project": id, "problem": format!("{path} is exported but missing")})),
                Err(e) => h.problems.push(json!({"id": format!("{id}:{table}"), "project": id, "problem": format!("{e:#}")})),
            }
        }
    }
    h
}

/// Writes the harvest as tad tables into `dir`: one file per table, projects.json, problems.json.
pub fn write(h: &mut Harvest, dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let as_rows = |v: &[Value]| {
        v.iter()
            .filter_map(|x| x.as_object().cloned())
            .collect::<Vec<_>>()
    };
    std::fs::write(
        dir.join("projects.json"),
        canonical(&mut as_rows(&h.projects)),
    )?;
    std::fs::write(
        dir.join("problems.json"),
        canonical(&mut as_rows(&h.problems)),
    )?;
    for (table, rows) in h.tables.iter_mut() {
        std::fs::write(dir.join(format!("{table}.json")), canonical(rows))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_tagged_with_their_project_and_bad_lines_counted() {
        let (rows, bad) = tag_rows("{\"id\":\"a\"}\nnot json\n\n[1]\n{\"id\":\"b\"}\n", "o/r");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["project"], "o/r");
        assert_eq!(bad, 2);
    }

    #[test]
    fn a_merged_table_is_ordered_by_project_then_id() {
        let (mut rows, _) = tag_rows("{\"id\":\"b\"}\n{\"id\":\"a\"}\n", "z/z");
        let (more, _) = tag_rows("{\"id\":\"c\"}\n", "a/a");
        rows.extend(more);
        let c = canonical(&mut rows);
        let ids: Vec<&str> = c
            .lines()
            .map(|l| {
                if l.contains("\"c\"") {
                    "c"
                } else if l.contains("\"a\"") {
                    "a"
                } else {
                    "b"
                }
            })
            .collect();
        assert_eq!(ids, ["c", "a", "b"], "a/a's rows first, then z/z's by id");
    }

    #[test]
    fn an_empty_table_is_zero_rows_not_a_problem() {
        let (rows, bad) = tag_rows("", "o/r");
        assert!(rows.is_empty());
        assert_eq!(bad, 0);
    }

    struct Lensy;
    impl Source for Lensy {
        fn read(&self, _: &crate::discover::Found, path: &str) -> anyhow::Result<Option<String>> {
            Ok((path == "lens.toml").then(|| "schema = 1\n[lens]\ntitle = \"Phones\"\n[family]\nowners = [\"o\"]\n[[view]]\nid = \"features\"\nkind = \"matrix\"\n".to_string()))
        }
    }

    #[test]
    fn a_lens_becomes_a_row_of_lenses() {
        let mark = crate::mark::Mark::parse("schema = 1\n[project]\nid = \"o/lens\"\nname = \"Phones\"\nkind = \"lens\"\nparent = \"o\"\n").unwrap();
        let found = crate::discover::Found {
            platform: "fake".into(),
            repo: "o/lens".into(),
            branch: "main".into(),
            url: String::new(),
        };
        let h = harvest(
            &[crate::discover::Candidate {
                found,
                mark: Some(mark),
                problem: None,
            }],
            &Lensy,
        );
        let lens = &h.tables["lenses"][0];
        assert_eq!(lens["id"], "o/lens");
        assert_eq!(lens["lens"]["lens"]["title"], "Phones");
        assert_eq!(lens["lens"]["view"][0]["kind"], "matrix");
        assert!(h.problems.is_empty());
    }
}
