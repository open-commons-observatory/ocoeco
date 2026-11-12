//! Reads lenses/*.toml and builds them into ocoeco. A lens id used twice, or a table defined by two
//! lenses, stops the build: a collision is met in the making, never in use. One lens defines a piece
//! of data; other lenses read it.
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=lenses");
    let mut files: Vec<_> = fs::read_dir("lenses")
        .expect("a lenses folder")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    files.sort();
    let (mut ids, mut tables): (BTreeMap<String, String>, BTreeMap<String, String>) =
        Default::default();
    let mut entries: BTreeMap<String, String> = BTreeMap::new();
    let mut reads: Vec<(String, String)> = Vec::new();
    let mut out = String::from("/// Every lens built into this ocoeco: its file's name and its text.\npub const LENSES: &[(&str, &str)] = &[\n");
    for file in &files {
        let name = file
            .file_name()
            .expect("a file name")
            .to_string_lossy()
            .to_string();
        let text = fs::read_to_string(file).expect("a lens file that can be read");
        let lens: toml::Table = text.parse().unwrap_or_else(|e| panic!("{name}: {e}"));
        let id = lens
            .get("lens")
            .and_then(|l| l.get("id"))
            .and_then(|i| i.as_str())
            .unwrap_or_else(|| panic!("{name}: a lens says its id - [lens] id = \"...\""))
            .to_string();
        if let Some(other) = ids.insert(id.clone(), name.clone()) {
            panic!("two lenses are called {id}: {other} and {name}");
        }
        for table in lens
            .get("tables")
            .and_then(|t| t.as_table())
            .into_iter()
            .flat_map(|t| t.keys())
        {
            if let Some(other) = tables.insert(table.clone(), name.clone()) {
                panic!("the table {table} is defined by two lenses, {other} and {name}: one lens defines a piece of data, others read it");
            }
        }
        for entry in lens
            .get("glimpse")
            .and_then(|g| g.as_array())
            .into_iter()
            .flatten()
        {
            let called = entry
                .get("entry")
                .and_then(|e| e.as_str())
                .unwrap_or_else(|| {
                    panic!("{name}: an entry says what it is called - entry = \"...\"")
                })
                .to_string();
            if let Some(other) = entries.insert(called.clone(), name.clone()) {
                panic!("two entries are called {called}: in {other} and in {name}");
            }
            let table = entry
                .get("table")
                .and_then(|t| t.as_str())
                .unwrap_or_default();
            if lens.get("tables").and_then(|t| t.get(table)).is_none() {
                panic!("{name}: the entry {called} fills the table {table:?}, which this lens does not define");
            }
        }
        for read in lens
            .get("lens")
            .and_then(|l| l.get("reads"))
            .and_then(|r| r.as_array())
            .into_iter()
            .flatten()
            .filter_map(|r| r.as_str())
        {
            reads.push((name.clone(), read.to_string()));
        }
        out.push_str(&format!("    ({name:?}, include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/lenses/{name}\"))),\n"));
    }
    for (lens, table) in &reads {
        if !tables.contains_key(table) {
            panic!("{lens} reads the table {table}, which no lens defines");
        }
    }
    out.push_str("];\n");
    fs::write(
        Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("lenses.rs"),
        out,
    )
    .expect("writing the lenses");
}
