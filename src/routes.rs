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
