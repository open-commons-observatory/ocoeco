//! The lenses built into this ocoeco (lenses/*.toml). A lens has two faces: its glimpse catches data -
//! the tables it defines and how they relate - and its focus brings it into view. Until 2026-11-12 the
//! tables sat together in one file, written from outside; now each belongs to the lens that needs it.
//! Regrouped, not rewritten: a test holds them to the language as it stood.

use crate::relations::Relation;

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
    /// The whole the measure is of.
    #[serde(default)]
    pub of: Option<f64>,
}

/// Every lens, in the order of their files.
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
