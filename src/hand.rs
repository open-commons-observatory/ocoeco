//! Hand-work saturation (SPEC.md section 11): a project's ledger of small actions done by hand. Each
//! record is one action - a verb and its object, a dozen words at most - with how often it was done.
//! Doing it again bumps the count instead of adding a line, so a repeated action stands out: past
//! three, it asks for a tool. The tool keeps the wording from drifting: a close match is bumped, a
//! possible one is shown and the person chooses. A record automated by a tool says so; a record kept
//! deliberately by hand (looking, listening, deciding) never asks to be automated.
use crate::mark::Mark;
use crate::note::Row;
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::path::Path;

fn words(s: &str) -> Vec<String> {
    let mut w: Vec<String> = s
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() > 2)
        .map(String::from)
        .collect();
    w.sort();
    w.dedup();
    w
}

/// How alike two actions are: the share of their words in common.
pub fn likeness(a: &str, b: &str) -> f64 {
    let (a, b) = (words(a), words(b));
    let common = a.iter().filter(|w| b.contains(w)).count();
    let all = a.len() + b.len() - common;
    if all == 0 {
        0.0
    } else {
        common as f64 / all as f64
    }
}

/// A record's id from its action: its first words, lowercase, joined by dashes.
pub fn slug(action: &str) -> String {
    action
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(7)
        .collect::<Vec<_>>()
        .join("-")
}

/// Records an action done by hand in the repository at `dir`: bumps the record it matches, adds a new
/// one, or - when it only resembles some - names them and asks for --id or --new.
pub fn hand(
    dir: &Path,
    action: &str,
    id: Option<&str>,
    new: bool,
    minutes: Option<i64>,
    today: &str,
) -> Result<String> {
    let mark = Mark::parse(
        &std::fs::read_to_string(dir.join("oco.toml"))
            .context("no oco.toml here: this repository has no mark")?,
    )?;
    let path = mark.exports.get("handwork").context("this repository does not export a handwork table (add handwork = \"oco/handwork.json\" to [exports] in oco.toml)")?;
    if path.contains('@') {
        bail!("handwork is kept on another branch ({path}): it is written there");
    }
    let n = action.split_whitespace().count();
    if id.is_none() && (n == 0 || n > 12) {
        bail!("a hand-work record is one small action - a verb and its object, at most 12 words (this has {n}): split it");
    }
    let file = dir.join(path);
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    let mut rows: Vec<(Row, Option<String>)> = Vec::new();
    for l in text.lines().filter(|l| !l.trim().is_empty()) {
        rows.push((
            serde_json::from_str(l)
                .with_context(|| format!("{}: a line is not a row", file.display()))?,
            Some(l.to_string()),
        ));
    }
    let target = match id {
        Some(i) => rows.iter().position(|(r, _)| r.text("id") == i),
        None => {
            let mut scored: Vec<(f64, usize)> = rows
                .iter()
                .enumerate()
                .map(|(k, (r, _))| (likeness(action, &r.text("action")), k))
                .collect();
            scored.sort_by(|a, b| b.0.total_cmp(&a.0));
            match scored.first() {
                Some((s, k)) if *s >= 0.75 => Some(*k),
                _ if new => None,
                _ => {
                    let near: Vec<String> = scored
                        .iter()
                        .filter(|(s, _)| *s >= 0.35)
                        .take(3)
                        .map(|(_, k)| {
                            format!("{} ({})", rows[*k].0.text("id"), rows[*k].0.text("action"))
                        })
                        .collect();
                    if !near.is_empty() {
                        bail!(
                            "this resembles: {} - bump one with --id, or add it with --new",
                            near.join("; ")
                        );
                    }
                    rows.iter().position(|(r, _)| r.text("id") == slug(action))
                }
            }
        }
    };
    let said = match target {
        Some(k) => {
            let mut r = rows[k].0.clone();
            let count = r.text("count").parse::<i64>().unwrap_or(0) + 1;
            r.set("count", Value::from(count));
            r.set("last", Value::String(today.into()));
            if let Some(m) = minutes {
                r.set("minutes", Value::from(m));
            }
            let note = match r.text("status").as_str() {
                "automated" => format!(
                    " - it was automated by {}: why by hand again?",
                    r.text("automated_by")
                ),
                "manual" if count >= 3 => " - saturated: worth a tool".to_string(),
                _ => String::new(),
            };
            let s = format!("bumped handwork/{} to {count}{note}", r.text("id"));
            rows[k] = (r, None);
            s
        }
        None => {
            let rid = id.map(String::from).unwrap_or_else(|| slug(action));
            if rid.is_empty() {
                bail!("a record needs an action");
            }
            let mut r = Row(vec![
                ("id".into(), Value::String(rid.clone())),
                ("action".into(), Value::String(action.trim().into())),
                ("count".into(), Value::from(1)),
                ("first".into(), Value::String(today.into())),
                ("last".into(), Value::String(today.into())),
                ("status".into(), Value::String("manual".into())),
                ("automated_by".into(), Value::String(String::new())),
            ]);
            if let Some(m) = minutes {
                r.set("minutes", Value::from(m));
            }
            rows.push((r, None));
            rows.sort_by_key(|(r, _)| r.text("id"));
            format!("added handwork/{rid}")
        }
    };
    if let Some(p) = file.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(
        &file,
        rows.iter()
            .map(|(r, o)| o.clone().unwrap_or_else(|| r.line()) + "\n")
            .collect::<String>(),
    )?;
    Ok(said)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ocoeco-hand-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("oco")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R\"\nkind = \"tool\"\nparent = \"open-commons-observatory\"\n\n[exports]\nhandwork = \"oco/handwork.json\"\n").unwrap();
        d
    }

    #[test]
    fn the_same_action_bumps_a_count_and_says_when_it_is_saturated() {
        let d = repo("bump");
        assert_eq!(
            hand(
                &d,
                "boot a test kernel and wait for the phone",
                None,
                false,
                None,
                "2026-11-11"
            )
            .unwrap(),
            "added handwork/boot-a-test-kernel-and-wait-for"
        );
        assert!(hand(
            &d,
            "Boot a test kernel, wait for the phone",
            None,
            false,
            None,
            "2026-11-12"
        )
        .unwrap()
        .starts_with("bumped handwork/boot-a-test-kernel-and-wait-for to 2"));
        let third = hand(
            &d,
            "boot the test kernel and wait for phone",
            None,
            false,
            Some(4),
            "2026-11-12",
        )
        .unwrap();
        assert!(third.ends_with("saturated: worth a tool"), "{third}");
        let t = std::fs::read_to_string(d.join("oco/handwork.json")).unwrap();
        assert!(
            t.contains("\"count\":3")
                && t.contains("\"first\":\"2026-11-11\"")
                && t.contains("\"last\":\"2026-11-12\"")
                && t.contains("\"minutes\":4"),
            "{t}"
        );
    }

    #[test]
    fn a_resemblance_is_shown_and_the_person_chooses() {
        let d = repo("near");
        hand(
            &d,
            "patch source code by matching text",
            None,
            false,
            None,
            "x",
        )
        .unwrap();
        let e = hand(
            &d,
            "patch the workflow by matching its text",
            None,
            false,
            None,
            "x",
        )
        .unwrap_err()
        .to_string();
        assert!(
            e.contains("resembles: patch-source-code-by-matching-text"),
            "{e}"
        );
        assert!(hand(
            &d,
            "patch the workflow by matching its text",
            None,
            true,
            None,
            "x"
        )
        .unwrap()
        .starts_with("added"));
        assert!(hand(
            &d,
            "",
            Some("patch-source-code-by-matching-text"),
            false,
            None,
            "x"
        )
        .unwrap()
        .contains("to 2"));
    }

    #[test]
    fn a_record_stays_small_and_an_automated_one_asks_why() {
        let d = repo("small");
        assert!(hand(
            &d,
            "one two three four five six seven eight nine ten eleven twelve thirteen",
            None,
            false,
            None,
            "x"
        )
        .unwrap_err()
        .to_string()
        .contains("split it"));
        std::fs::write(d.join("oco/handwork.json"), "{\"id\":\"w\",\"action\":\"write a row by hand\",\"count\":9,\"first\":\"a\",\"last\":\"b\",\"status\":\"automated\",\"automated_by\":\"ocoeco note\"}\n").unwrap();
        assert!(hand(&d, "write a row by hand", None, false, None, "x")
            .unwrap()
            .contains("automated by ocoeco note: why by hand again?"));
    }
}
