//! The relations between rows: which field refers to which table (each lens declares its own), and whether every
//! link holds. Until 2026-11-12 the relations were kept by discipline - about 860 links, a handful
//! pointing nowhere, and nothing to say so. The fulldome is one sky only if they are true.

use crate::join::rows;
use crate::mark::Mark;
use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// One declared relation: the rows of `from` (table.field) refer to rows of the tables in `to`.
#[derive(serde::Deserialize, Clone, Debug)]
pub struct Relation {
    pub from: String,
    pub to: Vec<String>,
    /// Marks of what is not a link to a row: a value holding one of these is passed over.
    #[serde(default)]
    pub passes_over: Vec<String>,
}

/// The relations of the shared language: those every lens declares.
pub fn relations() -> Vec<Relation> {
    crate::lenses::relations()
}

/// A link as written: an id, a table and an id (table:id), or a star, a table and an id.
#[derive(Debug, PartialEq, Eq)]
pub struct Link {
    pub star: Option<String>,
    pub table: Option<String>,
    pub id: String,
}

/// The links in one field's value: separated by commas or spaces.
pub fn links(value: &str) -> Vec<Link> {
    value
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|w| !w.is_empty())
        .map(|w| match w.split_once(':') {
            None => Link {
                star: None,
                table: None,
                id: w.to_string(),
            },
            Some((place, id)) => match place.rsplit_once('/') {
                Some((star, table)) => Link {
                    star: Some(star.to_string()),
                    table: Some(table.to_string()),
                    id: id.to_string(),
                },
                None => Link {
                    star: None,
                    table: Some(place.to_string()),
                    id: id.to_string(),
                },
            },
        })
        .collect()
}

/// The ids of every table a star keeps here.
fn ids(dir: &Path, mark: &Mark) -> BTreeMap<String, BTreeSet<String>> {
    mark.exports
        .keys()
        .map(|table| {
            (
                table.clone(),
                rows(dir, mark, table)
                    .iter()
                    .filter_map(|r| r["id"].as_str().map(str::to_string))
                    .collect(),
            )
        })
        .collect()
}

/// A link as it was written: an id, a table and an id, or a star, a table and an id.
fn written(link: &Link) -> String {
    match (&link.star, &link.table) {
        (Some(star), Some(table)) => format!("{star}/{table}:{}", link.id),
        (_, Some(table)) => format!("{table}:{}", link.id),
        _ => link.id.clone(),
    }
}

/// The edges of a graph, and the things they join.
pub type Graph = BTreeMap<&'static str, Vec<serde_json::Map<String, serde_json::Value>>>;

/// Every link of the star at `dir` that points nowhere, read off the graph - the one place that knows.
/// With `others` - the stars it may point into, by their id - links into another star are followed too,
/// and a star not among them is said to be missing; without, such links are left alone.
pub fn broken(dir: &Path, others: &BTreeMap<String, PathBuf>) -> Result<Vec<String>> {
    let here =
        Mark::parse(&std::fs::read_to_string(dir.join("oco.toml")).context("no oco.toml here")?)?
            .project
            .id;
    let mut dirs = vec![dir.to_path_buf()];
    dirs.extend(
        others
            .iter()
            .filter(|(id, _)| **id != here)
            .map(|(_, d)| d.clone()),
    );
    let graph = as_graph_from(&dirs, Some(&here))?;
    let text = |e: &serde_json::Map<String, serde_json::Value>, key: &str| {
        e.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    let mut said: Vec<String> = Vec::new();
    for e in &graph["edges"] {
        let through = text(e, "via_id");
        let from = if through.is_empty() {
            format!("{} {}", text(e, "from_table"), text(e, "from_id"))
        } else {
            format!("connections {through}")
        };
        let mut say = |field: &str, written: String, holds: Option<bool>, table: String| {
            let shown = if written.contains(':') {
                written.clone()
            } else {
                format!("{written} (in {table})")
            };
            match holds {
                Some(false) => said.push(format!("{from}: {field} -> {shown} - points nowhere")),
                None if !others.is_empty() => {
                    said.push(format!("{from}: {field} -> {written} - no such star here"))
                }
                _ => {}
            }
        };
        let holds = |key: &str| e.get(key).and_then(|v| v.as_bool());
        if through.is_empty() {
            say(
                &text(e, "how"),
                text(e, "written"),
                holds("holds"),
                text(e, "to_table"),
            );
        } else {
            say(
                "from",
                text(e, "from_written"),
                holds("from_holds"),
                "*".into(),
            );
            say("to", text(e, "written"), holds("holds"), "*".into());
        }
    }
    let mut once = BTreeSet::new();
    said.retain(|s| once.insert(s.clone()));
    Ok(said)
}

/// The tables that hold every link between everything in view as rows. No place keeps them: ocoeco fills
/// them from the links the places hold, as it fills the language's own from the lenses.
pub const OF_THE_GRAPH: [&str; 2] = ["things", "edges"];

/// A place in view: where it is, its mark, and the ids each of its tables holds.
type Held = (PathBuf, Mark, BTreeMap<String, BTreeSet<String>>);

/// Every row of every place in view as a thing, and every link between them as an edge: this, how, that.
/// A link a row holds in a declared relation is an edge named by its field (serves, refines, advances); a
/// connection is an edge named by how it bears (led to, needed, halted, repeats), through the connection
/// that says so. Whether an edge holds is said when the place it points into is in view, and left unsaid
/// when it is not. A thing is named by where it is held, its table and its id.
pub fn as_graph(dirs: &[PathBuf]) -> Result<Graph> {
    as_graph_from(dirs, None)
}

/// The graph of the places in view, with edges only from the place `only` when it is given: what one
/// place holds, followed into the others. The check and the guard on writes read it this way.
pub fn as_graph_from(dirs: &[PathBuf], only: Option<&str>) -> Result<Graph> {
    use serde_json::json;
    let mut places: BTreeMap<String, Held> = BTreeMap::new();
    for dir in dirs {
        let Ok(mark) = std::fs::read_to_string(dir.join("oco.toml"))
            .map_err(anyhow::Error::from)
            .and_then(|t| Mark::parse(&t))
        else {
            continue;
        };
        let own = ids(dir, &mark);
        places.insert(mark.project.id.clone(), (dir.clone(), mark, own));
    }
    let short = |id: &str| id.rsplit('/').next().unwrap_or(id).to_string();
    // where a link points, from the place that holds it, and whether that is there
    let resolve = |here: &str, link: &Link, to: &[String]| -> (String, Option<bool>) {
        let has = |place: &str, table: &str| {
            places
                .get(place)
                .map(|(_, _, own)| own.get(table).is_some_and(|ids| ids.contains(&link.id)))
        };
        match (&link.star, &link.table) {
            (Some(star), Some(table)) => (format!("{star}/{table}:{}", link.id), has(star, table)),
            (_, Some(table)) => (format!("{here}/{table}:{}", link.id), has(here, table)),
            (_, None) => {
                let own = places.get(here).map(|(_, _, own)| own);
                let any = to.iter().any(|t| t == "*");
                let found = own.and_then(|own| {
                    own.iter()
                        .find(|(table, ids)| (any || to.contains(table)) && ids.contains(&link.id))
                        .map(|(table, _)| table.clone())
                });
                match found {
                    Some(table) => (format!("{here}/{table}:{}", link.id), Some(true)),
                    None => (
                        format!(
                            "{here}/{}:{}",
                            to.first().map_or("?", String::as_str),
                            link.id
                        ),
                        Some(false),
                    ),
                }
            }
        }
    };
    let mut things = Vec::new();
    for (id, (_, _, own)) in &places {
        for (table, rows) in own {
            for row in rows {
                things.push(json!({"id": format!("{id}/{table}:{row}"), "of_table": table, "place": id, "star": short(id)}));
            }
        }
    }
    let mut edges: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    // an edge: from where, how, to where and whether that holds; the link as written at its far end; and,
    // for a connection, the one it came through and whether its near end holds as well
    let mut edge = |from: String,
                    how: &str,
                    (to, holds): (String, Option<bool>),
                    via: String,
                    here: &str,
                    written: String,
                    (from_written, from_holds): (String, Option<bool>)| {
        let id = if via.is_empty() {
            format!("{from} {how} {to}")
        } else {
            format!("{from} {how} {to} via {via}")
        };
        // each end said as its parts - where it is held, its table, its id - so that a lens joins on them
        // and nothing takes a written link apart again
        let ends = |node: &str| -> (String, String, String) {
            let (held, id) = node.split_once(':').unwrap_or((node, ""));
            let (place, table) = held.rsplit_once('/').unwrap_or(("", held));
            (place.to_string(), table.to_string(), id.to_string())
        };
        let ((fp, ft, fi), (tp, tt, ti), (vp, _, vi)) = (ends(&from), ends(&to), ends(&via));
        edges.insert(
            id.clone(),
            json!({"id": id, "from": from, "how": how, "to": to, "holds": holds, "via": via, "star": short(here),
                   "from_place": fp, "from_star": short(&fp), "from_table": ft, "from_id": fi,
                   "to_place": tp, "to_star": short(&tp), "to_table": tt, "to_id": ti,
                   "via_star": short(&vp), "via_id": vi,
                   "written": written, "from_written": from_written, "from_holds": from_holds}),
        );
    };
    for (here, (dir, mark, _)) in places
        .iter()
        .filter(|(here, _)| only.is_none_or(|o| o == here.as_str()))
    {
        for relation in relations() {
            let Some((table, field)) = relation.from.split_once('.') else {
                continue;
            };
            // a connection is not a row pointing at two things: it says how one bears on the other
            if table == "connections" {
                continue;
            }
            for row in rows(dir, mark, table) {
                let (Some(value), Some(id)) = (row[field].as_str(), row["id"].as_str()) else {
                    continue;
                };
                for link in links(value).iter().filter(|l| {
                    !relation
                        .passes_over
                        .iter()
                        .any(|m| l.id.contains(m.as_str()))
                }) {
                    edge(
                        format!("{here}/{table}:{id}"),
                        field,
                        resolve(here, link, &relation.to),
                        String::new(),
                        here,
                        written(link),
                        (String::new(), Some(true)),
                    );
                }
            }
        }
        let anywhere = ["*".to_string()];
        for row in rows(dir, mark, "connections") {
            let (Some(from), Some(to), Some(how), Some(id)) = (
                row["from"].as_str(),
                row["to"].as_str(),
                row["how"].as_str(),
                row["id"].as_str(),
            ) else {
                continue;
            };
            for this in links(from) {
                for that in links(to) {
                    let (near, near_holds) = resolve(here, &this, &anywhere);
                    edge(
                        near,
                        how,
                        resolve(here, &that, &anywhere),
                        format!("{here}/connections:{id}"),
                        here,
                        written(&that),
                        (written(&this), near_holds),
                    );
                }
            }
        }
    }
    let rows_of = |values: Vec<serde_json::Value>| {
        values
            .into_iter()
            .filter_map(|v| v.as_object().cloned())
            .collect::<Vec<_>>()
    };
    Ok(BTreeMap::from([
        ("things", rows_of(things)),
        ("edges", rows_of(edges.into_values().collect())),
    ]))
}

/// Runs `write` on the place at `dir`, refusing what it wrote if it adds a link that points nowhere: the
/// place's tables are put back as they were, and the new broken links are named. Links already broken
/// stay as they are - they are the check's to report, not the write's.
pub fn guarded<T>(dir: &Path, write: impl FnOnce() -> Result<T>) -> Result<T> {
    let mark =
        Mark::parse(&std::fs::read_to_string(dir.join("oco.toml")).context("no oco.toml here")?)?;
    let before: BTreeSet<String> = broken(dir, &BTreeMap::new())?.into_iter().collect();
    let saved: Vec<(PathBuf, Option<String>)> = mark
        .exports
        .values()
        .map(|p| {
            let path = dir.join(p.split('@').next().unwrap_or(p));
            let text = std::fs::read_to_string(&path).ok();
            (path, text)
        })
        .collect();
    let done = write()?;
    let added: Vec<String> = broken(dir, &BTreeMap::new())?
        .into_iter()
        .filter(|p| !before.contains(p))
        .collect();
    if !added.is_empty() {
        for (path, text) in saved {
            match text {
                Some(text) => std::fs::write(&path, text)?,
                None => {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
        bail!(
            "nothing written - the change above was put back: it would add a link that points nowhere\n  {}",
            added.join("\n  ")
        );
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_holds_links_in_three_forms() {
        assert_eq!(
            links("crash-only, kubernetes-jobs"),
            vec![
                Link {
                    star: None,
                    table: None,
                    id: "crash-only".into()
                },
                Link {
                    star: None,
                    table: None,
                    id: "kubernetes-jobs".into()
                }
            ]
        );
        assert_eq!(
            links("backlog:a  decisions:2026-11-12-b")[1],
            Link {
                star: None,
                table: Some("decisions".into()),
                id: "2026-11-12-b".into()
            }
        );
        assert_eq!(
            links("creation-guidelines/lab-agent/decisions:2026-11-12-the-switch")[0],
            Link {
                star: Some("creation-guidelines/lab-agent".into()),
                table: Some("decisions".into()),
                id: "2026-11-12-the-switch".into()
            }
        );
    }

    #[test]
    fn every_declared_relation_names_a_table_and_a_field() {
        let all = relations();
        assert!(all.len() >= 12);
        assert!(all
            .iter()
            .all(|r| r.from.split_once('.').is_some() && !r.to.is_empty()));
    }

    #[test]
    fn a_write_that_adds_a_link_to_nowhere_is_put_back() {
        let d = std::env::temp_dir().join(format!("ocoeco-guarded-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\ndecisions = \"data/decisions.json\"\n").unwrap();
        let was = "{\"id\":\"d1\"}\n{\"id\":\"old\",\"refines\":\"long-gone\"}\n";
        std::fs::write(d.join("data/decisions.json"), was).unwrap();
        let add = |line: &'static str| {
            let file = d.join("data/decisions.json");
            move || -> Result<()> {
                let mut text = std::fs::read_to_string(&file)?;
                text.push_str(line);
                std::fs::write(&file, text)?;
                Ok(())
            }
        };
        let refused = guarded(&d, add("{\"id\":\"d2\",\"refines\":\"d9\"}\n"))
            .unwrap_err()
            .to_string();
        assert!(
            refused.contains("nothing written")
                && refused.contains("d9")
                && !refused.contains("long-gone"),
            "{refused}"
        );
        assert_eq!(
            std::fs::read_to_string(d.join("data/decisions.json")).unwrap(),
            was,
            "put back as it was"
        );
        guarded(&d, add("{\"id\":\"d3\",\"refines\":\"d1\"}\n")).unwrap();
        assert!(std::fs::read_to_string(d.join("data/decisions.json"))
            .unwrap()
            .contains("d3"));
    }

    #[test]
    fn a_link_that_points_nowhere_is_found_and_one_that_holds_is_not() {
        let d = std::env::temp_dir().join(format!("ocoeco-relations-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\ndecisions = \"data/decisions.json\"\nsources = \"data/sources.json\"\n").unwrap();
        std::fs::write(d.join("data/sources.json"), "{\"id\":\"known\"}\n").unwrap();
        std::fs::write(d.join("data/decisions.json"), "{\"id\":\"d1\",\"sources\":\"known, unknown\"}\n{\"id\":\"d2\",\"refines\":\"d1\"}\n{\"id\":\"d3\",\"refines\":\"d9\"}\n").unwrap();
        let problems = broken(&d, &BTreeMap::new()).unwrap();
        assert_eq!(
            problems,
            vec![
                "decisions d1: sources -> unknown (in sources) - points nowhere",
                "decisions d3: refines -> d9 (in decisions) - points nowhere"
            ]
        );
    }
}

#[cfg(test)]
mod the_graph {
    use super::*;

    fn place(base: &Path, id: &str, tables: &[(&str, &str)]) -> PathBuf {
        let d = base.join(id.replace('/', "-"));
        std::fs::create_dir_all(d.join("data")).unwrap();
        let exports: String = tables
            .iter()
            .map(|(t, _)| format!("{t} = \"data/{t}.json\"\n"))
            .collect();
        std::fs::write(d.join("oco.toml"), format!("schema = 1\n[project]\nid = \"{id}\"\nname = \"P: a place\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\n{exports}")).unwrap();
        for (t, rows) in tables {
            std::fs::write(d.join(format!("data/{t}.json")), rows).unwrap();
        }
        d
    }

    #[test]
    fn every_row_is_a_thing_and_every_link_an_edge_this_how_that() {
        let base = std::env::temp_dir().join(format!("ocoeco-graph-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let a = place(&base, "o/a", &[
            ("backlog", "{\"id\":\"a1\",\"serves\":\"milestones:m1\"}\n"),
            ("milestones", "{\"id\":\"m1\"}\n{\"id\":\"m2\"}\n"),
            ("connections", "{\"id\":\"c1\",\"from\":\"milestones:m1\",\"how\":\"led to\",\"to\":\"o/b/decisions:x\"}\n"),
        ]);
        let b = place(&base, "o/b", &[("decisions", "{\"id\":\"x\"}\n")]);
        let graph = as_graph(&[a.clone(), b]).unwrap();
        let edge = |from: &str, how: &str| {
            graph["edges"]
                .iter()
                .find(|e| e["from"] == from && e["how"] == how)
                .cloned()
        };
        let served = edge("o/a/backlog:a1", "serves")
            .expect("a link a row holds is an edge named by its field");
        assert_eq!(
            (served["to"].as_str(), served["holds"].as_bool()),
            (Some("o/a/milestones:m1"), Some(true))
        );
        let led = edge("o/a/milestones:m1", "led to")
            .expect("a connection is an edge named by how it bears");
        assert_eq!(
            (
                led["to"].as_str(),
                led["holds"].as_bool(),
                led["via"].as_str()
            ),
            (
                Some("o/b/decisions:x"),
                Some(true),
                Some("o/a/connections:c1")
            )
        );
        assert_eq!(
            graph["things"].len(),
            5,
            "every row of every place is a thing"
        );
        let pointed: BTreeSet<&str> = graph["edges"]
            .iter()
            .filter_map(|e| e["to"].as_str())
            .collect();
        assert!(
            !pointed.contains("o/a/milestones:m2"),
            "and some are pointed at by nothing"
        );
        // from one place alone, a link into another is neither held nor broken: it is not seen
        let alone = as_graph(&[a]).unwrap();
        let led = alone["edges"]
            .iter()
            .find(|e| e["how"] == "led to")
            .unwrap();
        assert!(led["holds"].is_null(), "{led:?}");
        // each end is said as its parts, and what an edge carries is what the lens of the graph declares
        let served = graph["edges"]
            .iter()
            .find(|e| e["how"] == "serves")
            .unwrap();
        assert_eq!(
            [
                &served["from_star"],
                &served["from_table"],
                &served["from_id"],
                &served["to_star"],
                &served["to_table"],
                &served["to_id"]
            ]
            .map(|v| v.as_str().unwrap_or_default()),
            ["a", "backlog", "a1", "a", "milestones", "m1"]
        );
        let lens = crate::lenses::lenses()
            .into_iter()
            .find(|l| l.lens.id == "graph")
            .unwrap();
        let declared: BTreeSet<String> = lens.tables["edges"]["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["name"].as_str().unwrap().to_string())
            .chain(["star".to_string()])
            .collect();
        for e in &graph["edges"] {
            assert_eq!(
                e.keys().cloned().collect::<BTreeSet<_>>(),
                declared,
                "an edge carries what the lens declares"
            );
        }
    }
}
