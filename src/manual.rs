//! Manual work saturation (SPEC.md section 11): a project's ledger of small actions done by hand. Each
//! record is one action - a verb and its object, a dozen words at most - and keeps how often it was
//! done, how many minutes it took in all, how draining it was on average (load, 0-10, after Borg's
//! CR10 scale of perceived exertion), and the effort it adds up to: minutes weighted by load, where 5
//! counts as one. Saturation is effort, not only count: a manual action done three times, or one past
//! 30 effort-minutes, asks for a tool - so a short heavy chore can outrank a long light one. Looking,
//! listening and deciding are counted like any other action: they can be automated too.
use crate::mark::Mark;
use crate::note::Row;
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::path::Path;

/// Done this many times by hand, a manual action asks for a tool.
pub const SATURATED_COUNT: i64 = 3;
/// Past this many effort-minutes, a manual action asks for a tool however rarely it was done.
pub const SATURATED_EFFORT: i64 = 30;

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

/// Minutes weighted by load: 5 counts as one.
pub fn effort(minutes: i64, load: i64) -> i64 {
    (minutes * load + 2) / 5
}

fn int(r: &Row, k: &str) -> Option<i64> {
    r.text(k).parse().ok()
}

/// Records an action done by hand in the repository at `dir`: bumps the record it matches, adds a new
/// one, or - when it only resembles some - names them and asks for --id or --new. `minutes` and `load`
/// are this time's; left out, a bump takes the record's usual ones.
pub fn manual(
    dir: &Path,
    action: &str,
    id: Option<&str>,
    new: bool,
    minutes: Option<i64>,
    load: Option<i64>,
    now: &str,
) -> Result<String> {
    let today = now.get(..10).unwrap_or(now);
    if let Some(l) = load {
        if !(0..=10).contains(&l) {
            bail!("load is 0 to 10 (0 nothing, 2 easy, 5 hard, 10 maximal), not {l}");
        }
    }
    if minutes.is_some_and(|m| m < 0) {
        bail!("minutes cannot be negative");
    }
    let mark = Mark::parse(
        &std::fs::read_to_string(dir.join("oco.toml"))
            .context("no oco.toml here: this repository has no mark")?,
    )?;
    let path = mark.exports.get("manual_work").context("this repository does not export a manual_work table (add manual_work = \"oco/manual_work.json\" to [exports] in oco.toml)")?;
    if path.contains('@') {
        bail!("manual work is kept on another branch ({path}): it is written there");
    }
    let n = action.split_whitespace().count();
    if id.is_none() && (n == 0 || n > 12) {
        bail!("a manual-work record is one small action - a verb and its object, at most 12 words (this has {n}): split it");
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
    let event: Option<(String, i64, i64)>;
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
            let count0 = int(&r, "count").unwrap_or(0);
            let total0 = int(&r, "minutes").unwrap_or(0);
            let load0 = int(&r, "load").unwrap_or(5);
            let per = minutes.unwrap_or(if count0 > 0 {
                (total0 / count0).max(1)
            } else {
                1
            });
            let l = load.unwrap_or(load0);
            event = Some((r.text("id"), per, l));
            let count = count0 + 1;
            let total = total0 + per;
            let eff = int(&r, "effort").unwrap_or_else(|| effort(total0, load0)) + effort(per, l);
            r.set("count", Value::from(count));
            r.set("minutes", Value::from(total));
            r.set(
                "load",
                Value::from((load0 * count0 + l + count / 2) / count),
            );
            r.set("effort", Value::from(eff));
            r.set("last", Value::String(today.into()));
            let note = match r.text("status").as_str() {
                "automated" => format!(
                    " - it was automated by {}: why by hand again?",
                    r.text("automated_by")
                ),
                _ if count >= SATURATED_COUNT || eff >= SATURATED_EFFORT => {
                    format!(" - saturated ({count}x, ~{eff} effort-minutes): worth a tool")
                }
                _ => String::new(),
            };
            let s = format!("bumped manual_work/{} to {count}{note}", r.text("id"));
            rows[k] = (r, None);
            s
        }
        None => {
            let rid = id.map(String::from).unwrap_or_else(|| slug(action));
            if rid.is_empty() {
                bail!("a record needs an action");
            }
            let (per, l) = (minutes.unwrap_or(1), load.unwrap_or(5));
            event = Some((rid.clone(), per, l));
            rows.push((
                Row(vec![
                    ("id".into(), Value::String(rid.clone())),
                    ("action".into(), Value::String(action.trim().into())),
                    ("count".into(), Value::from(1)),
                    ("first".into(), Value::String(today.into())),
                    ("last".into(), Value::String(today.into())),
                    ("status".into(), Value::String("manual".into())),
                    ("automated_by".into(), Value::String(String::new())),
                    ("minutes".into(), Value::from(per)),
                    ("load".into(), Value::from(l)),
                    ("effort".into(), Value::from(effort(per, l))),
                ]),
                None,
            ));
            rows.sort_by_key(|(r, _)| r.text("id"));
            format!("added manual_work/{rid}")
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
    // when it was done: one event per record or bump, appended - so the views can set manual work beside
    // the commits and releases of the same days
    let when = match (event, mark.exports.get("manual_events")) {
        (Some((rid, per, l)), Some(ep)) if !ep.contains('@') => {
            let ef = dir.join(ep);
            let mut ev = Row(vec![
                ("id".into(), Value::String(format!("{now}-{rid}"))),
                ("record".into(), Value::String(rid)),
                ("at".into(), Value::String(now.into())),
                ("minutes".into(), Value::from(per)),
                ("load".into(), Value::from(l)),
                ("effort".into(), Value::from(effort(per, l))),
            ]);
            if let Ok(c) = std::env::var("OCOECO_CHANGE") {
                if !c.is_empty() {
                    ev.set("change", Value::String(c));
                }
            }
            if let Some(p) = ef.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut t = std::fs::read_to_string(&ef).unwrap_or_default();
            if !t.is_empty() && !t.ends_with('\n') {
                t.push('\n');
            }
            t.push_str(&(ev.line() + "\n"));
            std::fs::write(&ef, t)?;
            String::new()
        }
        (Some(_), _) => " (add manual_events = \"oco/manual_events.json\" to [exports] to keep when it was done)".to_string(),
        _ => String::new(),
    };
    Ok(said + &when)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ocoeco-manual-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("oco")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R\"\nkind = \"tool\"\nparent = \"open-commons-observatory\"\n\n[exports]\nmanual_work = \"oco/manual_work.json\"\nmanual_events = \"oco/manual_events.json\"\n").unwrap();
        d
    }
    fn file(d: &Path) -> String {
        std::fs::read_to_string(d.join("oco/manual_work.json")).unwrap()
    }

    #[test]
    fn a_bump_adds_time_and_effort_and_counts_saturation_by_either() {
        let d = repo("bump");
        assert_eq!(
            manual(
                &d,
                "boot a test kernel and wait for the phone",
                None,
                false,
                Some(6),
                Some(5),
                "2026-11-11"
            )
            .unwrap(),
            "added manual_work/boot-a-test-kernel-and-wait-for"
        );
        assert!(
            manual(
                &d,
                "Boot a test kernel, wait for the phone",
                None,
                false,
                None,
                None,
                "2026-11-12"
            )
            .unwrap()
            .ends_with("to 2"),
            "usual minutes and load when left out"
        );
        let third = manual(
            &d,
            "boot the test kernel and wait for phone",
            None,
            false,
            Some(6),
            Some(8),
            "2026-11-12",
        )
        .unwrap();
        assert!(third.contains("saturated (3x"), "{third}");
        let t = file(&d);
        assert!(
            t.contains("\"count\":3")
                && t.contains("\"minutes\":18")
                && t.contains("\"load\":6")
                && t.contains("\"effort\":22"),
            "{t}"
        );
    }

    #[test]
    fn a_heavy_chore_saturates_before_three() {
        let d = repo("heavy");
        manual(
            &d,
            "carry the heavy box upstairs",
            None,
            false,
            Some(20),
            Some(9),
            "x",
        )
        .unwrap();
        let second = manual(
            &d,
            "carry the heavy box upstairs",
            None,
            false,
            Some(20),
            Some(9),
            "x",
        )
        .unwrap();
        assert!(
            second.contains("saturated (2x, ~72 effort-minutes)"),
            "{second}"
        );
    }

    #[test]
    fn a_resemblance_is_shown_and_the_person_chooses() {
        let d = repo("near");
        manual(
            &d,
            "patch source code by matching text",
            None,
            false,
            None,
            None,
            "x",
        )
        .unwrap();
        let e = manual(
            &d,
            "patch the workflow by matching its text",
            None,
            false,
            None,
            None,
            "x",
        )
        .unwrap_err()
        .to_string();
        assert!(
            e.contains("resembles: patch-source-code-by-matching-text"),
            "{e}"
        );
        assert!(manual(
            &d,
            "patch the workflow by matching its text",
            None,
            true,
            None,
            None,
            "x"
        )
        .unwrap()
        .starts_with("added"));
        assert!(manual(
            &d,
            "",
            Some("patch-source-code-by-matching-text"),
            false,
            None,
            None,
            "x"
        )
        .unwrap()
        .contains("to 2"));
    }

    #[test]
    fn records_stay_small_loads_stay_on_the_scale_and_an_automated_one_asks_why() {
        let d = repo("small");
        assert!(manual(
            &d,
            "one two three four five six seven eight nine ten eleven twelve thirteen",
            None,
            false,
            None,
            None,
            "x"
        )
        .unwrap_err()
        .to_string()
        .contains("split it"));
        assert!(manual(&d, "lift a box", None, false, None, Some(11), "x")
            .unwrap_err()
            .to_string()
            .contains("0 to 10"));
        std::fs::write(d.join("oco/manual_work.json"), "{\"id\":\"w\",\"action\":\"write a row by hand\",\"count\":9,\"first\":\"a\",\"last\":\"b\",\"status\":\"automated\",\"automated_by\":\"ocoeco note\"}\n").unwrap();
        assert!(
            manual(&d, "write a row by hand", None, false, None, None, "x")
                .unwrap()
                .contains("automated by ocoeco note: why by hand again?")
        );
    }

    #[test]
    fn every_record_and_bump_keeps_when_it_was_done() {
        let d = repo("when");
        manual(
            &d,
            "flash the phone by hand",
            None,
            false,
            Some(4),
            Some(3),
            "2026-11-11T10:00:00Z",
        )
        .unwrap();
        manual(
            &d,
            "flash the phone by hand",
            None,
            false,
            None,
            None,
            "2026-11-12T09:30:00Z",
        )
        .unwrap();
        let ev = std::fs::read_to_string(d.join("oco/manual_events.json")).unwrap();
        assert_eq!(ev.lines().count(), 2, "{ev}");
        assert!(
            ev.contains("\"at\":\"2026-11-11T10:00:00Z\"")
                && ev.contains("\"record\":\"flash-the-phone-by-hand\"")
        );
        assert!(
            ev.lines()
                .nth(1)
                .unwrap()
                .contains("\"minutes\":4,\"load\":3,\"effort\":2"),
            "a bump takes the usual minutes and load"
        );
        let t = file(&d);
        assert!(
            t.contains("\"first\":\"2026-11-11\"") && t.contains("\"last\":\"2026-11-12\""),
            "the record keeps the day"
        );
    }
}
