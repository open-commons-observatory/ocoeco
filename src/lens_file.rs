//! A lens a place keeps as a file is changed in that file: an entry writes where the row is kept.
//!
//! The rows of a lens - what it says of itself, its parts - are read from its file wherever the language is
//! read. This is the same way, the other way round: a row of `lenses` or of `lens_parts` given for a lens
//! kept as a file is written into that file, in place - what is written around it, and every comment, as
//! it was. A lens file names no part: a part is known by where it stands among the parts.

use crate::note::Change;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use toml_edit::{value, Array, DocumentMut, Item, Table};

/// The folders a place keeps lens files in: the language, where it is the language's home - and its own.
const FOLDERS: [&str; 2] = ["language", "lenses"];

/// What a lens kept as a file says of itself, as a row of `lenses` names it.
const OF_A_LENS: &str = "says, reads, looks_from, grew_from";

/// What a part says, in the order a lens file is read in: a part added says its own in this order.
const OF_A_PART: [&str; 15] = [
    "title", "named", "always", "urgent", "stops", "places", "only", "through", "per", "measure",
    "of", "whole", "chain", "query", "lays_out",
];

/// The lens files of the place at `dir`: each with the id of the lens it keeps.
fn files(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut found = Vec::new();
    for folder in FOLDERS {
        let Ok(entries) = std::fs::read_dir(dir.join(folder)) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|x| x == "toml"))
            .collect();
        paths.sort();
        for path in paths {
            let id = std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| text.parse::<DocumentMut>().ok())
                .and_then(|doc| {
                    doc.get("lens")
                        .and_then(|lens| lens.get("id"))
                        .and_then(|id| id.as_str())
                        .map(str::to_string)
                });
            if let Some(id) = id {
                found.push((id, path));
            }
        }
    }
    found
}

/// Words as a lens file writes them: on one line - or, where they are more than one line, as the lines
/// they are.
fn words(said: &str) -> Item {
    if said.contains('\n') {
        let lines = said
            .trim_end_matches('\n')
            .replace('\\', "\\\\")
            .replace("\"\"\"", "\"\"\\\"");
        let written = format!("it = \"\"\"\n{lines}\n\"\"\"\n");
        if let Some(item) = written
            .parse::<DocumentMut>()
            .ok()
            .and_then(|doc| doc.get("it").cloned())
        {
            return item;
        }
    }
    value(said)
}

/// Sets words under a key - or takes the key away, where no words are given.
fn set_words(table: &mut Table, key: &str, said: &str) {
    if said.trim().is_empty() {
        table.remove(key);
    } else {
        table[key] = words(said);
    }
}

/// Sets what is listed under a key, each on its own - or takes the key away, where nothing is.
fn set_listed(table: &mut Table, key: &str, said: &str) {
    let listed: Vec<&str> = said
        .split(',')
        .map(str::trim)
        .filter(|one| !one.is_empty())
        .collect();
    if listed.is_empty() {
        table.remove(key);
    } else {
        table[key] = value(Array::from_iter(listed));
    }
}

/// Sets whether something is so - written only where it is.
fn set_whether(table: &mut Table, key: &str, said: &str) -> Result<()> {
    match said.trim() {
        "true" | "yes" => table[key] = value(true),
        "false" | "no" | "" => {
            table.remove(key);
        }
        other => bail!("{key} must be true or false, not '{other}'"),
    }
    Ok(())
}

/// What a lens says of itself, set in its file. Returns what was set.
fn of_the_lens(doc: &mut DocumentMut, changes: &[Change]) -> Result<Vec<String>> {
    let lens = doc["lens"]
        .or_insert(toml_edit::table())
        .as_table_mut()
        .context("what a lens says of itself is not written under [lens]")?;
    let mut said = Vec::new();
    for change in changes {
        match change.field.as_str() {
            "says" | "grew_from" => set_words(lens, &change.field, &change.value),
            "reads" | "looks_from" => set_listed(lens, &change.field, &change.value),
            other => {
                bail!("{other} is not something a lens kept as a file says of itself: {OF_A_LENS}")
            }
        }
        said.push(change.field.clone());
    }
    Ok(said)
}

/// A part of a lens, set in its file: changed where it stands, or added after the last. Returns whether it
/// was added, where it stands, and what was set.
fn of_a_part(
    doc: &mut DocumentMut,
    lens: &str,
    id: &str,
    changes: &[Change],
) -> Result<(bool, usize, Vec<String>)> {
    let at: usize = id
        .strip_prefix(lens)
        .and_then(|rest| rest.strip_prefix('/'))
        .and_then(|n| n.parse().ok())
        .filter(|n| *n >= 1)
        .with_context(|| {
            format!("a part of a lens kept as a file is known by where it stands - {lens}/1, {lens}/2 and so on: {id} is not so named")
        })?;
    let parts = doc["focus"]
        .or_insert(Item::ArrayOfTables(Default::default()))
        .as_array_of_tables_mut()
        .context("the parts of the lens are not written as [[focus]]")?;
    let added = at == parts.len() + 1;
    if added {
        parts.push(Table::new());
    }
    let count = parts.len();
    let Some(part) = parts.get_mut(at - 1) else {
        bail!(
            "the lens {lens} has {count} part(s): a part is changed where it stands, or added after the last - as {lens}/{}",
            count + 1
        );
    };
    let mut given: Vec<&Change> = changes.iter().collect();
    given.sort_by_key(|change| {
        OF_A_PART
            .iter()
            .position(|field| *field == change.field)
            .unwrap_or(OF_A_PART.len())
    });
    let mut said = Vec::new();
    for change in given {
        match change.field.as_str() {
            "lens" if change.value.trim() == lens => continue,
            "lens" => bail!("the part {id} is a part of the lens {lens}"),
            "part" if change.value.trim() == at.to_string() => continue,
            "part" => bail!("a part stands where its name says: {id} is part {at}"),
            "title" | "named" | "places" | "only" | "through" | "per" | "measure" | "whole" => {
                set_words(part, &change.field, &change.value)
            }
            "query" => set_words(part, "sql", &change.value),
            "lays_out" => set_words(part, "line", &change.value),
            "always" | "urgent" | "stops" => set_whether(part, &change.field, &change.value)?,
            "chain" => set_listed(part, "chain", &change.value),
            "of" => match change.value.trim() {
                "" => {
                    part.remove("of");
                }
                n => {
                    part["of"] = value(
                        n.parse::<f64>()
                            .with_context(|| format!("of must be a number, not '{n}'"))?,
                    )
                }
            },
            other => bail!("{other} is not something a part of a lens says"),
        }
        said.push(change.field.clone());
    }
    Ok((added, at, said))
}

/// Writes the row `id` of `table` as its changes stand, where it is the row of a lens the place at `dir`
/// keeps as a file - or of a new lens, where the place keeps its lenses as files and keeps no rows of them
/// (`exported` says whether it does). Says what it did; or nothing, where the row is not kept so: it is then
/// written as any row is. What results must still be a lens - else nothing is written.
pub fn written(
    dir: &Path,
    exported: bool,
    table: &str,
    id: &str,
    changes: &[Change],
) -> Result<Option<String>> {
    let lens = match table {
        "lenses" => id.to_string(),
        "lens_parts" => changes
            .iter()
            .find(|change| change.field == "lens")
            .map(|change| change.value.trim().to_string())
            .filter(|lens| !lens.is_empty())
            .or_else(|| id.rsplit_once('/').map(|(lens, _)| lens.to_string()))
            .unwrap_or_default(),
        _ => return Ok(None),
    };
    let path = match files(dir).into_iter().find(|(kept, _)| *kept == lens) {
        Some((_, path)) => path,
        // a new lens is a new file, where this place keeps its lenses as files and keeps no rows of them
        None if table == "lenses" && !exported => {
            match FOLDERS
                .iter()
                .map(|folder| dir.join(folder))
                .find(|folder| folder.is_dir())
            {
                Some(folder) => folder.join(format!("{id}.toml")),
                None => return Ok(None),
            }
        }
        None => return Ok(None),
    };
    let was = std::fs::read_to_string(&path).unwrap_or_default();
    let read = if was.is_empty() {
        format!("[lens]\nid = \"{id}\"\n")
    } else {
        was.clone()
    };
    let mut doc: DocumentMut = read
        .parse()
        .with_context(|| format!("{} is not written as a lens", path.display()))?;
    let said = match table {
        "lenses" => format!(
            "the lens {lens}: {}",
            of_the_lens(&mut doc, changes)?.join(", ")
        ),
        _ => {
            let (added, at, said) = of_a_part(&mut doc, &lens, id, changes)?;
            format!(
                "{} part {at} of the lens {lens}: {}",
                if added { "added" } else { "changed" },
                said.join(", ")
            )
        }
    };
    let now = doc.to_string();
    toml::from_str::<crate::lenses::Lens>(&now)
        .with_context(|| format!("what was given does not make a lens of {}", path.display()))?;
    let shown = path
        .strip_prefix(dir)
        .unwrap_or(&path)
        .display()
        .to_string();
    if now == was {
        return Ok(Some(format!("{said} - as it was, in {shown}")));
    }
    std::fs::write(&path, now)?;
    Ok(Some(format!("{said} - where it is kept, {shown}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(field: &str, value: &str) -> Change {
        Change {
            field: field.to_string(),
            value: value.to_string(),
            append: false,
        }
    }

    #[test]
    fn a_row_of_a_lens_kept_as_a_file_is_written_into_the_file_in_place() {
        let d = std::env::temp_dir().join(format!("ocoeco-lens-file-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("lenses")).unwrap();
        let file = d.join("lenses/notes.toml");
        std::fs::write(
            &file,
            "# The notes: written above the lens.\n[lens]\nid = \"notes\"\nsays = \"The notes, as they are.\"\n\n[tables.notes]\nkey = [\"id\"]\nfields = [{ name = \"id\", type = \"string\", required = true }]\n\n# The first part: every note.\n[[focus]]\ntitle = \"notes ({count})\"\nsql = \"\"\"\nSELECT id FROM notes\nORDER BY id\n\"\"\"\nline = \"{id}\"\n\n# The second part: how many.\n[[focus]]\ntitle = \"in all\"\nsql = \"SELECT count(*) AS n FROM notes\"\nline = \"{n}\"\n",
        )
        .unwrap();
        // a part changed where it stands: what it asks, as the lines it is - and all around it as it was
        let said = written(
            &d,
            false,
            "lens_parts",
            "notes/2",
            &[change(
                "query",
                "SELECT count(*) AS n\nFROM notes\nWHERE id <> ''",
            )],
        )
        .unwrap()
        .unwrap();
        assert!(
            said.starts_with("changed part 2 of the lens notes: query"),
            "{said}"
        );
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(
            text.contains("# The notes: written above the lens.")
                && text.contains("# The first part: every note.")
                && text.contains("# The second part: how many.")
                && text.contains("SELECT id FROM notes\nORDER BY id\n"),
            "what is written around it is as it was: {text}"
        );
        assert!(
            text.contains("sql = \"\"\"\nSELECT count(*) AS n\nFROM notes\nWHERE id <> ''\n\"\"\""),
            "what a part asks is kept as the lines it is: {text}"
        );
        let lens: crate::lenses::Lens = toml::from_str(&text).unwrap();
        assert_eq!(
            lens.focus[1].sql.trim(),
            "SELECT count(*) AS n\nFROM notes\nWHERE id <> ''"
        );
        // a part added after the last; and none added past that
        let said = written(
            &d,
            false,
            "lens_parts",
            "notes/3",
            &[
                change("lays_out", "{id}"),
                change("query", "SELECT id FROM notes LIMIT 1"),
                change("title", "the first"),
                change("stops", "true"),
            ],
        )
        .unwrap()
        .unwrap();
        assert!(said.starts_with("added part 3 of the lens notes"), "{said}");
        let lens: crate::lenses::Lens =
            toml::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(lens.focus.len(), 3);
        assert!(lens.focus[2].stops && lens.focus[2].title.as_deref() == Some("the first"));
        assert!(written(&d, false, "lens_parts", "notes/5", &[change("title", "x")]).is_err());
        // what the lens says of itself; and what does not make a lens is not written
        written(
            &d,
            false,
            "lenses",
            "notes",
            &[
                change("says", "The notes, said anew."),
                change("reads", "edges, things"),
            ],
        )
        .unwrap()
        .unwrap();
        let lens: crate::lenses::Lens =
            toml::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(lens.lens.says, "The notes, said anew.");
        assert_eq!(lens.lens.reads, ["edges", "things"]);
        let before = std::fs::read_to_string(&file).unwrap();
        assert!(written(&d, false, "lenses", "notes", &[change("says", "")]).is_err());
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            before,
            "nothing is written"
        );
        // a new lens is a new file, where the place keeps no rows of lenses - and is left to the rows where
        // it does; and a row of another kind is not kept so at all
        written(
            &d,
            false,
            "lenses",
            "more",
            &[change("says", "More, in a file of its own.")],
        )
        .unwrap()
        .unwrap();
        assert!(d.join("lenses/more.toml").is_file());
        assert!(written(&d, true, "lenses", "other", &[change("says", "x")])
            .unwrap()
            .is_none());
        assert!(
            written(&d, false, "decisions", "x", &[change("decision", "x")])
                .unwrap()
                .is_none()
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
