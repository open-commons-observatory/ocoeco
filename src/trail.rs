//! The trail: where a driver stood, as it happens. Looking through a lens from a thing leaves a step; a
//! record of work says where it was made - the things stood on since the last one - and the trail begins
//! again. Nothing is typed: the act of looking supplies it. The trail is the driver's own, kept on the
//! machine beside what ocoeco keeps there, and is no part of any place.

use std::path::PathBuf;

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
    let _ = std::fs::write(file.with_extension("stands"), thing);
    let so_far = std::fs::read_to_string(&file).unwrap_or_default();
    if so_far.lines().last() == Some(thing) {
        return;
    }
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&file, format!("{so_far}{thing}\n"));
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
