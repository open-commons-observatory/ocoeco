//! The harvest: every confirmed project's exported tables, each row tagged with its project, merged
//! by table name into the observatory's own tad data - with a registry of the projects and a list of
//! problems (a broken mark, a table that could not be read), shown rather than lost.

use crate::discover::{Candidate, Runs, Source};
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
            // a table may live on another branch of its repository: path@branch
            let (file, branch) = path.split_once('@').unwrap_or((path.as_str(), ""));
            let at = if branch.is_empty() {
                c.found.clone()
            } else {
                crate::discover::Found {
                    branch: branch.to_string(),
                    ..c.found.clone()
                }
            };
            match source.read(&at, file) {
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

/// Observes what has run in every confirmed candidate: the latest finished run of each workflow on its
/// default branch, as rows of the table runs - each named by its project and workflow. A run that
/// failed is a problem derived from the data: shown while it is true, gone when the next run passes.
pub fn observe_runs(candidates: &[Candidate], runs: &dyn Runs, h: &mut Harvest) {
    h.tables.entry("runs".into()).or_default();
    for c in candidates {
        let Some(mark) = &c.mark else { continue };
        let id = &mark.project.id;
        match runs.latest(&c.found) {
            Ok(latest) => {
                for r in latest {
                    let row = json!({"id": format!("{id}:{}", r.workflow), "project": id, "workflow": r.workflow, "conclusion": r.conclusion, "at": r.at, "url": r.url});
                    if let Some(row) = row.as_object() {
                        h.tables.entry("runs".into()).or_default().push(row.clone());
                    }
                }
            }
            Err(e) => h.problems.push(json!({"id": format!("{id}:runs"), "project": id, "problem": format!("its runs could not be read: {e:#}")})),
        }
    }
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
    fn runs_are_observed_as_rows_and_an_unreadable_place_is_a_problem() {
        use crate::discover::{Found, Run};
        struct Seen;
        impl Runs for Seen {
            fn latest(&self, found: &Found) -> Result<Vec<Run>> {
                if found.repo == "o/broken" {
                    anyhow::bail!("403");
                }
                Ok(vec![Run {
                    workflow: "release".into(),
                    conclusion: "failure".into(),
                    at: "2026-11-12".into(),
                    url: "u".into(),
                }])
            }
        }
        let candidate = |repo: &str| {
            Candidate {
            found: Found { platform: "github".into(), repo: repo.into(), branch: "main".into(), url: String::new() },
            mark: Some(crate::mark::Mark::parse(&format!("schema = 1\n[project]\nid = \"{repo}\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n")).unwrap()),
            problem: None,
        }
        };
        let mut h = Harvest {
            projects: Vec::new(),
            tables: BTreeMap::new(),
            problems: Vec::new(),
        };
        observe_runs(&[candidate("o/r"), candidate("o/broken")], &Seen, &mut h);
        let rows = &h.tables["runs"];
        assert_eq!(
            (
                rows.len(),
                rows[0]["id"].as_str(),
                rows[0]["conclusion"].as_str()
            ),
            (1, Some("o/r:release"), Some("failure"))
        );
        assert!(h.problems[0]["problem"]
            .as_str()
            .unwrap()
            .contains("its runs could not be read: 403"));
    }

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

    struct OnDataBranch;
    impl Source for OnDataBranch {
        fn read(&self, f: &crate::discover::Found, path: &str) -> anyhow::Result<Option<String>> {
            Ok((f.branch == "oco-data" && path == "oco/gathered/ci.json")
                .then(|| "{\"id\":\"CI\"}\n".to_string()))
        }
    }

    #[test]
    fn a_table_is_read_from_the_branch_its_mark_names() {
        let mark = crate::mark::Mark::parse("schema = 1\n[project]\nid = \"o/r\"\nname = \"R\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nci = \"oco/gathered/ci.json@oco-data\"\n").unwrap();
        let found = crate::discover::Found {
            platform: "fake".into(),
            repo: "o/r".into(),
            branch: "main".into(),
            url: String::new(),
        };
        let h = harvest(
            &[crate::discover::Candidate {
                found,
                mark: Some(mark),
                problem: None,
            }],
            &OnDataBranch,
        );
        assert_eq!(h.tables["ci"].len(), 1, "read from oco-data, not from main");
        assert!(h.problems.is_empty());
    }
}
