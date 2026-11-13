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
    (places, said)
}
