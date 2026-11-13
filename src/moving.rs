//! Moving rows from one place to another, every link kept true. A row is not only its own line: other
//! rows point at it, and it points at others. So when it moves, each link to it - in the place it
//! leaves, in the place it comes to, and in any other place held here - is rewritten to where it now
//! is, and its own links to what stays behind say which place that is. If anything would then point
//! nowhere, nothing is moved: every file is put back as it was.

use crate::mark::Mark;
use crate::note::Row;
use crate::relations::{broken, links, relations, Relation};
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// What a move did.
#[derive(Debug, Default, PartialEq)]
pub struct Moved {
    pub rows: usize,
    /// Links rewritten, by the place they were in.
    pub rewritten: BTreeMap<String, usize>,
}

fn mark(dir: &Path) -> Result<Mark> {
    Mark::parse(
        &std::fs::read_to_string(dir.join("oco.toml"))
            .with_context(|| format!("{} is not a marked place", dir.display()))?,
    )
}

/// Where a place keeps a table on this branch, if it does.
fn kept(dir: &Path, mark: &Mark, table: &str) -> Option<PathBuf> {
    mark.exports
        .get(table)
        .filter(|path| !path.contains('@'))
        .map(|path| dir.join(path))
}

fn read(path: &Path) -> Result<Vec<Row>> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            serde_json::from_str::<Row>(l)
                .with_context(|| format!("a line of {} is not a row", path.display()))
        })
        .collect()
}

fn id(row: &Row) -> String {
    row.0
        .iter()
        .find(|(k, _)| k == "id")
        .and_then(|(_, v)| v.as_str())
        .unwrap_or_default()
        .to_string()
}

/// Each link in a field's value rewritten by `to` (None leaves it as written), separators kept.
pub fn rewritten(value: &str, mut to: impl FnMut(&str) -> Option<String>) -> String {
    let (mut out, mut token) = (String::new(), String::new());
    let mut flush = |token: &mut String, out: &mut String| {
        if !token.is_empty() {
            out.push_str(&to(token).unwrap_or_else(|| token.clone()));
            token.clear();
        }
    };
    for c in value.chars() {
        if c == ',' || c.is_whitespace() {
            flush(&mut token, &mut out);
            out.push(c);
        } else {
            token.push(c);
        }
    }
    flush(&mut token, &mut out);
    out
}

/// A link to a row of this place, as it is written within it: its bare id where the field can only
/// mean that table, table:id otherwise.
fn local(relation: &Relation, table: &str, id: &str) -> String {
    if relation.to.len() == 1 && relation.to[0] == table {
        id.to_string()
    } else {
        format!("{table}:{id}")
    }
}

/// Rewrites, in every table of the place at `dir` that holds links, each link by `to` (given the
/// relation it is in and the link as written); `skip` leaves some rows of a table out. Keeps what each
/// file was in `before`, and says how many links changed.
fn relink(
    dir: &Path,
    mark: &Mark,
    before: &mut BTreeMap<PathBuf, String>,
    mut to: impl FnMut(&Relation, &str) -> Option<String>,
) -> Result<usize> {
    let mut changed = 0;
    let mut by_table: BTreeMap<&str, Vec<(&str, Relation)>> = BTreeMap::new();
    let all = relations();
    for relation in &all {
        if let Some((table, field)) = relation.from.split_once('.') {
            by_table
                .entry(table)
                .or_default()
                .push((field, relation.clone()));
        }
    }
    for (table, fields) in by_table {
        let Some(path) = kept(dir, mark, table) else {
            continue;
        };
        let mut rows = read(&path)?;
        let mut here = 0;
        for row in &mut rows {
            for (field, relation) in &fields {
                let field: &str = field;
                let Some(Value::String(value)) = row
                    .0
                    .iter()
                    .find(|(k, _)| k == field)
                    .map(|(_, v)| v.clone())
                else {
                    continue;
                };
                let new = rewritten(&value, |token| to(relation, token));
                if new != value {
                    here += value
                        .split(|c: char| c == ',' || c.is_whitespace())
                        .filter(|t| !t.is_empty())
                        .zip(
                            new.split(|c: char| c == ',' || c.is_whitespace())
                                .filter(|t| !t.is_empty()),
                        )
                        .filter(|(a, b)| a != b)
                        .count();
                    if let Some(entry) = row.0.iter_mut().find(|(k, _)| k == field) {
                        entry.1 = Value::String(new);
                    }
                }
            }
        }
        if here > 0 {
            before
                .entry(path.clone())
                .or_insert(std::fs::read_to_string(&path).unwrap_or_default());
            std::fs::write(
                &path,
                rows.iter().map(|r| r.line() + "\n").collect::<String>(),
            )?;
            changed += here;
        }
    }
    Ok(changed)
}

/// Moves the rows `ids` of `table` from the place at `from` to the place at `to`. `others` are the
/// places held here, by their id: links in them to the moved rows are rewritten too.
pub fn move_rows(
    from: &Path,
    to: &Path,
    table: &str,
    ids: &[String],
    others: &BTreeMap<String, PathBuf>,
) -> Result<Moved> {
    let (a, b) = (mark(from)?, mark(to)?);
    let (a_id, b_id) = (a.project.id.clone(), b.project.id.clone());
    if a_id == b_id {
        bail!("{a_id} is where they already are");
    }
    let a_file =
        kept(from, &a, table).with_context(|| format!("{a_id} does not keep a table {table}"))?;
    let b_file = kept(to, &b, table).with_context(|| {
        format!("{b_id} does not keep a table {table}: add it to its mark first")
    })?;
    let moving: BTreeSet<&String> = ids.iter().collect();
    let (go, stay): (Vec<Row>, Vec<Row>) = read(&a_file)?
        .into_iter()
        .partition(|row| moving.contains(&id(row)));
    if let Some(absent) = ids.iter().find(|i| !go.iter().any(|row| &id(row) == *i)) {
        bail!("{a_id} has no {table} called {absent}");
    }
    if let Some(taken) = read(&b_file)?.iter().map(id).find(|i| moving.contains(i)) {
        bail!("{b_id} already has a {table} called {taken}");
    }
    // which table of the place they leave holds each id: a bare id is told by it
    let in_a: BTreeMap<String, BTreeSet<String>> = a
        .exports
        .keys()
        .filter_map(|t| {
            kept(from, &a, t).map(|p| {
                (
                    t.clone(),
                    read(&p).unwrap_or_default().iter().map(id).collect(),
                )
            })
        })
        .collect();
    let all: BTreeMap<String, PathBuf> = others
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .chain([
            (a_id.clone(), from.to_path_buf()),
            (b_id.clone(), to.to_path_buf()),
        ])
        .collect();
    let broken_before: BTreeSet<String> = [from, to]
        .iter()
        .flat_map(|d| broken(d, &all).unwrap_or_default())
        .collect();
    let mut before: BTreeMap<PathBuf, String> = BTreeMap::new();
    let done = (|| -> Result<Moved> {
        let mut moved = Moved {
            rows: go.len(),
            ..Default::default()
        };
        // the rows themselves: their links to what stays behind say which place that is
        let mut go = go.clone();
        for row in &mut go {
            for relation in relations()
                .iter()
                .filter(|r| r.from.split_once('.').is_some_and(|(t, _)| t == table))
            {
                let field = relation
                    .from
                    .split_once('.')
                    .map(|(_, f)| f)
                    .unwrap_or_default();
                let Some(Value::String(value)) = row
                    .0
                    .iter()
                    .find(|(k, _)| k == field)
                    .map(|(_, v)| v.clone())
                else {
                    continue;
                };
                let new = rewritten(&value, |token| {
                    let link = links(token).into_iter().next()?;
                    match (&link.star, &link.table) {
                        (Some(star), Some(t)) if *star == b_id => {
                            Some(local(relation, t, &link.id))
                        }
                        (Some(_), _) => None,
                        (None, given) => {
                            let t = match given {
                                Some(t) => Some(t.clone()),
                                None => relation
                                    .to
                                    .iter()
                                    .find(|t| in_a.get(*t).is_some_and(|s| s.contains(&link.id)))
                                    .cloned(),
                            }?;
                            let comes_too = t == table && moving.contains(&link.id);
                            (!comes_too && in_a.get(&t).is_some_and(|s| s.contains(&link.id)))
                                .then(|| format!("{a_id}/{t}:{}", link.id))
                        }
                    }
                });
                if let Some(entry) = row.0.iter_mut().find(|(k, _)| k == field) {
                    entry.1 = Value::String(new);
                }
            }
        }
        // out of the place they leave ...
        before.insert(
            a_file.clone(),
            std::fs::read_to_string(&a_file).unwrap_or_default(),
        );
        std::fs::write(
            &a_file,
            stay.iter().map(|r| r.line() + "\n").collect::<String>(),
        )?;
        // ... where what pointed at them now says where they went
        let left = relink(from, &a, &mut before, |relation, token| {
            let link = links(token).into_iter().next()?;
            let meant = match (&link.star, &link.table) {
                (None, Some(t)) => t == table,
                (None, None) => relation
                    .to
                    .iter()
                    .find(|t| in_a.get(*t).is_some_and(|s| s.contains(&link.id)))
                    .is_some_and(|t| t == table),
                _ => false,
            };
            (meant && moving.contains(&link.id)).then(|| format!("{b_id}/{table}:{}", link.id))
        })?;
        // into the place they come to, where what pointed across at them now points at home
        let came = relink(to, &b, &mut before, |relation, token| {
            let link = links(token).into_iter().next()?;
            (link.star.as_deref() == Some(a_id.as_str())
                && link.table.as_deref() == Some(table)
                && moving.contains(&link.id))
            .then(|| local(relation, table, &link.id))
        })?;
        before
            .entry(b_file.clone())
            .or_insert(std::fs::read_to_string(&b_file).unwrap_or_default());
        let mut there = std::fs::read_to_string(&b_file).unwrap_or_default();
        there.push_str(&go.iter().map(|r| r.line() + "\n").collect::<String>());
        std::fs::write(&b_file, there)?;
        for (place, n) in [(&a_id, left), (&b_id, came)] {
            if n > 0 {
                moved.rewritten.insert(place.clone(), n);
            }
        }
        // and every other place held here that pointed at them
        for (other_id, dir) in others.iter().filter(|(i, _)| **i != a_id && **i != b_id) {
            let Ok(other) = mark(dir) else { continue };
            let n = relink(dir, &other, &mut before, |_, token| {
                let link = links(token).into_iter().next()?;
                (link.star.as_deref() == Some(a_id.as_str())
                    && link.table.as_deref() == Some(table)
                    && moving.contains(&link.id))
                .then(|| format!("{b_id}/{table}:{}", link.id))
            })?;
            if n > 0 {
                moved.rewritten.insert(other_id.clone(), n);
            }
        }
        let added: Vec<String> = [from, to]
            .iter()
            .flat_map(|d| broken(d, &all).unwrap_or_default())
            .filter(|p| !broken_before.contains(p))
            .collect();
        if !added.is_empty() {
            bail!(
                "it would leave links pointing nowhere:\n  {}",
                added.join("\n  ")
            );
        }
        Ok(moved)
    })();
    if done.is_err() {
        for (path, text) in &before {
            std::fs::write(path, text)?;
        }
    }
    done.map_err(|e| e.context("nothing moved - every file is as it was"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(name: &str, id: &str, tables: &[(&str, &str)]) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ocoeco-moving-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("data")).unwrap();
        let exports: String = tables
            .iter()
            .map(|(t, _)| format!("{t} = \"data/{t}.json\"\n"))
            .collect();
        std::fs::write(d.join("oco.toml"), format!("schema = 1\n[project]\nid = \"{id}\"\nname = \"P: a place\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\n{exports}")).unwrap();
        for (t, text) in tables {
            std::fs::write(d.join(format!("data/{t}.json")), text).unwrap();
        }
        d
    }
    fn table(d: &Path, t: &str) -> String {
        std::fs::read_to_string(d.join(format!("data/{t}.json"))).unwrap()
    }

    #[test]
    fn links_are_rewritten_and_their_separators_kept() {
        let r = rewritten("a, b  c", |t| (t == "b").then(|| "o/x/t:b".to_string()));
        assert_eq!(r, "a, o/x/t:b  c");
        assert_eq!(rewritten("", |_| Some("x".into())), "");
    }

    #[test]
    fn a_row_moves_and_every_link_stays_true() {
        let a = place("a", "o/a", &[
            ("decisions", "{\"id\":\"d1\",\"research\":\"r1\"}\n{\"id\":\"d2\",\"refines\":\"d1\"}\n{\"id\":\"d3\",\"refines\":\"d1, d2\"}\n"),
            ("research", "{\"id\":\"r1\"}\n"),
            ("vocabulary", "{\"id\":\"w\",\"decided\":\"d1\"}\n"),
            ("reflections", "{\"id\":\"x\",\"touches\":\"decisions:d1 decisions:d3\"}\n"),
        ]);
        let b = place(
            "b",
            "o/b",
            &[(
                "decisions",
                "{\"id\":\"e1\",\"refines\":\"o/a/decisions:d1\"}\n",
            )],
        );
        let c = place(
            "c",
            "o/c",
            &[(
                "reflections",
                "{\"id\":\"y\",\"touches\":\"o/a/decisions:d1, o/a/decisions:d2\"}\n",
            )],
        );
        let others: BTreeMap<String, PathBuf> = [("o/c".to_string(), c.clone())].into();
        let moved = move_rows(&a, &b, "decisions", &["d1".into()], &others).unwrap();
        assert_eq!(
            (moved.rows, moved.rewritten),
            (
                1,
                [
                    ("o/a".to_string(), 4),
                    ("o/b".to_string(), 1),
                    ("o/c".to_string(), 1)
                ]
                .into()
            )
        );
        // what stayed points across at where it went; what did not point at it is untouched
        assert_eq!(table(&a, "decisions"), "{\"id\":\"d2\",\"refines\":\"o/b/decisions:d1\"}\n{\"id\":\"d3\",\"refines\":\"o/b/decisions:d1, d2\"}\n");
        assert_eq!(
            table(&a, "vocabulary"),
            "{\"id\":\"w\",\"decided\":\"o/b/decisions:d1\"}\n"
        );
        assert_eq!(
            table(&a, "reflections"),
            "{\"id\":\"x\",\"touches\":\"o/b/decisions:d1 decisions:d3\"}\n"
        );
        // where it came, what pointed across now points at home; and it says where what it rests on stayed
        assert_eq!(table(&b, "decisions"), "{\"id\":\"e1\",\"refines\":\"d1\"}\n{\"id\":\"d1\",\"research\":\"o/a/research:r1\"}\n");
        assert_eq!(
            table(&c, "reflections"),
            "{\"id\":\"y\",\"touches\":\"o/b/decisions:d1, o/a/decisions:d2\"}\n"
        );
        let all: BTreeMap<String, PathBuf> = [
            ("o/a".to_string(), a.clone()),
            ("o/b".to_string(), b.clone()),
            ("o/c".to_string(), c.clone()),
        ]
        .into();
        for d in [&a, &b, &c] {
            assert!(broken(d, &all).unwrap().is_empty(), "{:?}", broken(d, &all));
        }
    }

    #[test]
    fn rows_that_move_together_keep_their_links_and_a_wrong_move_changes_nothing() {
        let a = place(
            "a2",
            "o/a",
            &[(
                "decisions",
                "{\"id\":\"d1\"}\n{\"id\":\"d2\",\"refines\":\"d1\"}\n",
            )],
        );
        let b = place("b2", "o/b", &[("decisions", "{\"id\":\"d2x\"}\n")]);
        let none = BTreeMap::new();
        let was = table(&a, "decisions");
        assert!(
            move_rows(&a, &b, "decisions", &["d1".into(), "nope".into()], &none)
                .unwrap_err()
                .to_string()
                .contains("has no decisions called nope")
        );
        assert!(move_rows(&a, &b, "research", &["d1".into()], &none)
            .unwrap_err()
            .to_string()
            .contains("does not keep a table research"));
        assert_eq!(
            (table(&a, "decisions"), table(&b, "decisions")),
            (was, "{\"id\":\"d2x\"}\n".to_string()),
            "nothing moved"
        );
        move_rows(&a, &b, "decisions", &["d1".into(), "d2".into()], &none).unwrap();
        assert_eq!(
            (table(&a, "decisions"), table(&b, "decisions")),
            (
                String::new(),
                "{\"id\":\"d2x\"}\n{\"id\":\"d1\"}\n{\"id\":\"d2\",\"refines\":\"d1\"}\n"
                    .to_string()
            )
        );
    }
}
