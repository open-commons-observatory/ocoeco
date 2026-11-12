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

/// Every link of the star at `dir` that points nowhere. With `others` - the stars it may point into,
/// by their id - links into another star are checked too; without, they are left alone.
pub fn broken(dir: &Path, others: &BTreeMap<String, PathBuf>) -> Result<Vec<String>> {
    let read = |d: &Path| -> Result<Mark> {
        Mark::parse(&std::fs::read_to_string(d.join("oco.toml")).context("no oco.toml here")?)
    };
    let mark = read(dir)?;
    let own = ids(dir, &mark);
    let mut abroad: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();
    let mut problems = Vec::new();
    for relation in relations() {
        let Some((table, field)) = relation.from.split_once('.') else {
            continue;
        };
        for row in rows(dir, &mark, table) {
            let Some(value) = row[field].as_str() else {
                continue;
            };
            let from = format!("{table} {}: {field}", row["id"].as_str().unwrap_or("?"));
            for link in links(value) {
                if relation
                    .passes_over
                    .iter()
                    .any(|mark| link.id.contains(mark.as_str()))
                {
                    continue;
                }
                let found = match (&link.star, &link.table) {
                    (Some(star), Some(t)) => {
                        let Some(there) = others.get(star) else {
                            if !others.is_empty() {
                                problems.push(format!(
                                    "{from} -> {star}/{t}:{} - no such star here",
                                    link.id
                                ));
                            }
                            continue;
                        };
                        if !abroad.contains_key(star) {
                            abroad.insert(star.clone(), ids(there, &read(there)?));
                        }
                        abroad[star].get(t).is_some_and(|s| s.contains(&link.id))
                    }
                    (_, Some(t)) => own.get(t).is_some_and(|s| s.contains(&link.id)),
                    (_, None) => relation
                        .to
                        .iter()
                        .any(|t| own.get(t).is_some_and(|s| s.contains(&link.id))),
                };
                if !found {
                    let written = match (&link.star, &link.table) {
                        (Some(s), Some(t)) => format!("{s}/{t}:{}", link.id),
                        (_, Some(t)) => format!("{t}:{}", link.id),
                        _ => format!("{} (in {})", link.id, relation.to.join(" or ")),
                    };
                    problems.push(format!("{from} -> {written} - points nowhere"));
                }
            }
        }
    }
    Ok(problems)
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
