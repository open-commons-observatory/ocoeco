//! What a place keeps out. Any lens may say what it keeps out - a token, a key, an address - told by its
//! shape. It is kept out as a row is written: the words around it stay, and what was kept out is named
//! where it stood. Wherever one is found in what a place keeps, it shows. Not a wall around a place: a
//! lens, seen like any other, whose shapes anyone can read.

use regex::Regex;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The tables the program makes of what is kept out: what the lenses say is kept out, and where in what
/// the places keep something was kept out or is showing.
pub const OF_THE_KEPT: [&str; 2] = ["kept_out", "kept"];

/// One thing a lens says is kept out: what it is, the shape it is told by, and what of that shape is
/// let be.
pub struct Told {
    pub what: String,
    pub lens: String,
    pub shape: String,
    pub unless: String,
    shaped: Regex,
    but: Option<Regex>,
}

/// What the lenses in view from these places say is kept out. A shape that cannot be read is left out:
/// the check of a lens's shape says so.
pub fn told(dirs: &[PathBuf]) -> Vec<Told> {
    let lenses = crate::lenses::with(dirs).unwrap_or_else(|_| crate::lenses::lenses().to_vec());
    let mut all = Vec::new();
    for lens in &lenses {
        for kept in &lens.keeps_out {
            let (Ok(shaped), Ok(but)) = (
                Regex::new(&kept.shape),
                kept.unless.as_deref().map(Regex::new).transpose(),
            ) else {
                continue;
            };
            all.push(Told {
                what: kept.what.clone(),
                lens: lens.lens.id.clone(),
                shape: kept.shape.clone(),
                unless: kept.unless.clone().unwrap_or_default(),
                shaped,
                but,
            });
        }
    }
    all
}

/// How what was kept out is named where it stood.
fn named(what: &str) -> String {
    format!("[{what}, kept out]")
}

/// The words with what is kept out taken from them - named where it stood - and what was kept out.
pub fn out_of(text: &str, told: &[Told]) -> (String, Vec<String>) {
    let mut kept = Vec::new();
    let mut text = text.to_string();
    for t in told {
        text = t
            .shaped
            .replace_all(&text, |found: &regex::Captures| {
                if t.but.as_ref().is_some_and(|but| but.is_match(&found[0])) {
                    found[0].to_string()
                } else {
                    kept.push(t.what.clone());
                    named(&t.what)
                }
            })
            .into_owned();
    }
    (text, kept)
}

/// As rows: what the lenses in view say is kept out; and, for every row the places in view keep, what
/// was kept out of it and what of that kind is showing in it still.
pub fn as_rows(
    dirs: &[PathBuf],
) -> anyhow::Result<BTreeMap<&'static str, Vec<serde_json::Map<String, serde_json::Value>>>> {
    use serde_json::json;
    let told = told(dirs);
    let row = |v: serde_json::Value| v.as_object().cloned().unwrap_or_default();
    let said: Vec<_> = told
        .iter()
        .map(|t| row(json!({"id": format!("{}/{}", t.lens, t.what), "lens": t.lens, "what": t.what, "shape": t.shape, "unless": t.unless, "star": t.lens})))
        .collect();
    let marked = Regex::new(r"\[([^\[\]]{1,40}), kept out\]").expect("a shape written here");
    let short = |id: &str| id.rsplit('/').next().unwrap_or(id).to_string();
    let mut kept = Vec::new();
    for (place, held) in crate::relations::held(dirs) {
        for (table, rows) in &held.tables {
            for r in rows {
                let Some(id) = r.get("id").and_then(|i| i.as_str()) else {
                    continue;
                };
                // what of each kind: how often kept out, and how often showing
                let mut found: BTreeMap<(String, &str), u64> = BTreeMap::new();
                for words in r
                    .as_object()
                    .into_iter()
                    .flat_map(|o| o.values())
                    .filter_map(|v| v.as_str())
                {
                    for what in marked.captures_iter(words) {
                        *found.entry((what[1].to_string(), "kept out")).or_default() += 1;
                    }
                    for (_, out) in [out_of(words, &told)] {
                        for what in out {
                            *found.entry((what, "showing")).or_default() += 1;
                        }
                    }
                }
                for ((what, state), times) in found {
                    let thing = format!("{place}/{table}:{id}");
                    kept.push(row(json!({"id": format!("{thing}#{what}#{state}"), "thing": thing, "of_table": table, "place": place, "star": short(&place),
                                         "what": what, "state": state, "times": times})));
                }
            }
        }
    }
    Ok(BTreeMap::from([("kept_out", said), ("kept", kept)]))
}

/// What this place keeps that holds something kept out, each as a line for the check: nothing of that
/// kind lands.
pub fn showing(dir: &Path) -> Vec<String> {
    let Some(own) = std::fs::read_to_string(dir.join("oco.toml"))
        .ok()
        .and_then(|t| crate::mark::Mark::parse(&t).ok())
        .map(|m| m.project.id)
    else {
        return Vec::new();
    };
    let said = |r: &serde_json::Map<String, serde_json::Value>, key: &str| {
        r.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    as_rows(&crate::board::in_view(dir).0)
        .map(|mut rows| rows.remove("kept").unwrap_or_default())
        .unwrap_or_default()
        .iter()
        .filter(|r| said(r, "state") == "showing" && said(r, "place") == own)
        .map(|r| {
            format!(
                "kept-out {}: holds {}, which is kept out",
                said(r, "thing").trim_start_matches(&format!("{own}/")),
                said(r, "what")
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // a thing of the shape of a token, made here so that none is written in this file
    fn token_shaped() -> String {
        format!("gh{}_{}", "p", "x".repeat(36))
    }

    #[test]
    fn what_is_kept_out_is_named_where_it_stood_and_the_words_around_it_stay() {
        let told = told(&[]);
        assert!(
            told.iter().any(|t| t.what == "a token"),
            "the language says what it keeps out"
        );
        let (words, kept) = out_of(
            &format!(
                "set it with {} and ask the phone at 172.16.42.1, not at 8.8.8.8",
                token_shaped()
            ),
            &told,
        );
        assert_eq!(words, "set it with [a token, kept out] and ask the phone at 172.16.42.1, not at [an address of a machine, kept out]");
        assert_eq!(
            kept,
            ["a token", "an address of a machine"],
            "an address no one beyond the machine can reach is let be"
        );
    }

    #[test]
    fn what_is_found_in_what_a_place_keeps_shows_and_the_check_says_so() {
        let d = std::env::temp_dir()
            .join(format!("ocoeco-what-is-kept-out-{}", std::process::id()))
            .join("o/p");
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/p\"\nname = \"P: a place\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nnotes = \"data/notes.json\"\n").unwrap();
        std::fs::write(d.join("data/notes.json"), format!("{{\"id\":\"a\",\"text\":\"it was [a token, kept out] once\"}}\n{{\"id\":\"b\",\"text\":\"written past the entry: {}\"}}\n", token_shaped())).unwrap();
        let rows = as_rows(std::slice::from_ref(&d)).unwrap();
        let said: Vec<(String, String)> = rows["kept"]
            .iter()
            .map(|r| {
                (
                    r["thing"].as_str().unwrap().to_string(),
                    r["state"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        assert_eq!(
            said,
            [
                ("o/p/notes:a".to_string(), "kept out".to_string()),
                ("o/p/notes:b".to_string(), "showing".to_string())
            ]
        );
        assert_eq!(
            showing(&d),
            ["kept-out notes:b: holds a token, which is kept out"]
        );
    }
}
