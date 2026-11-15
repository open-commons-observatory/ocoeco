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
/// A table a place keeps on another branch of its own repository, read there: as of when that branch last
/// changed.
pub const ON_A_BRANCH: &str = "kept on another branch";

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
        let (mut route, mut as_of) = route(dir);
        let rows = if route == OBSERVED {
            crate::harvest::read_rows(&dir.join(format!("{table}.json"))).len()
        } else {
            let mark = std::fs::read_to_string(dir.join("oco.toml"))
                .ok()
                .and_then(|t| crate::mark::Mark::parse(&t).ok());
            // a table the place keeps on another branch of its own repository is as of when that branch
            // last changed, not as the folder is now
            if let Some(when) = mark
                .as_ref()
                .and_then(|mark| crate::join::kept_on_a_branch(dir, mark, table))
            {
                (route, as_of) = (ON_A_BRANCH, written(Some(when)));
            }
            mark.map_or(0, |mark| crate::join::rows(dir, &mark, table).len())
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

/// The kinds of surface a route may read from. Each is a small adapter here that makes rows when asked,
/// from each place in view that is held where it can be read; nothing is stored. A new kind is a function
/// here; a new route over a kind that exists is said in the language, by a lens.
pub const KINDS: [&str; 2] = ["git history", "files"];

/// The rows a route reads, each with the place it is of. What the lenses in view say is kept out is kept
/// out of them as they are read - named where it stood - for a surface holds whatever was written on it.
pub fn read(
    route: &crate::lenses::Route,
    dirs: &[PathBuf],
) -> Vec<(String, serde_json::Map<String, serde_json::Value>)> {
    let told = crate::kept::told(dirs);
    // each place is read on its own, side by side: one place's surface does not wait for another's
    let read: Vec<Vec<(String, serde_json::Map<String, serde_json::Value>)>> =
        std::thread::scope(|scope| {
            let reading: Vec<_> = dirs
                .iter()
                .map(|dir| scope.spawn(move || of_one_place(route, dir)))
                .collect();
            reading
                .into_iter()
                .map(|one| one.join().unwrap_or_default())
                .collect()
        });
    let mut rows = Vec::new();
    for (place, mut row) in read.into_iter().flatten() {
        for value in row.values_mut() {
            if let Some(words) = value.as_str() {
                *value = crate::kept::out_of(words, &told).0.into();
            }
        }
        rows.push((place, row));
    }
    // the same row reached by two routes - a place held here, and observed - is one row: the first to
    // arrive, which is the place as it stands where one looks from it
    let mut seen = std::collections::BTreeSet::new();
    rows.retain(
        |(place, row)| match row.get("id").and_then(|id| id.as_str()) {
            Some(id) => seen.insert((place.clone(), id.to_string())),
            None => true,
        },
    );
    rows
}

/// What a route reads of one folder: of a place held there, from its surface; of what was observed, the
/// rows an observatory read when it held each place - each naming the place it is of, as they were then.
fn of_one_place(
    route: &crate::lenses::Route,
    dir: &Path,
) -> Vec<(String, serde_json::Map<String, serde_json::Value>)> {
    let Some(place) = std::fs::read_to_string(dir.join("oco.toml"))
        .ok()
        .and_then(|t| crate::mark::Mark::parse(&t).ok())
        .map(|m| m.project.id)
    else {
        if !dir.join("projects.json").is_file() {
            return Vec::new();
        }
        return crate::harvest::read_rows(&dir.join(format!("{}.json", route.table)))
            .into_iter()
            .filter_map(|row| Some((row.get("project")?.as_str()?.to_string(), row)))
            .collect();
    };
    let read = match route.from.as_str() {
        "git history" => git_history(dir),
        "files" => files(dir, route.which.as_deref(), route.shape.as_deref()),
        _ => Vec::new(),
    };
    read.into_iter().map(|row| (place.clone(), row)).collect()
}

/// The history of the repository a place is held in, as git has it: each change - its name, when, by
/// whom, and what it says of itself - the latest first. Nothing where the folder is no repository.
fn git_history(dir: &Path) -> Vec<serde_json::Map<String, serde_json::Value>> {
    // what git says of a history depends only on the commit it is asked from, and a commit does not
    // change: so it is asked once for each commit, and found again while the place stands at it
    let ask = || {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["log", "-n", "2000", "--format=%h%x1f%cI%x1f%an%x1f%s"])
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let Some(said) = crate::join::answered_for(
        dir,
        "history",
        crate::join::commit_at(dir, "HEAD").as_deref(),
        ask,
    ) else {
        return Vec::new();
    };
    // a change made through ocoeco says what its record of work says, after the kind of change: so the
    // record a change is part of is read off what both say - never typed. A saying several records of
    // the place share is the saying of none of them
    let mut records: std::collections::BTreeMap<String, Option<String>> = Default::default();
    if let Some(mark) = std::fs::read_to_string(dir.join("oco.toml"))
        .ok()
        .and_then(|text| crate::mark::Mark::parse(&text).ok())
    {
        for row in crate::join::rows(dir, &mark, "glimpses") {
            let said = |key: &str| {
                row.get(key)
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim().to_string())
            };
            if let (Some(summary), Some(id)) = (said("summary"), said("id")) {
                records
                    .entry(summary)
                    .and_modify(|one| *one = None)
                    .or_insert(Some(id));
            }
        }
    }
    said.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(4, '\u{1f}');
            let (id, at, by, says) = (
                parts.next()?,
                parts.next()?,
                parts.next()?,
                parts.next().unwrap_or_default(),
            );
            // what the change says, without the number of its pull request and the kind of change
            let numbered = says
                .rsplit_once(" (#")
                .filter(|(_, n)| n.trim_end_matches(')').chars().all(|c| c.is_ascii_digit()));
            let plain = numbered.map_or(says, |(before, _)| before);
            let plain = plain
                .split_once(": ")
                .map_or(plain, |(_, after)| after)
                .trim();
            let mut row = serde_json::json!({"id": id, "at": at, "by": by, "says": says});
            if let Some(record) = records.get(plain).cloned().flatten() {
                row["record"] = record.into();
            }
            row.as_object().cloned()
        })
        .collect()
}

/// The files of the place itself - those within its folder whose path fits `which` - read as rows. Where
/// no shape is said, each file is a row: its path, how many lines, how many bytes. Where one is, every
/// place in a file the shape is found is a row: what the shape names are its fields, with the path and the
/// line it was found on. A place reads its own files: a path that leaves its folder names nothing.
fn files(
    dir: &Path,
    which: Option<&str>,
    shape: Option<&str>,
) -> Vec<serde_json::Map<String, serde_json::Value>> {
    use serde_json::json;
    let Some(which) = which.filter(|w| {
        !w.is_empty()
            && !w.starts_with('/')
            && !w.starts_with('~')
            && !w.split('/').any(|part| part == "..")
    }) else {
        return Vec::new();
    };
    let (Ok(wanted), Ok(carved)) = (
        regex::Regex::new(&as_a_shape(which)),
        shape.map(regex::Regex::new).transpose(),
    ) else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    let (mut folders, mut looked_at) = (vec![dir.to_path_buf()], 0usize);
    while let Some(folder) = folders.pop() {
        for path in std::fs::read_dir(&folder)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
        {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            if path.is_symlink() {
                continue;
            }
            if path.is_dir() {
                // not what a place wrote: git's own folder, and what a build leaves
                if !matches!(name.as_str(), ".git" | "target" | "node_modules") {
                    folders.push(path);
                }
                continue;
            }
            looked_at += 1;
            if looked_at > 50_000 {
                break;
            }
            let Ok(within) = path.strip_prefix(dir) else {
                continue;
            };
            let within = within.to_string_lossy().replace('\\', "/");
            if !wanted.is_match(&within) {
                continue;
            }
            let Some(text) = std::fs::metadata(&path)
                .ok()
                .filter(|m| m.len() <= 2_000_000)
                .and_then(|_| std::fs::read_to_string(&path).ok())
            else {
                continue;
            };
            let Some(carved) = &carved else {
                rows.extend(json!({"id": within, "path": within, "lines": text.lines().count(), "bytes": text.len()}).as_object().cloned());
                continue;
            };
            for found in carved.captures_iter(&text) {
                let line = text[..found.get(0).map_or(0, |whole| whole.start())]
                    .matches('\n')
                    .count()
                    + 1;
                let mut row = serde_json::Map::new();
                for name in carved.capture_names().flatten() {
                    if let Some(part) = found.name(name) {
                        row.insert(name.to_string(), part.as_str().into());
                    }
                }
                // a row is named by what the shape names - its id, else its name - within its file; by
                // the line it was found on only where the shape names nothing: a line moves when
                // anything above it is edited, and a thing that is pointed at must keep its name
                let named = ["id", "name"]
                    .iter()
                    .find_map(|part| row.get(*part).and_then(|v| v.as_str()).map(str::to_string));
                row.insert(
                    "id".into(),
                    format!("{within}:{}", named.unwrap_or_else(|| line.to_string())).into(),
                );
                row.insert("path".into(), within.clone().into());
                row.insert("line".into(), line.into());
                rows.push(row);
            }
        }
    }
    let placed = |row: &serde_json::Map<String, serde_json::Value>| {
        (
            row.get("path")
                .and_then(|p| p.as_str())
                .unwrap_or_default()
                .to_string(),
            row.get("line").and_then(|l| l.as_u64()).unwrap_or(0),
        )
    };
    rows.sort_by_key(placed);
    rows
}

/// Which paths, as a shape: ** any folders, * any name within one, ? any one letter of a name.
fn as_a_shape(which: &str) -> String {
    let mut shape = String::from("^");
    let mut letters = which.chars().peekable();
    while let Some(letter) = letters.next() {
        match letter {
            '*' if letters.peek() == Some(&'*') => {
                letters.next();
                if letters.peek() == Some(&'/') {
                    letters.next();
                    shape.push_str("(?:.*/)?");
                } else {
                    shape.push_str(".*");
                }
            }
            '*' => shape.push_str("[^/]*"),
            '?' => shape.push_str("[^/]"),
            other => shape.push_str(&regex::escape(&other.to_string())),
        }
    }
    shape.push('$');
    shape
}

#[cfg(test)]
mod a_route {
    use super::*;

    #[test]
    fn git_history_is_read_as_rows_where_a_place_is_held_and_nothing_where_it_is_no_repository() {
        let d = std::env::temp_dir()
            .join(format!("ocoeco-a-route-{}", std::process::id()))
            .join("o/held");
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/held\"\nname = \"H: a place\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\n").unwrap();
        let route = crate::lenses::Route {
            table: "commits".into(),
            from: "git history".into(),
            which: None,
            shape: None,
        };
        assert!(
            read(&route, std::slice::from_ref(&d)).is_empty(),
            "a folder that is no repository has no history"
        );
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .arg("-C")
                .arg(&d)
                .args(args)
                .env("GIT_AUTHOR_NAME", "someone")
                .env("GIT_AUTHOR_EMAIL", "s@x")
                .env("GIT_COMMITTER_NAME", "someone")
                .env("GIT_COMMITTER_EMAIL", "s@x")
                .output()
                .unwrap()
        };
        git(&["init", "-q"]);
        git(&["add", "oco.toml"]);
        git(&["commit", "-q", "-m", "feat: a first change"]);
        let rows = read(&route, std::slice::from_ref(&d));
        assert_eq!(rows.len(), 1);
        assert_eq!(
            (
                rows[0].0.as_str(),
                rows[0].1["says"].as_str(),
                rows[0].1["by"].as_str()
            ),
            ("o/held", Some("feat: a first change"), Some("someone"))
        );
        assert!(
            rows[0].1["at"]
                .as_str()
                .is_some_and(|at| at.starts_with("20"))
                && rows[0].1["id"].as_str().is_some_and(|id| id.len() >= 7)
        );
        assert!(read(
            &crate::lenses::Route {
                table: "x".into(),
                from: "a kind there is no adapter for".into(),
                which: None,
                shape: None,
            },
            std::slice::from_ref(&d)
        )
        .is_empty());
    }
}

#[cfg(test)]
mod a_shape_carved_from_files {
    use super::*;

    #[test]
    fn a_place_s_own_files_are_rows_whole_or_as_a_shape_is_found_in_them() {
        let d = std::env::temp_dir()
            .join(format!("ocoeco-carved-{}", std::process::id()))
            .join("o/held");
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("src/deep")).unwrap();
        std::fs::create_dir_all(d.join("target")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/held\"\nname = \"H: a place\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\n").unwrap();
        std::fs::write(d.join("src/lib.rs"), "fn plain() {}\n\n#[test]\nfn it_holds() {}\n\n    #[test]\n    #[ignore]\n    fn and_this_too() {}\n").unwrap();
        std::fs::write(d.join("src/deep/more.rs"), "#[test]\nfn deep_down() {}\n").unwrap();
        std::fs::write(
            d.join("target/built.rs"),
            "#[test]\nfn what_a_build_left() {}\n",
        )
        .unwrap();
        std::fs::write(d.join("notes.txt"), "not asked for\n").unwrap();
        let route = |which: &str, shape: Option<&str>| crate::lenses::Route {
            table: "t".into(),
            from: "files".into(),
            which: Some(which.into()),
            shape: shape.map(str::to_string),
        };
        let whole = read(&route("**/*.rs", None), std::slice::from_ref(&d));
        let paths: Vec<(&str, u64)> = whole
            .iter()
            .map(|(_, r)| (r["path"].as_str().unwrap(), r["lines"].as_u64().unwrap()))
            .collect();
        assert_eq!(
            paths,
            [("src/deep/more.rs", 2), ("src/lib.rs", 8)],
            "each file a row - in any folder, but not what a build left"
        );
        let guards = read(
            &route(
                "**/*.rs",
                Some(r"#\[test\]\s*(?:#\[[^\]]*\]\s*)*fn\s+(?P<name>[a-z0-9_]+)"),
            ),
            std::slice::from_ref(&d),
        );
        let found: Vec<(&str, &str, u64)> = guards
            .iter()
            .map(|(_, r)| {
                (
                    r["path"].as_str().unwrap(),
                    r["name"].as_str().unwrap(),
                    r["line"].as_u64().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [
                ("src/deep/more.rs", "deep_down", 1),
                ("src/lib.rs", "it_holds", 3),
                ("src/lib.rs", "and_this_too", 6)
            ],
            "every place the shape is found is a row, with what it names"
        );
        assert!(
            read(&route("../**/*.rs", None), std::slice::from_ref(&d)).is_empty()
                && read(&route("/etc/*", None), std::slice::from_ref(&d)).is_empty(),
            "a place reads its own files"
        );
        assert_eq!(as_a_shape("src/*.rs"), r"^src/[^/]*\.rs$");
    }
}

#[cfg(test)]
mod held_and_observed {
    use super::*;

    #[test]
    fn what_a_route_reads_of_a_place_held_and_observed_is_read_once() {
        let base = std::env::temp_dir().join(format!("ocoeco-route-once-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let here = base.join("o/here");
        std::fs::create_dir_all(&here).unwrap();
        std::fs::write(here.join("oco.toml"), "schema = 1\n[project]\nid = \"o/here\"\nname = \"Here: an observatory\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\n").unwrap();
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .arg("-C")
                .arg(&here)
                .args(args)
                .env("GIT_AUTHOR_NAME", "someone")
                .env("GIT_AUTHOR_EMAIL", "s@x")
                .env("GIT_COMMITTER_NAME", "someone")
                .env("GIT_COMMITTER_EMAIL", "s@x")
                .output()
                .unwrap()
        };
        git(&["init", "-q"]);
        git(&["add", "oco.toml"]);
        git(&["commit", "-q", "-m", "feat: a first change"]);
        let route = crate::lenses::Route {
            table: "commits".into(),
            from: "git history".into(),
            which: None,
            shape: None,
        };
        let held = read(&route, std::slice::from_ref(&here));
        assert_eq!(held.len(), 1, "the place as it stands has one change");
        // what it observed: that same change of its own, and one of a place far away
        let seen = base.join("seen");
        std::fs::create_dir_all(&seen).unwrap();
        std::fs::write(
            seen.join("projects.json"),
            "{\"id\":\"o/here\"}\n{\"id\":\"o/far\"}\n",
        )
        .unwrap();
        let mut own = held[0].1.clone();
        own.insert("project".into(), "o/here".into());
        std::fs::write(
            seen.join("commits.json"),
            format!(
                "{}\n{{\"id\":\"abc1234\",\"project\":\"o/far\",\"says\":\"feat: far away\"}}\n",
                serde_json::Value::Object(own)
            ),
        )
        .unwrap();
        let both = read(&route, &[here.clone(), seen]);
        let of = |place: &str| both.iter().filter(|(p, _)| p == place).count();
        assert_eq!(
            (of("o/here"), of("o/far")),
            (1, 1),
            "a change of the place one stands in is one row; what was observed of another is there"
        );
    }
}
