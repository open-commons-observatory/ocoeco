//! The lenses built into this ocoeco (lenses/*.toml). A lens has two faces: its glimpse catches data -
//! the tables it defines and how they relate - and its focus brings it into view. Until 2026-11-12 the
//! tables sat together in one file, written from outside; now each belongs to the lens that needs it.
//! Regrouped, not rewritten: a test holds them to the language as it stood.

use crate::relations::Relation;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

include!(concat!(env!("OUT_DIR"), "/lenses.rs"));

/// What a lens says of itself.
#[derive(serde::Deserialize, Clone, Debug)]
pub struct About {
    pub id: String,
    pub says: String,
    /// The lens this one grew from, if any: lenses are additive, and a changed lens is a new one.
    #[serde(default)]
    pub grew_from: Option<String>,
    /// Tables it reads that other lenses define: one lens defines a piece of data, many read it.
    #[serde(default)]
    pub reads: Vec<String>,
}

/// One lens, as declared.
#[derive(serde::Deserialize, Clone, Debug)]
pub struct Lens {
    pub lens: About,
    /// The tables its glimpse defines (tad's declaration of each).
    #[serde(default)]
    pub tables: toml::Table,

    /// How its rows relate to others.
    #[serde(default)]
    pub relation: Vec<Relation>,
    /// Its glimpse: what can be entered through it.
    #[serde(default)]
    pub glimpse: Vec<Entry>,
    /// Its focus: the parts it brings into view.
    #[serde(default)]
    pub focus: Vec<Part>,
}

/// One thing that can be entered through a lens: the table it fills, what a person is asked for, and
/// what the computer supplies.
#[derive(serde::Deserialize, Clone, Debug)]
pub struct Entry {
    pub entry: String,
    pub says: String,
    pub table: String,
    /// The fields a person is asked for, in the order they are shown.
    pub asks: Vec<String>,
    #[serde(default)]
    pub supplies: Vec<Supply>,
}

/// A field the computer supplies: from what it knows (today, who), and when (new, always, FIELD
/// changes, FIELD becomes VALUE).
#[derive(serde::Deserialize, Clone, Debug)]
pub struct Supply {
    pub field: String,
    pub from: String,
    pub when: String,
}

/// One part of a focus: a query over the lens's gathered tables, and how one answer is laid out.
#[derive(serde::Deserialize, Clone, Debug)]
pub struct Part {
    /// A heading; {count} is the number of answers. A part with none has no heading.
    pub title: Option<String>,
    /// Shown even when there are no answers.
    #[serde(default)]
    pub always: bool,
    pub sql: String,
    /// One answer, laid out: each {name} is that answer's value.
    pub line: String,
    /// The answer's column that is a measure of a whole, if its lines are such: meaning a surface may
    /// show in its own way - a meter on a page, a bar in a terminal.
    #[serde(default)]
    pub measure: Option<String>,
    /// The whole the measure is of, when it is the same for every line.
    #[serde(default)]
    pub of: Option<f64>,
    /// The answer's column that is the whole, when each line has its own: so much, of so much.
    #[serde(default)]
    pub whole: Option<String>,
    /// This part is a chain: the two columns a line goes from and to (the second may name several,
    /// separated by spaces or commas). What it means, not how it looks.
    #[serde(default)]
    pub chain: Option<Vec<String>>,
    /// Lay this part out once for each value of this column, in the order the values first appear -
    /// a red line, a driver, a star; its heading may name the value, as {column}.
    #[serde(default)]
    pub per: Option<String>,
}

/// Every lens, in the order of their files.
/// The lenses a place brings: the lens files kept beside its mark, in `lenses/`, read as the program
/// runs - each with its text, so that two bringings of one lens are known to be one.
pub fn brought_by(dir: &Path) -> Result<Vec<(String, Lens)>> {
    let Ok(entries) = std::fs::read_dir(dir.join("lenses")) else {
        return Ok(Vec::new());
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|file| {
            let text =
                std::fs::read_to_string(&file).with_context(|| format!("{}", file.display()))?;
            let lens: Lens = toml::from_str(&text)
                .with_context(|| format!("{} is not a lens", file.display()))?;
            Ok((text, lens))
        })
        .collect()
}

/// Every lens there is to look through from these places: those built into ocoeco, and those the places
/// bring. A brought lens is held to what the build holds the built-in ones to, said when it is looked
/// through: no id used twice, no table defined by two lenses. One lens brought by several places is one
/// lens. A lens a place keeps under the id of a built-in one is that built-in lens's own text: the one
/// built in is looked through (whether the two still agree is the check's to say - `differing`).
pub fn with(dirs: &[PathBuf]) -> Result<Vec<Lens>> {
    let mut all = lenses();
    let built_in = all.len();
    let mut brought: Vec<(String, String)> = Vec::new();
    for dir in dirs {
        for (text, lens) in brought_by(dir)? {
            let id = lens.lens.id.clone();
            if all[..built_in].iter().any(|l| l.lens.id == id)
                || brought.iter().any(|(i, t)| *i == id && *t == text)
            {
                continue;
            }
            if brought.iter().any(|(i, _)| *i == id) {
                bail!(
                    "two lenses are called {id}: a second, differing, is brought by {}",
                    dir.display()
                );
            }
            for table in lens.tables.keys() {
                if let Some(other) = all.iter().find(|l| l.tables.contains_key(table)) {
                    bail!("the table {table} is defined by two lenses, {} and {id} (brought by {}): one lens defines a piece of data, others read it", other.lens.id, dir.display());
                }
            }
            brought.push((id, text));
            all.push(lens);
        }
    }
    Ok(all)
}

/// The lenses a place keeps under the id of a built-in one whose text is no longer the built-in one's:
/// what is looked through is not what is kept there.
pub fn differing(dir: &Path) -> Vec<String> {
    brought_by(dir)
        .unwrap_or_default()
        .into_iter()
        .filter(|(text, lens)| {
            LENSES.iter().any(|(_, built)| {
                toml::from_str::<Lens>(built).is_ok_and(|b| b.lens.id == lens.lens.id)
                    && *built != text.as_str()
            })
        })
        .map(|(_, lens)| lens.lens.id)
        .collect()
}

pub fn lenses() -> Vec<Lens> {
    LENSES
        .iter()
        .map(|(name, text)| toml::from_str(text).unwrap_or_else(|e| panic!("{name}: {e}")))
        .collect()
}

/// The shared language: every lens's tables together.
pub fn tables() -> toml::Table {
    let mut all = toml::Table::new();
    for lens in lenses() {
        all.extend(lens.tables);
    }
    all
}

/// Every relation the lenses declare.
pub fn relations() -> Vec<Relation> {
    lenses()
        .into_iter()
        .flat_map(|lens| lens.relation)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lenses_together_are_the_language_as_it_stood() {
        // regrouped, not rewritten: the one file of 2026-11-12, table for table and relation for relation
        let was: toml::Table = include_str!("../tests/fixtures/language-as-it-stood.toml")
            .parse()
            .unwrap();
        // lenses are additive: every table as it stood is still there, unchanged; new lenses may add
        let now_tables = tables();
        for (name, table) in was["tables"].as_table().unwrap() {
            assert_eq!(
                now_tables.get(name),
                Some(table),
                "the table {name} changed"
            );
        }
        #[derive(serde::Deserialize)]
        struct Declared {
            relation: Vec<Relation>,
        }
        let key = |r: &Relation| (r.from.clone(), r.to.clone(), r.passes_over.clone());
        let mut stood: Vec<_> = toml::from_str::<Declared>(include_str!(
            "../tests/fixtures/relations-as-they-stood.toml"
        ))
        .unwrap()
        .relation
        .iter()
        .map(key)
        .collect();
        let now: Vec<_> = relations().iter().map(key).collect();
        stood.sort();
        for relation in &stood {
            assert!(now.contains(relation), "the relation {relation:?} is gone");
        }
    }

    #[test]
    fn every_lens_says_what_it_is_and_each_table_has_one_lens() {
        let all = lenses();
        assert_eq!(
            all.iter().map(|l| l.lens.id.as_str()).collect::<Vec<_>>(),
            [
                "insight",
                "knowledge",
                "looking-back",
                "problems",
                "purpose",
                "reasoning",
                "results",
                "work"
            ]
        );
        assert!(all.iter().all(|l| l.lens.says.len() > 20));
        let problems = all.iter().find(|l| l.lens.id == "problems").unwrap();
        assert_eq!(
            problems.lens.reads,
            ["backlog"],
            "the problems lens reads the work"
        );
        assert_eq!(
            all.iter().map(|l| l.tables.len()).sum::<usize>(),
            tables().len(),
            "no table is defined twice"
        );
    }
}

#[cfg(test)]
mod brought {
    use super::*;

    fn place(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ocoeco-brought-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("lenses")).unwrap();
        for (file, text) in files {
            std::fs::write(d.join("lenses").join(file), text).unwrap();
        }
        d
    }

    const NOW: &str = "[lens]\nid = \"now\"\nsays = \"What is happening.\"\n[tables.journal]\nkey = [\"place\"]\nfields = [{ name = \"place\", type = \"integer\", required = true }]\n";

    #[test]
    fn a_place_brings_its_lens_and_it_is_held_to_what_the_build_holds_lenses_to() {
        let built_in = lenses().len();
        let (lab, other) = (
            place("lab", &[("now.toml", NOW)]),
            place("other", &[("now.toml", NOW)]),
        );
        // brought, and one lens though two places bring it
        let all = with(&[lab.clone(), other]).unwrap();
        assert_eq!(
            (all.len(), all.last().unwrap().lens.id.as_str()),
            (built_in + 1, "now")
        );
        assert_eq!(
            with(&[]).unwrap().len(),
            built_in,
            "no place, no more lenses"
        );
        // a second lens of that name, differing: said
        let other_words = NOW.replace("What is happening.", "Something else.");
        let differs = place("differs", &[("now.toml", other_words.as_str())]);
        assert!(with(&[lab.clone(), differs])
            .unwrap_err()
            .to_string()
            .contains("two lenses are called now"));
        // a table a built-in lens defines: said, with both
        let twice = place("twice", &[("mine.toml", "[lens]\nid = \"mine\"\nsays = \"Mine.\"\n[tables.backlog]\nkey = [\"id\"]\nfields = []\n")]);
        let said = with(&[twice]).unwrap_err().to_string();
        assert!(
            said.contains("the table backlog is defined by two lenses, work and mine"),
            "{said}"
        );
        // a place keeping a built-in lens's own text: the built-in one is looked through; a change to it is told
        let (name, text) = LENSES
            .iter()
            .find(|(_, t)| t.contains("id = \"work\""))
            .unwrap();
        let own = place("own", &[(*name, *text)]);
        assert_eq!(
            (
                with(std::slice::from_ref(&own)).unwrap().len(),
                differing(&own)
            ),
            (built_in, vec![])
        );
        let changed_text = format!("{text}\n# changed\n");
        let changed = place("changed", &[(*name, changed_text.as_str())]);
        assert_eq!(differing(&changed), ["work"]);
    }
}
