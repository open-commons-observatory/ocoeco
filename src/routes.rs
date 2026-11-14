//! The ways in: how a place is got to, and as of when. A place is what its mark says it is, the same along
//! every route; a route is how what it keeps reaches a lens. Routes differ in as of when, and a route must
//! not hide it: what was observed is as of when it was observed, not now.

use std::path::{Path, PathBuf};

/// A folder with a mark, read where the mark says each table is kept: as it is now.
pub const HELD: &str = "held here";
/// What an observatory observed and wrote down, each row naming its place: as of when it observed.
pub const OBSERVED: &str = "observed";
/// A record or a copy the machine keeps in its own data folder: as of when it was last written.
pub const KEPT: &str = "kept on this machine";

/// By which route what a folder holds is got to, and as of when it is so.
pub fn route(dir: &Path) -> (&'static str, String) {
    if !dir.join("oco.toml").is_file() && dir.join("projects.json").is_file() {
        return (
            OBSERVED,
            written(committed(dir).or_else(|| changed(&dir.join("projects.json")))),
        );
    }
    let kept = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("share"))
        });
    let within = |home: &PathBuf| {
        std::fs::canonicalize(dir)
            .ok()
            .zip(std::fs::canonicalize(home).ok())
            .is_some_and(|(d, h)| d.starts_with(h))
    };
    if kept.as_ref().is_some_and(within) {
        let newest = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|f| changed(&f.path()))
            .max();
        return (KEPT, written(newest));
    }
    (HELD, "as it is now".into())
}

/// When what a folder holds was last committed, in seconds - where it is kept in git.
fn committed(dir: &Path) -> Option<i64> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["log", "-1", "--format=%ct", "--", "."])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// When a file was last changed, in seconds.
fn changed(file: &Path) -> Option<i64> {
    let at = std::fs::metadata(file).ok()?.modified().ok()?;
    Some(at.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64)
}

/// A moment as words: the day and the minute, or that it is not known.
fn written(secs: Option<i64>) -> String {
    match secs {
        Some(secs) => {
            let (moment, _) = crate::glimpse::now_and_id(secs);
            format!("as of {} {} UTC", &moment[..10], &moment[11..16])
        }
        None => "as of a moment not known".into(),
    }
}

/// How many rows of a table each route brings, among these places: the route, the places that came by it,
/// their rows, and as of when - the earliest and the latest where they differ.
pub fn of_a_table(table: &str, dirs: &[PathBuf]) -> Vec<String> {
    let mut by: std::collections::BTreeMap<&'static str, (usize, usize, Vec<String>)> =
        Default::default();
    for dir in dirs {
        let (route, as_of) = route(dir);
        let rows = if route == OBSERVED {
            crate::harvest::read_rows(&dir.join(format!("{table}.json"))).len()
        } else {
            std::fs::read_to_string(dir.join("oco.toml"))
                .ok()
                .and_then(|t| crate::mark::Mark::parse(&t).ok())
                .map_or(0, |mark| crate::join::rows(dir, &mark, table).len())
        };
        if rows > 0 {
            let entry = by.entry(route).or_default();
            entry.0 += 1;
            entry.1 += rows;
            entry.2.push(as_of);
        }
    }
    by.into_iter()
        .map(|(route, (places, rows, mut moments))| {
            moments.sort();
            moments.dedup();
            let when = match moments.as_slice() {
                [one] => one.clone(),
                [first, .., last] => format!("{first} to {}", last.trim_start_matches("as of ")),
                [] => String::new(),
            };
            match route {
                OBSERVED => format!("{rows} rows {route}, {when}"),
                _ => format!("{rows} rows from {places} place(s) {route}, {when}"),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_route_says_how_a_place_is_got_to_and_as_of_when() {
        let base = std::env::temp_dir().join(format!("ocoeco-routes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (held, seen) = (base.join("o/held"), base.join("seen"));
        std::fs::create_dir_all(held.join("data")).unwrap();
        std::fs::create_dir_all(&seen).unwrap();
        std::fs::write(held.join("oco.toml"), "schema = 1\n[project]\nid = \"o/held\"\nname = \"H: a place\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nnotes = \"data/notes.json\"\n").unwrap();
        std::fs::write(
            held.join("data/notes.json"),
            "{\"id\":\"a\"}\n{\"id\":\"b\"}\n",
        )
        .unwrap();
        std::fs::write(seen.join("projects.json"), "{\"id\":\"o/far\"}\n").unwrap();
        std::fs::write(
            seen.join("notes.json"),
            "{\"id\":\"c\",\"project\":\"o/far\"}\n",
        )
        .unwrap();
        assert_eq!(route(&held), (HELD, "as it is now".to_string()));
        let (by, as_of) = route(&seen);
        assert!(
            by == OBSERVED && as_of.starts_with("as of 20") && as_of.ends_with(" UTC"),
            "what was observed says as of when: {as_of}"
        );
        let said = of_a_table("notes", &[held, seen]);
        assert_eq!(said[0], "2 rows from 1 place(s) held here, as it is now");
        assert!(said[1].starts_with("1 rows observed, as of 20"), "{said:?}");
    }
}

/// The places in view among these folders, by their id: each folder with a mark, and each place a folder of
/// what was observed tells of.
pub fn places(dirs: &[PathBuf]) -> std::collections::BTreeSet<String> {
    let mut known = std::collections::BTreeSet::new();
    for dir in dirs {
        if let Some(mark) = std::fs::read_to_string(dir.join("oco.toml"))
            .ok()
            .and_then(|t| crate::mark::Mark::parse(&t).ok())
        {
            known.insert(mark.project.id);
        } else {
            known.extend(
                crate::harvest::read_rows(&dir.join("projects.json"))
                    .iter()
                    .filter_map(|p| p.get("id").and_then(|i| i.as_str()).map(str::to_string)),
            );
        }
    }
    known
}

/// The rows of a table as the places in view have them, each once, with the place each is of. A row is of
/// the place it names - its project - where that place is in view; else of the place that keeps it. The
/// same row reached by more than one route - observed by two observatories; held here and observed - is one
/// row: the one whose own moment (at) is the latest, and where it says none, the one that came first.
pub fn rows_by_place(
    table: &str,
    dirs: &[PathBuf],
) -> Vec<(String, serde_json::Map<String, serde_json::Value>)> {
    let known = places(dirs);
    let mut found: Vec<(String, serde_json::Map<String, serde_json::Value>)> = Vec::new();
    let mut at: std::collections::BTreeMap<(String, String), usize> = Default::default();
    let moment = |row: &serde_json::Map<String, serde_json::Value>| {
        row.get("at")
            .and_then(|a| a.as_str())
            .unwrap_or_default()
            .to_string()
    };
    for dir in dirs {
        let (keeper, rows): (
            Option<String>,
            Vec<serde_json::Map<String, serde_json::Value>>,
        ) = match std::fs::read_to_string(dir.join("oco.toml"))
            .ok()
            .and_then(|t| crate::mark::Mark::parse(&t).ok())
        {
            Some(mark) => (
                Some(mark.project.id.clone()),
                crate::join::rows(dir, &mark, table)
                    .into_iter()
                    .filter_map(|r| r.as_object().cloned())
                    .collect(),
            ),
            None if dir.join("projects.json").is_file() => (
                None,
                crate::harvest::read_rows(&dir.join(format!("{table}.json"))),
            ),
            None => continue,
        };
        for row in rows {
            let named = row
                .get("project")
                .and_then(|p| p.as_str())
                .filter(|p| !p.is_empty());
            let Some(place) = named
                .filter(|p| keeper.is_none() || known.contains(*p))
                .map(str::to_string)
                .or_else(|| keeper.clone())
            else {
                continue;
            };
            // a row with no id is its own row, wherever it came from
            let Some(id) = row.get("id").and_then(|i| i.as_str()).map(str::to_string) else {
                found.push((place, row));
                continue;
            };
            match at.get(&(place.clone(), id.clone())) {
                Some(&index) => {
                    if moment(&row) > moment(&found[index].1) {
                        found[index].1 = row;
                    }
                }
                None => {
                    at.insert((place.clone(), id), found.len());
                    found.push((place, row));
                }
            }
        }
    }
    found
}

#[cfg(test)]
mod one_thing_by_two_routes {
    use super::*;

    #[test]
    fn a_row_seen_by_two_routes_is_one_row_the_freshest_and_is_of_the_place_it_names() {
        let base = std::env::temp_dir().join(format!("ocoeco-two-routes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let place = |id: &str, tables: &[(&str, &str)]| {
            let d = base.join(id);
            std::fs::create_dir_all(d.join("data")).unwrap();
            let exports: String = tables
                .iter()
                .map(|(t, _)| format!("{t} = \"data/{t}.json\"\n"))
                .collect();
            std::fs::write(d.join("oco.toml"), format!("schema = 1\n[project]\nid = \"{id}\"\nname = \"N: a place\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\n{exports}")).unwrap();
            for (t, rows) in tables {
                std::fs::write(d.join(format!("data/{t}.json")), rows).unwrap();
            }
            d
        };
        let star = place("o/star", &[]);
        let first = place("o/first", &[("runs", "{\"id\":\"o/star:ci\",\"project\":\"o/star\",\"at\":\"2026-11-13T11:00:00Z\",\"conclusion\":\"failure\"}\n{\"id\":\"x:ci\",\"project\":\"far/away\",\"at\":\"2026-11-13T11:00:00Z\"}\n")]);
        let second = place("o/second", &[("runs", "{\"id\":\"o/star:ci\",\"project\":\"o/star\",\"at\":\"2026-11-14T10:00:00Z\",\"conclusion\":\"success\"}\n")]);
        let rows = rows_by_place("runs", &[star, first, second]);
        let said: Vec<(&str, &str, &str)> = rows
            .iter()
            .map(|(place, r)| {
                (
                    place.as_str(),
                    r["id"].as_str().unwrap(),
                    r.get("conclusion").and_then(|c| c.as_str()).unwrap_or("-"),
                )
            })
            .collect();
        assert_eq!(said, [("o/star", "o/star:ci", "success"), ("o/first", "x:ci", "-")], "one run, as it last was, of the place it names; a row naming a place not in view stays with its keeper");
    }
}

/// How many rows of a table arrive by every route among these places, before the same thing seen twice
/// is taken as one.
pub fn rows_by_route(table: &str, dirs: &[PathBuf]) -> usize {
    dirs.iter()
        .map(|dir| {
            match std::fs::read_to_string(dir.join("oco.toml"))
                .ok()
                .and_then(|t| crate::mark::Mark::parse(&t).ok())
            {
                Some(mark) => crate::join::rows(dir, &mark, table).len(),
                None => crate::harvest::read_rows(&dir.join(format!("{table}.json"))).len(),
            }
        })
        .sum()
}
