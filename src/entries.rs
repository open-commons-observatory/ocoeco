//! The glimpse of each lens: what can be entered through it. An entry is declared in its lens - the
//! table it fills, what a person is asked for, and what the computer supplies and when - so that what
//! is already known is not retyped and what the computer can get is not asked. `ocoeco glimpse` alone
//! shows every entry of every lens; `ocoeco glimpse ENTRY --id ID --FIELD VALUE` makes one.

use crate::join::rows;
use crate::lenses::{lenses, Entry, Lens};
use crate::mark::Mark;
use crate::note::{note, Change};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// When the computer supplies a field.
#[derive(Debug, PartialEq, Eq)]
pub enum When {
    New,
    Always,
    Changes(String),
    Becomes(String, String),
}

/// Reads a condition as it is written in a lens: new, always, FIELD changes, FIELD becomes VALUE.
pub fn when(text: &str) -> Result<When> {
    let words: Vec<&str> = text.split_whitespace().collect();
    Ok(match words.as_slice() {
        ["new"] => When::New,
        ["always"] => When::Always,
        [field, "changes"] => When::Changes(field.to_string()),
        [field, "becomes", value] => When::Becomes(field.to_string(), value.to_string()),
        _ => {
            bail!("when = {text:?} is not one of: new, always, FIELD changes, FIELD becomes VALUE")
        }
    })
}

/// The entry of that name, and the lens that declares it.
pub fn entry(name: &str) -> Option<(Lens, Entry)> {
    lenses().into_iter().find_map(|l| {
        l.glimpse
            .iter()
            .find(|e| e.entry == name)
            .cloned()
            .map(|e| (l, e))
    })
}

/// The entry of that name among the lenses in view from the place at `dir` - built into ocoeco, or brought
/// by the place, as files or kept as rows.
pub fn entry_in(name: &str, dir: &Path) -> Option<(Lens, Entry)> {
    crate::lenses::with(&[dir.to_path_buf()])
        .unwrap_or_else(|_| lenses())
        .into_iter()
        .find_map(|l| {
            l.glimpse
                .iter()
                .find(|e| e.entry == name)
                .cloned()
                .map(|e| (l, e))
        })
}

/// Whether `name` is an entry some lens declares, among those in view from where it is typed.
pub fn is_entry(name: &str) -> bool {
    entry_in(name, Path::new(".")).is_some()
}

/// The fields a table declares: name, whether it is needed, and the values it allows.
pub fn fields(lens: &Lens, table: &str) -> Vec<(String, bool, Vec<String>)> {
    let declared = lens
        .tables
        .get(table)
        .and_then(|t| t.get("fields"))
        .and_then(|f| f.as_array());
    declared
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let name = f.get("name")?.as_str()?.to_string();
            let needed = f.get("required").and_then(|r| r.as_bool()).unwrap_or(false);
            let allowed = f
                .get("enum")
                .and_then(|e| e.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            Some((name, needed, allowed))
        })
        .collect()
}

/// What one entry asks a person for: each field with whether it is needed and the values it allows,
/// and what the computer supplies, in words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asks {
    pub entry: String,
    pub says: String,
    pub fields: Vec<(String, bool, Vec<String>)>,
    pub supplied: Vec<String>,
}

/// The entries of one lens, as what each asks a person for - the same declaration ocoeco glimpse reads.
pub fn asks_of(lens: &str) -> Vec<Asks> {
    let Some(lens) = lenses().into_iter().find(|l| l.lens.id == lens) else {
        return Vec::new();
    };
    lens.glimpse
        .iter()
        .map(|e| {
            let declared = fields(&lens, &e.table);
            Asks {
                entry: e.entry.clone(),
                says: e.says.clone(),
                fields: e
                    .asks
                    .iter()
                    .map(|a| {
                        declared
                            .iter()
                            .find(|(n, _, _)| n == a)
                            .cloned()
                            .unwrap_or((a.clone(), false, Vec::new()))
                    })
                    .collect(),
                supplied: e
                    .supplies
                    .iter()
                    .map(|s| format!("{} ({}, when {})", s.field, s.from, s.when))
                    .collect(),
            }
        })
        .collect()
}

/// What can be entered through one lens: its entries, with what a person is asked for and what the
/// computer supplies.
pub fn of(lens: &Lens) -> String {
    let mut text = format!("{}: {}\n", lens.lens.id, lens.lens.says);
    if lens.glimpse.is_empty() {
        let tables: Vec<&str> = lens.tables.keys().map(String::as_str).collect();
        text.push_str(&format!(
            "  no entries yet - its tables are written with ocoeco note: {}\n",
            tables.join(", ")
        ));
    }
    for e in &lens.glimpse {
        text.push_str(&format!("  {} - {}\n", e.entry, e.says));
        let declared = fields(lens, &e.table);
        let asked: Vec<String> = e
            .asks
            .iter()
            .map(|a| match declared.iter().find(|(n, _, _)| n == a) {
                Some((_, true, allowed)) if !allowed.is_empty() => {
                    format!("{a} (needed: {})", allowed.join(", "))
                }
                Some((_, true, _)) => format!("{a} (needed)"),
                Some((_, false, allowed)) if !allowed.is_empty() => {
                    format!("{a} ({})", allowed.join(", "))
                }
                _ => a.clone(),
            })
            .collect();
        text.push_str(&format!("    asks: {}\n", asked.join("; ")));
        let supplied: Vec<String> = e
            .supplies
            .iter()
            .map(|s| format!("{} ({}, when {})", s.field, s.from, s.when))
            .collect();
        if !supplied.is_empty() {
            text.push_str(&format!(
                "    the computer supplies: {}\n",
                supplied.join("; ")
            ));
        }
    }
    text
}

/// Every entry of every lens: what a person is asked for and what the computer supplies.
pub fn composed() -> String {
    let mut text = String::from("what can be glimpsed - ocoeco glimpse ENTRY --id ID --FIELD VALUE ... (--add-FIELD MORE adds to a field's text)\n");
    for lens in lenses() {
        text.push('\n');
        text.push_str(&of(&lens));
    }
    text.push_str(
        "\nthe record of work as it is done: ocoeco glimpse \"what was done\" --kind KIND\n",
    );
    text
}

/// Makes one entry: `args` are --id ID, --FIELD VALUE, --add-FIELD MORE and --dir DIR, in any order.
pub fn enter(name: &str, args: &[String], today: &str, who: &str) -> Result<String> {
    // the entry is found where it is entered: the place named by --dir, else here
    let at = args
        .iter()
        .position(|a| a == "--dir")
        .and_then(|i| args.get(i + 1).cloned())
        .or_else(|| {
            args.iter()
                .find_map(|a| a.strip_prefix("--dir=").map(str::to_string))
        })
        .unwrap_or_else(|| ".".to_string());
    let (lens, e) = entry_in(name, Path::new(&at))
        .with_context(|| format!("no lens declares an entry called {name}"))?;
    let declared: Vec<String> = fields(&lens, &e.table)
        .into_iter()
        .map(|(n, _, _)| n)
        .collect();
    // what was typed is read back by the typed line's own reader, which says itself what it could not
    // take; the entry adds only what it knows - what it asks
    let all = fields(&lens, &e.table);
    let given = crate::typed::read(&crate::typed::records(name, &all), args).map_err(|not| {
        let mut said = not.said.join("\n");
        if not.unknown {
            let also: Vec<&str> = declared
                .iter()
                .filter(|d| !e.asks.contains(d) && *d != "id")
                .map(String::as_str)
                .collect();
            said.push_str(&format!(
                "\n{name} asks for {} - and may also carry {}",
                e.asks.join(", "),
                also.join(", ")
            ));
        }
        anyhow::anyhow!(said)
    })?;
    let id = given
        .get("id")
        .cloned()
        .with_context(|| format!("which {name}? give --id"))?;
    let dir = given
        .get("dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    // what is set, then what is added to it - whichever way round they were typed
    let mut changes: Vec<Change> = Vec::new();
    for append in [false, true] {
        for (key, value) in given
            .iter()
            .filter(|(key, _)| *key != "id" && *key != "dir")
        {
            let (field, adds) = key
                .strip_prefix("add-")
                .map_or((key.as_str(), false), |f| (f, true));
            if adds == append {
                changes.push(Change {
                    field: field.to_string(),
                    value: value.clone(),
                    append,
                });
            }
        }
    }
    let was = existing(&dir, &e.table, &id)?;
    for supply in &e.supplies {
        if changes.iter().any(|c| c.field == supply.field) {
            continue; // what a person gives always stands
        }
        let given = |field: &str| {
            changes
                .iter()
                .find(|c| c.field == field && !c.append)
                .map(|c| c.value.clone())
        };
        let changed = |field: &str| {
            given(field)
                .filter(|v| was.as_ref().and_then(|w| w[field].as_str()) != Some(v.as_str()))
        };
        let due = match when(&supply.when)? {
            When::Always => true,
            When::New => was.is_none(),
            When::Changes(field) => changed(&field).is_some(),
            When::Becomes(field, value) => changed(&field).is_some_and(|v| v == value),
        };
        if due {
            let value = match supply.from.as_str() {
                "today" => today.to_string(),
                "who" => who.to_string(),
                other => bail!("the computer cannot supply {other:?}: it knows today and who"),
            };
            changes.push(Change {
                field: supply.field.clone(),
                value,
                append: false,
            });
        }
    }
    note(&dir, &e.table, &id, &changes, today)
}

/// The row as it is now, if there is one.
fn existing(dir: &Path, table: &str, id: &str) -> Result<Option<serde_json::Value>> {
    let mark = Mark::parse(
        &std::fs::read_to_string(dir.join("oco.toml"))
            .context("no oco.toml here: this is not a marked place")?,
    )?;
    Ok(rows(dir, &mark, table)
        .into_iter()
        .find(|r| r["id"].as_str() == Some(id)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn star(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ocoeco-entries-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nbacklog = \"data/backlog.json\"\n").unwrap();
        std::fs::write(d.join("data/backlog.json"), "").unwrap();
        d
    }
    fn args(d: &Path, words: &[&str]) -> Vec<String> {
        let mut all: Vec<String> = words.iter().map(|w| w.to_string()).collect();
        all.extend(["--dir".to_string(), d.display().to_string()]);
        all
    }
    fn row(d: &Path, id: &str) -> serde_json::Value {
        existing(d, "backlog", id).unwrap().unwrap()
    }

    #[test]
    fn a_condition_is_read_as_written() {
        assert_eq!(when("new").unwrap(), When::New);
        assert_eq!(
            when("status changes").unwrap(),
            When::Changes("status".into())
        );
        assert_eq!(
            when("status becomes in-progress").unwrap(),
            When::Becomes("status".into(), "in-progress".into())
        );
        assert!(when("whenever").is_err());
    }

    #[test]
    fn the_computer_supplies_what_it_knows_and_what_a_person_gives_stands() {
        let d = star("supplies");
        enter(
            "item",
            &args(&d, &["--id", "a", "--title", "A", "--status", "open"]),
            "2026-11-01",
            "ann",
        )
        .unwrap();
        let r = row(&d, "a");
        assert_eq!(
            (
                r["opened"].as_str(),
                r["since"].as_str(),
                r["driver"].as_str()
            ),
            (Some("2026-11-01"), Some("2026-11-01"), None),
            "new: since supplied, nobody holds it"
        );
        // taken up later: since moves to that day and whoever takes it holds it
        enter(
            "item",
            &args(
                &d,
                &[
                    "--id",
                    "a",
                    "--status",
                    "in-progress",
                    "--next",
                    "the first step",
                ],
            ),
            "2026-11-05",
            "ann",
        )
        .unwrap();
        let r = row(&d, "a");
        assert_eq!(
            (
                r["since"].as_str(),
                r["driver"].as_str(),
                r["opened"].as_str()
            ),
            (Some("2026-11-05"), Some("ann"), Some("2026-11-01"))
        );
        // a change that is not of state supplies nothing
        enter(
            "item",
            &args(
                &d,
                &[
                    "--id",
                    "a",
                    "--next",
                    "the second step",
                    "--add-note",
                    "went well",
                ],
            ),
            "2026-11-09",
            "bob",
        )
        .unwrap();
        let r = row(&d, "a");
        assert_eq!(
            (
                r["since"].as_str(),
                r["driver"].as_str(),
                r["next"].as_str()
            ),
            (Some("2026-11-05"), Some("ann"), Some("the second step"))
        );
        // what a person gives always stands
        enter(
            "item",
            &args(
                &d,
                &[
                    "--id",
                    "b",
                    "--title",
                    "B",
                    "--status",
                    "in-progress",
                    "--driver",
                    "cy",
                    "--since",
                    "2026-10-30",
                ],
            ),
            "2026-11-09",
            "bob",
        )
        .unwrap();
        let r = row(&d, "b");
        assert_eq!(
            (r["since"].as_str(), r["driver"].as_str()),
            (Some("2026-10-30"), Some("cy"))
        );
    }

    #[test]
    fn a_field_the_entry_does_not_have_is_refused_and_the_view_shows_what_can_be_entered() {
        let d = star("refused");
        let e = enter(
            "item",
            &args(&d, &["--id", "a", "--tilte", "A"]),
            "2026-11-01",
            "ann",
        )
        .unwrap_err()
        .to_string();
        assert!(
            e.contains("--tilte is not a declared flag\nitem asks for title, status, next")
                && !e.contains("is not understood"),
            "{e}"
        );
        assert!(
            enter("item", &args(&d, &["--title", "A"]), "2026-11-01", "ann")
                .unwrap_err()
                .to_string()
                .contains("id is required; supply it with --id")
        );
        // a word with a letter missing is answered with what was meant - the reader's own doing
        let meant = enter(
            "item",
            &args(&d, &["--id", "a", "--staus", "next"]),
            "2026-11-01",
            "ann",
        )
        .unwrap_err()
        .to_string();
        assert!(
            meant.contains("--staus is not a declared flag; did you mean --status?")
                && meant.contains("item asks for title"),
            "{meant}"
        );
        let view = composed();
        assert!(view.contains("  item - A piece of work") && view.contains("the computer supplies: since (today, when status changes); driver (who, when status becomes in-progress)"), "{view}");
        assert!(
            view.contains("status (needed: open, next, in-progress"),
            "{view}"
        );
        assert!(is_entry("item") && !is_entry("what was done"));
    }
}
