//! Where the places are: every marked project under a root, two levels down (owner/repository), as
//! the lab keeps them. What was the board - a hand-written view of every backlog - is now the work
//! lens's focus, declared as data (lenses/work.toml), proven equal to it on 2026-11-12 and then removed.

use std::path::{Path, PathBuf};

/// Every marked place under `root`, two levels down.
pub fn projects(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for owner in std::fs::read_dir(root).into_iter().flatten().flatten() {
        for repo in std::fs::read_dir(owner.path())
            .into_iter()
            .flatten()
            .flatten()
        {
            if repo.path().join("oco.toml").is_file() {
                found.push(repo.path());
            }
        }
    }
    found.sort();
    found
}

/// What is in view standing at `dir`: the place itself and, if its mark says it gathers, the stars it
/// gathers that are held beside it (the lab keeps every place two levels under one root). Says in
/// words what is in view, and which named stars are not held here - never passing over them silently.
pub fn in_view(dir: &Path) -> (Vec<PathBuf>, String) {
    let kept = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("share"))
        });
    in_view_from(dir, kept.as_deref())
}

/// What a machine's programs keep of themselves as places: each folder of `kept` (where programs keep
/// their own data) that carries a mark.
fn held_by_the_machine(kept: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(kept)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("oco.toml").is_file())
        .collect();
    found.sort();
    found
}

/// `in_view`, told where this machine's programs keep their own data. An observatory is where one looks
/// from: standing in one, what the machine itself holds is in view too - never gathered, never leaving
/// the machine, and said in words.
pub fn in_view_from(dir: &Path, kept: Option<&Path>) -> (Vec<PathBuf>, String) {
    let read = |d: &Path| {
        std::fs::read_to_string(d.join("oco.toml"))
            .ok()
            .and_then(|t| crate::mark::Mark::parse(&t).ok())
    };
    let Some(mark) = read(dir) else {
        return (vec![dir.to_path_buf()], "this place".into());
    };
    let Some(place) = &mark.place else {
        return (vec![dir.to_path_buf()], "this place".into());
    };
    let here = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let mut places = vec![dir.to_path_buf()];
    let mut held = Vec::new();
    if let Some(root) = here.parent().and_then(Path::parent) {
        for other in projects(root) {
            if std::fs::canonicalize(&other).ok().as_deref() == Some(here.as_path()) {
                continue;
            }
            if let Some(id) = read(&other)
                .map(|m| m.project.id)
                .filter(|id| mark.gathers(id))
            {
                held.push(id);
                places.push(other);
            }
        }
    }
    let absent: Vec<&String> = place
        .gathers
        .iter()
        .filter(|g| !g.ends_with("/*") && !held.contains(g))
        .collect();
    let mut said = format!(
        "this {} and the {} star(s) it gathers",
        place.is,
        held.len()
    );
    if !absent.is_empty() {
        said.push_str(&format!(
            " - not held here: {}",
            absent
                .iter()
                .map(|a| a.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if place.is == "observatory" {
        let own: Vec<(PathBuf, String)> = kept
            .map(held_by_the_machine)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|at| Some((at.clone(), read(&at)?.project.id)))
            .filter(|(at, _)| !places.contains(at))
            .collect();
        if !own.is_empty() {
            said.push_str(&format!(
                ", and what this machine holds: {}",
                own.iter()
                    .map(|(_, id)| id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            places.extend(own.into_iter().map(|(at, _)| at));
        }
    }
    (places, said)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marked(at: &Path, id: &str, kind: &str, place: &str) {
        std::fs::create_dir_all(at).unwrap();
        std::fs::write(at.join("oco.toml"), format!("schema = 1\n[project]\nid = \"{id}\"\nname = \"N: a place\"\nkind = \"{kind}\"\nparent = \"p\"\n{place}")).unwrap();
    }

    #[test]
    fn from_an_observatory_what_the_machine_holds_is_in_view() {
        let base = std::env::temp_dir().join(format!("ocoeco-in-view-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (root, kept) = (base.join("projects"), base.join("share"));
        marked(
            &root.join("me/observatory"),
            "me/observatory",
            "observatory",
            "[place]\nis = \"observatory\"\ngathers = [\"a/*\"]\n",
        );
        marked(
            &root.join("a/gathers"),
            "a/gathers",
            "constellation",
            "[place]\nis = \"constellation\"\ngathers = [\"a/star\"]\n",
        );
        marked(&root.join("a/star"), "a/star", "tool", "");
        marked(
            &kept.join("some-program"),
            "grig.local/some-program",
            "device-lab",
            "",
        );
        std::fs::create_dir_all(kept.join("no-mark")).unwrap();
        // an observatory is where one looks from: what the machine holds is in view, and said
        let (places, said) = in_view_from(&root.join("me/observatory"), Some(&kept));
        assert_eq!(places.len(), 4, "{places:?}");
        assert!(
            places.contains(&kept.join("some-program")) && !places.contains(&kept.join("no-mark"))
        );
        assert!(
            said.ends_with(
                "the 2 star(s) it gathers, and what this machine holds: grig.local/some-program"
            ),
            "{said}"
        );
        // a constellation shows what it gathers, and no more - and so does an observatory looked from nowhere
        let (places, said) = in_view_from(&root.join("a/gathers"), Some(&kept));
        assert_eq!(
            (places.len(), said.as_str()),
            (2, "this constellation and the 1 star(s) it gathers")
        );
        assert_eq!(in_view_from(&root.join("me/observatory"), None).0.len(), 3);
    }
}
