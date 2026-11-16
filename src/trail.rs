//! The trail: where a driver stood, as it happens. Looking through a lens from a thing leaves a step; a
//! record of work says where it was made - the things stood on since the last one - and the trail begins
//! again. Nothing is typed: the act of looking supplies it. The trail is the driver's own, kept on the
//! machine beside what ocoeco keeps there. Each step is kept too, as a row that says who stood where and
//! when: where the trail is kept is a place of this machine, in view from an observatory standing there.

use std::path::{Path, PathBuf};

fn home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("share"))
        })
}

fn kept(home: Option<PathBuf>, who: &str) -> Option<PathBuf> {
    let home = home?;
    let name: String = who
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    Some(
        home.join("ocoeco")
            .join("trail")
            .join(format!("{name}.txt")),
    )
}

/// A step: the driver stood on this thing. Standing where one already stands is no new step.
pub fn step(who: &str, thing: &str) {
    step_in(home(), who, thing)
}

fn step_in(home: Option<PathBuf>, who: &str, thing: &str) {
    let Some(file) = kept(home, who) else { return };
    // where one stands is kept beside the trail, and stays when the trail begins again
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let stood = std::fs::read_to_string(file.with_extension("stands")).unwrap_or_default();
    let _ = std::fs::write(file.with_extension("stands"), thing);
    // and a step is kept as a row - who stood where, and when - of a place this machine holds: the act
    // of looking, as data. A step is a move: standing again where one stands is not one
    if stood.trim() != thing {
        if let Some(dir) = file.parent() {
            keep_step(dir, who, thing);
        }
    }
    let so_far = std::fs::read_to_string(&file).unwrap_or_default();
    if so_far.lines().last() == Some(thing) {
        return;
    }
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&file, format!("{so_far}{thing}\n"));
}

/// How many steps are kept: when there are more, the earliest are let go.
const STEPS_KEPT: usize = 2000;

/// Keeps a step as a row of the table of steps, beside the trail, and says - once - that where the trail
/// is kept is a place of this machine: its mark, so that standing in an observatory there the steps are
/// in view. Never gathered, never leaving the machine.
fn keep_step(trail: &Path, who: &str, thing: &str) {
    keep_as_a_step(trail, who, thing, false)
}

/// A step kept as a row: who stood on what, and when - or, going up, who left it. So where a driver
/// stands is read off the steps: the thing of their last one, unless it says they went up from it.
fn keep_as_a_step(trail: &Path, who: &str, thing: &str, up: bool) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default();
    let at = format!(
        "{}T{:02}:{:02}:{:02}Z",
        crate::gather::day(secs / 86_400),
        secs % 86_400 / 3_600,
        secs % 3_600 / 60,
        secs % 60
    );
    // a thing is said without its owner where it is spoken of: place, table and id
    let short = thing.split_once('/').map_or(thing, |(_, rest)| rest);
    let file = trail.join("steps.json");
    let mut rows: Vec<String> = std::fs::read_to_string(&file)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect();
    // two steps within a second are two rows: each named apart
    let named = format!("\"id\":\"{who}-{secs}");
    let id = match rows.iter().filter(|row| row.contains(&named)).count() {
        0 => format!("{who}-{secs}"),
        nth => format!("{who}-{secs}-{}", nth + 1),
    };
    let does = if up { "went up from" } else { "stood on" };
    let mut step = serde_json::json!({"id": id, "by": who, "where": thing, "at": at,
                                      "says": format!("{who} {does} {short}")});
    if up {
        step["up"] = true.into();
    }
    rows.push(step.to_string());
    if rows.len() > STEPS_KEPT {
        rows.drain(..rows.len() - STEPS_KEPT * 3 / 4);
    }
    let _ = std::fs::write(&file, rows.join("\n") + "\n");
    let Some(home) = trail.parent() else { return };
    if !home.join("oco.toml").is_file() {
        let host = std::fs::read_to_string("/proc/sys/kernel/hostname")
            .ok()
            .map(|h| h.trim().to_string())
            .filter(|h| !h.is_empty())
            .unwrap_or_else(|| "this-machine".into());
        let _ = std::fs::write(
            home.join("oco.toml"),
            format!(
                "schema = 1\n\n[project]\nid = \"{host}/trail\"\nname = \"The trail on {host}: where the drivers of this machine stood, step by step\"\nkind = \"tool\"\nparent = \"{host}\"\n\n[exports]\nsteps = \"trail/steps.json\"\n"
            ),
        );
    }
}

/// What was marked in what the driver was last shown: each thing, in the order of its mark - kept beside
/// the trail, so that a mark said next names what was shown.
pub fn marked(who: &str) -> Vec<String> {
    marked_in(home(), who)
}

fn marked_in(home: Option<PathBuf>, who: &str) -> Vec<String> {
    kept(home, who)
        .and_then(|file| std::fs::read_to_string(file.with_extension("marked")).ok())
        .map(|text| text.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

/// Keeps what is marked in what the driver is shown now.
pub fn mark(who: &str, things: &[String]) {
    mark_in(home(), who, things)
}

fn mark_in(home: Option<PathBuf>, who: &str, things: &[String]) {
    let Some(file) = kept(home, who) else { return };
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(file.with_extension("marked"), things.join("\n"));
}

/// Where the driver stood before where they stand now: the step before the last, as the steps are kept.
pub fn before(who: &str) -> Option<String> {
    before_in(home(), who)
}

fn before_in(home: Option<PathBuf>, who: &str) -> Option<String> {
    let file = kept(home, who)?;
    let steps = std::fs::read_to_string(file.parent()?.join("steps.json")).ok()?;
    let mut stood: Vec<String> = steps
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        // a leaving is no place one stood: back leads to where one stood before
        .filter(|step| step["by"].as_str() == Some(who) && step["up"] != true)
        .filter_map(|step| step["where"].as_str().map(str::to_string))
        .collect();
    stood.pop()?;
    stood.pop()
}

/// The driver stands on nothing: one step up from any thing, where a place shows what it shows first.
pub fn leave(who: &str) {
    leave_in(home(), who)
}

fn leave_in(home: Option<PathBuf>, who: &str) {
    if let Some(file) = kept(home, who) {
        // going up is a step too - who left what, and when: where one stands is read off the steps
        let stood = std::fs::read_to_string(file.with_extension("stands")).unwrap_or_default();
        if let Some(dir) = file.parent().filter(|_| !stood.trim().is_empty()) {
            keep_as_a_step(dir, who, stood.trim(), true);
        }
        let _ = std::fs::remove_file(file.with_extension("stands"));
    }
}

/// The language the driver reads in: said once, it stays - kept beside the trail, as where one stands is.
pub fn reads_in(who: &str) -> Option<String> {
    let lang = std::fs::read_to_string(kept(home(), who)?.with_extension("reads")).ok()?;
    Some(lang.trim().to_string()).filter(|lang| !lang.is_empty())
}

/// From now on the driver reads in this language.
pub fn read_in(who: &str, lang: &str) {
    if let Some(file) = kept(home(), who) {
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(file.with_extension("reads"), lang.trim());
    }
}

/// Who is driving: as the program is told, else as the machine knows them.
pub fn who() -> String {
    std::env::var("OCOECO_BY")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "someone".into())
}

/// Where the driver stands: the last thing stood on, on whatever surface that was. It stays when the
/// trail begins again - a record of work says where it was made, and one is still standing there after.
pub fn stands(who: &str) -> Option<String> {
    stands_in(home(), who)
}

fn stands_in(home: Option<PathBuf>, who: &str) -> Option<String> {
    let thing = std::fs::read_to_string(kept(home, who)?.with_extension("stands")).ok()?;
    Some(thing.trim().to_string()).filter(|thing| !thing.is_empty())
}

/// The things stood on since the trail began, each once, in the order they were first stood on - and the
/// trail begins again.
pub fn taken(who: &str) -> Vec<String> {
    taken_in(home(), who)
}

fn taken_in(home: Option<PathBuf>, who: &str) -> Vec<String> {
    let Some(file) = kept(home, who) else {
        return Vec::new();
    };
    let so_far = std::fs::read_to_string(&file).unwrap_or_default();
    let _ = std::fs::remove_file(&file);
    let mut seen = std::collections::BTreeSet::new();
    so_far
        .lines()
        .filter(|l| !l.trim().is_empty() && seen.insert(l.to_string()))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trail_keeps_each_thing_stood_on_once_and_begins_again_when_taken() {
        let home = std::env::temp_dir().join(format!("ocoeco-trail-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        for thing in [
            "o/a/backlog:x",
            "o/a/backlog:x",
            "o/b/decisions:y",
            "o/a/backlog:x",
        ] {
            step_in(Some(home.clone()), "someone else", thing);
        }
        assert_eq!(
            taken_in(Some(home.clone()), "someone else"),
            ["o/a/backlog:x", "o/b/decisions:y"],
            "each once, as first stood on"
        );
        assert!(
            taken_in(Some(home.clone()), "someone else").is_empty(),
            "and the trail begins again"
        );
        assert!(
            taken_in(Some(home), "another driver").is_empty(),
            "a trail is one driver's own"
        );
    }
}

#[cfg(test)]
mod where_one_stands {
    use super::*;

    #[test]
    fn where_one_stands_stays_when_the_trail_begins_again() {
        let home = std::env::temp_dir().join(format!("ocoeco-stands-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        assert_eq!(
            stands_in(Some(home.clone()), "x"),
            None,
            "nowhere, before a first step"
        );
        step_in(Some(home.clone()), "x", "r/backlog:a");
        step_in(Some(home.clone()), "x", "r/decisions:b");
        assert_eq!(
            stands_in(Some(home.clone()), "x").as_deref(),
            Some("r/decisions:b")
        );
        assert_eq!(
            taken_in(Some(home.clone()), "x").len(),
            2,
            "a record of work takes the trail"
        );
        assert_eq!(
            stands_in(Some(home.clone()), "x").as_deref(),
            Some("r/decisions:b"),
            "and one still stands there"
        );
        assert_eq!(
            stands_in(Some(home), "y"),
            None,
            "each driver stands where they stand"
        );
    }
}

#[cfg(test)]
mod steps_kept {
    use super::*;

    #[test]
    fn a_step_is_kept_as_a_row_that_says_who_stood_where_and_when() {
        let home = std::env::temp_dir().join(format!("ocoeco-steps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        step_in(Some(home.clone()), "x", "o/r/backlog:a");
        step_in(Some(home.clone()), "x", "o/r/backlog:a");
        step_in(Some(home.clone()), "x", "o/r/decisions:b");
        step_in(Some(home.clone()), "y", "o/r/backlog:a");
        let kept = std::fs::read_to_string(home.join("ocoeco/trail/steps.json")).unwrap();
        let rows: Vec<serde_json::Value> = kept
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let told: Vec<(&str, &str)> = rows
            .iter()
            .map(|r| (r["by"].as_str().unwrap(), r["where"].as_str().unwrap()))
            .collect();
        assert_eq!(
            told,
            [
                ("x", "o/r/backlog:a"),
                ("x", "o/r/decisions:b"),
                ("y", "o/r/backlog:a")
            ],
            "a step is a move: standing again where one stands is not one"
        );
        assert_eq!(rows[0]["says"], "x stood on r/backlog:a");
        assert!(
            crate::relations::when(&rows[0], None).is_some_and(|at| at.len() == 20),
            "it says when, as a moment"
        );
        // and where the trail is kept says it is a place of this machine, once
        let mark = crate::mark::Mark::parse(
            &std::fs::read_to_string(home.join("ocoeco/oco.toml")).unwrap(),
        )
        .unwrap();
        assert!(mark.project.id.ends_with("/trail") && mark.exports["steps"] == "trail/steps.json");
        // steps do not pile up without end
        for n in 0..STEPS_KEPT + 10 {
            keep_step(&home.join("ocoeco/trail"), "z", &format!("o/r/backlog:{n}"));
        }
        assert!(
            std::fs::read_to_string(home.join("ocoeco/trail/steps.json"))
                .unwrap()
                .lines()
                .count()
                <= STEPS_KEPT
        );
    }
}

#[cfg(test)]
mod of_the_window {
    use super::*;

    #[test]
    fn what_was_marked_is_kept_and_one_goes_back_and_up() {
        let home = std::env::temp_dir().join(format!("ocoeco-window-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        step_in(Some(home.clone()), "d", "o/r/notes:a");
        step_in(Some(home.clone()), "d", "o/r/notes:b");
        assert_eq!(
            before_in(Some(home.clone()), "d").as_deref(),
            Some("o/r/notes:a"),
            "back: where one stood before"
        );
        mark_in(
            Some(home.clone()),
            "d",
            &["o/r/notes:a".to_string(), "o/r/notes:c".to_string()],
        );
        assert_eq!(
            marked_in(Some(home.clone()), "d"),
            ["o/r/notes:a", "o/r/notes:c"],
            "a mark said next names what was shown"
        );
        leave_in(Some(home.clone()), "d");
        assert_eq!(
            stands_in(Some(home.clone()), "d"),
            None,
            "up: one stands on nothing"
        );
        // and going up is kept as a step - who left what: where one stands is read off the steps
        let steps = std::fs::read_to_string(home.join("ocoeco/trail/steps.json")).unwrap();
        let last: serde_json::Value = serde_json::from_str(steps.lines().last().unwrap()).unwrap();
        assert!(
            last["up"] == true
                && last["where"] == "o/r/notes:b"
                && last["says"] == "d went up from r/notes:b",
            "going up is a step that says so: {last}"
        );
        let named: std::collections::BTreeSet<String> = steps
            .lines()
            .filter_map(|row| serde_json::from_str::<serde_json::Value>(row).ok())
            .filter_map(|row| row["id"].as_str().map(str::to_string))
            .collect();
        assert_eq!(
            named.len(),
            steps.lines().count(),
            "steps within a second are named apart"
        );
        assert_eq!(
            before_in(Some(home.clone()), "d").as_deref(),
            Some("o/r/notes:a"),
            "back, after going up: a leaving is no place one stood"
        );
        let _ = std::fs::remove_dir_all(&home);
    }
}
