//! Joining the ecosystem (SPEC.md section 10): `init` writes what a repository needs - its mark, the
//! gatherer's caller, a place for its own records - and never overwrites what is there; `check`
//! reads a repository as the harvest would, before anything is pushed.

use crate::mark::{Mark, KINDS};
use anyhow::{bail, Context, Result};
use std::path::Path;

pub struct Join {
    pub id: String,
    pub name: String,
    pub title: Option<String>,
    pub kind: String,
    pub parent: String,
}

/// The five-line caller of ocoeco's gatherer.
pub const CALLER: &str = r#"name: gather
# ocoeco gathers this repository's activity, releases and CI onto its oco-data branch - data made by work.
on:
  schedule:
    - cron: "41 3 * * *"
  workflow_dispatch:
permissions:
  contents: write
jobs:
  gather:
    uses: open-commons-observatory/ocoeco/.github/workflows/gather.yml@main
"#;

/// The mark of a joining repository: its own records under oco/, its gathered data on oco-data.
pub fn mark_text(j: &Join) -> String {
    let title = j
        .title
        .as_ref()
        .map(|t| format!("title = \"{t}\"\n"))
        .unwrap_or_default();
    format!(
        r#"# This repository's mark: how it fits the Open Commons Observatory's picture.
schema = 1

[project]
id = "{}"
name = "{}"
{title}kind = "{}"
parent = "{}"

[exports]
backlog = "oco/backlog.json"
milestones = "oco/milestones.json"
session_log = "oco/session_log.json"
activity = "oco/gathered/activity.json@oco-data"
releases = "oco/gathered/releases.json@oco-data"
ci = "oco/gathered/ci.json@oco-data"
"#,
        j.id, j.name, j.kind, j.parent
    )
}

/// Writes what is missing in `dir`; returns the files written. Nothing existing is overwritten.
pub fn init(dir: &Path, j: &Join, today: &str) -> Result<Vec<String>> {
    if !KINDS.contains(&j.kind.as_str()) {
        bail!("kind {:?} is not one of {}", j.kind, KINDS.join(", "));
    }
    let mark = mark_text(j);
    Mark::parse(&mark).context("the mark these answers make is not valid")?;
    let first = format!(
        "{{\"id\":\"{today}-joined\",\"entry_date\":\"{today}\",\"title\":\"Joined the Open Commons Observatory\",\"done\":\"The mark, the gatherer's caller and a place for this project's own records, written by ocoeco init.\",\"considered\":\"\",\"rejected\":\"\"}}\n"
    );
    let files = [
        ("oco.toml", mark.as_str()),
        (".github/workflows/gather.yml", CALLER),
        ("oco/backlog.json", ""),
        ("oco/milestones.json", ""),
        ("oco/session_log.json", first.as_str()),
    ];
    let mut written = Vec::new();
    for (path, text) in files {
        let p = dir.join(path);
        if p.exists() {
            continue;
        }
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&p, text)?;
        written.push(path.to_string());
    }
    Ok(written)
}

/// What the harvest would find in `dir`: (what is well, what is not).
pub fn check(dir: &Path) -> Result<(Vec<String>, Vec<String>)> {
    let text = std::fs::read_to_string(dir.join("oco.toml"))
        .context("no oco.toml here: this repository has no mark (ocoeco init writes one)")?;
    let mark = Mark::parse(&text)?;
    let (mut well, mut problems) = (
        vec![format!(
            "the mark: {} ({})",
            mark.project.id, mark.project.kind
        )],
        Vec::new(),
    );
    for (table, path) in &mark.exports {
        let (file, branch) = path.split_once('@').unwrap_or((path.as_str(), ""));
        if !branch.is_empty() {
            well.push(format!(
                "{table}: on the {branch} branch - gathered there, not checked here"
            ));
            continue;
        }
        match std::fs::read_to_string(dir.join(file)) {
            Err(_) => problems.push(format!("{table}: {file} is exported but missing")),
            Ok(t) => {
                let lines: Vec<&str> = t.lines().filter(|l| !l.trim().is_empty()).collect();
                let bad = lines
                    .iter()
                    .filter(|l| {
                        !matches!(
                            serde_json::from_str::<serde_json::Value>(l),
                            Ok(serde_json::Value::Object(_))
                        )
                    })
                    .count();
                if bad > 0 {
                    problems.push(format!("{table}: {bad} of {} lines in {file} are not rows (one JSON object per line)", lines.len()));
                } else {
                    well.push(format!("{table}: {} rows", lines.len()));
                }
            }
        }
    }
    Ok((well, problems))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ocoeco-join-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn join() -> Join {
        Join {
            id: "o/r".into(),
            name: "R: a thing".into(),
            title: Some("R".into()),
            kind: "tool".into(),
            parent: "open-commons-observatory".into(),
        }
    }

    #[test]
    fn init_writes_what_is_missing_and_check_finds_it_well() {
        let d = scratch("init");
        let written = init(&d, &join(), "2026-11-10").unwrap();
        assert_eq!(
            written,
            [
                "oco.toml",
                ".github/workflows/gather.yml",
                "oco/backlog.json",
                "oco/milestones.json",
                "oco/session_log.json"
            ]
        );
        let (well, problems) = check(&d).unwrap();
        assert!(problems.is_empty(), "{problems:?}");
        assert!(
            well.iter().any(|w| w == "session_log: 1 rows")
                && well
                    .iter()
                    .any(|w| w.starts_with("ci: on the oco-data branch"))
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn init_never_overwrites() {
        let d = scratch("keep");
        std::fs::write(d.join("oco.toml"), "mine").unwrap();
        let written = init(&d, &join(), "2026-11-10").unwrap();
        assert!(!written.contains(&"oco.toml".to_string()));
        assert_eq!(std::fs::read_to_string(d.join("oco.toml")).unwrap(), "mine");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn check_says_what_is_wrong() {
        let d = scratch("wrong");
        init(&d, &join(), "2026-11-10").unwrap();
        std::fs::write(d.join("oco/backlog.json"), "{\"id\":\"a\"}\nnot a row\n").unwrap();
        std::fs::remove_file(d.join("oco/milestones.json")).unwrap();
        let (_, problems) = check(&d).unwrap();
        assert_eq!(problems.len(), 2);
        assert!(
            problems.iter().any(|p| p.contains("1 of 2 lines"))
                && problems.iter().any(|p| p.contains("exported but missing"))
        );
        assert!(
            init(
                &d,
                &Join {
                    kind: "blog".into(),
                    ..join()
                },
                "2026-11-10"
            )
            .is_err(),
            "an unknown kind is refused"
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
