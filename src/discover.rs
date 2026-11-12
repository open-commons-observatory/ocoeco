//! Discovery, hexagonally: an `Outlet` finds candidate projects on one platform, a `Source` reads a
//! file from one; the core confirms each candidate by its mark. GitHub is the first outlet - its
//! index finds every repository with the `oco-project` topic in one search; others plug in later.

use crate::mark::Mark;
use anyhow::{Context, Result};
use serde::Serialize;

/// A repository an outlet found: where it lives, and its default branch.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Found {
    pub platform: String,
    pub repo: String,
    pub branch: String,
    pub url: String,
}

/// A port: finds candidates on one platform.
pub trait Outlet {
    fn platform(&self) -> &str;
    fn find(&self, kind: Option<&str>) -> Result<Vec<Found>>;
}

/// The latest finished run of one workflow on a repository's default branch.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub workflow: String,
    pub conclusion: String,
    pub at: String,
    pub url: String,
}

/// A port: what has run in a found repository - the latest finished run of each of its active
/// workflows on its default branch.
pub trait Runs {
    fn latest(&self, found: &Found) -> Result<Vec<Run>>;
}

/// The latest finished run of each active workflow, from a platform's list of workflows and its runs
/// on the default branch, newest first. Runs for pull requests and Dependabot's own jobs are not the
/// branch's: they are left out, as are workflows no longer active.
pub fn latest_runs(workflows: &str, runs: &str) -> Result<Vec<Run>> {
    let workflows: serde_json::Value =
        serde_json::from_str(workflows).context("the workflows are not JSON")?;
    let active: std::collections::BTreeSet<&str> = workflows["workflows"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|w| w["state"] == "active")
        .filter_map(|w| w["name"].as_str())
        .collect();
    let runs: serde_json::Value = serde_json::from_str(runs).context("the runs are not JSON")?;
    let mut seen = std::collections::BTreeSet::new();
    let mut latest = Vec::new();
    for r in runs["workflow_runs"].as_array().into_iter().flatten() {
        let (Some(name), Some(event)) = (r["name"].as_str(), r["event"].as_str()) else {
            continue;
        };
        if r["status"] != "completed"
            || matches!(event, "pull_request" | "pull_request_target" | "dynamic")
            || !active.contains(name)
            || !seen.insert(name)
        {
            continue;
        }
        latest.push(Run {
            workflow: name.to_string(),
            conclusion: r["conclusion"].as_str().unwrap_or("unknown").to_string(),
            at: r["created_at"].as_str().unwrap_or_default().to_string(),
            url: r["html_url"].as_str().unwrap_or_default().to_string(),
        });
    }
    Ok(latest)
}

/// A port: reads a file from a found repository (None when it has no such file).
pub trait Source {
    fn read(&self, found: &Found, path: &str) -> Result<Option<String>>;
}

pub const TOPIC: &str = "oco-project";

/// The search for one page: every marked repository, or those of one kind.
pub fn github_query(kind: Option<&str>) -> String {
    match kind {
        Some(k) => format!("topic:{TOPIC}+topic:oco-kind-{k}"),
        None => format!("topic:{TOPIC}"),
    }
}

/// A GitHub search response -> the repositories it lists.
pub fn parse_github_search(text: &str) -> Result<Vec<Found>> {
    let v: serde_json::Value =
        serde_json::from_str(text).context("the search answer is not JSON")?;
    Ok(v["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|i| {
                    Some(Found {
                        platform: "github".into(),
                        repo: i["full_name"].as_str()?.to_string(),
                        branch: i["default_branch"].as_str().unwrap_or("main").to_string(),
                        url: i["html_url"].as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default())
}

/// Where a file of a repository is read: GitHub's contents API, asked for the raw file - current and
/// authoritative (the raw file server caches for minutes, and answered some runners 501 for empty files).
pub fn contents_url(repo: &str, branch: &str, path: &str) -> String {
    format!("https://api.github.com/repos/{repo}/contents/{path}?ref={branch}")
}

/// Whether an address is GitHub's API, which alone gets the API's headers: its media type, the token.
pub fn is_api(url: &str) -> bool {
    url.starts_with("https://api.github.com/")
}

/// The Accept header a GitHub address wants: the API's own media type for the API, none for raw
/// files - the raw file server answers 501 to an empty file asked for as the API's type.
pub fn accept_for(url: &str) -> Option<&'static str> {
    url.starts_with("https://api.github.com/")
        .then_some("application/vnd.github+json")
}

/// GitHub: search by topic, read files raw. A token ($GITHUB_TOKEN) raises the rate limits.
pub struct GitHub {
    token: Option<String>,
}

impl GitHub {
    pub fn new() -> GitHub {
        GitHub {
            token: std::env::var("GITHUB_TOKEN").ok().filter(|t| !t.is_empty()),
        }
    }

    /// A GET on GitHub's API (None for 404), with the token when there is one.
    pub fn get(&self, url: &str) -> Result<Option<String>> {
        self.get_as(url, accept_for(url))
    }

    /// A GET asking for the given media type (None for 404).
    pub fn get_as(&self, url: &str, accept: Option<&str>) -> Result<Option<String>> {
        let mut r = ureq::get(url).set("User-Agent", "ocoeco");
        if let Some(a) = accept {
            r = r.set("Accept", a);
        }
        // the token, like the media type, is for the API only: raw files of public repositories need

        // neither, and the raw file server answers 501 to an empty file asked for with them

        if let (Some(t), true) = (&self.token, is_api(url)) {
            r = r.set("Authorization", &format!("Bearer {t}"));
        }
        match r.call() {
            Ok(resp) => Ok(Some(resp.into_string()?)),
            Err(ureq::Error::Status(404, _)) => Ok(None),
            Err(e) => Err(e).with_context(|| format!("GET {url}")),
        }
    }
}

impl Default for GitHub {
    fn default() -> Self {
        Self::new()
    }
}

impl Outlet for GitHub {
    fn platform(&self) -> &str {
        "github"
    }

    fn find(&self, kind: Option<&str>) -> Result<Vec<Found>> {
        let mut all = Vec::new();
        for page in 1..=10 {
            let url = format!(
                "https://api.github.com/search/repositories?q={}&per_page=100&page={page}",
                github_query(kind)
            );
            let found = parse_github_search(&self.get(&url)?.unwrap_or_default())?;
            let last = found.len() < 100;
            all.extend(found);
            if last {
                break;
            }
        }
        Ok(all)
    }
}

impl Source for GitHub {
    fn read(&self, found: &Found, path: &str) -> Result<Option<String>> {
        self.get_as(
            &contents_url(&found.repo, &found.branch, path),
            Some("application/vnd.github.raw"),
        )
    }
}

impl Runs for GitHub {
    fn latest(&self, found: &Found) -> Result<Vec<Run>> {
        let base = format!("https://api.github.com/repos/{}/actions", found.repo);
        let workflows = self
            .get(&format!("{base}/workflows?per_page=100"))?
            .unwrap_or_else(|| "{}".into());
        let runs = self
            .get(&format!(
                "{base}/runs?branch={}&per_page=100&exclude_pull_requests=true",
                found.branch
            ))?
            .unwrap_or_else(|| "{}".into());
        latest_runs(&workflows, &runs)
    }
}

/// An outlet and a source for the places a machine holds: every marked repository under a root, two
/// levels down (owner/repository) - found where it lives on its platform by its git remote, and read
/// from the disk. The same act, observe, then works without searching.
pub struct Held {
    root: std::path::PathBuf,
}

impl Held {
    pub fn new(root: std::path::PathBuf) -> Held {
        Held { root }
    }
}

/// owner/repository from a git remote on GitHub (https or ssh), if it is one.
pub fn github_repo(remote: &str) -> Option<String> {
    let rest = remote
        .trim()
        .split_once("github.com")?
        .1
        .trim_start_matches([':', '/']);
    let repo = rest.trim_end_matches('/').trim_end_matches(".git");
    (repo.split('/').count() == 2).then(|| repo.to_string())
}

impl Outlet for Held {
    fn platform(&self) -> &str {
        "github"
    }
    fn find(&self, _kind: Option<&str>) -> Result<Vec<Found>> {
        let git = |dir: &std::path::Path, args: &[&str]| {
            std::process::Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(args)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        };
        Ok(crate::board::projects(&self.root)
            .into_iter()
            .filter_map(|dir| {
                let repo = github_repo(&git(&dir, &["remote", "get-url", "origin"])?)?;
                let branch = git(
                    &dir,
                    &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
                )
                .and_then(|b| b.strip_prefix("origin/").map(str::to_string))
                .unwrap_or_else(|| "main".into());
                Some(Found {
                    platform: "github".into(),
                    repo,
                    branch,
                    url: dir.display().to_string(),
                })
            })
            .collect())
    }
}

impl Source for Held {
    fn read(&self, found: &Found, path: &str) -> Result<Option<String>> {
        match std::fs::read_to_string(std::path::Path::new(&found.url).join(path)) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

/// A candidate, confirmed or not: its mark, or why it is not one.
#[derive(Debug, Serialize)]
pub struct Candidate {
    pub found: Found,
    pub mark: Option<Mark>,
    pub problem: Option<String>,
}

/// Every candidate the outlets find, each checked against its mark.
pub fn discover(
    outlets: &[(&dyn Outlet, &dyn Source)],
    kind: Option<&str>,
) -> Result<Vec<Candidate>> {
    let mut out = Vec::new();
    for (outlet, source) in outlets {
        for found in outlet
            .find(kind)
            .with_context(|| format!("finding projects on {}", outlet.platform()))?
        {
            let (mark, problem) = match source.read(&found, "oco.toml") {
                Ok(Some(t)) => match Mark::parse(&t) {
                    Ok(m) => (Some(m), None),
                    Err(e) => (None, Some(format!("{e:#}"))),
                },
                Ok(None) => (None, Some("no oco.toml at its root".into())),
                Err(e) => (None, Some(format!("{e:#}"))),
            };
            out.push(Candidate {
                found,
                mark,
                problem,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_latest_finished_run_of_each_active_workflow_is_kept() {
        let workflows = r#"{"workflows":[{"name":"ci","state":"active"},{"name":"release","state":"active"},{"name":"old","state":"disabled_manually"}]}"#;
        let runs = r#"{"workflow_runs":[
            {"name":"ci","event":"push","status":"in_progress","conclusion":null,"created_at":"5","html_url":"u5"},
            {"name":"release","event":"push","status":"completed","conclusion":"failure","created_at":"4","html_url":"u4"},
            {"name":"ci","event":"pull_request","status":"completed","conclusion":"failure","created_at":"3","html_url":"u3"},
            {"name":"pip in /x","event":"dynamic","status":"completed","conclusion":"failure","created_at":"3","html_url":"d"},
            {"name":"ci","event":"push","status":"completed","conclusion":"success","created_at":"2","html_url":"u2"},
            {"name":"release","event":"push","status":"completed","conclusion":"success","created_at":"1","html_url":"u1"},
            {"name":"old","event":"push","status":"completed","conclusion":"failure","created_at":"1","html_url":"o"}]}"#;
        let latest = latest_runs(workflows, runs).unwrap();
        let seen: Vec<(&str, &str, &str)> = latest
            .iter()
            .map(|r| (r.workflow.as_str(), r.conclusion.as_str(), r.at.as_str()))
            .collect();
        assert_eq!(seen, [("release", "failure", "4"), ("ci", "success", "2")]);
    }

    #[test]
    fn a_held_place_is_found_on_github_by_its_remote() {
        assert_eq!(
            github_repo("https://github.com/o/r.git").as_deref(),
            Some("o/r")
        );
        assert_eq!(
            github_repo("git@github.com:o/r.git\n").as_deref(),
            Some("o/r")
        );
        assert_eq!(
            github_repo("https://github.com/o/r").as_deref(),
            Some("o/r")
        );
        assert_eq!(github_repo("https://gitlab.com/o/r.git"), None);
    }

    #[test]
    fn the_query_finds_all_or_one_kind() {
        assert_eq!(github_query(None), "topic:oco-project");
        assert_eq!(
            github_query(Some("tool")),
            "topic:oco-project+topic:oco-kind-tool"
        );
    }

    #[test]
    fn a_search_answer_lists_its_repositories() {
        let t = r#"{"total_count":1,"items":[{"full_name":"linux-phone-oco/docs","default_branch":"main","html_url":"https://github.com/linux-phone-oco/docs"}]}"#;
        let f = parse_github_search(t).unwrap();
        assert_eq!(f[0].repo, "linux-phone-oco/docs");
        assert_eq!(f[0].branch, "main");
        assert!(parse_github_search(r#"{"items":[]}"#).unwrap().is_empty());
    }

    struct Fake;
    impl Outlet for Fake {
        fn platform(&self) -> &str {
            "fake"
        }
        fn find(&self, _: Option<&str>) -> Result<Vec<Found>> {
            Ok(["a/good", "a/none", "a/bad"]
                .iter()
                .map(|r| Found {
                    platform: "fake".into(),
                    repo: r.to_string(),
                    branch: "main".into(),
                    url: String::new(),
                })
                .collect())
        }
    }
    impl Source for Fake {
        fn read(&self, f: &Found, _: &str) -> Result<Option<String>> {
            Ok(match f.repo.as_str() {
                "a/good" => Some("schema = 1\n[project]\nid = \"a/good\"\nname = \"g\"\nkind = \"tool\"\nparent = \"x\"\n".into()),
                "a/bad" => Some("schema = 9\n".into()),
                _ => None,
            })
        }
    }

    #[test]
    fn candidates_are_confirmed_or_told_why_not() {
        let c = discover(&[(&Fake, &Fake)], None).unwrap();
        assert!(c[0].mark.is_some() && c[0].problem.is_none());
        assert_eq!(c[1].problem.as_deref(), Some("no oco.toml at its root"));
        assert!(
            c[2].mark.is_none() && c[2].problem.is_some(),
            "a broken mark is reported, not dropped"
        );
    }

    #[test]
    fn the_api_type_goes_only_to_the_api() {
        assert_eq!(
            accept_for("https://api.github.com/search/repositories?q=x"),
            Some("application/vnd.github+json")
        );
        assert_eq!(
            accept_for("https://raw.githubusercontent.com/o/r/main/oco/gathered/releases.json"),
            None,
            "raw files answer 501 to it when empty"
        );
    }

    #[test]
    fn files_are_read_through_the_api() {
        let u = contents_url("o/r", "main", "oco/gathered/releases.json");
        assert_eq!(
            u,
            "https://api.github.com/repos/o/r/contents/oco/gathered/releases.json?ref=main"
        );
        assert!(is_api(&u), "so it is current, and the token applies");
    }
}
