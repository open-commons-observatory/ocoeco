//! The relations between rows: which field refers to which table (each lens declares its own), and whether every
//! link holds. Until 2026-11-12 the relations were kept by discipline - about 860 links, a handful
//! pointing nowhere, and nothing to say so. The fulldome is one sky only if they are true.

use crate::mark::Mark;
use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// One declared relation: the rows of `from` (table.field) refer to rows of the tables in `to`.
#[derive(serde::Deserialize, Clone, Debug)]
pub struct Relation {
    pub from: String,
    pub to: Vec<String>,
    /// Marks of what is not a link to a row: a value holding one of these is passed over.
    #[serde(default)]
    pub passes_over: Vec<String>,
    /// Where a link holds this mark, what follows it is the level its row must reach (feature:x@best),
    /// not part of its id.
    #[serde(default)]
    pub level: Option<String>,
    /// The nature of this pointing, where its field's name is one word among several for it: what a
    /// thing rests on, what it is for, what it follows, what it is about. Where none is said, the
    /// field's own name is its nature.
    #[serde(default)]
    pub nature: Option<String>,
    /// The rows of `from` say what was: each link points at a thing as it was when the row was written - a
    /// record of work, at what it touched and where it was made; a step, at what was stood on. A thing taken
    /// back since is what the row said then: the link does not point nowhere, and it keeps nothing from
    /// being taken back.
    #[serde(default)]
    pub of_what_was: bool,
}

/// The relations of the shared language: those every lens declares.
pub fn relations() -> Vec<Relation> {
    crate::lenses::relations()
}

/// A link as written: an id, a table and an id (table:id), or a star, a table and an id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub star: Option<String>,
    pub table: Option<String>,
    pub id: String,
}

/// The links in one field's value: separated by commas or spaces.
pub fn links(value: &str) -> Vec<Link> {
    value
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|w| !w.is_empty())
        .map(|w| match w.split_once(':') {
            None => Link {
                star: None,
                table: None,
                id: w.to_string(),
            },
            Some((place, id)) => match place.rsplit_once('/') {
                Some((star, table)) => Link {
                    star: Some(star.to_string()),
                    table: Some(table.to_string()),
                    id: id.to_string(),
                },
                None => Link {
                    star: None,
                    table: Some(place.to_string()),
                    id: id.to_string(),
                },
            },
        })
        .collect()
}

/// What a row is, in a few words: the first of its fields that says something.
/// When a row happened, in words that sort in time: the field its table says dates it, or else its `at`,
/// or else its `date` - where that is shaped like a day or a moment. A moment kept as a whole number of
/// seconds is said as a moment. A row that says nothing of when has none.
pub fn when(row: &serde_json::Value, dated: Option<&str>) -> Option<String> {
    let shaped = |value: &serde_json::Value| -> Option<String> {
        match value {
            serde_json::Value::String(text) => {
                let (text, b) = (text.trim(), text.trim().as_bytes());
                (b.len() >= 10
                    && b[..4].iter().all(u8::is_ascii_digit)
                    && b[4] == b'-'
                    && b[5..7].iter().all(u8::is_ascii_digit)
                    && b[7] == b'-'
                    && b[8..10].iter().all(u8::is_ascii_digit))
                .then(|| text.to_string())
            }
            serde_json::Value::Number(n) => n
                .as_i64()
                .filter(|secs| (1_000_000_000..100_000_000_000).contains(secs))
                .map(|secs| {
                    format!(
                        "{}T{:02}:{:02}:{:02}Z",
                        crate::gather::day(secs / 86_400),
                        secs % 86_400 / 3_600,
                        secs % 3_600 / 60,
                        secs % 60
                    )
                }),
            _ => None,
        }
    };
    dated
        .into_iter()
        .chain(["at", "date"])
        .find_map(|field| row.get(field).and_then(shaped))
}

/// A row in a few words, as its table says its rows are told - by one field, or by several laid out as a
/// line is - and, where its table says nothing of it, by the words a list in the program finds.
pub fn told(row: &serde_json::Value, tells: Option<&String>) -> String {
    let said = tells
        .zip(row.as_object())
        .map(|(tells, row)| {
            let line = if tells.contains('{') {
                tells.clone()
            } else {
                format!("{{{}}}", tells.trim())
            };
            crate::focus::laid_out(&line, row)
        })
        .filter(|said| !said.trim().is_empty() && !said.contains('{'));
    match said {
        Some(said) if said.chars().count() > 70 => {
            format!("{}...", said.chars().take(70).collect::<String>())
        }
        Some(said) => said,
        None => in_a_few_words(row),
    }
}

pub fn in_a_few_words(row: &serde_json::Value) -> String {
    let words = |key: &str| {
        row.get(key)
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
    };
    // a part of a lens is said by its heading, else by what it is - and only then by what is written above it
    let a_part = row.get("lens").is_some() && row.get("part").is_some();
    let said = [
        "title", "what", "decision", "term", "says", "question", "name", "text", "practice",
        "over", "summary",
    ]
    .iter()
    .filter(|key| !(a_part && **key == "says"))
    .find_map(|key| words(key))
    .unwrap_or_default();
    // a part of a lens with no heading says what it is: the lens it places, and the lens that is seen
    // through - or the shape it names. So that walking a lens one passes no blank line
    let as_a_part = match (
        said.is_empty(),
        words("places"),
        words("through"),
        words("named"),
    ) {
        (true, Some("*"), None, _) => "places every lens in view".to_string(),
        (true, Some(placed), None, _) => format!("places {placed}"),
        (true, Some(placed), Some(behind), _) => {
            format!("places {placed}, seen through {behind}")
        }
        (true, None, _, Some(shape)) => format!("the shape {shape}"),
        _ => String::new(),
    };
    let said = if !as_a_part.is_empty() {
        as_a_part.as_str()
    } else if said.is_empty() && a_part {
        words("says").unwrap_or_default()
    } else {
        said
    };
    let mut short: String = said.chars().take(70).collect();
    if said.chars().count() > 70 {
        short.push_str("...");
    }
    short
}

/// A link as it was written: an id, a table and an id, or a star, a table and an id.
fn written(link: &Link) -> String {
    match (&link.star, &link.table) {
        (Some(star), Some(table)) => format!("{star}/{table}:{}", link.id),
        (_, Some(table)) => format!("{table}:{}", link.id),
        _ => link.id.clone(),
    }
}

/// The edges of a graph, and the things they join.
pub type Graph = BTreeMap<&'static str, Vec<serde_json::Map<String, serde_json::Value>>>;

/// Every link of the star at `dir` that points nowhere, read off the graph - the one place that knows.
/// With `others` - the stars it may point into, by their id - links into another star are followed too,
/// and a star not among them is said to be missing; without, such links are left alone.
pub fn broken(dir: &Path, others: &BTreeMap<String, PathBuf>) -> Result<Vec<String>> {
    let here =
        Mark::parse(&std::fs::read_to_string(dir.join("oco.toml")).context("no oco.toml here")?)?
            .project
            .id;
    let mut dirs = vec![dir.to_path_buf()];
    dirs.extend(
        others
            .iter()
            .filter(|(id, _)| **id != here)
            .map(|(_, d)| d.clone()),
    );
    let graph = as_graph_from(&dirs, Some(&here))?;
    let text = |e: &serde_json::Map<String, serde_json::Value>, key: &str| {
        e.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    let mut said: Vec<String> = Vec::new();
    for e in &graph["edges"] {
        // a link said of what was, whose thing was taken back since, points at what was - not nowhere
        if e.get("of_what_was").and_then(|v| v.as_bool()) == Some(true)
            && e.get("holds").and_then(|v| v.as_bool()) == Some(false)
        {
            continue;
        }
        let through = text(e, "via_id");
        let from = if through.is_empty() {
            format!("{} {}", text(e, "from_table"), text(e, "from_id"))
        } else {
            format!("connections {through}")
        };
        let mut say = |field: &str, written: String, holds: Option<bool>, table: String| {
            let shown = if written.contains(':') {
                written.clone()
            } else {
                format!("{written} (in {table})")
            };
            match holds {
                Some(false) => said.push(format!("{from}: {field} -> {shown} - points nowhere")),
                None if !others.is_empty() => {
                    said.push(format!("{from}: {field} -> {written} - no such star here"))
                }
                _ => {}
            }
        };
        let holds = |key: &str| e.get(key).and_then(|v| v.as_bool());
        if through.is_empty() {
            say(
                &text(e, "how"),
                text(e, "written"),
                holds("holds"),
                text(e, "to_table"),
            );
        } else {
            say(
                "from",
                text(e, "from_written"),
                holds("from_holds"),
                "*".into(),
            );
            say("to", text(e, "written"), holds("holds"), "*".into());
        }
    }
    let mut once = BTreeSet::new();
    said.retain(|s| once.insert(s.clone()));
    Ok(said)
}

/// The tables that hold every link between everything in view as rows. No place keeps them: ocoeco fills
/// them from the links the places hold, as it fills the language's own from the lenses.
pub const OF_THE_GRAPH: [&str; 2] = ["things", "edges"];

/// A place in view, by whatever route what it keeps arrived - a folder with a mark, or what an observatory
/// observed of it: what it says it is for, the other names its tables go by, what it gathers, and the
/// rows of each table it keeps. The graph is made of these, and does not ask how they came.
#[derive(Default)]
pub(crate) struct Held {
    says: String,
    also: BTreeMap<String, String>,
    gathers: Vec<String>,
    pub(crate) tables: BTreeMap<String, Vec<serde_json::Value>>,
    /// What it has, by table and id: asked for every link, so kept to be found - not gone through each
    /// time. Made once all its rows are read.
    ids: BTreeMap<String, BTreeSet<String>>,
}

impl Held {
    fn has(&self, table: &str, id: &str) -> bool {
        self.ids.get(table).is_some_and(|ids| ids.contains(id))
    }
}

/// Since when the stretch under way has run, and how long each stretch before it took.
type Stretches = (Option<std::time::Instant>, Vec<(&'static str, u128)>);

thread_local! {
    /// While the graph is being made, its stretches: so that a look can say where the graph's time went.
    static STRETCHES: std::cell::RefCell<Stretches> = Default::default();
}

/// A stretch of the graph's making has ended: how long it took is kept under its name, and the next
/// begins. Says nothing where no graph is being made.
fn stretch(name: &'static str) {
    STRETCHES.with(|kept| {
        let mut kept = kept.borrow_mut();
        let now = std::time::Instant::now();
        if let Some(since) = kept.0.replace(now) {
            kept.1.push((name, now.duration_since(since).as_millis()));
        }
    });
}

/// How long each stretch of making the graph took, the last time it was made in this run: by name, in
/// thousandths of a second, in the order they came.
pub fn stretches() -> Vec<(&'static str, u128)> {
    STRETCHES.with(|kept| kept.borrow().1.clone())
}

/// The places among these folders, by their id: each folder with a mark is one; a folder of what was
/// observed holds many, each row naming the place it is of.
pub(crate) fn held(dirs: &[PathBuf]) -> BTreeMap<String, Held> {
    let mut places: BTreeMap<String, Held> = BTreeMap::new();
    // what each place says of itself, and every table there is to read
    let mut tables: BTreeSet<String> = BTreeSet::new();
    for dir in dirs {
        if let Ok(mark) = std::fs::read_to_string(dir.join("oco.toml"))
            .map_err(anyhow::Error::from)
            .and_then(|t| Mark::parse(&t))
        {
            tables.extend(mark.exports.keys().cloned());
            let gathers = mark
                .place
                .as_ref()
                .map(|p| p.gathers.clone())
                .unwrap_or_default();
            places.insert(
                mark.project.id.clone(),
                Held {
                    says: mark.project.name.clone(),
                    also: mark.also.clone(),
                    gathers,
                    tables: BTreeMap::new(),
                    ids: BTreeMap::new(),
                },
            );
        } else if dir.join("projects.json").is_file() {
            let text = |v: &serde_json::Value, key: &str| {
                v.get(key)
                    .and_then(|s| s.as_str())
                    .unwrap_or_default()
                    .to_string()
            };
            for place in crate::harvest::read_rows(&dir.join("projects.json")) {
                let place = serde_json::Value::Object(place);
                let also = place
                    .get("also")
                    .and_then(|a| a.as_object())
                    .map(|a| {
                        a.iter()
                            .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                            .collect()
                    })
                    .unwrap_or_default();
                let gathers = place
                    .get("gathers")
                    .and_then(|g| g.as_array())
                    .map(|g| {
                        g.iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                places.entry(text(&place, "id")).or_insert(Held {
                    says: text(&place, "name"),
                    also,
                    gathers,
                    tables: BTreeMap::new(),
                    ids: BTreeMap::new(),
                });
            }
            tables.extend(
                std::fs::read_dir(dir)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter_map(|f| {
                        f.file_name()
                            .to_str()
                            .and_then(|n| n.strip_suffix(".json"))
                            .filter(|t| *t != "projects")
                            .map(str::to_string)
                    }),
            );
        }
    }
    stretch("the marks read");
    // and the rows of each, as the ways in give them: each once, with the place it is of
    for table in tables {
        for (place, row) in crate::routes::rows_by_place(&table, dirs) {
            if let Some(held) = places.get_mut(&place) {
                held.tables
                    .entry(table.clone())
                    .or_default()
                    .push(serde_json::Value::Object(row));
            }
        }
    }
    stretch("what the places keep, read");
    // and the language: the rows the program makes of every lens in view are kept where the lens is - a
    // lens a place brings, in that place; a lens built into the program, in the program's own place where
    // that is in view. So a lens is a thing to point at as it is a thing to stand on.
    if let Ok(language) = crate::lenses::as_rows(dirs) {
        let short = |id: &str| id.rsplit('/').next().unwrap_or(id).to_string();
        for (table, rows) in language {
            for row in rows {
                let star = row
                    .get("star")
                    .and_then(|s| s.as_str())
                    .unwrap_or_default()
                    .to_string();
                let Some(place) = places.keys().find(|id| short(id) == star).cloned() else {
                    continue;
                };
                if let Some(held) = places.get_mut(&place) {
                    let there = held.tables.entry(table.to_string()).or_default();
                    if !there.iter().any(|r| r["id"] == row["id"]) {
                        there.push(serde_json::Value::Object(row));
                    }
                }
            }
        }
    }
    stretch("the language, as rows");
    // and what a route reads - a commit, a guard, a file of source: things like any other, read where the
    // place is held. Of what was observed they are among its tables already, and are not added twice.
    let routes: Vec<crate::lenses::Route> = crate::lenses::with(dirs)
        .map(|lenses| lenses.into_iter().flat_map(|l| l.route).collect())
        .unwrap_or_default();
    for route in &routes {
        let mut read: BTreeMap<String, Vec<serde_json::Value>> = BTreeMap::new();
        for (place, row) in crate::routes::read(route, dirs) {
            read.entry(place)
                .or_default()
                .push(serde_json::Value::Object(row));
        }
        for (place, rows) in read {
            let Some(held) = places.get_mut(&place) else {
                continue;
            };
            let there = held.tables.entry(route.table.clone()).or_default();
            let known: BTreeSet<String> = there
                .iter()
                .filter_map(|r| r["id"].as_str().map(str::to_string))
                .collect();
            there.extend(
                rows.into_iter()
                    .filter(|r| r["id"].as_str().is_some_and(|id| !known.contains(id))),
            );
        }
    }
    stretch("what the routes read");
    // what each place has, by table and id - made once, now that all its rows are read
    for held in places.values_mut() {
        held.ids = held
            .tables
            .iter()
            .map(|(table, rows)| {
                (
                    table.clone(),
                    rows.iter()
                        .filter_map(|r| r["id"].as_str().map(str::to_string))
                        .collect(),
                )
            })
            .collect();
    }
    stretch("kept by id");
    places
}

/// Every row of every place in view as a thing, and every link between them as an edge: this, how, that.
/// A link a row holds in a declared relation is an edge named by its field (serves, refines, advances); a
/// connection is an edge named by how it bears (led to, needed, halted, repeats), through the connection
/// that says so. Whether an edge holds is said when the place it points into is in view, and left unsaid
/// when it is not. A thing is named by where it is held, its table and its id.
pub fn as_graph(dirs: &[PathBuf]) -> Result<Graph> {
    // within one look the graph is made once: every lens that reads it, and each table of it a lens asks
    // of, is given the same one. Not kept beyond the look - what stays open sees what has changed.
    if let Some(made) = LOOK.with(|look| look.borrow().1.get(dirs).cloned()) {
        return Ok(made);
    }
    let made = as_graph_from(dirs, None)?;
    LOOK.with(|look| {
        let mut look = look.borrow_mut();
        if look.0 > 0 {
            look.1.insert(dirs.to_vec(), made.clone());
        }
    });
    Ok(made)
}

thread_local! {
    /// How many looks are under way, one within another, and the graphs made in them.
    static LOOK: std::cell::RefCell<(usize, std::collections::HashMap<Vec<PathBuf>, Graph>)> = Default::default();
}

/// One look: while it lasts, the graph of the places in view is made once and given to all who ask.
pub struct OneLook;

/// Begins one look - a lens brought into focus, with all it places; a place checked. Where one is begun
/// within another, they are one look: what was made lasts until the outermost ends.
pub fn one_look() -> OneLook {
    LOOK.with(|look| look.borrow_mut().0 += 1);
    OneLook
}

impl Drop for OneLook {
    fn drop(&mut self) {
        LOOK.with(|look| {
            let mut look = look.borrow_mut();
            look.0 = look.0.saturating_sub(1);
            if look.0 == 0 {
                look.1.clear();
                crate::focus::forget_what_was_brought();
            }
        });
    }
}

/// The graph of the places in view, with edges only from the place `only` when it is given: what one
/// place holds, followed into the others. The check and the guard on writes read it this way.
pub fn as_graph_from(dirs: &[PathBuf], only: Option<&str>) -> Result<Graph> {
    use serde_json::json;
    // the making of the graph is timed, stretch by stretch, from here
    STRETCHES.with(|kept| *kept.borrow_mut() = (Some(std::time::Instant::now()), Vec::new()));
    let places = held(dirs);
    // the relations of every lens in view - those built in, and those a place brings: a relation is a
    // relation whoever declares it
    let declared: Vec<Relation> = crate::lenses::with(dirs)
        .map(|lenses| lenses.into_iter().flat_map(|lens| lens.relation).collect())
        .unwrap_or_else(|_| relations().to_vec());
    let short = |id: &str| id.rsplit('/').next().unwrap_or(id).to_string();
    let lenses_in_view = crate::lenses::with(dirs).unwrap_or_default();
    let said_of = |key: &str| -> BTreeMap<String, String> {
        lenses_in_view
            .iter()
            .flat_map(|lens| lens.tables.iter())
            .filter_map(|(name, table)| Some((name.clone(), table.get(key)?.as_str()?.to_string())))
            .collect()
    };
    // which field dates a row of each table, where the lens that defines the table says so: a thing says
    // when by it, so that what is in view has time in it - before and after, not only what points at what
    let dated = said_of("dated");
    // and which words tell a row in a few, where the lens that defines its table says so - a field, or
    // several laid out as a line is: a thing says itself by them
    let tells = said_of("tells");
    // where a link points, from the place that holds it, and whether that is there
    let resolve = |here: &str, link: &Link, to: &[String]| -> (String, Option<bool>) {
        // a table may be written by another name its place knows it by: a name is not the place
        let named = |place: &str, table: &str| -> String {
            places
                .get(place)
                .and_then(|place| place.also.get(table))
                .cloned()
                .unwrap_or_else(|| table.to_string())
        };
        let has = |place: &str, table: &str| {
            // a place is a thing the graph makes, not a row a place keeps: a pointing at one holds
            // where that place is in view
            if table == "places" {
                return places.get(place).map(|_| short(place) == link.id);
            }
            places
                .get(place)
                .map(|held| held.has(&named(place, table), &link.id))
        };
        // a star may be written by its shorter name: it is then the place that goes by it, where one place
        // in view does - so a link written so is told like one written in full, and ends at the thing
        let in_full = |star: &str| -> String {
            if places.contains_key(star) {
                return star.to_string();
            }
            let mut by = places.keys().filter(|id| short(id) == star);
            match (by.next(), by.next()) {
                (Some(one), None) => one.clone(),
                _ => star.to_string(),
            }
        };
        match (&link.star, &link.table) {
            (Some(star), Some(table)) => {
                let star = in_full(star);
                (
                    format!("{star}/{}:{}", named(&star, table), link.id),
                    has(&star, table),
                )
            }
            (_, Some(table)) => (
                format!("{here}/{}:{}", named(here, table), link.id),
                has(here, table),
            ),
            (_, None) => {
                let any = to.iter().any(|t| t == "*");
                let found = places.get(here).and_then(|held| {
                    held.tables
                        .keys()
                        .find(|table| (any || to.contains(*table)) && held.has(table, &link.id))
                        .cloned()
                });
                match found {
                    Some(table) => (format!("{here}/{table}:{}", link.id), Some(true)),
                    None => (
                        format!(
                            "{here}/{}:{}",
                            to.first().map_or("?", String::as_str),
                            link.id
                        ),
                        Some(false),
                    ),
                }
            }
        }
    };
    stretch("the lenses in view, for their relations and what dates a row");
    let mut things = Vec::new();
    for (id, held) in &places {
        for (table, rows) in &held.tables {
            for row in rows {
                let Some(of) = row.get("id").and_then(|v| v.as_str()) else {
                    continue;
                };
                things.push(json!({"id": format!("{id}/{table}:{of}"), "of_table": table, "place": id, "star": short(id),
                                   "says": told(row, tells.get(table)), "at": when(row, dated.get(table).map(String::as_str)),
                                   "rows_kept": row.get("rows_kept")}));
            }
        }
    }
    // every place is a thing too, and what it says of itself is what it is for: so a walk arrives at it
    for (id, held) in &places {
        things.push(json!({"id": format!("{id}/places:{}", short(id)), "of_table": "places", "place": id, "star": short(id),
                           "says": held.says}));
    }
    stretch("the things");
    let mut edges: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    // an edge: from where, how, to where and whether that holds; the link as written at its far end; and,
    // for a connection, the one it came through and whether its near end holds as well
    let mut edge = |from: String,
                    how: &str,
                    (to, holds): (String, Option<bool>),
                    via: String,
                    here: &str,
                    written: String,
                    level: String,
                    nature: String,
                    (from_written, from_holds): (String, Option<bool>),
                    was: bool| {
        let id = if via.is_empty() {
            format!("{from} {how} {to}")
        } else {
            format!("{from} {how} {to} via {via}")
        };
        // each end said as its parts - where it is held, its table, its id - so that a lens joins on them
        // and nothing takes a written link apart again
        let ends = |node: &str| -> (String, String, String) {
            let (held, id) = node.split_once(':').unwrap_or((node, ""));
            let (place, table) = held.rsplit_once('/').unwrap_or(("", held));
            (place.to_string(), table.to_string(), id.to_string())
        };
        let ((fp, ft, fi), (tp, tt, ti), (vp, _, vi)) = (ends(&from), ends(&to), ends(&via));
        edges.insert(
            id.clone(),
            json!({"id": id, "from": from, "how": how, "to": to, "holds": holds, "via": via, "star": short(here),
                   "from_place": fp, "from_star": short(&fp), "from_table": ft, "from_id": fi,
                   "to_place": tp, "to_star": short(&tp), "to_table": tt, "to_id": ti,
                   "via_star": short(&vp), "via_id": vi,
                   "written": written, "from_written": from_written, "from_holds": from_holds,
                   "level": level, "nature": nature, "of_what_was": was}),
        );
    };
    for (here, held) in places
        .iter()
        .filter(|(here, _)| only.is_none_or(|o| o == here.as_str()))
    {
        for relation in &declared {
            let Some((table, field)) = relation.from.split_once('.') else {
                continue;
            };
            // a connection is not a row pointing at two things: it says how one bears on the other
            if table == "connections" {
                continue;
            }
            for row in held.tables.get(table).into_iter().flatten() {
                let (Some(value), Some(id)) = (row[field].as_str(), row["id"].as_str()) else {
                    continue;
                };
                // a value that names, whole, a row this place has is one pointing - a path with spaces
                // in it is one name; only otherwise is it several, parted by spaces or commas
                let whole = value.trim();
                let any = relation.to.iter().any(|t| t == "*");
                let one = whole.contains(char::is_whitespace)
                    && held
                        .tables
                        .keys()
                        .any(|t| (any || relation.to.contains(t)) && held.has(t, whole));
                let found = if one {
                    vec![Link {
                        star: None,
                        table: None,
                        id: whole.to_string(),
                    }]
                } else {
                    links(value)
                };
                for link in found.iter().filter(|l| {
                    !relation
                        .passes_over
                        .iter()
                        .any(|m| l.id.contains(m.as_str()))
                }) {
                    // where the relation says a link carries a level, what follows its mark is the level, not the id
                    let (row, level) = match relation
                        .level
                        .as_deref()
                        .and_then(|m| link.id.split_once(m))
                    {
                        Some((id, level)) => (
                            Link {
                                id: id.to_string(),
                                ..link.clone()
                            },
                            level.to_string(),
                        ),
                        None => (link.clone(), String::new()),
                    };
                    // a level said of no kind of row (tests@a-device) names no row: it is a condition, not a link
                    if !level.is_empty() && row.table.is_none() {
                        continue;
                    }
                    edge(
                        format!("{here}/{table}:{id}"),
                        field,
                        resolve(here, &row, &relation.to),
                        String::new(),
                        here,
                        written(link),
                        level,
                        relation.nature.clone().unwrap_or_else(|| field.to_string()),
                        (String::new(), Some(true)),
                        relation.of_what_was,
                    );
                }
            }
        }
        // what a place gathers, as its mark says: a constellation its stars, an observatory what it looks at
        for pattern in &held.gathers {
            let gathered = |other: &&String| match pattern.strip_suffix("/*") {
                Some(owner) => other.starts_with(&format!("{owner}/")),
                None => *other == pattern,
            };
            for other in places
                .keys()
                .filter(|other| *other != here)
                .filter(gathered)
            {
                edge(
                    format!("{here}/places:{}", short(here)),
                    "gathers",
                    (format!("{other}/places:{}", short(other)), Some(true)),
                    String::new(),
                    here,
                    other.clone(),
                    String::new(),
                    "gathers".to_string(),
                    (String::new(), Some(true)),
                    false,
                );
            }
        }
        let anywhere = ["*".to_string()];
        for row in held.tables.get("connections").into_iter().flatten() {
            let (Some(from), Some(to), Some(how), Some(id)) = (
                row["from"].as_str(),
                row["to"].as_str(),
                row["how"].as_str(),
                row["id"].as_str(),
            ) else {
                continue;
            };
            for this in links(from) {
                for that in links(to) {
                    let (near, near_holds) = resolve(here, &this, &anywhere);
                    edge(
                        near,
                        how,
                        resolve(here, &that, &anywhere),
                        format!("{here}/connections:{id}"),
                        here,
                        written(&that),
                        String::new(),
                        how.to_string(),
                        (written(&this), near_holds),
                        false,
                    );
                }
            }
        }
    }
    // the language itself is in the graph: every lens in view and what it is made of - its tables and
    // their fields, its entries, its relations, its parts - are things, and how they go together are
    // links: a part is part of its lens, rests on the tables and shapes it asks of, and places another
    // lens; an entry is for the table it fills; a lens rests on what it reads. So one stands on a lens
    // as on anything, and walks from what is laid out back to all it was made of.
    stretch("the links");
    if only.is_none() {
        type Row = serde_json::Map<String, serde_json::Value>;
        let language = crate::lenses::as_rows(dirs).unwrap_or_default();
        let said = |row: &Row, key: &str| {
            row.get(key)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string()
        };
        // where a lens is kept: the place of that star among those in view, or the program itself
        let kept_in = |star: &str| {
            places
                .keys()
                .find(|id| short(id) == star)
                .cloned()
                .unwrap_or_else(|| star.to_string())
        };
        let named = |table: &str, row: &Row| {
            format!(
                "{}/{table}:{}",
                kept_in(&said(row, "star")),
                said(row, "id")
            )
        };
        let known: BTreeSet<String> = things
            .iter()
            .filter_map(|t| t["id"].as_str().map(str::to_string))
            .collect();
        for (table, rows) in &language {
            for row in rows {
                let id = named(table, row);
                if known.contains(&id) {
                    continue;
                }
                let says = match *table {
                    "lens_relations" => format!("{} - {}", said(row, "from"), said(row, "to")),
                    _ => in_a_few_words(&serde_json::Value::Object(row.clone())),
                };
                things.push(json!({"id": id, "of_table": table, "place": kept_in(&said(row, "star")), "star": said(row, "star"), "says": says}));
            }
        }
        let of_kind = |table: &str| language.get(table).cloned().unwrap_or_default();
        // the thing a lens is, as the row that names it means it - a lens is of its place: the one of that
        // name its own place keeps; else the one lens of that name in view. Where several places keep one
        // and its own keeps none, the name alone is no one lens. And the thing a table is, by the lens that
        // defines it
        let lens_thing = |lens: &str, star: &str| {
            let lenses = of_kind("lenses");
            let called: Vec<&Row> = lenses.iter().filter(|l| said(l, "id") == lens).collect();
            called
                .iter()
                .find(|l| said(l, "star") == star)
                .or_else(|| called.first().filter(|_| called.len() == 1))
                .map(|l| named("lenses", l))
        };
        let table_thing = |table: &str| {
            of_kind("lens_tables")
                .iter()
                .find(|t| said(t, "name") == table)
                .map(|t| named("lens_tables", t))
        };
        let mut link = |from: String, how: &str, to: Option<String>, nature: &str| {
            if let Some(to) = to {
                let here = from
                    .split_once(':')
                    .and_then(|(left, _)| left.rsplit_once('/'))
                    .map(|(place, _)| place.to_string())
                    .unwrap_or_default();
                edge(
                    from,
                    how,
                    (to.clone(), Some(true)),
                    String::new(),
                    &here,
                    to,
                    String::new(),
                    nature.to_string(),
                    (String::new(), Some(true)),
                    false,
                );
            }
        };
        for kind in [
            "lens_tables",
            "lens_entries",
            "lens_relations",
            "lens_parts",
        ] {
            for row in of_kind(kind) {
                link(
                    named(kind, &row),
                    "of",
                    lens_thing(&said(&row, "lens"), &said(&row, "star")),
                    "is part of",
                );
            }
        }
        for field in of_kind("lens_fields") {
            let table = of_kind("lens_tables")
                .iter()
                .find(|t| {
                    said(t, "lens") == said(&field, "lens")
                        && said(t, "name") == said(&field, "of_table")
                })
                .map(|t| named("lens_tables", t));
            link(named("lens_fields", &field), "of", table, "is part of");
        }
        for entry in of_kind("lens_entries") {
            link(
                named("lens_entries", &entry),
                "fills",
                table_thing(&said(&entry, "fills")),
                "is for",
            );
        }
        for lens in of_kind("lenses") {
            for read in said(&lens, "reads")
                .split(',')
                .map(str::trim)
                .filter(|r| !r.is_empty())
            {
                link(
                    named("lenses", &lens),
                    "reads",
                    table_thing(read),
                    "rests on",
                );
            }
        }
        let parts = of_kind("lens_parts");
        for part in &parts {
            let from = named("lens_parts", part);
            let placed = said(part, "places");
            if !placed.is_empty() && placed != "*" {
                link(
                    from.clone(),
                    "places",
                    lens_thing(&placed, &said(part, "star")),
                    "places",
                );
            }
            // and the lens it is seen through: the lens behind what it places
            let behind = said(part, "through");
            if !behind.is_empty() {
                link(
                    from.clone(),
                    "through",
                    lens_thing(&behind, &said(part, "star")),
                    "is seen through",
                );
            }
            let query = said(part, "query");
            for table in of_kind("lens_tables") {
                if crate::focus::names_table(&query, &said(&table, "name")) {
                    link(
                        from.clone(),
                        "asks of",
                        Some(named("lens_tables", &table)),
                        "rests on",
                    );
                }
            }
            // a shape another part names
            for shape in parts
                .iter()
                .filter(|p| !said(p, "named").is_empty() && said(p, "id") != said(part, "id"))
            {
                if crate::focus::names_table(&query, &said(shape, "named")) {
                    link(
                        from.clone(),
                        "asks of",
                        Some(named("lens_parts", shape)),
                        "rests on",
                    );
                }
            }
        }
    }
    stretch("the language, as things and links");
    let rows_of = |values: Vec<serde_json::Value>| {
        values
            .into_iter()
            .filter_map(|v| v.as_object().cloned())
            .collect::<Vec<_>>()
    };
    let made = BTreeMap::from([
        ("things", rows_of(things)),
        ("edges", rows_of(edges.into_values().collect())),
    ]);
    stretch("laid out as rows");
    // the timing ends here: what is read afterwards is not the graph's making
    STRETCHES.with(|kept| kept.borrow_mut().0 = None);
    Ok(made)
}

/// Runs `write` on the place at `dir`, refusing what it wrote if it adds a link that points nowhere: the
/// place's tables are put back as they were, and the new broken links are named. Links already broken
/// stay as they are - they are the check's to report, not the write's.
pub fn guarded<T>(dir: &Path, write: impl FnOnce() -> Result<T>) -> Result<T> {
    let mark =
        Mark::parse(&std::fs::read_to_string(dir.join("oco.toml")).context("no oco.toml here")?)?;
    let before: BTreeSet<String> = broken(dir, &BTreeMap::new())?.into_iter().collect();
    let saved: Vec<(PathBuf, Option<String>)> = mark
        .exports
        .values()
        .map(|p| {
            let path = dir.join(p.split('@').next().unwrap_or(p));
            let text = std::fs::read_to_string(&path).ok();
            (path, text)
        })
        .collect();
    let done = write()?;
    let added: Vec<String> = broken(dir, &BTreeMap::new())?
        .into_iter()
        .filter(|p| !before.contains(p))
        .collect();
    if !added.is_empty() {
        for (path, text) in saved {
            match text {
                Some(text) => std::fs::write(&path, text)?,
                None => {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
        bail!(
            "nothing written - the change above was put back: it would add a link that points nowhere\n  {}",
            added.join("\n  ")
        );
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_part_is_said_by_its_heading_else_by_what_it_is_else_by_what_is_written_above_it() {
        let part = |more: serde_json::Value| {
            let mut row = serde_json::json!({"id": "l/1", "lens": "l", "part": 1, "says": "Written above it."});
            if let (Some(row), Some(more)) = (row.as_object_mut(), more.as_object()) {
                row.extend(more.clone());
            }
            in_a_few_words(&row)
        };
        assert_eq!(
            part(serde_json::json!({"title": "a heading ({count})"})),
            "a heading ({count})"
        );
        assert_eq!(part(serde_json::json!({"named": "kept"})), "the shape kept");
        assert_eq!(part(serde_json::json!({"places": "signs"})), "places signs");
        assert_eq!(part(serde_json::json!({})), "Written above it.");
        // and anything else by what it says, as before
        assert_eq!(
            in_a_few_words(&serde_json::json!({"id": "x", "says": "A lens."})),
            "A lens."
        );
    }

    #[test]
    fn a_thing_is_told_as_its_table_says_its_rows_are_told() {
        let d = std::env::temp_dir().join(format!("ocoeco-told-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("oco")).unwrap();
        std::fs::create_dir_all(d.join("lenses")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nfiles_kept = \"oco/files_kept.json\"\nchores_kept = \"oco/chores_kept.json\"\nnotes_kept = \"oco/notes_kept.json\"\n").unwrap();
        std::fs::write(d.join("lenses/kept.toml"), "[lens]\nid = \"kept\"\nsays = \"Files, chores and notes.\"\n[tables.files_kept]\nkey = [\"id\"]\ntells = \"{lines} lines\"\nfields = [{ name = \"id\", type = \"string\", required = true }, { name = \"lines\", type = \"integer\" }]\n[tables.chores_kept]\nkey = [\"id\"]\ntells = \"action\"\nfields = [{ name = \"id\", type = \"string\", required = true }, { name = \"action\", type = \"string\" }]\n[tables.notes_kept]\nkey = [\"id\"]\nfields = [{ name = \"id\", type = \"string\", required = true }, { name = \"what\", type = \"string\" }]\n").unwrap();
        std::fs::write(
            d.join("oco/files_kept.json"),
            "{\"id\":\"README.md\",\"lines\":5}\n",
        )
        .unwrap();
        std::fs::write(
            d.join("oco/chores_kept.json"),
            "{\"id\":\"c1\",\"action\":\"patch source by hand\"}\n",
        )
        .unwrap();
        std::fs::write(
            d.join("oco/notes_kept.json"),
            "{\"id\":\"n1\",\"what\":\"a note\"}\n",
        )
        .unwrap();
        let graph = as_graph(std::slice::from_ref(&d)).unwrap();
        let says = |id: &str| -> String {
            graph["things"]
                .iter()
                .find(|t| t["id"] == id)
                .and_then(|t| t["says"].as_str())
                .unwrap_or("not there")
                .to_string()
        };
        // laid out as a line is, or by one field it names
        assert_eq!(says("o/r/files_kept:README.md"), "5 lines");
        assert_eq!(says("o/r/chores_kept:c1"), "patch source by hand");
        // and where its table says nothing of how its rows are told: by the words the program's list finds
        assert_eq!(says("o/r/notes_kept:n1"), "a note");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_link_said_of_what_was_points_at_what_was_and_keeps_nothing_from_being_taken_back() {
        let d = std::env::temp_dir().join(format!("ocoeco-what-was-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("oco")).unwrap();
        std::fs::create_dir_all(d.join("lenses")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nmemos = \"oco/memos.json\"\nnotes = \"oco/notes.json\"\n").unwrap();
        std::fs::write(d.join("lenses/memos.toml"), "[lens]\nid = \"memos\"\nsays = \"Memos, and what each touched.\"\n[tables.memos]\nkey = [\"id\"]\nfields = [{ name = \"id\", type = \"string\", required = true }, { name = \"touched\", type = \"string\" }]\n[tables.notes]\nkey = [\"id\"]\nfields = [{ name = \"id\", type = \"string\", required = true }]\n[[relation]]\nfrom = \"memos.touched\"\nto = [\"*\"]\nof_what_was = true\n").unwrap();
        std::fs::write(
            d.join("oco/notes.json"),
            "{\"id\":\"kept\"}\n{\"id\":\"left\"}\n",
        )
        .unwrap();
        std::fs::write(
            d.join("oco/memos.json"),
            "{\"id\":\"m1\",\"touched\":\"notes:kept notes:gone\"}\n{\"id\":\"m2\",\"touched\":\"notes:left\"}\n",
        )
        .unwrap();
        let graph = as_graph(std::slice::from_ref(&d)).unwrap();
        let to = |id: &str| -> (Option<bool>, Option<bool>) {
            graph["edges"]
                .iter()
                .find(|e| e["to"] == format!("o/r/notes:{id}"))
                .map(|e| (e["holds"].as_bool(), e["of_what_was"].as_bool()))
                .unwrap_or_default()
        };
        assert_eq!(to("kept"), (Some(true), Some(true)));
        assert_eq!(to("gone"), (Some(false), Some(true)));
        // what was taken back since is no link pointing nowhere
        assert!(
            broken(&d, &BTreeMap::new()).unwrap().is_empty(),
            "{:?}",
            broken(&d, &BTreeMap::new())
        );
        // and a link said of what was keeps nothing from being taken back
        crate::note::take_back(&d, "notes", "left").unwrap();
        assert!(!std::fs::read_to_string(d.join("oco/notes.json"))
            .unwrap()
            .contains("left"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_field_holds_links_in_three_forms() {
        assert_eq!(
            links("crash-only, kubernetes-jobs"),
            vec![
                Link {
                    star: None,
                    table: None,
                    id: "crash-only".into()
                },
                Link {
                    star: None,
                    table: None,
                    id: "kubernetes-jobs".into()
                }
            ]
        );
        assert_eq!(
            links("backlog:a  decisions:2026-11-12-b")[1],
            Link {
                star: None,
                table: Some("decisions".into()),
                id: "2026-11-12-b".into()
            }
        );
        assert_eq!(
            links("creation-guidelines/lab-agent/decisions:2026-11-12-the-switch")[0],
            Link {
                star: Some("creation-guidelines/lab-agent".into()),
                table: Some("decisions".into()),
                id: "2026-11-12-the-switch".into()
            }
        );
    }

    #[test]
    fn every_declared_relation_names_a_table_and_a_field() {
        let all = relations();
        assert!(all.len() >= 12);
        assert!(all
            .iter()
            .all(|r| r.from.split_once('.').is_some() && !r.to.is_empty()));
    }

    #[test]
    fn a_write_that_adds_a_link_to_nowhere_is_put_back() {
        let d = std::env::temp_dir().join(format!("ocoeco-guarded-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\ndecisions = \"data/decisions.json\"\n").unwrap();
        let was = "{\"id\":\"d1\"}\n{\"id\":\"old\",\"refines\":\"long-gone\"}\n";
        std::fs::write(d.join("data/decisions.json"), was).unwrap();
        let add = |line: &'static str| {
            let file = d.join("data/decisions.json");
            move || -> Result<()> {
                let mut text = std::fs::read_to_string(&file)?;
                text.push_str(line);
                std::fs::write(&file, text)?;
                Ok(())
            }
        };
        let refused = guarded(&d, add("{\"id\":\"d2\",\"refines\":\"d9\"}\n"))
            .unwrap_err()
            .to_string();
        assert!(
            refused.contains("nothing written")
                && refused.contains("d9")
                && !refused.contains("long-gone"),
            "{refused}"
        );
        assert_eq!(
            std::fs::read_to_string(d.join("data/decisions.json")).unwrap(),
            was,
            "put back as it was"
        );
        guarded(&d, add("{\"id\":\"d3\",\"refines\":\"d1\"}\n")).unwrap();
        assert!(std::fs::read_to_string(d.join("data/decisions.json"))
            .unwrap()
            .contains("d3"));
    }

    #[test]
    fn a_link_that_points_nowhere_is_found_and_one_that_holds_is_not() {
        let d = std::env::temp_dir().join(format!("ocoeco-relations-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\ndecisions = \"data/decisions.json\"\nsources = \"data/sources.json\"\n").unwrap();
        std::fs::write(d.join("data/sources.json"), "{\"id\":\"known\"}\n").unwrap();
        std::fs::write(d.join("data/decisions.json"), "{\"id\":\"d1\",\"sources\":\"known, unknown\"}\n{\"id\":\"d2\",\"refines\":\"d1\"}\n{\"id\":\"d3\",\"refines\":\"d9\"}\n").unwrap();
        let problems = broken(&d, &BTreeMap::new()).unwrap();
        assert_eq!(
            problems,
            vec![
                "decisions d1: sources -> unknown (in sources) - points nowhere",
                "decisions d3: refines -> d9 (in decisions) - points nowhere"
            ]
        );
    }
}

#[cfg(test)]
mod the_graph {
    use super::*;

    fn place(base: &Path, id: &str, tables: &[(&str, &str)]) -> PathBuf {
        let d = base.join(id.replace('/', "-"));
        std::fs::create_dir_all(d.join("data")).unwrap();
        let exports: String = tables
            .iter()
            .map(|(t, _)| format!("{t} = \"data/{t}.json\"\n"))
            .collect();
        std::fs::write(d.join("oco.toml"), format!("schema = 1\n[project]\nid = \"{id}\"\nname = \"P: a place\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\n{exports}")).unwrap();
        for (t, rows) in tables {
            std::fs::write(d.join(format!("data/{t}.json")), rows).unwrap();
        }
        d
    }

    #[test]
    fn every_row_is_a_thing_and_every_link_an_edge_this_how_that() {
        let base = std::env::temp_dir().join(format!("ocoeco-graph-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let a = place(&base, "o/a", &[
            ("backlog", "{\"id\":\"a1\",\"serves\":\"milestones:m1\"}\n"),
            ("milestones", "{\"id\":\"m1\"}\n{\"id\":\"m2\"}\n"),
            ("connections", "{\"id\":\"c1\",\"from\":\"milestones:m1\",\"how\":\"led to\",\"to\":\"o/b/decisions:x\"}\n"),
        ]);
        let b = place(&base, "o/b", &[("decisions", "{\"id\":\"x\"}\n")]);
        let graph = as_graph(&[a.clone(), b]).unwrap();
        let edge = |from: &str, how: &str| {
            graph["edges"]
                .iter()
                .find(|e| e["from"] == from && e["how"] == how)
                .cloned()
        };
        let served = edge("o/a/backlog:a1", "serves")
            .expect("a link a row holds is an edge named by its field");
        assert_eq!(
            served["nature"].as_str(),
            Some("is for"),
            "and says the nature of its pointing, where its lens says one"
        );
        assert_eq!(
            (served["to"].as_str(), served["holds"].as_bool()),
            (Some("o/a/milestones:m1"), Some(true))
        );
        let led = edge("o/a/milestones:m1", "led to")
            .expect("a connection is an edge named by how it bears");
        assert_eq!(
            (
                led["to"].as_str(),
                led["holds"].as_bool(),
                led["via"].as_str()
            ),
            (
                Some("o/b/decisions:x"),
                Some(true),
                Some("o/a/connections:c1")
            )
        );
        let star = graph["things"]
            .iter()
            .find(|t| t["of_table"] == "places" && t["place"] == "o/a")
            .expect("a place is a thing");
        assert_eq!(
            (star["id"].as_str(), star["says"].as_str()),
            (Some("o/a/places:a"), Some("P: a place")),
            "and says what it is for"
        );
        assert_eq!(
            graph["things"]
                .iter()
                .filter(|t| t["place"].as_str().is_some_and(|p| p.starts_with("o/")))
                .count(),
            7,
            "every row of every place is a thing, and so is each place"
        );
        // and the language is in the graph: a lens is a thing, and a part of it rests on the table it asks of
        let of_the_language = |how: &str, from: &str| {
            graph["edges"]
                .iter()
                .find(|e| e["how"] == how && e["from"] == from)
                .map(|e| {
                    (
                        e["to"].as_str().unwrap_or_default().to_string(),
                        e["nature"].as_str().unwrap_or_default().to_string(),
                    )
                })
        };
        assert!(graph["things"]
            .iter()
            .any(|t| t["id"] == "ocoeco/lenses:work" && t["of_table"] == "lenses"));
        assert_eq!(
            of_the_language("of", "ocoeco/lens_tables:work/backlog"),
            Some(("ocoeco/lenses:work".to_string(), "is part of".to_string()))
        );
        assert!(graph["edges"].iter().any(|e| e["how"] == "asks of"
            && e["nature"] == "rests on"
            && e["from"]
                .as_str()
                .is_some_and(|f| f.starts_with("ocoeco/lens_parts:work/"))
            && e["to"] == "ocoeco/lens_tables:work/backlog"));
        let pointed: BTreeSet<&str> = graph["edges"]
            .iter()
            .filter_map(|e| e["to"].as_str())
            .collect();
        assert!(
            !pointed.contains("o/a/milestones:m2"),
            "and some are pointed at by nothing"
        );
        // from one place alone, a link into another is neither held nor broken: it is not seen
        let alone = as_graph(&[a]).unwrap();
        let led = alone["edges"]
            .iter()
            .find(|e| e["how"] == "led to")
            .unwrap();
        assert!(led["holds"].is_null(), "{led:?}");
        // each end is said as its parts, and what an edge carries is what the lens of the graph declares
        let served = graph["edges"]
            .iter()
            .find(|e| e["how"] == "serves")
            .unwrap();
        assert_eq!(
            [
                &served["from_star"],
                &served["from_table"],
                &served["from_id"],
                &served["to_star"],
                &served["to_table"],
                &served["to_id"]
            ]
            .map(|v| v.as_str().unwrap_or_default()),
            ["a", "backlog", "a1", "a", "milestones", "m1"]
        );
        let lens = crate::lenses::lenses()
            .into_iter()
            .find(|l| l.lens.id == "graph")
            .unwrap();
        let declared: BTreeSet<String> = lens.tables["edges"]["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["name"].as_str().unwrap().to_string())
            .chain(["star".to_string()])
            .collect();
        for e in &graph["edges"] {
            assert_eq!(
                e.keys().cloned().collect::<BTreeSet<_>>(),
                declared,
                "an edge carries what the lens declares"
            );
        }
    }
}

#[cfg(test)]
mod the_top {
    use super::*;

    #[test]
    fn what_a_place_gathers_is_drawn_from_it_so_a_walk_reaches_the_whole() {
        let base = std::env::temp_dir().join(format!("ocoeco-top-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let mark = |id: &str, place: &str| {
            format!("schema = 1\n[project]\nid = \"{id}\"\nname = \"N: what {id} is for\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\n{place}")
        };
        for (id, place) in [
            (
                "o/whole",
                "[place]\nis = \"constellation\"\ngathers = [\"o/star\", \"p/*\"]\n",
            ),
            ("o/star", ""),
            ("p/other", ""),
            ("q/apart", ""),
        ] {
            let d = base.join(id);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("oco.toml"), mark(id, place)).unwrap();
        }
        let dirs: Vec<PathBuf> = ["o/whole", "o/star", "p/other", "q/apart"]
            .iter()
            .map(|id| base.join(id))
            .collect();
        let graph = as_graph(&dirs).unwrap();
        let gathered: BTreeSet<&str> = graph["edges"]
            .iter()
            .filter(|e| e["how"] == "gathers" && e["from"] == "o/whole/places:whole")
            .filter_map(|e| e["to"].as_str())
            .collect();
        assert_eq!(gathered, BTreeSet::from(["o/star/places:star", "p/other/places:other"]), "the one it names, and every one under the owner it names - not what it does not gather");
    }
}

#[cfg(test)]
mod by_any_route {
    use super::*;

    #[test]
    fn the_graph_is_made_of_what_was_observed_as_of_what_is_held() {
        let base = std::env::temp_dir().join(format!("ocoeco-any-route-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        // what an observatory observed of two places: each row names the place it is of
        std::fs::write(base.join("projects.json"), "{\"id\":\"o/a\",\"name\":\"A: what a is for\",\"also\":{\"feature\":\"features\"},\"gathers\":[\"o/b\"]}\n{\"id\":\"o/b\",\"name\":\"B: what b is for\"}\n").unwrap();
        std::fs::write(
            base.join("backlog.json"),
            "{\"id\":\"a1\",\"project\":\"o/a\",\"serves\":\"o/b/decisions:x\"}\n",
        )
        .unwrap();
        std::fs::write(
            base.join("decisions.json"),
            "{\"id\":\"x\",\"project\":\"o/b\",\"decision\":\"Decided there\"}\n",
        )
        .unwrap();
        std::fs::write(
            base.join("features.json"),
            "{\"id\":\"calls\",\"project\":\"o/a\"}\n",
        )
        .unwrap();
        std::fs::write(
            base.join("milestones.json"),
            "{\"id\":\"m\",\"project\":\"o/a\",\"criteria\":\"feature:calls@best\"}\n",
        )
        .unwrap();
        let graph = as_graph(&[base]).unwrap();
        let edge = |how: &str| {
            graph["edges"]
                .iter()
                .find(|e| e["how"] == how)
                .cloned()
                .unwrap_or_else(|| panic!("no edge {how}"))
        };
        assert_eq!(
            (
                edge("serves")["to"].as_str(),
                edge("serves")["holds"].as_bool()
            ),
            (Some("o/b/decisions:x"), Some(true)),
            "a link between two observed places holds"
        );
        assert_eq!(
            (
                edge("criteria")["to"].as_str(),
                edge("criteria")["holds"].as_bool()
            ),
            (Some("o/a/features:calls"), Some(true)),
            "by the other name the place says its table goes by"
        );
        assert_eq!(
            edge("gathers")["to"].as_str(),
            Some("o/b/places:b"),
            "and what a place gathers is drawn"
        );
        assert!(graph["things"]
            .iter()
            .any(|t| t["id"] == "o/b/decisions:x" && t["says"] == "Decided there"));
    }
}

#[cfg(test)]
mod whoever_declares_it {
    use super::*;

    #[test]
    fn a_relation_a_place_s_lens_declares_is_followed_as_one_built_in() {
        let d = std::env::temp_dir()
            .join(format!("ocoeco-brought-relation-{}", std::process::id()))
            .join("o/talk");
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::create_dir_all(d.join("lenses")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/talk\"\nname = \"T: a talk\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nturns = \"data/turns.json\"\n").unwrap();
        std::fs::write(
            d.join("data/turns.json"),
            "{\"id\":\"t1\"}\n{\"id\":\"t2\",\"after\":\"t1\"}\n",
        )
        .unwrap();
        std::fs::write(d.join("lenses/talk.toml"), "[lens]\nid = \"talk\"\nsays = \"What was said.\"\n[tables.turns]\nkey = [\"id\"]\nextra_fields = true\nfields = [{ name = \"id\", type = \"string\" }, { name = \"after\", type = \"string\" }]\n[[relation]]\nfrom = \"turns.after\"\nto = [\"turns\"]\nnature = \"follows\"\n").unwrap();
        let graph = as_graph(std::slice::from_ref(&d)).unwrap();
        let edge = graph["edges"]
            .iter()
            .find(|e| e["how"] == "after")
            .expect("the relation the place's lens declares");
        assert_eq!(
            (
                edge["from"].as_str(),
                edge["to"].as_str(),
                edge["nature"].as_str(),
                edge["holds"].as_bool()
            ),
            (
                Some("o/talk/turns:t2"),
                Some("o/talk/turns:t1"),
                Some("follows"),
                Some(true)
            )
        );
    }
}

#[cfg(test)]
mod once_in_a_look {
    use super::*;

    #[test]
    fn within_one_look_the_graph_is_made_once_and_after_it_what_changed_is_seen() {
        let d = std::env::temp_dir()
            .join(format!("ocoeco-one-look-{}", std::process::id()))
            .join("o/p");
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/p\"\nname = \"P: a place\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nnotes = \"data/notes.json\"\n").unwrap();
        std::fs::write(d.join("data/notes.json"), "{\"id\":\"a\"}\n").unwrap();
        let notes = |dirs: &[PathBuf]| {
            as_graph(dirs).unwrap()["things"]
                .iter()
                .filter(|t| t["of_table"] == "notes")
                .count()
        };
        let dirs = [d.clone()];
        {
            let _look = one_look();
            assert_eq!(notes(&dirs), 1);
            std::fs::write(
                d.join("data/notes.json"),
                "{\"id\":\"a\"}\n{\"id\":\"b\"}\n",
            )
            .unwrap();
            let _within = one_look();
            assert_eq!(
                notes(&dirs),
                1,
                "within one look - and a look within it - the graph is the one made"
            );
        }
        assert_eq!(notes(&dirs), 2, "after the look, what changed is seen");
        assert_eq!(
            notes(&dirs),
            2,
            "and where no look is under way, nothing is kept"
        );
    }
}

#[cfg(test)]
mod a_thing_says_when {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_row_says_when_by_what_dates_it_or_its_moment_or_its_day() {
        // the field its table says dates it comes first; then its moment; then its day
        assert_eq!(
            when(
                &json!({"opened": "2026-01-02", "at": "2026-03-04T05:06:07Z"}),
                Some("opened")
            )
            .as_deref(),
            Some("2026-01-02")
        );
        assert_eq!(
            when(
                &json!({"at": "2026-03-04T05:06:07Z", "date": "2026-01-01"}),
                None
            )
            .as_deref(),
            Some("2026-03-04T05:06:07Z")
        );
        assert_eq!(
            when(&json!({"date": "2026-01-01"}), None).as_deref(),
            Some("2026-01-01")
        );
        // a moment kept as seconds is said as a moment, so that all of them sort as one
        assert_eq!(
            when(&json!({"at": 1_700_000_000}), None).as_deref(),
            Some("2023-11-14T22:13:20Z")
        );
        // what is not shaped like a day or a moment is not when: a field may be called at and say where
        assert_eq!(when(&json!({"at": "the lab, and GitHub"}), None), None);
        assert_eq!(when(&json!({"at": 7}), None), None);
        assert_eq!(
            when(&json!({"title": "no word of when"}), Some("opened")),
            None
        );
    }

    #[test]
    fn the_things_of_a_place_say_when_and_so_can_be_told_in_order() {
        let d = std::env::temp_dir()
            .join(format!("ocoeco-when-{}", std::process::id()))
            .join("o/p");
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/p\"\nname = \"P: a place\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nbacklog = \"data/backlog.json\"\nglimpses = \"data/glimpses.json\"\n").unwrap();
        std::fs::write(
            d.join("data/backlog.json"),
            "{\"id\":\"a\",\"title\":\"A\",\"status\":\"next\",\"opened\":\"2026-01-02\"}\n",
        )
        .unwrap();
        std::fs::write(d.join("data/glimpses.json"), "{\"id\":\"g1\",\"at\":\"2026-01-03T10:00:00Z\",\"summary\":\"did it\",\"kind\":\"feat\",\"by\":\"x\"}\n").unwrap();
        let graph = as_graph(std::slice::from_ref(&d)).unwrap();
        let at = |id: &str| {
            graph["things"]
                .iter()
                .find(|t| t["id"] == id)
                .unwrap()
                .get("at")
                .cloned()
                .unwrap_or(serde_json::Value::Null)
        };
        assert_eq!(
            at("o/p/backlog:a"),
            "2026-01-02",
            "an item, by when it was opened - what its table says dates it"
        );
        assert_eq!(
            at("o/p/glimpses:g1"),
            "2026-01-03T10:00:00Z",
            "a record of work, by its moment"
        );
        assert!(at("o/p/places:p").is_null(), "a place says nothing of when");
    }
}

#[cfg(test)]
mod a_part_by_its_name {
    use super::*;

    #[test]
    fn a_star_written_by_its_shorter_name_is_the_place_that_goes_by_it() {
        let base = std::env::temp_dir().join(format!("ocoeco-short-star-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let place = |star: &str, rows: &str, with_a_lens: bool| {
            let d = base.join(star);
            std::fs::create_dir_all(d.join("data")).unwrap();
            std::fs::create_dir_all(d.join("lenses")).unwrap();
            std::fs::write(d.join("oco.toml"), format!("schema = 1\n[project]\nid = \"o/{star}\"\nname = \"{star}: a star\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nnotes = \"data/notes.json\"\n")).unwrap();
            std::fs::write(d.join("data/notes.json"), rows).unwrap();
            if with_a_lens {
                std::fs::write(d.join("lenses/notes.toml"), "[lens]\nid = \"notes\"\nsays = \"Notes, and what each is about.\"\n[tables.notes]\nkey = [\"id\"]\nfields = [{ name = \"id\", type = \"string\", required = true }, { name = \"about\", type = \"string\" }]\n[[relation]]\nfrom = \"notes.about\"\nto = [\"notes\"]\n[[focus]]\ntitle = \"notes ({count})\"\nsql = \"SELECT id FROM notes ORDER BY id\"\nline = \"{id}\"\n").unwrap();
            }
            d
        };
        let a = place(
            "a",
            "{\"id\":\"p\",\"about\":\"b/notes:x\"}\n{\"id\":\"q\",\"about\":\"b/notes:none\"}\n{\"id\":\"r\",\"about\":\"zz/notes:x\"}\n",
            true,
        );
        let b = place("b", "{\"id\":\"x\"}\n", false);
        let graph = as_graph(&[a, b]).unwrap();
        let about = |from: &str| {
            graph["edges"]
                .iter()
                .find(|e| e.get("from").and_then(|v| v.as_str()) == Some(from))
                .map(|e| {
                    (
                        e.get("to")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        e.get("holds").and_then(|v| v.as_bool()),
                    )
                })
        };
        assert_eq!(
            about("o/a/notes:p"),
            Some(("o/b/notes:x".to_string(), Some(true))),
            "a star by its shorter name is the place that goes by it: the link holds, and ends at the thing"
        );
        assert_eq!(
            about("o/a/notes:q"),
            Some(("o/b/notes:none".to_string(), Some(false))),
            "and where the thing is not there, it points nowhere - told, not passed over"
        );
        assert_eq!(
            about("o/a/notes:r").map(|(_, holds)| holds),
            Some(None),
            "a star that is not in view is still not told"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_part_of_a_lens_kept_as_rows_is_known_by_the_id_it_is_kept_as() {
        let d = std::env::temp_dir()
            .join(format!("ocoeco-part-named-{}", std::process::id()))
            .join("o/a");
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/a\"\nname = \"A: a place\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nlenses = \"data/lenses.json\"\nlens_parts = \"data/lens_parts.json\"\nglimpses = \"data/glimpses.json\"\n").unwrap();
        std::fs::write(
            d.join("data/lenses.json"),
            "{\"id\":\"sheet\",\"says\":\"What goes together.\"}\n",
        )
        .unwrap();
        // two parts, kept as sheet/5 and sheet/9: what stood between them was taken back
        std::fs::write(d.join("data/lens_parts.json"), "{\"id\":\"sheet/5\",\"lens\":\"sheet\",\"part\":1,\"places\":\"work\"}\n{\"id\":\"sheet/9\",\"lens\":\"sheet\",\"part\":2,\"places\":\"signs\"}\n").unwrap();
        std::fs::write(d.join("data/glimpses.json"), "{\"id\":\"g1\",\"summary\":\"a part made\",\"touches\":\"lens_parts:sheet/5 lens_parts:sheet/2\"}\n").unwrap();
        let graph = as_graph(std::slice::from_ref(&d)).unwrap();
        let touched = |part: &str| {
            graph["edges"]
                .iter()
                .find(|e| {
                    e["from"] == "o/a/glimpses:g1" && e["to"] == format!("o/a/lens_parts:{part}")
                })
                .map(|e| e["holds"].as_bool())
        };
        assert_eq!(
            touched("sheet/5"),
            Some(Some(true)),
            "a part is found by the id it is kept as"
        );
        assert_eq!(
            touched("sheet/2"),
            Some(Some(false)),
            "and not by where it stands: the second part is sheet/9, and nothing is kept as sheet/2"
        );
        let parts: Vec<&str> = graph["things"]
            .iter()
            .filter(|t| t["of_table"] == "lens_parts" && t["place"] == "o/a")
            .filter_map(|t| t["id"].as_str())
            .collect();
        assert_eq!(
            parts,
            ["o/a/lens_parts:sheet/5", "o/a/lens_parts:sheet/9"],
            "each part is one thing, by one name"
        );
    }

    #[test]
    fn a_lens_s_rows_mean_the_lens_their_own_place_keeps() {
        let root =
            std::env::temp_dir().join(format!("ocoeco-lens-of-its-place-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let place = |id: &str| {
            let d = root.join(id);
            std::fs::create_dir_all(d.join("data")).unwrap();
            std::fs::write(d.join("oco.toml"), format!("schema = 1\n[project]\nid = \"{id}\"\nname = \"A place: {id}\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nlenses = \"data/lenses.json\"\nlens_parts = \"data/lens_parts.json\"\n")).unwrap();
            std::fs::write(
                d.join("data/lenses.json"),
                format!("{{\"id\":\"site\",\"says\":\"The pages of {id}.\"}}\n"),
            )
            .unwrap();
            std::fs::write(
                d.join("data/lens_parts.json"),
                "{\"id\":\"site/1\",\"lens\":\"site\",\"part\":1,\"places\":\"work\"}\n",
            )
            .unwrap();
            d
        };
        let graph = as_graph(&[place("o/a"), place("o/b")]).unwrap();
        let to = |from: &str, how: &str| -> Vec<String> {
            graph["edges"]
                .iter()
                .filter(|e| e["from"] == from && e["how"] == how)
                .filter_map(|e| e["to"].as_str().map(str::to_string))
                .collect()
        };
        // two places, each keeping a lens called site: each part is of its own place's lens
        assert_eq!(to("o/a/lens_parts:site/1", "of"), ["o/a/lenses:site"]);
        assert_eq!(to("o/b/lens_parts:site/1", "of"), ["o/b/lenses:site"]);
        // and a lens of a name only one place keeps is that one, from anywhere
        assert_eq!(to("o/b/lens_parts:site/1", "places").len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }
}
