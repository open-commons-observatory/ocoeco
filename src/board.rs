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
