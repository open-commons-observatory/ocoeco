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

/// The rows of one exported table kept in this repository (not on another branch).
fn rows(dir: &Path, mark: &Mark, table: &str) -> Vec<serde_json::Value> {
    mark.exports
        .iter()
        .find(|(t, path)| t.as_str() == table && !path.contains('@'))
        .and_then(|(_, path)| std::fs::read_to_string(dir.join(path)).ok())
        .map(|t| {
            t.lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// How completely the repository tells its essence (SPEC.md section 11): how many of the four are
/// told - why it exists, what it must reach, how each step is delivered and proven, what is
/// happening - and what is still to tell. Advice, never a failure: a gap is the truth, not an error.
pub fn essence(dir: &Path) -> Result<(usize, Vec<String>)> {
    let mark =
        Mark::parse(&std::fs::read_to_string(dir.join("oco.toml")).context("no oco.toml here")?)?;
    let text = |v: &serde_json::Value, k: &str| v[k].as_str().unwrap_or("").to_string();
    let (idea, milestones, workings) = (
        rows(dir, &mark, "idea"),
        rows(dir, &mark, "milestones"),
        rows(dir, &mark, "workings"),
    );
    let mut lacks = Vec::new();
    if idea.is_empty() {
        lacks.push("its idea is not written: why it exists, for someone who knows nothing of it (the idea table)".to_string());
    } else {
        for (kind, what) in [
            ("lead", "does not open with anything (a lead)"),
            ("limit", "does not admit a limit"),
            ("start", "does not say where it stands"),
        ] {
            if !idea.iter().any(|r| text(r, "kind") == kind) {
                lacks.push(format!("its idea {what}"));
            }
        }
    }
    if milestones.is_empty() {
        lacks.push("it has no path: what it must reach, as steps with their evidence (the milestones table)".to_string());
    }
    let mut unserved = 0;
    for m in &milestones {
        if !workings.iter().any(|w| text(w, "serves") == text(m, "id")) {
            unserved += 1;
            lacks.push(format!(
                "nothing delivers the step '{}' (no working serves {})",
                text(m, "title"),
                text(m, "id")
            ));
        }
    }
    if !milestones.is_empty() {
        for w in &workings {
            if !milestones
                .iter()
                .any(|m| text(m, "id") == text(w, "serves"))
            {
                lacks.push(format!("the working '{}' serves '{}', which is not one of this repository's milestones", text(w, "what"), text(w, "serves")));
            }
        }
    }
    let work = mark
        .exports
        .iter()
        .any(|(t, _)| matches!(t.as_str(), "backlog" | "session_log" | "activity"));
    if !work {
        lacks.push(
            "its work is not shown: a backlog, a log, or what the gatherer writes".to_string(),
        );
    }
    let told = [
        !idea.is_empty(),
        !milestones.is_empty(),
        !milestones.is_empty() && unserved == 0,
        work,
    ];
    Ok((told.iter().filter(|t| **t).count(), lacks))
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

    #[test]
    fn essence_says_what_is_still_to_tell() {
        let d = scratch("essence");
        init(&d, &join(), "2026-11-11").unwrap();
        let (told, lacks) = essence(&d).unwrap();
        assert_eq!(
            told, 1,
            "a fresh repository shows its work and nothing else: {lacks:?}"
        );
        assert!(
            lacks.iter().any(|l| l.contains("its idea is not written"))
                && lacks.iter().any(|l| l.contains("no path"))
        );
        let toml = std::fs::read_to_string(d.join("oco.toml"))
            .unwrap()
            .replace(
                "[exports]\n",
                "[exports]\nidea = \"oco/idea.json\"\nworkings = \"oco/workings.json\"\n",
            );
        std::fs::write(d.join("oco.toml"), toml).unwrap();
        std::fs::write(d.join("oco/idea.json"), "{\"id\":\"1\",\"ord\":1,\"kind\":\"lead\",\"text\":\"Why.\"}\n{\"id\":\"2\",\"ord\":2,\"kind\":\"start\",\"text\":\"Here.\"}\n").unwrap();
        std::fs::write(
            d.join("oco/milestones.json"),
            "{\"id\":\"m1\",\"title\":\"First\"}\n{\"id\":\"m2\",\"title\":\"Second\"}\n",
        )
        .unwrap();
        std::fs::write(d.join("oco/workings.json"), "{\"id\":\"w\",\"what\":\"A tool\",\"serves\":\"m1\"}\n{\"id\":\"x\",\"what\":\"A typo\",\"serves\":\"m9\"}\n").unwrap();
        let (told, lacks) = essence(&d).unwrap();
        assert_eq!(
            told, 3,
            "idea, path and work - not yet every step served: {lacks:?}"
        );
        assert!(lacks.contains(&"its idea does not admit a limit".to_string()));
        assert!(lacks
            .iter()
            .any(|l| l.contains("nothing delivers the step 'Second'")));
        assert!(
            lacks.iter().any(|l| l.contains("'A typo' serves 'm9'")),
            "a working that serves nothing known is named"
        );
        assert!(!lacks.iter().any(|l| l.contains("'First'")));
    }
}
