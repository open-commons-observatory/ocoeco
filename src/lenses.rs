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
    #[serde(default)]
    pub sql: String,
    /// One answer, laid out: each {name} is that answer's value.
    #[serde(default)]
    pub line: String,
    /// This part is what another lens brings into focus, placed here: that lens's id. It asks nothing
    /// itself. A lens made of such parts is a lens of lenses: it says only what to place together, and
    /// knows nothing of where the data comes from or how it is shaped.
    #[serde(default)]
    pub through: Option<String>,
    /// Of what that lens brings into focus, only the part under this heading.
    #[serde(default)]
    pub only: Option<String>,
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
/// The lenses a place brings: the lens files kept beside its mark, in `lenses/`, and the lenses it keeps
/// as rows (its tables lenses and lens_parts) - read as the program
/// runs - each with its text, so that two bringings of one lens are known to be one.
pub fn brought_by(dir: &Path) -> Result<Vec<(String, Lens)>> {
    // a place may keep lens files, rows, both or neither: no folder of files is not the end of the looking
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir.join("lenses"))
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    files.sort();
    let mut found: Vec<(String, Lens)> = files
        .into_iter()
        .map(|file| {
            let text =
                std::fs::read_to_string(&file).with_context(|| format!("{}", file.display()))?;
            let lens: Lens = toml::from_str(&text)
                .with_context(|| format!("{} is not a lens", file.display()))?;
            Ok((text, lens))
        })
        .collect::<Result<_>>()?;
    // and the lenses the place keeps as rows: entered like anything else, looked through like any lens
    if let Some(mark) = std::fs::read_to_string(dir.join("oco.toml"))
        .ok()
        .and_then(|text| crate::mark::Mark::parse(&text).ok())
    {
        let (kept, parts) = (
            crate::join::rows(dir, &mark, "lenses"),
            crate::join::rows(dir, &mark, "lens_parts"),
        );
        found.extend(
            from_rows(&kept, &parts)
                .into_iter()
                .map(|lens| (format!("{lens:?}"), lens)),
        );
    }
    Ok(found)
}

/// Every lens there is to look through from these places, each with where it is kept: those built into
/// ocoeco, and those the places bring. A brought lens is held to what the build holds the built-in ones
/// to, said when it is looked through: no id used twice, no table defined by two lenses. One lens brought
/// by several places is one lens. A lens a place keeps under the id of a built-in one is that built-in
/// lens's own text: the one built in is looked through (whether the two still agree is the check's to
/// say - `differing`).
pub fn in_view(dirs: &[PathBuf]) -> Result<Vec<(String, Lens)>> {
    let mut all: Vec<(String, Lens)> = lenses()
        .into_iter()
        .map(|lens| ("ocoeco".to_string(), lens))
        .collect();
    let built_in = all.len();
    let mut brought: Vec<(String, String)> = Vec::new();
    for dir in dirs {
        let kept_by = std::fs::read_to_string(dir.join("oco.toml"))
            .ok()
            .and_then(|text| crate::mark::Mark::parse(&text).ok())
            .map(|mark| {
                mark.project
                    .id
                    .rsplit('/')
                    .next()
                    .unwrap_or_default()
                    .to_string()
            })
            .unwrap_or_else(|| {
                dir.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            });
        for (text, lens) in brought_by(dir)? {
            let id = lens.lens.id.clone();
            if all[..built_in].iter().any(|(_, l)| l.lens.id == id)
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
                if let Some((_, other)) = all.iter().find(|(_, l)| l.tables.contains_key(table)) {
                    bail!("the table {table} is defined by two lenses, {} and {id} (brought by {}): one lens defines a piece of data, others read it", other.lens.id, dir.display());
                }
            }
            brought.push((id, text));
            all.push((kept_by.clone(), lens));
        }
    }
    Ok(all)
}

/// Every lens there is to look through from these places.
pub fn with(dirs: &[PathBuf]) -> Result<Vec<Lens>> {
    Ok(in_view(dirs)?.into_iter().map(|(_, lens)| lens).collect())
}

/// The lenses that rows describe: each lens row, with its parts in their order. Rows carry what a lens
/// says of itself, what it reads, and each part whole - so a lens of lenses, or one that asks of tables
/// other lenses define, can be kept as rows and is then the lens it describes. A lens's own tables and
/// entries are not yet carried by rows.
pub fn from_rows(lenses: &[serde_json::Value], parts: &[serde_json::Value]) -> Vec<Lens> {
    let raw = |row: &serde_json::Value, key: &str| {
        row.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    let some = |text: String| Some(text).filter(|t| !t.trim().is_empty());
    let listed = |text: String| -> Vec<String> {
        text.split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect()
    };
    lenses
        .iter()
        .filter(|row| !raw(row, "id").trim().is_empty() && !raw(row, "says").trim().is_empty())
        .map(|row| {
            let id = raw(row, "id").trim().to_string();
            let mut theirs: Vec<&serde_json::Value> = parts
                .iter()
                .filter(|p| raw(p, "lens").trim() == id)
                .collect();
            theirs.sort_by_key(|p| {
                p.get("part")
                    .and_then(|n| {
                        n.as_i64()
                            .or_else(|| n.as_str().and_then(|s| s.trim().parse().ok()))
                    })
                    .unwrap_or(0)
            });
            Lens {
                lens: About {
                    id,
                    says: raw(row, "says"),
                    grew_from: some(raw(row, "grew_from")),
                    reads: listed(raw(row, "reads")),
                },
                tables: toml::Table::new(),
                relation: Vec::new(),
                glimpse: Vec::new(),
                focus: theirs
                    .into_iter()
                    .map(|p| Part {
                        title: some(raw(p, "title")),
                        always: p
                            .get("always")
                            .and_then(|a| {
                                a.as_bool()
                                    .or_else(|| a.as_str().map(|s| s.trim() == "true"))
                            })
                            .unwrap_or(false),
                        sql: raw(p, "query"),
                        line: raw(p, "lays_out"),
                        through: some(raw(p, "through")),
                        only: some(raw(p, "only")),
                        measure: some(raw(p, "measure")),
                        of: raw(p, "of").trim().parse().ok(),
                        whole: some(raw(p, "whole")),
                        chain: Some(listed(raw(p, "chain"))).filter(|c| !c.is_empty()),
                        per: some(raw(p, "per")),
                    })
                    .collect(),
            }
        })
        .collect()
}

/// The tables that hold the lenses themselves as rows. No place keeps them: they are the language, read
/// as data - so that it can be looked at through a lens like everything it describes.
pub const OF_THE_LANGUAGE: [&str; 5] = [
    "lenses",
    "lens_tables",
    "lens_fields",
    "lens_entries",
    "lens_parts",
];

/// The lenses in view, as rows: each lens, each table it defines, each field of each table, each entry it
/// takes, each part it brings into focus - every row saying where its lens is kept.
pub fn as_rows(
    dirs: &[PathBuf],
) -> Result<std::collections::BTreeMap<&'static str, Vec<serde_json::Map<String, serde_json::Value>>>>
{
    use serde_json::json;
    let mut rows: std::collections::BTreeMap<
        &'static str,
        Vec<serde_json::Map<String, serde_json::Value>>,
    > = OF_THE_LANGUAGE
        .iter()
        .map(|table| (*table, Vec::new()))
        .collect();
    let mut add = |table: &'static str, row: serde_json::Value| {
        if let (Some(into), serde_json::Value::Object(row)) = (rows.get_mut(table), row) {
            into.push(row);
        }
    };
    for (star, lens) in in_view(dirs)? {
        let id = &lens.lens.id;
        add(
            "lenses",
            json!({"id": id, "says": lens.lens.says, "grew_from": lens.lens.grew_from.clone().unwrap_or_default(), "reads": lens.lens.reads.join(", "), "star": star}),
        );
        for (table, declared) in &lens.tables {
            add(
                "lens_tables",
                json!({"id": format!("{id}/{table}"), "lens": id, "name": table, "star": star}),
            );
            for field in declared
                .get("fields")
                .and_then(|f| f.as_array())
                .into_iter()
                .flatten()
            {
                let said = |key: &str| {
                    field
                        .get(key)
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string()
                };
                let allowed: Vec<&str> = field
                    .get("enum")
                    .and_then(|e| e.as_array())
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v.as_str())
                    .collect();
                add(
                    "lens_fields",
                    json!({"id": format!("{id}/{table}/{}", said("name")), "lens": id, "of_table": table, "name": said("name"), "kind": said("type"), "needed": field.get("required").and_then(|r| r.as_bool()).unwrap_or(false), "allowed": allowed.join(", "), "star": star}),
                );
            }
        }
        for entry in &lens.glimpse {
            add(
                "lens_entries",
                json!({"id": format!("{id}/{}", entry.entry), "lens": id, "entry": entry.entry, "fills": entry.table, "asks": entry.asks.join(", "), "star": star}),
            );
        }
        for (at, part) in lens.focus.iter().enumerate() {
            add(
                "lens_parts",
                json!({"id": format!("{id}/{}", at + 1), "lens": id, "part": at + 1, "title": part.title.clone().unwrap_or_default(), "always": part.always, "query": part.sql, "lays_out": part.line, "through": part.through.clone().unwrap_or_default(), "only": part.only.clone().unwrap_or_default(), "measure": part.measure.clone().unwrap_or_default(), "of": part.of.map(|n| n.to_string()).unwrap_or_default(), "whole": part.whole.clone().unwrap_or_default(), "chain": part.chain.as_ref().map(|c| c.join(", ")).unwrap_or_default(), "per": part.per.clone().unwrap_or_default(), "star": star}),
            );
        }
    }
    Ok(rows)
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
                "graph",
                "insight",
                "knowledge",
                "language",
                "looking-back",
                "purpose",
                "reasoning",
                "results",
                "signs",
                "work"
            ]
        );
        assert!(all.iter().all(|l| l.lens.says.len() > 20));
        let signs = all.iter().find(|l| l.lens.id == "signs").unwrap();
        assert_eq!(
            signs.lens.reads,
            ["backlog", "connections", "edges"],
            "the lens of signs reads the work, and what came of a sign"
        );
        assert_eq!(
            signs.lens.grew_from.as_deref(),
            Some("problems"),
            "it grew from the lens of problems, whose table it keeps"
        );
        assert!(
            !signs
                .glimpse
                .iter()
                .any(|e| e.asks.iter().any(|a| a == "countermeasure")),
            "nothing is asked about what was done"
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

#[cfg(test)]
mod of_the_language {
    use super::*;

    #[test]
    fn the_lenses_are_rows_and_the_lens_of_language_declares_exactly_what_they_carry() {
        let rows = as_rows(&[]).unwrap();
        let language = lenses()
            .into_iter()
            .find(|l| l.lens.id == "language")
            .expect("the lens of language");
        assert_eq!(rows.keys().copied().collect::<Vec<_>>(), {
            let mut t = OF_THE_LANGUAGE.to_vec();
            t.sort();
            t
        });
        for (table, found) in &rows {
            assert!(
                !found.is_empty(),
                "{table} has rows: the language says something of itself"
            );
            let declared: Vec<String> = language.tables[*table]["fields"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| f["name"].as_str().unwrap().to_string())
                .collect();
            for row in found {
                let mut carried: Vec<&str> = row
                    .keys()
                    .map(String::as_str)
                    .filter(|k| *k != "star")
                    .collect();
                carried.sort();
                let mut said: Vec<&str> = declared.iter().map(String::as_str).collect();
                said.sort();
                assert_eq!(carried, said, "{table}: what a row carries is what the lens declares - and where its lens is kept");
            }
        }
        // it counts itself: every lens there is, the lens of language among them
        assert_eq!(rows["lenses"].len(), lenses().len());
        assert!(rows["lens_tables"]
            .iter()
            .any(|r| r["lens"] == "language" && r["name"] == "lens_fields"));
        assert!(
            rows["lens_fields"]
                .iter()
                .filter(|r| r["name"] == "id")
                .count()
                > 5,
            "id is declared in many tables"
        );
        assert!(
            rows["lens_fields"]
                .iter()
                .any(|r| r["id"] == "work/backlog/status"),
            "each thing of the language has an id it can be pointed at by"
        );
    }
}

#[cfg(test)]
mod kept_as_rows {
    use super::*;

    #[test]
    fn a_lens_turned_into_rows_and_back_is_the_lens_it_was() {
        let rows = as_rows(&[]).unwrap();
        let of = |table: &str| -> Vec<serde_json::Value> {
            rows[table]
                .iter()
                .cloned()
                .map(serde_json::Value::Object)
                .collect()
        };
        let (back, was) = (from_rows(&of("lenses"), &of("lens_parts")), lenses());
        assert_eq!(back.len(), was.len(), "every lens is there");
        for (b, w) in back.iter().zip(&was) {
            assert_eq!(
                format!("{:?}", b.lens),
                format!("{:?}", w.lens),
                "what a lens says of itself, grew from and reads"
            );
            assert_eq!(b.focus.len(), w.focus.len(), "{}: every part", w.lens.id);
            for (at, (bp, wp)) in b.focus.iter().zip(&w.focus).enumerate() {
                assert_eq!(
                    format!("{bp:?}"),
                    format!("{wp:?}"),
                    "part {} of the lens {} is carried whole by its row",
                    at + 1,
                    w.lens.id
                );
            }
        }
    }

    #[test]
    fn a_place_brings_the_lenses_it_keeps_as_rows() {
        let d = std::env::temp_dir().join(format!("ocoeco-kept-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("oco")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nlenses = \"oco/lenses.json\"\nlens_parts = \"oco/lens_parts.json\"\n").unwrap();
        std::fs::write(
            d.join("oco/lenses.json"),
            "{\"id\":\"sheet\",\"says\":\"What goes together, and nothing else.\"}\n",
        )
        .unwrap();
        std::fs::write(d.join("oco/lens_parts.json"), "{\"id\":\"sheet/2\",\"lens\":\"sheet\",\"part\":2,\"through\":\"signs\",\"only\":\"showing\"}\n{\"id\":\"sheet/1\",\"lens\":\"sheet\",\"part\":1,\"through\":\"work\"}\n").unwrap();
        let all = with(std::slice::from_ref(&d)).unwrap();
        let sheet = all
            .iter()
            .find(|l| l.lens.id == "sheet")
            .expect("the lens kept as rows is in view");
        let placed: Vec<(Option<&str>, Option<&str>)> = sheet
            .focus
            .iter()
            .map(|p| (p.through.as_deref(), p.only.as_deref()))
            .collect();
        assert_eq!(
            placed,
            [(Some("work"), None), (Some("signs"), Some("showing"))],
            "its parts in their order, as the rows say"
        );
        assert!(
            as_rows(std::slice::from_ref(&d)).unwrap()["lenses"]
                .iter()
                .any(|r| r["id"] == "sheet" && r["star"] == "r"),
            "and the lens of language sees it, kept by that place"
        );
    }
}
