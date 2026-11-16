//! A lens a place keeps as a file is changed in that file: an entry writes where the row is kept.
//!
//! The rows of a lens - what it says of itself, its parts, the tables it defines and their fields, its
//! entries, its links, its ways in - are read from its file wherever the language is read. This is the same
//! way, the other way round: such a row given for a lens kept as a file is written into that file, in
//! place - what is written around it, and every comment, as it was. A lens file names no part and no link:
//! each is known by where it stands among its kind; a table, a field, an entry and a way in by their names.

use crate::note::Change;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use toml_edit::{value, Array, ArrayOfTables, DocumentMut, InlineTable, Item, Table, Value};

/// The folders a place keeps lens files in: the language, where it is the language's home - and its own.
const FOLDERS: [&str; 2] = ["language", "lenses"];

/// The tables whose rows a lens file keeps.
const KEPT_IN_A_LENS: [&str; 7] = [
    "lenses",
    "lens_parts",
    "lens_tables",
    "lens_fields",
    "lens_entries",
    "lens_relations",
    "lens_routes",
];

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

/// What is listed, each on its own.
fn listed(said: &str) -> Vec<&str> {
    said.split(',')
        .map(str::trim)
        .filter(|one| !one.is_empty())
        .collect()
}

/// Sets words under a key - or takes the key away, where no words are given.
fn set_words(table: &mut Table, key: &str, said: &str) {
    if said.trim().is_empty() {
        table.remove(key);
    } else {
        table[key] = words(said);
    }
}

/// Sets what is listed under a key - or takes the key away, where nothing is.
fn set_listed(table: &mut Table, key: &str, said: &str) {
    let listed = listed(said);
    if listed.is_empty() {
        table.remove(key);
    } else {
        table[key] = value(Array::from_iter(listed));
    }
}

/// Whether something is said to be so.
fn whether(key: &str, said: &str) -> Result<bool> {
    match said.trim() {
        "true" | "yes" => Ok(true),
        "false" | "no" | "" => Ok(false),
        other => bail!("{key} must be true or false, not '{other}'"),
    }
}

/// Sets whether something is so - written only where it is.
fn set_whether(table: &mut Table, key: &str, said: &str) -> Result<()> {
    if whether(key, said)? {
        table[key] = value(true);
    } else {
        table.remove(key);
    }
    Ok(())
}

/// The name a thing of a lens goes by, after the lens: lens/NAME.
fn after<'a>(lens: &str, id: &'a str) -> Option<&'a str> {
    id.strip_prefix(lens)
        .and_then(|rest| rest.strip_prefix('/'))
        .filter(|rest| !rest.is_empty())
}

/// Where a thing known by its place among its kind stands: lens/1, lens/2 and so on.
fn standing(lens: &str, id: &str, what: &str) -> Result<usize> {
    after(lens, id)
        .and_then(|n| n.parse().ok())
        .filter(|n| *n >= 1)
        .with_context(|| {
            format!("{what} of a lens kept as a file is known by where it stands - {lens}/1, {lens}/2 and so on: {id} is not so named")
        })
}

/// One of a kind a lens file writes as tables one after another - its parts, its links: the one standing at
/// `at`, or a new one where `at` is the place after the last. Returns it, and whether it was added.
fn standing_at<'a>(
    doc: &'a mut DocumentMut,
    kind: &str,
    lens: &str,
    at: usize,
) -> Result<(&'a mut Table, bool)> {
    let all = doc[kind]
        .or_insert(Item::ArrayOfTables(ArrayOfTables::new()))
        .as_array_of_tables_mut()
        .with_context(|| format!("what the lens keeps as [[{kind}]] is not written so"))?;
    let added = at == all.len() + 1;
    if added {
        all.push(Table::new());
    }
    let count = all.len();
    match all.get_mut(at - 1) {
        Some(one) => Ok((one, added)),
        None => bail!(
            "the lens {lens} has {count} of these: one is changed where it stands, or added after the last - as {lens}/{}",
            count + 1
        ),
    }
}

/// One of a kind a lens file writes as tables one after another and knows by a name of its own - an entry
/// by what it is called, a way in by its table: the one so named under `key`, or a new one. Returns it, and
/// whether it was added.
fn named_so<'a>(
    doc: &'a mut DocumentMut,
    kind: &str,
    key: &str,
    name: &str,
) -> Result<(&'a mut Table, bool)> {
    let all = doc[kind]
        .or_insert(Item::ArrayOfTables(ArrayOfTables::new()))
        .as_array_of_tables_mut()
        .with_context(|| format!("what the lens keeps as [[{kind}]] is not written so"))?;
    let at = all
        .iter()
        .position(|one| one.get(key).and_then(|named| named.as_str()) == Some(name));
    if at.is_none() {
        let mut one = Table::new();
        one[key] = value(name);
        all.push(one);
    }
    let count = all.len();
    let one = all
        .get_mut(at.unwrap_or(count - 1))
        .context("what was just added is not there")?;
    Ok((one, at.is_none()))
}

/// Holds a row to what it is known by: what names it is not changed by a field.
fn is_still(field: &str, said: &str, is: &str, id: &str) -> Result<()> {
    if said.trim() == is {
        Ok(())
    } else {
        bail!("{id} is known by its {field}, {is}: what a thing is known by is not changed by a field")
    }
}

/// What a lens says of itself, set in its file.
fn of_the_lens(doc: &mut DocumentMut, lens: &str, changes: &[Change]) -> Result<String> {
    let about = doc["lens"]
        .or_insert(toml_edit::table())
        .as_table_mut()
        .context("what a lens says of itself is not written under [lens]")?;
    let mut said = Vec::new();
    for change in changes {
        match change.field.as_str() {
            "says" | "grew_from" => set_words(about, &change.field, &change.value),
            "reads" | "looks_from" => set_listed(about, &change.field, &change.value),
            other => bail!(
                "{other} is not something a lens says of itself: says, reads, looks_from, grew_from"
            ),
        }
        said.push(change.field.clone());
    }
    Ok(format!("the lens {lens}: {}", said.join(", ")))
}

/// A part of a lens, set in its file: changed where it stands, or added after the last.
fn of_a_part(doc: &mut DocumentMut, lens: &str, id: &str, changes: &[Change]) -> Result<String> {
    let at = standing(lens, id, "a part")?;
    let (part, added) = standing_at(doc, "focus", lens, at)?;
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
            "lens" => {
                is_still("lens", &change.value, lens, id)?;
                continue;
            }
            "part" => {
                is_still("place among the parts", &change.value, &at.to_string(), id)?;
                continue;
            }
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
    Ok(format!(
        "{} part {at} of the lens {lens}: {}",
        if added { "added" } else { "changed" },
        said.join(", ")
    ))
}

/// A table a lens defines, set in its file: [tables.NAME], known by its name. A new one is keyed by its id,
/// which is its first field.
fn of_a_table(doc: &mut DocumentMut, lens: &str, id: &str, changes: &[Change]) -> Result<String> {
    let name = after(lens, id)
        .filter(|name| !name.contains('/'))
        .with_context(|| {
            format!("a table of a lens is known by the lens and its name - {lens}/NAME: {id} is not so named")
        })?;
    if !doc.contains_key("tables") {
        let mut tables = Table::new();
        tables.set_implicit(true);
        doc["tables"] = Item::Table(tables);
    }
    let tables = doc["tables"]
        .as_table_mut()
        .context("the tables of the lens are not written under [tables]")?;
    let added = !tables.contains_key(name);
    if added {
        let made: DocumentMut = format!(
            "[tables.{name}]\nkey = [\"id\"]\nfields = [\n    {{ name = \"id\", type = \"string\", required = true }},\n]\n"
        )
        .parse()
        .with_context(|| format!("{name} is no name a table can go by"))?;
        let mut table = made
            .get("tables")
            .and_then(|tables| tables.get(name))
            .and_then(|table| table.as_table())
            .cloned()
            .context("a table could not be written")?;
        table.set_position(usize::MAX / 2);
        tables.insert(name, Item::Table(table));
    }
    let table = tables[name]
        .as_table_mut()
        .with_context(|| format!("the table {name} is not written as [tables.{name}]"))?;
    let mut said = Vec::new();
    for change in changes {
        match change.field.as_str() {
            "lens" => {
                is_still("lens", &change.value, lens, id)?;
                continue;
            }
            "name" => {
                is_still("name", &change.value, name, id)?;
                continue;
            }
            "key" => set_listed(table, "key", &change.value),
            "extra_fields" => set_whether(table, "extra_fields", &change.value)?,
            "dated" => set_words(table, "dated", &change.value),
            other => {
                bail!("{other} is not something a table of a lens says: key, extra_fields, dated")
            }
        }
        said.push(change.field.clone());
    }
    Ok(format!(
        "{} the table {name} of the lens {lens}: {}",
        if added { "added" } else { "changed" },
        said.join(", ")
    ))
}

/// A field of a table a lens defines, set in its file: one of the fields of [tables.NAME], known by its
/// name. A new one is written after the last, on a line of its own as they are.
fn of_a_field(doc: &mut DocumentMut, lens: &str, id: &str, changes: &[Change]) -> Result<String> {
    let (of_table, name) = after(lens, id)
        .and_then(|rest| rest.split_once('/'))
        .filter(|(table, name)| !table.is_empty() && !name.is_empty())
        .with_context(|| {
            format!("a field is known by the lens, its table and its name - {lens}/TABLE/NAME: {id} is not so named")
        })?;
    let fields = doc
        .get_mut("tables")
        .and_then(|tables| tables.get_mut(of_table))
        .and_then(|table| table.get_mut("fields"))
        .and_then(|fields| fields.as_array_mut())
        .with_context(|| {
            format!("the lens {lens} defines no table {of_table} with fields: a field is of a table the lens defines")
        })?;
    let named = |field: &Value| {
        field
            .as_inline_table()
            .and_then(|field| field.get("name"))
            .and_then(|name| name.as_str())
            .map(str::to_string)
    };
    let at = fields
        .iter()
        .position(|field| named(field).as_deref() == Some(name));
    if at.is_none() {
        // on a line of its own where the fields before it are - without what is written above the last one
        let last = fields
            .iter()
            .last()
            .and_then(|last| last.decor().prefix())
            .and_then(|prefix| prefix.as_str())
            .unwrap_or("\n    ")
            .to_string();
        let before = match last.rsplit_once('\n') {
            Some((_, indent)) => format!("\n{indent}"),
            None => " ".to_string(),
        };
        let mut field = InlineTable::new();
        field.insert("name", name.into());
        field.insert("type", "string".into());
        let mut field = Value::InlineTable(field);
        field.decor_mut().set_prefix(before);
        fields.push_formatted(field);
    }
    let stands = at.unwrap_or(fields.len() - 1);
    let field = fields
        .get_mut(stands)
        .and_then(|field| field.as_inline_table_mut())
        .context("a field is not written as { name = ... }")?;
    let mut said = Vec::new();
    for change in changes {
        match change.field.as_str() {
            "lens" => {
                is_still("lens", &change.value, lens, id)?;
                continue;
            }
            "of_table" => {
                is_still("table", &change.value, of_table, id)?;
                continue;
            }
            "name" => {
                is_still("name", &change.value, name, id)?;
                continue;
            }
            "ordinal" => {
                is_still(
                    "place among the fields",
                    &change.value,
                    &(stands + 1).to_string(),
                    id,
                )?;
                continue;
            }
            "kind" | "pattern" => {
                let key = if change.field == "kind" {
                    "type"
                } else {
                    "pattern"
                };
                if change.value.trim().is_empty() {
                    field.remove(key);
                } else {
                    field.insert(key, change.value.trim().into());
                }
            }
            "needed" => {
                if whether("needed", &change.value)? {
                    field.insert("required", true.into());
                } else {
                    field.remove("required");
                }
            }
            "allowed" => {
                let allowed = listed(&change.value);
                if allowed.is_empty() {
                    field.remove("enum");
                } else {
                    field.insert("enum", Value::Array(Array::from_iter(allowed)));
                }
            }
            other => bail!("{other} is not something a field says: kind, needed, allowed, pattern"),
        }
        said.push(change.field.clone());
    }
    Ok(format!(
        "{} the field {name} of the table {of_table} of the lens {lens}: {}",
        if at.is_none() { "added" } else { "changed" },
        said.join(", ")
    ))
}

/// An entry of a lens, set in its file: one [[glimpse]], known by what the entry is called.
fn of_an_entry(doc: &mut DocumentMut, lens: &str, id: &str, changes: &[Change]) -> Result<String> {
    let name = after(lens, id)
        .filter(|name| !name.contains('/'))
        .with_context(|| {
            format!("an entry of a lens is known by the lens and what it is called - {lens}/NAME: {id} is not so named")
        })?;
    let (entry, added) = named_so(doc, "glimpse", "entry", name)?;
    let mut said = Vec::new();
    for change in changes {
        match change.field.as_str() {
            "lens" => {
                is_still("lens", &change.value, lens, id)?;
                continue;
            }
            "entry" => {
                is_still("name", &change.value, name, id)?;
                continue;
            }
            "says" => set_words(entry, "says", &change.value),
            "fills" => set_words(entry, "table", &change.value),
            "asks" => set_listed(entry, "asks", &change.value),
            "supplies" => {
                // each said as it is read back: the field, what it is supplied from, and when
                let mut supplies = Array::new();
                for one in change
                    .value
                    .split(';')
                    .map(str::trim)
                    .filter(|one| !one.is_empty())
                {
                    let mut said = one.splitn(3, ' ');
                    let (Some(field), Some(from), Some(when)) =
                        (said.next(), said.next(), said.next())
                    else {
                        bail!("what an entry supplies is said as FIELD FROM WHEN - seen today new: '{one}' is not");
                    };
                    let mut supply = InlineTable::new();
                    supply.insert("field", field.into());
                    supply.insert("from", from.into());
                    supply.insert("when", when.trim().into());
                    supplies.push(supply);
                }
                if supplies.is_empty() {
                    entry.remove("supplies");
                } else {
                    entry["supplies"] = value(supplies);
                }
            }
            other => bail!(
                "{other} is not something an entry of a lens says: says, fills, asks, supplies"
            ),
        }
        said.push(change.field.clone());
    }
    Ok(format!(
        "{} the entry {name} of the lens {lens}: {}",
        if added { "added" } else { "changed" },
        said.join(", ")
    ))
}

/// A link a lens declares, set in its file: one [[relation]], changed where it stands or added after the
/// last.
fn of_a_link(doc: &mut DocumentMut, lens: &str, id: &str, changes: &[Change]) -> Result<String> {
    let at = standing(lens, id, "a link")?;
    let (link, added) = standing_at(doc, "relation", lens, at)?;
    let mut said = Vec::new();
    for change in changes {
        match change.field.as_str() {
            "lens" => {
                is_still("lens", &change.value, lens, id)?;
                continue;
            }
            "from" | "level" | "nature" => set_words(link, &change.field, &change.value),
            "to" | "passes_over" => set_listed(link, &change.field, &change.value),
            other => bail!("{other} is not something a link of a lens says: from, to, passes_over, level, nature"),
        }
        said.push(change.field.clone());
    }
    Ok(format!(
        "{} link {at} of the lens {lens}: {}",
        if added { "added" } else { "changed" },
        said.join(", ")
    ))
}

/// A way in a lens declares, set in its file: one [[route]], known by the table it brings.
fn of_a_way_in(doc: &mut DocumentMut, lens: &str, id: &str, changes: &[Change]) -> Result<String> {
    let of_table = after(lens, id)
        .filter(|name| !name.contains('/'))
        .with_context(|| {
            format!("a way in is known by the lens and the table it brings - {lens}/TABLE: {id} is not so named")
        })?;
    let (way, added) = named_so(doc, "route", "table", of_table)?;
    let mut said = Vec::new();
    for change in changes {
        match change.field.as_str() {
            "lens" => {
                is_still("lens", &change.value, lens, id)?;
                continue;
            }
            "of_table" => {
                is_still("table", &change.value, of_table, id)?;
                continue;
            }
            "from" | "which" | "shape" => set_words(way, &change.field, &change.value),
            other => bail!("{other} is not something a way in says: from, which, shape"),
        }
        said.push(change.field.clone());
    }
    Ok(format!(
        "{} the way in of {of_table}, of the lens {lens}: {}",
        if added { "added" } else { "changed" },
        said.join(", ")
    ))
}

/// Writes the row `id` of `table` as its changes stand, where it is a row of a lens the place at `dir`
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
    if !KEPT_IN_A_LENS.contains(&table) {
        return Ok(None);
    }
    let lens = match table {
        "lenses" => id.to_string(),
        _ => changes
            .iter()
            .find(|change| change.field == "lens")
            .map(|change| change.value.trim().to_string())
            .filter(|lens| !lens.is_empty())
            .or_else(|| id.split_once('/').map(|(lens, _)| lens.to_string()))
            .unwrap_or_default(),
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
        "lenses" => of_the_lens(&mut doc, &lens, changes)?,
        "lens_parts" => of_a_part(&mut doc, &lens, id, changes)?,
        "lens_tables" => of_a_table(&mut doc, &lens, id, changes)?,
        "lens_fields" => of_a_field(&mut doc, &lens, id, changes)?,
        "lens_entries" => of_an_entry(&mut doc, &lens, id, changes)?,
        "lens_relations" => of_a_link(&mut doc, &lens, id, changes)?,
        _ => of_a_way_in(&mut doc, &lens, id, changes)?,
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

    fn a_place_with_a_lens(named: &str) -> (PathBuf, PathBuf) {
        let d =
            std::env::temp_dir().join(format!("ocoeco-lens-file-{named}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("lenses")).unwrap();
        std::fs::write(
            d.join("oco.toml"),
            "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n",
        )
        .unwrap();
        let file = d.join("lenses/notes.toml");
        std::fs::write(
            &file,
            "# The notes: written above the lens.\n[lens]\nid = \"notes\"\nsays = \"The notes, as they are.\"\n\n# The table of notes.\n[tables.notes]\nkey = [\"id\"]\nfields = [\n    { name = \"id\", type = \"string\", required = true },\n    # what a note is about\n    { name = \"about\", type = \"string\" },\n]\n\n# A note is about another.\n[[relation]]\nfrom = \"notes.about\"\nto = [\"notes\"]\n\n# The entry that makes a note.\n[[glimpse]]\nentry = \"note\"\nsays = \"A note: what it says.\"\ntable = \"notes\"\nasks = [\"about\"]\n\n# The first part: every note.\n[[focus]]\ntitle = \"notes ({count})\"\nsql = \"\"\"\nSELECT id FROM notes\nORDER BY id\n\"\"\"\nline = \"{id}\"\n\n# The second part: how many.\n[[focus]]\ntitle = \"in all\"\nsql = \"SELECT count(*) AS n FROM notes\"\nline = \"{n}\"\n",
        )
        .unwrap();
        (d, file)
    }

    #[test]
    fn a_row_of_a_lens_kept_as_a_file_is_written_into_the_file_in_place() {
        let (d, file) = a_place_with_a_lens("parts");
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
                && text.contains("# what a note is about")
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

    #[test]
    fn the_rest_of_what_a_lens_file_keeps_is_written_the_same_way() {
        let (d, file) = a_place_with_a_lens("rest");
        let rows = || crate::lenses::as_rows(std::slice::from_ref(&d)).unwrap();
        let of = |table: &str, id: &str, field: &str| -> String {
            rows()[table]
                .iter()
                .find(|row| row["id"] == id)
                .map(|row| match &row[field] {
                    serde_json::Value::String(said) => said.clone(),
                    other => other.to_string(),
                })
                .unwrap_or_else(|| format!("no {table} {id}"))
        };
        // a field changed where it is written, and one added after the last - on a line of its own
        written(
            &d,
            false,
            "lens_fields",
            "notes/notes/about",
            &[change("needed", "true"), change("allowed", "this, that")],
        )
        .unwrap()
        .unwrap();
        written(
            &d,
            false,
            "lens_fields",
            "notes/notes/weight",
            &[change("kind", "integer")],
        )
        .unwrap()
        .unwrap();
        assert_eq!(of("lens_fields", "notes/notes/about", "needed"), "true");
        assert_eq!(
            of("lens_fields", "notes/notes/about", "allowed"),
            "this, that"
        );
        assert_eq!(of("lens_fields", "notes/notes/weight", "kind"), "integer");
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(
            text.contains("\n    { name = \"weight\", type = \"integer\" }")
                && text.contains("# what a note is about"),
            "a field added stands on a line of its own, and what is written around it is as it was: {text}"
        );
        assert!(written(
            &d,
            false,
            "lens_fields",
            "notes/none/x",
            &[change("kind", "string")]
        )
        .is_err());
        // a table added, keyed by its id; and what it says of itself changed
        written(
            &d,
            false,
            "lens_tables",
            "notes/more",
            &[change("dated", "seen")],
        )
        .unwrap()
        .unwrap();
        assert_eq!(of("lens_tables", "notes/more", "key"), "id");
        assert_eq!(of("lens_tables", "notes/more", "dated"), "seen");
        assert_eq!(of("lens_fields", "notes/more/id", "needed"), "true");
        // an entry changed by what it is called, and one added
        written(
            &d,
            false,
            "lens_entries",
            "notes/note",
            &[
                change("asks", "about, weight"),
                change("supplies", "seen today new; by who new"),
            ],
        )
        .unwrap()
        .unwrap();
        assert_eq!(of("lens_entries", "notes/note", "asks"), "about, weight");
        assert_eq!(
            of("lens_entries", "notes/note", "supplies"),
            "seen today new; by who new"
        );
        written(
            &d,
            false,
            "lens_entries",
            "notes/more",
            &[
                change("says", "One more."),
                change("fills", "more"),
                change("asks", "about"),
            ],
        )
        .unwrap()
        .unwrap();
        assert_eq!(of("lens_entries", "notes/more", "fills"), "more");
        // a link changed where it stands, and one added after the last
        written(
            &d,
            false,
            "lens_relations",
            "notes/1",
            &[change("nature", "is about")],
        )
        .unwrap()
        .unwrap();
        written(
            &d,
            false,
            "lens_relations",
            "notes/2",
            &[change("from", "more.about"), change("to", "notes, more")],
        )
        .unwrap()
        .unwrap();
        assert_eq!(of("lens_relations", "notes/1", "nature"), "is about");
        assert_eq!(of("lens_relations", "notes/2", "to"), "notes, more");
        assert!(written(
            &d,
            false,
            "lens_relations",
            "notes/9",
            &[change("from", "x.y")]
        )
        .is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
