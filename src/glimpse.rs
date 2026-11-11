//! Glimpses: one look at work just done, recorded while it is fresh (SPEC.md section 11). Most of a
//! glimpse is supplied - what `ocoeco note` wrote since the last glimpse, and the backlog items and
//! milestones those rows serve; a person or agent types one line. A glimpse needs no git. `commit` is
//! git's side: it renders a commit from the glimpses not yet committed - git depends on ocoeco, never
//! the reverse.
use crate::mark::Mark;
use crate::note::{self, Change, Row};
use anyhow::{bail, Context, Result};
use std::path::Path;

/// Rows `ocoeco note` wrote since the last glimpse.
const PENDING: &str = ".oco/pending";
/// Glimpses not yet in a commit.
const UNCOMMITTED: &str = ".oco/uncommitted";
/// A summary must fit in a commit title of at most 100 characters, with its kind.
pub const MAX_SUMMARY: usize = 88;

/// Remembers a row `ocoeco note` wrote, for the next glimpse.
pub fn remember(dir: &Path, table: &str, id: &str) -> Result<()> {
    if table == "glimpses" {
        return Ok(());
    }
    append(dir, PENDING, &format!("{table}:{id}"))
}

fn append(dir: &Path, file: &str, line: &str) -> Result<()> {
    let p = dir.join(file);
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    let mut text = std::fs::read_to_string(&p).unwrap_or_default();
    if !text.lines().any(|l| l == line) {
        text.push_str(line);
        text.push('\n');
        std::fs::write(&p, text)?;
    }
    Ok(())
}

fn lines(dir: &Path, file: &str) -> Vec<String> {
    std::fs::read_to_string(dir.join(file))
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(String::from)
        .collect()
}

fn mark(dir: &Path) -> Result<Mark> {
    Mark::parse(
        &std::fs::read_to_string(dir.join("oco.toml"))
            .context("no oco.toml here: this repository has no mark")?,
    )
}

fn rows_of(dir: &Path, mark: &Mark, table: &str) -> Vec<Row> {
    let Some(path) = mark.exports.get(table).filter(|p| !p.contains('@')) else {
        return Vec::new();
    };
    std::fs::read_to_string(dir.join(path))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Row>(l).ok())
        .collect()
}

/// What the touched rows advance: the backlog items among them, then the milestones whose criteria
/// name those items.
pub fn advances(dir: &Path, touches: &[String]) -> Result<Vec<String>> {
    let mark = mark(dir)?;
    let mut out: Vec<String> = touches
        .iter()
        .filter(|t| t.starts_with("backlog:"))
        .cloned()
        .collect();
    for m in rows_of(dir, &mark, "milestones") {
        let criteria = m.text("criteria");
        if out
            .iter()
            .filter(|a| a.starts_with("backlog:"))
            .any(|b| criteria.split_whitespace().any(|c| c == b))
        {
            let id = format!("milestone:{}", m.text("id"));
            if !out.contains(&id) {
                out.push(id);
            }
        }
    }
    Ok(out)
}

/// What a person or agent says in a glimpse; everything else is supplied.
pub struct Glimpse {
    pub summary: String,
    pub kind: String,
    pub breaking: Option<String>,
    pub advances: Vec<String>,
    pub minutes: Option<String>,
    pub load: Option<String>,
    pub by: String,
}

fn change(field: &str, value: &str) -> Change {
    Change {
        field: field.into(),
        value: value.into(),
        append: false,
    }
}

/// The time now (UTC) as written in a glimpse, and a glimpse id made from it.
pub fn now_and_id(secs: i64) -> (String, String) {
    let date = crate::gather::day(secs.div_euclid(86_400));
    let s = secs.rem_euclid(86_400);
    let (h, m, sec) = (s / 3600, s % 3600 / 60, s % 60);
    (
        format!("{date}T{h:02}:{m:02}:{sec:02}Z"),
        format!("g-{}-{h:02}{m:02}{sec:02}", date.replace('-', "")),
    )
}

/// Records a glimpse; returns what it recorded and what it supplied.
pub fn glimpse(dir: &Path, g: &Glimpse, now: &str, id: &str) -> Result<String> {
    let summary = g.summary.trim();
    if summary.is_empty() {
        bail!("a glimpse needs one line saying what was done");
    }
    if summary.chars().count() > MAX_SUMMARY {
        bail!(
            "say it in at most {MAX_SUMMARY} characters (this is {}): it becomes a commit title",
            summary.chars().count()
        );
    }
    if !mark(dir)?.exports.contains_key("glimpses") {
        bail!("this repository does not export a glimpses table yet: add glimpses = \"data/glimpses.json\" to [exports] in oco.toml");
    }
    let touches = lines(dir, PENDING);
    let adv = if g.advances.is_empty() {
        advances(dir, &touches)?
    } else {
        g.advances.clone()
    };
    let mut changes = vec![
        change("at", now),
        change("by", &g.by),
        change("kind", &g.kind),
        change("summary", summary),
    ];
    if let Some(b) = g.breaking.as_deref().filter(|b| !b.trim().is_empty()) {
        changes.push(change("breaking", b));
    }
    if !adv.is_empty() {
        changes.push(change("advances", &adv.join(" ")));
    }
    if !touches.is_empty() {
        changes.push(change("touches", &touches.join(" ")));
    }
    if let Some(m) = &g.minutes {
        changes.push(change("minutes", m));
    }
    if let Some(l) = &g.load {
        changes.push(change("load", l));
    }
    let said = note::note(dir, "glimpses", id, &changes, now.get(..10).unwrap_or(now))?;
    let _ = std::fs::remove_file(dir.join(PENDING));
    append(dir, UNCOMMITTED, id)?;
    let mut out = said;
    if !adv.is_empty() {
        out.push_str(&format!(" - advances {}", adv.join(", ")));
    }
    if !touches.is_empty() {
        out.push_str(&format!(" - {} row(s) it touched", touches.len()));
    }
    Ok(out)
}

/// A commit message rendered from glimpses: the weightiest kind as the title (breaking, then feat,
/// then fix, then the rest; the latest of them), every glimpse in the body, and a trailer for each.
pub fn message(glimpses: &[Row]) -> (String, String) {
    let weight = |r: &Row| {
        if !r.text("breaking").is_empty() {
            0
        } else {
            match r.text("kind").as_str() {
                "feat" => 1,
                "fix" => 2,
                _ => 3,
            }
        }
    };
    let lead = glimpses
        .iter()
        .rev()
        .min_by_key(|r| weight(r))
        .expect("at least one glimpse");
    let bang = if glimpses.iter().any(|r| !r.text("breaking").is_empty()) {
        "!"
    } else {
        ""
    };
    let title = format!("{}{bang}: {}", lead.text("kind"), lead.text("summary"));
    let mut text = format!("{title}\n\n");
    if glimpses.len() > 1 {
        for r in glimpses {
            text.push_str(&format!("- {}: {}\n", r.text("kind"), r.text("summary")));
        }
        text.push('\n');
    }
    for r in glimpses.iter().filter(|r| !r.text("breaking").is_empty()) {
        text.push_str(&format!("BREAKING CHANGE: {}\n", r.text("breaking")));
    }
    for r in glimpses {
        text.push_str(&format!("Glimpse: {}\n", r.text("id")));
    }
    (title, text)
}

fn ensure_ignored(dir: &Path) -> Result<()> {
    let p = dir.join(".gitignore");
    let mut t = std::fs::read_to_string(&p).unwrap_or_default();
    if !t.lines().any(|l| l.trim() == ".oco/") {
        if !t.is_empty() && !t.ends_with('\n') {
            t.push('\n');
        }
        t.push_str(".oco/\n");
        std::fs::write(&p, t)?;
    }
    Ok(())
}

/// Commits what changed (everything, or only what is staged), with a message rendered from the
/// glimpses not yet committed. Refuses when there is none: say what was done first.
pub fn commit(dir: &Path, everything: bool) -> Result<String> {
    let ids = lines(dir, UNCOMMITTED);
    if ids.is_empty() {
        bail!("no glimpse since the last commit: say what you did first (ocoeco glimpse \"...\")");
    }
    let glimpses: Vec<Row> = rows_of(dir, &mark(dir)?, "glimpses")
        .into_iter()
        .filter(|r| ids.contains(&r.text("id")))
        .collect();
    if glimpses.is_empty() {
        bail!("the glimpses waiting for a commit are not in this repository's glimpses table");
    }
    let (title, text) = message(&glimpses);
    ensure_ignored(dir)?;
    let git = |args: &[&str]| -> Result<()> {
        let ok = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .status()
            .context("git is not available")?
            .success();
        if !ok {
            bail!("git {} failed", args.join(" "));
        }
        Ok(())
    };
    if everything {
        git(&["add", "-A"])?;
    }
    let file = dir.join(".oco/message");
    std::fs::write(&file, &text)?;
    git(&[
        "commit",
        "-q",
        "-F",
        file.to_str().context("the path is not text")?,
    ])?;
    let _ = std::fs::remove_file(&file);
    let _ = std::fs::remove_file(dir.join(UNCOMMITTED));
    Ok(format!(
        "committed - {title} ({} glimpse{})",
        glimpses.len(),
        if glimpses.len() == 1 { "" } else { "s" }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ocoeco-glimpse-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("oco")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R\"\nkind = \"tool\"\nparent = \"open-commons-observatory\"\n\n[exports]\nbacklog = \"oco/backlog.json\"\nmilestones = \"oco/milestones.json\"\nglimpses = \"oco/glimpses.json\"\n").unwrap();
        std::fs::write(d.join("oco/milestones.json"), "{\"id\":\"m\",\"subject\":\"o/r\",\"ord\":1,\"title\":\"M\",\"horizon\":\"working\",\"readiness\":2,\"criteria\":\"backlog:b backlog:c\"}\n").unwrap();
        d
    }

    #[test]
    fn a_glimpse_is_mostly_supplied() {
        let d = repo();
        note::note(
            &d,
            "backlog",
            "b",
            &[change("title", "B"), change("status", "in-progress")],
            "2026-11-11",
        )
        .unwrap();
        remember(&d, "backlog", "b").unwrap();
        let g = Glimpse {
            summary: "Did B".into(),
            kind: "feat".into(),
            breaking: None,
            advances: vec![],
            minutes: Some("5".into()),
            load: None,
            by: "mll".into(),
        };
        let said = glimpse(&d, &g, "2026-11-11T21:00:00Z", "g-20261111-210000").unwrap();
        assert!(
            said.contains("advances backlog:b, milestone:m") && said.contains("1 row(s)"),
            "{said}"
        );
        let line = std::fs::read_to_string(d.join("oco/glimpses.json")).unwrap();
        assert!(
            line.contains("\"touches\":\"backlog:b\"")
                && line.contains("\"minutes\":5")
                && line.contains("\"kind\":\"feat\""),
            "{line}"
        );
        assert!(lines(&d, PENDING).is_empty() && lines(&d, UNCOMMITTED) == ["g-20261111-210000"]);
        let long = Glimpse {
            summary: "x".repeat(MAX_SUMMARY + 1),
            ..g
        };
        assert!(glimpse(&d, &long, "2026-11-11T21:01:00Z", "g2")
            .unwrap_err()
            .to_string()
            .contains("commit title"));
    }

    #[test]
    fn a_commit_message_is_rendered_from_glimpses() {
        let row = |s: &str| serde_json::from_str::<Row>(s).unwrap();
        let (title, text) = message(&[
            row(r#"{"id":"g1","kind":"docs","summary":"Wrote it down"}"#),
            row(r#"{"id":"g2","kind":"feat","summary":"Built it"}"#),
            row(r#"{"id":"g3","kind":"fix","summary":"Fixed it"}"#),
        ]);
        assert_eq!(title, "feat: Built it");
        assert!(
            text.contains("- docs: Wrote it down\n")
                && text.ends_with("Glimpse: g1\nGlimpse: g2\nGlimpse: g3\n"),
            "{text}"
        );
        let (title, text) = message(&[row(
            r#"{"id":"g4","kind":"refactor","summary":"Moved it","breaking":"the old name is gone"}"#,
        )]);
        assert_eq!(title, "refactor!: Moved it");
        assert!(
            text.contains("BREAKING CHANGE: the old name is gone\nGlimpse: g4\n"),
            "{text}"
        );
    }

    #[test]
    fn the_time_and_the_id() {
        assert_eq!(
            now_and_id(1_794_431_069),
            (
                "2026-11-11T21:04:29Z".to_string(),
                "g-20261111-210429".to_string()
            )
        );
    }
}
