//! Gatherers: data made by work (SPEC.md section 7). Each reads a real source of a repository's
//! activity and writes a light tad table: `activity` (commits per day), `releases`, `ci` (the latest
//! run of each workflow on the default branch). Rows a gatherer writes are never edited by hand.

use crate::discover::GitHub;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

/// How far back activity is gathered.
pub const DAYS: i64 = 90;

/// Commit dates (ISO 8601) -> one row per day: { id: "YYYY-MM-DD", commits }.
pub fn activity(dates: &[String]) -> Vec<Value> {
    let mut per_day: BTreeMap<String, u64> = BTreeMap::new();
    for d in dates.iter().filter(|d| d.len() >= 10) {
        *per_day.entry(d[..10].to_string()).or_default() += 1;
    }
    per_day
        .into_iter()
        .map(|(day, n)| json!({"id": day, "commits": n}))
        .collect()
}

/// A GitHub releases answer -> { id: tag, name, published, link }.
pub fn releases(text: &str) -> Vec<Value> {
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|r| {
                    Some(json!({
                        "id": r["tag_name"].as_str()?,
                        "name": r["name"].as_str().unwrap_or(""),
                        "published": r["published_at"].as_str().unwrap_or("").chars().take(10).collect::<String>(),
                        "link": r["html_url"].as_str().unwrap_or(""),
                    }))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A GitHub workflow-runs answer (newest first) -> the latest run of each workflow:
/// { id: workflow, conclusion, at, link }; a run still going counts as its status.
pub fn ci(text: &str) -> Vec<Value> {
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let mut latest: BTreeMap<String, Value> = BTreeMap::new();
    for r in v["workflow_runs"].as_array().into_iter().flatten() {
        let Some(name) = r["name"].as_str() else {
            continue;
        };
        latest.entry(name.to_string()).or_insert_with(|| {
            json!({
                "id": name,
                "conclusion": r["conclusion"].as_str().or(r["status"].as_str()).unwrap_or("unknown"),
                "at": r["created_at"].as_str().unwrap_or(""),
                "link": r["html_url"].as_str().unwrap_or(""),
            })
        });
    }
    latest.into_values().collect()
}

/// A unix day number -> "YYYY-MM-DD" (civil-from-days).
pub fn day(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{m:02}-{d:02}", yoe + era * 400 + i64::from(m <= 2))
}

/// Rows as a light tad table: one compact object per line, ordered by id.
pub fn lines(mut rows: Vec<Value>) -> String {
    rows.sort_by_key(|r| r["id"].as_str().unwrap_or("").to_string());
    rows.iter().map(|r| format!("{r}\n")).collect()
}

/// Gathers a repository's activity, releases and CI into `out`; returns the row counts.
pub fn gather(gh: &GitHub, repo: &str, out: &Path) -> Result<[usize; 3]> {
    let api = |path: String| -> Result<String> {
        let url = if path.is_empty() {
            format!("https://api.github.com/repos/{repo}")
        } else {
            format!("https://api.github.com/repos/{repo}/{path}")
        };
        Ok(gh.get(&url)?.unwrap_or_default())
    };
    let info: Value =
        serde_json::from_str(&api(String::new())?).context("the repository could not be read")?;
    let branch = info["default_branch"]
        .as_str()
        .unwrap_or("main")
        .to_string();
    let today = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64 / 86_400)
        .unwrap_or(0);
    let since = day(today - DAYS);
    let mut dates = Vec::new();
    for page in 1..=10 {
        let v: Value = serde_json::from_str(&api(format!(
            "commits?sha={branch}&since={since}T00:00:00Z&per_page=100&page={page}"
        ))?)
        .unwrap_or(Value::Null);
        let batch: Vec<String> = v
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| c["commit"]["committer"]["date"].as_str().map(String::from))
            .collect();
        let last = batch.len() < 100;
        dates.extend(batch);
        if last {
            break;
        }
    }
    let act = activity(&dates);
    let rel = releases(&api("releases?per_page=30".into())?);
    let runs = ci(&api(format!("actions/runs?branch={branch}&per_page=50"))?);
    std::fs::create_dir_all(out)?;
    let counts = [act.len(), rel.len(), runs.len()];
    std::fs::write(out.join("activity.json"), lines(act))?;
    std::fs::write(out.join("releases.json"), lines(rel))?;
    std::fs::write(out.join("ci.json"), lines(runs))?;
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commits_become_days() {
        let d = [
            "2026-11-10T09:00:00Z",
            "2026-11-10T17:00:00Z",
            "2026-11-09T01:00:00Z",
            "bad",
        ]
        .map(String::from);
        let a = activity(&d);
        assert_eq!(
            a,
            vec![
                json!({"id": "2026-11-09", "commits": 1}),
                json!({"id": "2026-11-10", "commits": 2})
            ]
        );
    }

    #[test]
    fn releases_are_read() {
        let r = releases(
            r#"[{"tag_name":"v0.1.40","name":"v0.1.40","published_at":"2026-11-10T15:00:00Z","html_url":"https://x"}]"#,
        );
        assert_eq!(r[0]["id"], "v0.1.40");
        assert_eq!(r[0]["published"], "2026-11-10");
        assert!(releases("not json").is_empty());
    }

    #[test]
    fn the_latest_run_of_each_workflow() {
        let t = r#"{"workflow_runs":[
            {"name":"ci","conclusion":"success","created_at":"2","html_url":"b"},
            {"name":"ci","conclusion":"failure","created_at":"1","html_url":"a"},
            {"name":"harvest","conclusion":null,"status":"in_progress","created_at":"3","html_url":"c"}]}"#;
        let c = ci(t);
        assert_eq!(c.len(), 2);
        assert_eq!(
            c[0]["conclusion"], "success",
            "newest first: the latest ci run"
        );
        assert_eq!(
            c[1]["conclusion"], "in_progress",
            "a run still going counts as its status"
        );
    }

    #[test]
    fn days_and_lines() {
        assert_eq!(day(0), "1970-01-01");
        assert_eq!(day(20_767), "2026-11-10");
        assert_eq!(
            lines(vec![json!({"id": "b"}), json!({"id": "a"})]),
            "{\"id\":\"a\"}\n{\"id\":\"b\"}\n"
        );
    }
}
