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
