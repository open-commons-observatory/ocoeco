//! The mark: `oco.toml` at a repository's root - which project it is, what kind, its parent in the
//! picture, and which of its tad tables it shares (schema 1).

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const KINDS: [&str; 8] = [
    "observatory",
    "lens",
    "canon",
    "picture",
    "conveyor",
    "device-lab",
    "tool",
    "research",
];

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Mark {
    pub schema: u32,
    pub project: Project,
    #[serde(default)]
    pub exports: BTreeMap<String, String>,
    /// What place this is beyond a star, and what it gathers; a mark without it is a star's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<Place>,
}

/// A place that gathers: a constellation keeps what holds between its stars; an observatory gathers
/// whatever its keeper looks at. `gathers` names stars as owner/repository, or every star of an owner
/// as owner/* - so a star may be gathered by many places, and a place may gather a star that lives
/// elsewhere. What a star is for (its kind) is another question, answered apart.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Place {
    pub is: String,
    #[serde(default)]
    pub gathers: Vec<String>,
}

/// The places that gather.
pub const PLACES: [&str; 2] = ["constellation", "observatory"];

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Project {
    pub id: String,
    pub name: String,
    /// A short title for navigation (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Its place among its siblings in the picture (optional: lower first, then by name)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<i64>,
    pub kind: String,
    pub parent: String,
}

impl Mark {
    /// Reads and checks a mark: schema 1, a known kind, an id of the form owner/repo.
    pub fn parse(text: &str) -> Result<Mark> {
        let m: Mark = toml::from_str(text).context("oco.toml is not valid TOML for the mark")?;
        if m.schema != 1 {
            bail!("schema {} is not one this engine reads (1)", m.schema);
        }
        if !KINDS.contains(&m.project.kind.as_str()) {
            bail!(
                "kind {:?} is not one of {}",
                m.project.kind,
                KINDS.join(", ")
            );
        }
        let parts: Vec<&str> = m.project.id.split('/').collect();
        if parts.len() != 2 || parts.iter().any(|p| p.is_empty()) {
            bail!("id {:?} is not owner/repo", m.project.id);
        }
        if let Some(place) = &m.place {
            if !PLACES.contains(&place.is.as_str()) {
                bail!("a place is {:?}, which is not one of {} (a mark that says nothing of it is a star's)", place.is, PLACES.join(", "));
            }
            for gathered in &place.gathers {
                let parts: Vec<&str> = gathered.split('/').collect();
                if parts.len() != 2 || parts.iter().any(|p| p.is_empty()) || parts[0] == "*" {
                    bail!("it gathers {gathered:?}, which is neither owner/repository nor owner/*");
                }
            }
        }
        Ok(m)
    }

    /// Whether this place gathers the star `id` (owner/repository): named itself, or by its owner.
    pub fn gathers(&self, id: &str) -> bool {
        let owner = id.split('/').next().unwrap_or_default();
        self.place.as_ref().is_some_and(|place| {
            place
                .gathers
                .iter()
                .any(|g| g == id || g.strip_suffix("/*") == Some(owner))
        })
    }

    /// Where this mark and an older lens.toml beside it disagree on what is gathered: that file's family
    /// (every star of its owners, and the projects it names) is the same truth, kept in agreement while
    /// the file lives.
    pub fn against_family(&self, lens_toml: &str) -> Vec<String> {
        let Ok(old) = toml::from_str::<toml::Value>(lens_toml) else {
            return vec!["lens.toml does not read".into()];
        };
        let names = |key: &str, each: fn(&str) -> String| -> Vec<String> {
            old.get("family")
                .and_then(|f| f.get(key))
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str())
                .map(each)
                .collect()
        };
        let mut family: Vec<String> = names("owners", |o| format!("{o}/*"));
        family.extend(names("projects", |p| p.to_string()));
        family.sort();
        let Some(place) = &self.place else {
            return if family.is_empty() {
                Vec::new()
            } else {
                vec![format!("lens.toml names a family ({}) but the mark does not say what place this is: add [place] is = \"constellation\" and what it gathers", family.join(", "))]
            };
        };
        let mut gathers = place.gathers.clone();
        gathers.sort();
        if gathers == family {
            Vec::new()
        } else {
            vec![format!(
                "the mark gathers {} but lens.toml's family is {}",
                gathers.join(", "),
                family.join(", ")
            )]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOCS: &str = "schema = 1\n[project]\nid = \"linux-phone-oco/docs\"\nname = \"Linux phones\"\nkind = \"picture\"\nparent = \"open-commons-observatory\"\n[exports]\nbacklog = \"data/backlog.json\"\n";

    #[test]
    fn a_mark_reads() {
        let m = Mark::parse(DOCS).unwrap();
        assert_eq!(m.project.kind, "picture");
        assert_eq!(m.exports["backlog"], "data/backlog.json");
    }

    #[test]
    fn a_place_says_what_it_gathers_and_a_star_says_nothing() {
        let star = "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n";
        assert!(
            Mark::parse(star).unwrap().place.is_none()
                && !Mark::parse(star).unwrap().gathers("o/r")
        );
        let place = Mark::parse(&format!(
            "{star}[place]\nis = \"constellation\"\ngathers = [\"phones/*\", \"elsewhere/one\"]\n"
        ))
        .unwrap();
        assert!(
            place.gathers("phones/docs")
                && place.gathers("phones/lab")
                && place.gathers("elsewhere/one")
        );
        assert!(!place.gathers("elsewhere/two") && !place.gathers("phonesque/docs"));
        assert!(Mark::parse(&format!("{star}[place]\nis = \"galaxy\"\n"))
            .unwrap_err()
            .to_string()
            .contains("not one of constellation, observatory"));
        assert!(Mark::parse(&format!(
            "{star}[place]\nis = \"constellation\"\ngathers = [\"everything\"]\n"
        ))
        .unwrap_err()
        .to_string()
        .contains("neither owner/repository nor owner/*"));
        // an older lens.toml beside the mark holds the same truth: kept in agreement
        let old = "[family]\nowners = [\"phones\"]\nprojects = [\"elsewhere/one\"]\n";
        assert!(place.against_family(old).is_empty());
        assert!(
            place.against_family("[family]\nowners = [\"phones\"]\n")[0].contains(
                "the mark gathers elsewhere/one, phones/* but lens.toml's family is phones/*"
            )
        );
        assert!(Mark::parse(star).unwrap().against_family(old)[0]
            .contains("the mark does not say what place this is"));
    }

    #[test]
    fn a_wrong_mark_is_refused_with_a_reason() {
        assert!(Mark::parse(&DOCS.replace("schema = 1", "schema = 2")).is_err());
        assert!(Mark::parse(&DOCS.replace("\"picture\"", "\"blog\"")).is_err());
        assert!(Mark::parse(&DOCS.replace("linux-phone-oco/docs", "docs")).is_err());
        assert!(
            Mark::parse(&DOCS.replace("\"picture\"", "\"observatory\"")).is_ok(),
            "the observatory marks itself"
        );
    }
}
