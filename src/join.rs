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
# ocoeco gathers this repository's activity and releases onto its oco-data branch - data made by work.
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
glimpses = "oco/glimpses.json"
decisions = "oco/decisions.json"
problems = "oco/problems.json"
activity = "oco/gathered/activity.json@oco-data"
releases = "oco/gathered/releases.json@oco-data"
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
        // from its first day a place can record its work, its reasoning and what is wrong: the lenses
        // of results, of reasoning and of problems have somewhere to catch them
        ("oco/glimpses.json", ""),
        ("oco/decisions.json", ""),
        ("oco/problems.json", ""),
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

/// The rows of one table a place keeps: in its folder, or on another branch of its own repository.
pub fn rows(dir: &Path, mark: &Mark, table: &str) -> Vec<serde_json::Value> {
    mark.exports
        .iter()
        .find(|(t, _)| t.as_str() == table)
        .and_then(|(_, path)| kept(dir, path))
        .map(|t| {
            t.lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// What a place keeps at a path, as text: a file in its folder - or, where the path names another branch
/// (path@branch), the file as it last was on that branch of the same repository, read with git. A table a
/// place keeps elsewhere in its own repository is kept by it all the same.
pub fn kept(dir: &Path, path: &str) -> Option<String> {
    let Some((path, branch)) = path.split_once('@') else {
        return std::fs::read_to_string(dir.join(path)).ok();
    };
    // asked once in a run: the same table is read by the graph, by a lens and by the beam
    type AlreadyRead =
        std::sync::Mutex<std::collections::HashMap<(std::path::PathBuf, String), Option<String>>>;
    static READ: std::sync::OnceLock<AlreadyRead> = std::sync::OnceLock::new();
    let read = READ.get_or_init(Default::default);
    let key = (dir.to_path_buf(), format!("{path}@{branch}"));
    if let Some(known) = read.lock().ok().and_then(|r| r.get(&key).cloned()) {
        return known;
    }
    // and, across runs, once for each commit the branch has stood at: what git shows of a file on a branch
    // depends only on the commit the branch is at, and a commit does not change
    let found = [
        ("refs/remotes/origin", format!("origin/{branch}:{path}")),
        ("refs/heads", format!("{branch}:{path}")),
    ]
    .iter()
    .find_map(|(under, at)| {
        let ask = || {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(["show", at])
                .output()
                .ok()?;
            out.status
                .success()
                .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
        };
        answered_for(
            dir,
            &format!("show {at}"),
            commit_at(dir, &format!("{under}/{branch}")).as_deref(),
            ask,
        )
    });
    if let Ok(mut r) = read.lock() {
        r.insert(key, found.clone());
    }
    found
}

/// The commit a name in a repository stands at - HEAD, or a branch by its whole name (refs/...) - read
/// from what git keeps of it, without asking git. Nothing where the folder is no plain repository, or
/// the name is kept in a way not read here: then git is asked, as before.
pub fn commit_at(dir: &Path, name: &str) -> Option<String> {
    let git = dir.join(".git");
    let commit = |said: &str| {
        let said = said.trim();
        (said.len() >= 40 && said.bytes().all(|b| b.is_ascii_hexdigit())).then(|| said.to_string())
    };
    let name = if name == "HEAD" {
        let head = std::fs::read_to_string(git.join("HEAD")).ok()?;
        match head.trim().strip_prefix("ref: ") {
            Some(named) => named.to_string(),
            None => return commit(&head),
        }
    } else {
        name.to_string()
    };
    if let Ok(said) = std::fs::read_to_string(git.join(&name)) {
        return commit(&said);
    }
    std::fs::read_to_string(git.join("packed-refs"))
        .ok()?
        .lines()
        .find_map(|line| {
            let (at, of) = line.split_once(' ')?;
            (of.trim() == name).then(|| commit(at)).flatten()
        })
}

/// What git answered, kept beside the repository under the commit it was answered for. A commit does not
/// change, so what was asked of one is asked once: found again for as long as the name stands at that
/// commit, and asked anew when it has moved. Where the commit is not known, git is asked each time.
pub fn answered_for(
    dir: &Path,
    asked: &str,
    at: Option<&str>,
    ask: impl FnOnce() -> Option<String>,
) -> Option<String> {
    let Some(at) = at else { return ask() };
    let name: String = asked
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let file = dir.join(".git").join("ocoeco").join(name);
    if let Ok(kept) = std::fs::read_to_string(&file) {
        if let Some((for_commit, answer)) = kept.split_once('\n') {
            if for_commit == at {
                return Some(answer.to_string());
            }
        }
    }
    let answer = ask()?;
    if let Some(folder) = file.parent() {
        let _ = std::fs::create_dir_all(folder);
    }
    let _ = std::fs::write(&file, format!("{at}\n{answer}"));
    Some(answer)
}

/// When the branch a table is kept on last changed, in seconds - where the mark says it is kept on one.
pub fn kept_on_a_branch(dir: &Path, mark: &Mark, table: &str) -> Option<i64> {
    let (_, branch) = mark
        .exports
        .iter()
        .find(|(t, _)| t.as_str() == table)?
        .1
        .split_once('@')?;
    [format!("origin/{branch}"), branch.to_string()]
        .iter()
        .find_map(|at| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(["log", "-1", "--format=%ct", at])
                .output()
                .ok()?;
            String::from_utf8_lossy(&out.stdout).trim().parse().ok()
        })
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

/// Tables a repository keeps beside the ones it exports, but does not export: the harvest never sees
/// them (2026-11-11: a device lab's whole backlog, and a tool's decisions and log, went unseen for days).
/// Advice: some may be kept deliberately for the project's own use.
pub fn unexported(dir: &Path) -> Result<Vec<String>> {
    let mark =
        Mark::parse(&std::fs::read_to_string(dir.join("oco.toml")).context("no oco.toml here")?)?;
    let local: Vec<&str> = mark
        .exports
        .values()
        .filter(|p| !p.contains('@'))
        .map(String::as_str)
        .collect();
    let mut folders: Vec<std::path::PathBuf> = local
        .iter()
        .filter_map(|p| Path::new(p).parent().map(Path::to_path_buf))
        .collect();
    folders.sort();
    folders.dedup();
    let mut kept = Vec::new();
    for f in folders {
        let Ok(entries) = std::fs::read_dir(dir.join(&f)) else {
            continue;
        };
        for e in entries.flatten() {
            let p = f.join(e.file_name());
            let rel = p.to_string_lossy().to_string();
            if rel.ends_with(".json") && !local.contains(&rel.as_str()) {
                kept.push(rel);
            }
        }
    }
    kept.sort();
    Ok(kept)
}

/// The words ocoeco itself defines, compiled in: the language every project speaks travels with the
/// tool, so a project learns offline when it redefines one.
pub const SHARED_VOCABULARY: &str = include_str!("../data/vocabulary.json");

/// A project's own words, and the shared words it redefines (advice, never a failure).
pub fn vocabulary(dir: &Path) -> Result<(usize, Vec<String>)> {
    let mark =
        Mark::parse(&std::fs::read_to_string(dir.join("oco.toml")).context("no oco.toml here")?)?;
    let own = rows(dir, &mark, "vocabulary");
    if mark.project.id == "open-commons-observatory/ocoeco" {
        return Ok((own.len(), Vec::new()));
    }
    let shared: Vec<String> = SHARED_VOCABULARY
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .map(|v| v["term"].as_str().unwrap_or("").to_lowercase())
        .collect();
    let clashes = own
        .iter()
        .filter_map(|v| {
            let t = v["term"].as_str().unwrap_or("");
            shared
                .contains(&t.to_lowercase())
                .then(|| format!("'{t}' is a shared word of ocoeco: use it as ocoeco defines it, or choose another word"))
        })
        .collect();
    Ok((own.len(), clashes))
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
                "oco/session_log.json",
                "oco/glimpses.json",
                "oco/decisions.json",
                "oco/problems.json"
            ]
        );
        let (well, problems) = check(&d).unwrap();
        assert!(problems.is_empty(), "{problems:?}");
        assert!(
            well.iter().any(|w| w == "session_log: 1 rows")
                && well
                    .iter()
                    .any(|w| w.starts_with("releases: on the oco-data branch"))
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

    #[test]
    fn the_shared_words_travel_with_the_tool_and_a_redefinition_is_named() {
        assert!(
            SHARED_VOCABULARY.lines().count() >= 15,
            "ocoeco's own words are compiled in"
        );
        assert!(SHARED_VOCABULARY.contains("\"term\":\"lens\""));
        let d = scratch("words");
        init(&d, &join(), "2026-11-11").unwrap();
        let toml = std::fs::read_to_string(d.join("oco.toml"))
            .unwrap()
            .replace(
                "[exports]\n",
                "[exports]\nvocabulary = \"oco/vocabulary.json\"\n",
            );
        std::fs::write(d.join("oco.toml"), toml).unwrap();
        std::fs::write(d.join("oco/vocabulary.json"), "{\"id\":\"a\",\"term\":\"Lens\",\"means\":\"x\"}\n{\"id\":\"b\",\"term\":\"shoot\",\"means\":\"y\"}\n").unwrap();
        let (n, clashes) = vocabulary(&d).unwrap();
        assert_eq!(n, 2);
        assert_eq!(clashes.len(), 1, "{clashes:?}");
        assert!(clashes[0].contains("'Lens' is a shared word"));
    }

    #[test]
    fn a_table_kept_but_not_exported_is_named() {
        let d = scratch("kept");
        init(&d, &join(), "2026-11-11").unwrap();
        assert!(
            unexported(&d).unwrap().is_empty(),
            "a fresh repository exports what it keeps"
        );
        std::fs::write(d.join("oco/concepts.json"), "").unwrap();
        assert_eq!(
            unexported(&d).unwrap(),
            vec!["oco/concepts.json".to_string()]
        );
    }
}

#[cfg(test)]
mod asked_once_for_a_commit {
    use super::*;

    #[test]
    fn what_git_answered_for_a_commit_is_asked_once_and_anew_when_the_name_has_moved() {
        let d = std::env::temp_dir().join(format!("ocoeco-asked-once-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join(".git/refs/heads")).unwrap();
        let (one, two) = ("1".repeat(40), "2".repeat(40));
        std::fs::write(d.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::write(d.join(".git/refs/heads/main"), format!("{one}\n")).unwrap();
        std::fs::write(
            d.join(".git/packed-refs"),
            format!("# pack-refs with: peeled\n{two} refs/remotes/origin/data\n"),
        )
        .unwrap();
        // where a name stands is read from what git keeps: HEAD through its branch, a branch kept loose or packed
        assert_eq!(commit_at(&d, "HEAD").as_deref(), Some(one.as_str()));
        assert_eq!(
            commit_at(&d, "refs/remotes/origin/data").as_deref(),
            Some(two.as_str())
        );
        assert_eq!(commit_at(&d, "refs/heads/nowhere"), None);
        // asked once for a commit, found again, and asked anew when the name has moved
        let asked = std::cell::Cell::new(0);
        let ask = |says: &str| {
            asked.set(asked.get() + 1);
            Some(says.to_string())
        };
        let at = |d: &Path| commit_at(d, "HEAD");
        assert_eq!(
            answered_for(&d, "history", at(&d).as_deref(), || ask("first\nlines")).as_deref(),
            Some("first\nlines")
        );
        assert_eq!(
            answered_for(&d, "history", at(&d).as_deref(), || ask("not asked")).as_deref(),
            Some("first\nlines")
        );
        assert_eq!(asked.get(), 1, "the same commit is asked of once");
        std::fs::write(d.join(".git/refs/heads/main"), format!("{two}\n")).unwrap();
        assert_eq!(
            answered_for(&d, "history", at(&d).as_deref(), || ask("second")).as_deref(),
            Some("second")
        );
        assert_eq!(asked.get(), 2, "a name that has moved is asked of anew");
        // where the commit is not known, git is asked each time - nothing is kept that could be wrong
        assert_eq!(
            answered_for(&d, "other", None, || ask("each time")).as_deref(),
            Some("each time")
        );
        assert_eq!(
            answered_for(&d, "other", None, || ask("each time")).as_deref(),
            Some("each time")
        );
        assert_eq!(asked.get(), 4);
    }
}
