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
        assert_eq!(&tables(), was["tables"].as_table().unwrap());
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
        let mut now: Vec<_> = relations().iter().map(key).collect();
        stood.sort();
        now.sort();
        assert_eq!(now, stood);
    }

    #[test]
    fn every_lens_says_what_it_is_and_each_table_has_one_lens() {
        let all = lenses();
        assert_eq!(
            all.iter().map(|l| l.lens.id.as_str()).collect::<Vec<_>>(),
            [
                "knowledge",
                "looking-back",
                "purpose",
                "reasoning",
                "results",
                "work"
            ]
        );
        assert!(all.iter().all(|l| l.lens.says.len() > 20));
        assert_eq!(
            all.iter().map(|l| l.tables.len()).sum::<usize>(),
            tables().len(),
            "no table is defined twice"
        );
    }
}
