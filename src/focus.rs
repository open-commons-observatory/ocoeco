//! Focus: a lens brings data into view. A lens's focus is declared as data (language/*.toml): parts, each
//! a query over the lens's tables and a line saying how one answer is laid out. The data of every place
//! asked for is gathered - each row with its star - and the queries are run by tad's SQL engine. Nothing
//! is made: the same data, brought into focus; and what is seen is laid out as plain text - the first
//! of the ways a focus may be laid out.

use crate::dome::Dome;
use crate::lenses::Lens;
use anyhow::{bail, Context, Result};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use tad::sql::SqlEngine;

/// Gathers the lens's tables from the places at `dirs` into `into`, each row with its star and seq -
/// its place in the order gathered - and returns the declaration of what was gathered.
/// The things there are, by name. A thing's whole name is where it is held - owner and place - its table
/// and its id; without the owner it is shorter, and is the name of one thing only where no two places
/// share a name. With the table left out too, a star and an id name a thing where they are one thing only.
#[derive(Default)]
pub struct Named {
    whole: std::collections::BTreeMap<String, String>,
    short: std::collections::BTreeMap<String, Option<String>>,
    by_star_and_id: std::collections::BTreeMap<(String, String), Option<String>>,
}

impl Named {
    /// The things of the places in view.
    pub fn of(dirs: &[PathBuf]) -> Result<Named> {
        let mut named = Named::default();
        let mut graph = crate::relations::as_graph(dirs)?;
        for thing in graph.remove("things").unwrap_or_default() {
            let said = |key: &str| thing.get(key).and_then(|v| v.as_str());
            let (Some(id), Some(star), Some(table), Some(place)) =
                (said("id"), said("star"), said("of_table"), said("place"))
            else {
                continue;
            };
            let Some(own) = id.strip_prefix(&format!("{place}/{table}:")) else {
                continue;
            };
            let name = format!("{star}/{table}:{own}");
            named
                .by_star_and_id
                .entry((star.to_string(), own.to_string()))
                .and_modify(|one| *one = None)
                .or_insert_with(|| Some(name.clone()));
            // the shorter name is the name of one thing, or of several - and then of none
            named
                .short
                .entry(name.clone())
                .and_modify(|one| *one = None)
                .or_insert_with(|| Some(id.to_string()));
            named.whole.insert(id.to_string(), name);
        }
        Ok(named)
    }

    /// A value as the name of a thing: its whole name, or its shorter one where that is one thing only.
    fn known(&self, value: &str) -> Option<String> {
        (self.whole.contains_key(value) || self.short.get(value).is_some_and(|one| one.is_some()))
            .then(|| value.to_string())
    }

    /// The thing an answer is about: the value it calls `thing`, or else the first of its values that is
    /// a thing's name - or its star and id, where they are one thing only. A name that is the name of
    /// several things is the name of none: a line about it leads nowhere.
    pub fn thing(&self, answer: &Map<String, Value>) -> Option<String> {
        let said = |key: &str| answer.get(key).and_then(|v| v.as_str());
        said("thing")
            .and_then(|thing| self.known(thing))
            .or_else(|| {
                answer
                    .values()
                    .filter_map(|v| v.as_str())
                    .find_map(|value| self.known(value))
            })
            .or_else(|| {
                self.by_star_and_id
                    .get(&(said("star")?.to_string(), said("id")?.to_string()))
                    .cloned()
                    .flatten()
                    .filter(|name| self.known(name).is_some())
            })
    }

    /// Every name a thing goes by that names it alone: its whole name, and its shorter one where no
    /// other thing goes by it. Given either; nothing where the name given is the name of several, or none.
    pub fn names_of(&self, name: &str) -> Vec<String> {
        let whole = match (self.whole.contains_key(name), self.short.get(name)) {
            (true, _) => name.to_string(),
            (false, Some(Some(whole))) => whole.clone(),
            _ => return Vec::new(),
        };
        let shorter = self
            .whole
            .get(&whole)
            .filter(|short| self.short.get(*short).is_some_and(|one| one.is_some()));
        std::iter::once(whole.clone())
            .chain(shorter.cloned())
            .collect()
    }
}

/// Whether the lens of that name says where one stands: it is looked through from a thing.
pub fn stands_somewhere(lens: &str, dirs: &[PathBuf]) -> bool {
    crate::lenses::with(dirs).is_ok_and(|all| all.iter().any(|l| l.lens.id == lens && says_here(l)))
}

thread_local! {
    /// The pages there are for things, while a site is laid out: each thing with where its page is.
    static PAGES: std::cell::RefCell<std::collections::BTreeMap<String, String>> = Default::default();
}

/// The tables that hold what was just given in the window, and what was marked in what was last shown:
/// made for a lens that says what an entry would keep. No place keeps them.
pub const WAS_GIVEN: &str = "given";
pub const WAS_MARKED: &str = "marked";

thread_local! {
    /// What was just given in the window, while a lens that reads it is looked through: each field
    /// named, and its words.
    static GIVEN: std::cell::RefCell<Vec<(String, String)>> = Default::default();
}

/// Says what was given in the window, for the lens that reads it - and, given nothing, that it is over.
pub fn given_is(given: Vec<(String, String)>) {
    GIVEN.with(|said| *said.borrow_mut() = given);
}

/// Who is looking, as a lens may say it - {who}: the driver, by the name the trail keeps their steps under.
fn whose() -> String {
    crate::trail::who().replace('\'', "''")
}

/// Says which things have a page, and where: a plane then says, for a line about such a thing, the page
/// to go to - so that a surface can lead there. Said by whoever lays a site out, before its pages.
pub fn pages_are(pages: std::collections::BTreeMap<String, String>) {
    PAGES.with(|said| *said.borrow_mut() = pages);
}

/// The table a surface reads: what another lens brings into focus, as rows.
pub const OF_A_PLANE: &str = "plane";

/// The table that holds the thing one stands on, in full: a row for each field it carries, as it is kept.
/// No place keeps it: it is made for a lens looked through from a thing, of that thing alone.
pub const IN_FULL: &str = "in_full";

/// Whether a lens is a surface: it reads a plane, and lays it out.
pub fn lays_out_a_plane(lens: &Lens) -> bool {
    lens.lens.reads.iter().any(|table| table == OF_A_PLANE) || lens.tables.contains_key(OF_A_PLANE)
}

/// What a lens brings into focus, as rows - a plane, in the order it is laid out: a lens placed within it,
/// by its name and what it says of itself, or why it brought nothing; a part's heading; and each line,
/// with how far along it is, its measure of a whole where it has one, and what it is about. A surface
/// said by a lens reads these, and lays them out its own way.
pub fn as_plane(parts: &[Brought], named: &Named) -> Vec<Map<String, Value>> {
    fn row(kind: &str, says: &str, depth: u64, urgent: bool) -> Map<String, Value> {
        let mut row = Map::new();
        row.insert("kind".into(), kind.into());
        row.insert("says".into(), says.into());
        row.insert("depth".into(), depth.into());
        row.insert("urgent".into(), urgent.into());
        row
    }
    fn rows(parts: &[Brought], depth: u64, into: &mut Vec<Map<String, Value>>, named: &Named) {
        for part in parts {
            if let Some(about) = &part.about {
                let mut placed = row("lens", about, depth, part.urgent);
                placed.insert(
                    "of_lens".into(),
                    part.title.clone().unwrap_or_default().into(),
                );
                placed.insert("of_every".into(), part.of_every.into());
                into.push(placed);
                match &part.nothing_because {
                    Some(why) => into.push(row("nothing", why, depth, part.urgent)),
                    None => rows(&part.within, depth + 1, into, named),
                }
                continue;
            }
            // each row says where it comes in its part, and how many lines the part has: so a surface
            // knows where a part begins and ends from the row itself
            let walk = crate::dome::walked(part);
            if let Some(title) = &part.title {
                let mut heading = row("heading", title, depth, part.urgent);
                heading.insert("nth".into(), 0u64.into());
                heading.insert("of_part".into(), (walk.len() as u64).into());
                // and what it is laid out once for each of, where it is one of several parts of one kind
                if let Some(per) = &part.per {
                    heading.insert("per".into(), per.clone().into());
                    // and the thing it is laid out for, where its value names one - the thing of that
                    // name, or the place of that star: so that a surface can lead there from the heading
                    let value = part
                        .values
                        .first()
                        .and_then(|answer| answer.get(per))
                        .and_then(|v| v.as_str());
                    if let Some(thing) = value.and_then(|v| {
                        named
                            .known(v)
                            .or_else(|| named.known(&format!("{v}/places:{v}")))
                    }) {
                        if let Some(page) = PAGES.with(|pages| pages.borrow().get(&thing).cloned())
                        {
                            heading.insert("page".into(), page.into());
                        }
                        heading.insert("thing".into(), thing.into());
                    }
                }
                into.push(heading);
            }
            for (nth, (at, along)) in walk.iter().copied().enumerate() {
                let mut line = row("line", &part.lines[at], depth, part.urgent);
                line.insert("nth".into(), ((nth + 1) as u64).into());
                line.insert("of_part".into(), (walk.len() as u64).into());
                line.insert("inset".into(), part.title.is_some().into());
                line.insert("along".into(), (along as u64).into());
                if let Some((so_much, of_all)) = part.measures.get(at).copied().flatten() {
                    line.insert("so_much".into(), so_much.into());
                    line.insert("of_all".into(), of_all.into());
                }
                if let Some(about) = part.keys.get(at) {
                    line.insert("about".into(), about.clone().into());
                }
                // the thing a line is about, where its answer names one: so that any surface can offer
                // standing on it, its own way - and none has to guess it from the words
                if let Some(thing) = part.values.get(at).and_then(|answer| named.thing(answer)) {
                    // and, while a site is laid out, the page there is for that thing: where to go
                    if let Some(page) = PAGES.with(|pages| pages.borrow().get(&thing).cloned()) {
                        line.insert("page".into(), page.into());
                    }
                    line.insert("thing".into(), thing.into());
                }
                into.push(line);
            }
        }
    }
    let mut all = Vec::new();
    rows(parts, 0, &mut all, named);
    for (nth, row) in all.iter_mut().enumerate() {
        row.insert("id".into(), (nth + 1).to_string().into());
    }
    // a mark for every thing a line is about, in the order the things first appear: what one says to
    // stand there, on a surface where one cannot point
    let mut marked: Vec<String> = Vec::new();
    for row in all.iter_mut() {
        let Some(thing) = row.get("thing").and_then(Value::as_str).map(str::to_string) else {
            continue;
        };
        let at = match marked.iter().position(|one| *one == thing) {
            Some(at) => at,
            None => {
                marked.push(thing);
                marked.len() - 1
            }
        };
        row.insert("mark".into(), ((at + 1) as u64).into());
    }
    all
}

/// What a lens gathers: its own tables and those it reads, each with what the places in view hold of it.
/// `among` are the lenses in view - the program's own and those the places bring: a table is read
/// whichever of them declares it.
fn gather(
    lens: &Lens,
    among: &[Lens],
    dirs: &[PathBuf],
    into: &Path,
    plane: Option<&[Map<String, Value>]>,
    here: Option<&str>,
) -> Result<tad::Declaration> {
    std::fs::create_dir_all(into)?;
    let mut tables = toml::Table::new();
    // the routes the lenses in view say: tables whose rows are read from a surface, not kept by a place
    let routes: Vec<crate::lenses::Route> =
        among.iter().flat_map(|l| l.route.iter().cloned()).collect();
    // its own tables, and those it reads from other lenses: one lens defines a piece of data, many read
    // it - whichever lens in view declares it, one built into the program or one a place brings
    let all = crate::lenses::tables();
    let declared_by = |table: &str| {
        all.get(table)
            .or_else(|| among.iter().find_map(|l| l.tables.get(table)))
    };
    // what it reads and nothing in view declares or names is said here, in words: not left for the engine
    // to say in its own, as a table that does not exist
    if let Some(unknown) = lens.lens.reads.iter().find(|read| {
        declared_by(read).is_none()
            && !among.iter().any(|l| {
                l.focus
                    .iter()
                    .any(|p| p.named.as_deref() == Some(read.as_str()))
            })
    }) {
        bail!(
            "the lens {} reads {unknown}, and no lens in view from here declares a table of that name or names a shape so",
            lens.lens.id
        );
    }
    let read: Vec<(String, toml::Value)> = lens
        .lens
        .reads
        .iter()
        .filter_map(|t| declared_by(t).map(|d| (t.clone(), d.clone())))
        .collect();
    for (table, declared) in lens
        .tables
        .iter()
        .map(|(t, d)| (t.clone(), d.clone()))
        .chain(read)
    {
        let (table, declared) = (&table, &declared);
        let (mut text, mut seq) = (String::new(), 0u64);
        // how long this table takes to gather is kept, so that a look can say where its time goes
        let began = std::time::Instant::now();
        // the lenses themselves, as rows: kept by no place - they are the language, read as data
        // and every link between everything in view: the graph, kept by no place either
        let language = crate::lenses::OF_THE_LANGUAGE.contains(&table.as_str());
        let graph = crate::relations::OF_THE_GRAPH.contains(&table.as_str());
        let kept = crate::kept::OF_THE_KEPT.contains(&table.as_str());
        // and a table a route reads: its rows come from a surface, where each place in view is held
        let routed = routes.iter().find(|route| route.table == *table);
        // and the plane a surface lays out: the rows of what another lens brings into focus
        let of_a_plane = table == OF_A_PLANE;
        // and the thing one stands on, in full: the rows of what it carries, where one stands on a thing
        let of_here = table == IN_FULL;
        // and what was just given in the window, and what was marked in what was last shown: the rows a
        // lens reads to say what an entry would keep
        let given = table == WAS_GIVEN;
        let marked = table == WAS_MARKED;
        let own = language
            || graph
            || kept
            || routed.is_some()
            || of_a_plane
            || of_here
            || given
            || marked;
        if own {
            let made = if of_a_plane {
                plane.map(<[_]>::to_vec).unwrap_or_default()
            } else if of_here {
                here.map(|here| in_full(here, among, dirs))
                    .unwrap_or_default()
            } else if given {
                GIVEN
                    .with(|said| said.borrow().clone())
                    .into_iter()
                    .enumerate()
                    .map(|(at, (field, words))| {
                        Map::from_iter([
                            ("id".to_string(), Value::from((at + 1).to_string())),
                            ("field".to_string(), Value::from(field)),
                            ("value".to_string(), Value::from(words)),
                        ])
                    })
                    .collect()
            } else if marked {
                crate::trail::marked(&crate::trail::who())
                    .into_iter()
                    .enumerate()
                    .map(|(at, thing)| {
                        Map::from_iter([
                            ("id".to_string(), Value::from((at + 1).to_string())),
                            ("thing".to_string(), Value::from(thing)),
                        ])
                    })
                    .collect()
            } else if let Some(route) = routed {
                crate::routes::read(route, dirs)
                    .into_iter()
                    .map(|(place, mut row)| {
                        row.insert(
                            "star".into(),
                            place.rsplit('/').next().unwrap_or(&place).into(),
                        );
                        row
                    })
                    .collect()
            } else {
                let mut made = if language {
                    crate::lenses::as_rows(dirs)?
                } else if graph {
                    crate::relations::as_graph(dirs)?
                } else {
                    crate::kept::as_rows(dirs)?
                };
                made.remove(table.as_str()).unwrap_or_default()
            };
            for row in made {
                seq += 1;
                let mut row = row;
                row.insert("seq".into(), seq.into());
                text.push_str(&serde_json::to_string(&row)?);
                text.push('\n');
            }
        }
        // the rows of the table as the places in view have them: each once, with the place it is of -
        // by whatever route it came (the ways in say how)
        for (place, mut fields) in crate::routes::rows_by_place(table, dirs)
            .into_iter()
            .filter(|_| !own)
        {
            seq += 1;
            fields.insert(
                "star".into(),
                place.rsplit('/').next().unwrap_or(&place).into(),
            );
            fields.insert("seq".into(), seq.into());
            text.push_str(&serde_json::to_string(&fields)?);
            text.push('\n');
        }
        std::fs::write(into.join(format!("{table}.json")), text)?;
        TOOK.with(|took| {
            took.borrow_mut()
                .push((table.clone(), began.elapsed().as_millis()))
        });
        let mut declared = declared.clone();
        if let Some(fields) = declared.get_mut("fields").and_then(|f| f.as_array_mut()) {
            for (name, kind) in [("star", "string"), ("seq", "integer")] {
                let mut field = toml::Table::new();
                field.insert("name".into(), name.into());
                field.insert("type".into(), kind.into());
                fields.push(toml::Value::Table(field));
            }
        }
        tables.insert(table.clone(), declared);
    }
    let mut whole = toml::Table::new();
    whole.insert("tables".into(), toml::Value::Table(tables));
    toml::Value::Table(whole)
        .try_into()
        .context("the lens's tables are a tad declaration")
}

/// What is said between two parts' answers, when the parts of a lens are all asked in one go.
const BETWEEN_PARTS: &str = "ocoeco: the next part";

/// Every question of a lens asked of the engine in one go: what the lens gathered is loaded once, and
/// each question answered after it - not loaded again for each part. Nothing where the engine does not
/// answer them as asked - one of them fails, or it answers some other way: then each part asks for
/// itself, as before, and says for itself what is wrong.
fn answers_to_all(
    engine: &dyn SqlEngine,
    load: &str,
    questions: &[String],
) -> Option<Vec<Vec<Map<String, Value>>>> {
    if questions.len() < 2 {
        return None;
    }
    let script = questions.iter().fold(load.to_string(), |script, sql| {
        format!(
            "{script}{};\nSELECT '{BETWEEN_PARTS}' AS between_parts;\n",
            sql.trim().trim_end_matches(';')
        )
    });
    let printed = engine.run(&script, "json").ok()?;
    // what was answered, part by part: all that is printed up to each saying of what stands between
    let (mut each, mut so_far) = (Vec::new(), String::new());
    for line in printed.lines() {
        if line.contains("\"between_parts\"") && line.contains(BETWEEN_PARTS) {
            each.push(std::mem::take(&mut so_far));
        } else {
            so_far.push_str(line);
            so_far.push('\n');
        }
    }
    if each.len() != questions.len() || !so_far.trim().is_empty() {
        return None;
    }
    each.iter()
        .map(|said| {
            if said.trim().is_empty() {
                return Some(Vec::new());
            }
            let rows: Value = serde_json::from_str(said.trim()).ok()?;
            Some(
                rows.as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|r| r.as_object().cloned())
                    .collect(),
            )
        })
        .collect()
}

/// The answers to one query: its rows.
fn answers(engine: &dyn SqlEngine, load: &str, sql: &str) -> Result<Vec<Map<String, Value>>> {
    let printed = engine.run(
        &format!("{load}{};\n", sql.trim().trim_end_matches(';')),
        "json",
    )?;
    if printed.trim().is_empty() {
        return Ok(Vec::new());
    }
    let rows: Value =
        serde_json::from_str(printed.trim()).context("the engine's answer is not rows")?;
    Ok(rows
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| r.as_object().cloned())
        .collect())
}

/// A line with each {name} replaced by that answer's value; what the answer does not hold stays as written.
pub fn laid_out(line: &str, answer: &Map<String, Value>) -> String {
    // the passthrough: the row itself - where it is held, its id, and every field it carries as it is
    if line == "*" {
        let said = |v: &Value| {
            v.as_str()
                .map(str::to_string)
                .unwrap_or_else(|| v.to_string())
        };
        let star = answer.get("star").map(said).filter(|s| !s.is_empty());
        let id = answer.get("id").map(said).unwrap_or_default();
        let rest: Vec<String> = answer
            .iter()
            .filter(|(k, v)| {
                !matches!(k.as_str(), "id" | "star" | "seq")
                    && !v.is_null()
                    && v.as_str() != Some("")
            })
            .map(|(k, v)| format!("{k}: {}", said(v)))
            .collect();
        return format!(
            "{}{id} - {}",
            star.map(|s| format!("{s}/")).unwrap_or_default(),
            rest.join("; ")
        );
    }
    let mut out = String::new();
    let mut rest = line;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        let name = &rest[open + 1..open + close];
        match answer.get(name) {
            Some(Value::String(s)) => out.push_str(s),
            Some(Value::Null) => {}
            Some(other) => out.push_str(&other.to_string()),
            None => out.push_str(&rest[open..=open + close]),
        }
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}

/// One part of a lens, brought into focus: its heading, if it has one, and each answer laid out as a line.
/// How the parts are shown - indented, as a list, in a pane - is the surface's to say, not the lens's.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Brought {
    pub title: Option<String>,
    pub lines: Vec<String>,
    /// What a line means beyond its words, when the lens says so: a measure of a whole (so much, of so
    /// much), one for each line, or none. A lens adds meaning; how it looks is each surface's to say.
    pub measures: Vec<Option<(f64, f64)>>,
    /// What newly comes into view here is to be said urgently, as the lens says.
    #[serde(default)]
    pub urgent: bool,
    /// A plane placed within this one - a whole lens, placed - its own shapes kept: this part is then that
    /// lens's name (title) and what it says of itself (about), or why it brought nothing.
    #[serde(default)]
    pub within: Vec<Brought>,
    #[serde(default)]
    pub about: Option<String>,
    #[serde(default)]
    pub nothing_because: Option<String>,
    /// Whether this lens was placed as one of every lens in view, and not by its name. What a lens places
    /// by name is what it says comes first; every other follows. A meaning: a surface may fold what
    /// follows, or lay it out as it does the rest.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub of_every: bool,
    /// What this part is laid out once for each of, where the lens says so: each star, each state, each
    /// thread - one of several parts of one kind. A meaning: a surface may stand them side by side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per: Option<String>,
    /// What each line is about, so it can be known again another time though its words change (an age,
    /// a count): its star and id when the answer names them, otherwise the line itself.
    #[serde(default)]
    pub keys: Vec<String>,
    /// Each line's answer as its named values - kept, not glued into the line - so a surface or a
    /// program can read what a line is made of.
    #[serde(default)]
    pub values: Vec<Map<String, Value>>,
    /// The shape of this part, when it is a chain: the names of the values a line goes from and to
    /// (the second may hold several). A lens says a part is a chain; each surface walks it its own way.
    #[serde(default)]
    pub chain: Option<(String, String)>,
}

/// What the lens `lens` brings into focus of the places at `dirs`, as of `today` (YYYY-MM-DD): its parts.
/// Whether a lens says anything of where one stands.
pub fn says_here(lens: &Lens) -> bool {
    // a lens that reads the thing stood on, in full, is looked through from a thing too
    lens.lens.reads.iter().any(|read| read == IN_FULL)
        || lens.focus.iter().any(|p| {
            p.sql.contains("{here}") || p.title.as_deref().is_some_and(|t| t.contains("{here}"))
        })
}

/// Every answer a focus holds: those of its own parts, and of every part of a lens placed in it - so that
/// what a lens of lenses places is read as its own.
pub fn every_answer(parts: &[Brought]) -> Vec<&Map<String, Value>> {
    parts
        .iter()
        .flat_map(|part| part.values.iter().chain(every_answer(&part.within)))
        .collect()
}

/// The thing of that name in full - where it is held, its table, its id; the place in full or by its
/// star: each field its row carries that says something, as it is kept. By whatever route its rows come:
/// read from a surface, said by the lenses, or kept by a place. Nothing where the name is the name of no
/// one thing in view.
fn in_full(here: &str, among: &[Lens], dirs: &[PathBuf]) -> Vec<Map<String, Value>> {
    let Some((held, id)) = here.split_once(':') else {
        return Vec::new();
    };
    let Some((place, table)) = held.rsplit_once('/') else {
        return Vec::new();
    };
    let star = |name: &str| name.rsplit('/').next().unwrap_or(name).to_string();
    let rows: Vec<(String, Map<String, Value>)> = if let Some(route) = among
        .iter()
        .flat_map(|l| l.route.iter())
        .find(|r| r.table == table)
    {
        crate::routes::read(route, dirs)
    } else if crate::lenses::OF_THE_LANGUAGE.contains(&table) {
        crate::lenses::as_rows(dirs)
            .ok()
            .and_then(|mut rows| rows.remove(table))
            .unwrap_or_default()
            .into_iter()
            .map(|row| {
                let at = row
                    .get("star")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                (at, row)
            })
            .collect()
    } else {
        crate::routes::rows_by_place(table, dirs)
    };
    // the one row so named: by its place in full, or by its star where that is how it was named
    let mut found = rows.into_iter().filter(|(at, row)| {
        (at == place || (star(at) == star(place) && !(at.contains('/') && place.contains('/'))))
            && row.get("id").and_then(Value::as_str) == Some(id)
    });
    let (Some((_, row)), None) = (found.next(), found.next()) else {
        return Vec::new();
    };
    row.iter()
        .filter(|(field, value)| {
            !matches!(field.as_str(), "id" | "star" | "seq" | "project")
                && !value.is_null()
                && value.as_str() != Some("")
        })
        .enumerate()
        .map(|(at, (field, value))| {
            let said = value
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| value.to_string());
            Map::from_iter([
                ("id".to_string(), Value::from((at + 1).to_string())),
                ("of_table".to_string(), Value::from(table)),
                ("field".to_string(), Value::from(field.as_str())),
                ("value".to_string(), Value::from(said)),
            ])
        })
        .collect()
}

/// The place of that name among those in view: by its id, by its star, or as the graph names it as a thing.
/// None where no place is so called; refused where several are.
fn place_named(name: &str, dirs: &[PathBuf]) -> Result<Option<PathBuf>> {
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    for dir in dirs {
        let Some(mark) = std::fs::read_to_string(dir.join("oco.toml"))
            .ok()
            .and_then(|t| crate::mark::Mark::parse(&t).ok())
        else {
            continue;
        };
        let id = mark.project.id;
        let star = id.rsplit('/').next().unwrap_or(&id).to_string();
        if name == id
            || name == star
            || name == format!("{star}/places:{star}")
            || name == format!("{id}/places:{star}")
        {
            found.push((id, dir.clone()));
        }
    }
    // the place named in full is the one meant, where a star of that name is held under several owners
    if let Some(exact) = found
        .iter()
        .position(|(id, _)| name == id || name.starts_with(&format!("{id}/")))
    {
        return Ok(Some(found.swap_remove(exact).1));
    }
    match found.len() {
        0 => Ok(None),
        1 => Ok(Some(found.swap_remove(0).1)),
        _ => bail!(
            "several places are called {name}: {} - name it in full",
            found
                .iter()
                .map(|(id, _)| id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// What was stood on, when a lens is looked through from a thing (lens@thing): the thing as the graph
/// names it in full - for the trail, which a record of work reads where it was made from. None where the
/// lens is not looked through from a thing, or the thing is not in view.
pub fn stood_on(lens: &str, dirs: &[PathBuf]) -> Option<String> {
    let (_, here) = lens.split_once('@')?;
    if let Ok(Some(place)) = place_named(here, dirs) {
        let id = crate::mark::Mark::parse(&std::fs::read_to_string(place.join("oco.toml")).ok()?)
            .ok()?
            .project
            .id;
        return Some(format!(
            "{id}/places:{}",
            id.rsplit('/').next().unwrap_or(&id)
        ));
    }
    let things = crate::relations::as_graph(dirs).ok()?.remove("things")?;
    things.iter().find_map(|t| {
        let (id, star, table, place) = (
            t.get("id")?.as_str()?,
            t.get("star")?.as_str()?,
            t.get("of_table")?.as_str()?,
            t.get("place")?.as_str()?,
        );
        let short = format!(
            "{star}/{table}:{}",
            id.strip_prefix(&format!("{place}/{table}:"))?
        );
        (here == id || here == short).then(|| id.to_string())
    })
}

/// The lens as looked through from a thing: wherever it says {here}, the thing stood on.
fn standing(lens: &Lens, here: &str) -> Lens {
    let mut stood = lens.clone();
    for part in &mut stood.focus {
        part.sql = part.sql.replace("{here}", &here.replace('\'', "''"));
        part.title = part.title.as_ref().map(|t| t.replace("{here}", here));
    }
    stood
}

/// A table the place at `dir` keeps, as a lens of itself - for entering a row of it as the table itself,
/// the identity's second face. None where the place keeps no such table, or keeps it on another branch,
/// where its own workflow writes it.
pub fn itself(table: &str, dir: &Path) -> Option<Lens> {
    let mark =
        crate::mark::Mark::parse(&std::fs::read_to_string(dir.join("oco.toml")).ok()?).ok()?;
    mark.exports
        .iter()
        .find(|(t, path)| t.as_str() == table && !path.contains('@'))?;
    let dirs = [dir.to_path_buf()];
    let all = crate::lenses::with(&dirs).unwrap_or_default();
    let mut lens = identity(table, &all, &dirs)?;
    // and the relations any lens in view declares for it: a link in one of its fields is offered and held
    // as it is for an entry a lens declares
    lens.relation = all
        .iter()
        .flat_map(|l| l.relation.iter())
        .filter(|r| r.from.split_once('.').is_some_and(|(of, _)| of == table))
        .cloned()
        .collect();
    Some(lens)
}

/// What a table no lens declares is, read off the rows the places in view keep of it: keyed by id, and
/// each field a whole number, a number, or yes or no where every value of it is so - otherwise words.
/// None where no place keeps such a table, or its rows have no id to be told apart by.
fn read_off(table: &str, dirs: &[PathBuf]) -> Option<toml::Value> {
    let rows = crate::routes::rows_by_place(table, dirs);
    if rows.is_empty()
        || !rows
            .iter()
            .all(|(_, row)| row.get("id").is_some_and(Value::is_string))
    {
        return None;
    }
    let mut kinds: std::collections::BTreeMap<&str, &'static str> = Default::default();
    for (key, value) in rows
        .iter()
        .flat_map(|(_, row)| row.iter())
        .filter(|(key, value)| !value.is_null() && !matches!(key.as_str(), "id" | "star" | "seq"))
    {
        let kind = match value {
            Value::Bool(_) => "boolean",
            Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
            Value::Number(_) => "number",
            _ => "string",
        };
        kinds
            .entry(key.as_str())
            .and_modify(|was| {
                if *was != kind {
                    *was = if matches!((*was, kind), ("integer", "number") | ("number", "integer"))
                    {
                        "number"
                    } else {
                        "string"
                    };
                }
            })
            .or_insert(kind);
    }
    let field = |name: &str, kind: &str| {
        toml::Value::Table(toml::Table::from_iter([
            ("name".to_string(), name.into()),
            ("type".to_string(), kind.into()),
        ]))
    };
    let fields: Vec<toml::Value> = std::iter::once(field("id", "string"))
        .chain(kinds.iter().map(|(name, kind)| field(name, kind)))
        .collect();
    Some(toml::Value::Table(toml::Table::from_iter([
        ("key".to_string(), toml::Value::Array(vec!["id".into()])),
        ("extra_fields".to_string(), true.into()),
        ("fields".to_string(), toml::Value::Array(fields)),
    ])))
}

/// The identity: a table looked through with nothing declared but its name - every row passed through,
/// each field as it is. A table is the passthrough lens over what a place keeps, so it needs no lens file;
/// it is looked through only where no lens has that name.
fn identity(table: &str, all: &[Lens], dirs: &[PathBuf]) -> Option<Lens> {
    let declared = crate::lenses::tables()
        .get(table)
        .cloned()
        .or_else(|| all.iter().find_map(|l| l.tables.get(table).cloned()))
        // a table no lens declares may be declared by the place that keeps it, in a declaration of its own
        .or_else(|| {
            dirs.iter()
                .find_map(|dir| crate::lenses::of_the_place(dir).get(table).cloned())
        })
        // and one declared nowhere is still a table a place keeps: its shape is read off its rows
        .or_else(|| read_off(table, dirs))?;
    Some(Lens {
        lens: crate::lenses::About {
            id: table.to_string(),
            says: format!(
                "The table {table} itself: every row passed through, each field as it is."
            ),
            grew_from: None,
            reads: Vec::new(),
        },
        keeps_out: Vec::new(),
        route: Vec::new(),
        tables: toml::Table::from_iter([(table.to_string(), declared)]),
        relation: Vec::new(),
        glimpse: Vec::new(),
        focus: vec![crate::lenses::Part {
            title: Some(format!("{table} ({{count}})")),
            always: false,
            sql: format!("SELECT * FROM \"{table}\" ORDER BY seq"),
            line: "*".into(),
            places: None,
            only: None,
            measure: None,
            of: None,
            whole: None,
            chain: None,
            per: None,
            named: None,
            urgent: false,
            id: format!("{table}/1"),
        }],
    })
}

pub fn brought(
    lens: &str,
    dirs: &[PathBuf],
    today: &str,
    engine: &dyn SqlEngine,
) -> Result<Vec<Brought>> {
    // within one look a lens is brought into focus once: a lens that places parts of another - a sheet
    // placing three parts of the lens of signs - is given the same bringing each time, not a new one
    let _one_look = crate::relations::one_look();
    let key = (lens.to_string(), dirs.to_vec(), today.to_string());
    if let Some(made) = BROUGHT.with(|brought| brought.borrow().get(&key).cloned()) {
        return Ok(made);
    }
    let made = brought_anew(lens, dirs, today, engine)?;
    BROUGHT.with(|brought| brought.borrow_mut().insert(key, made.clone()));
    Ok(made)
}

/// What was brought into focus in the look under way: which lens, over which places, on which day.
type WhatWasBrought = std::collections::HashMap<(String, Vec<PathBuf>, String), Vec<Brought>>;

thread_local! {
    static BROUGHT: std::cell::RefCell<WhatWasBrought> = Default::default();
}

/// The look has ended: what was brought into focus in it is not kept.
pub(crate) fn forget_what_was_brought() {
    BROUGHT.with(|brought| brought.borrow_mut().clear());
    TOOK.with(|took| took.borrow_mut().clear());
}

thread_local! {
    /// How long gathering each table took, in the look under way: the table, and thousandths of a second.
    static TOOK: std::cell::RefCell<Vec<(String, u128)>> = Default::default();
}

/// What takes less than this, in thousandths of a second, is not worth saying.
const WORTH_SAYING: u128 = 20;

fn brought_anew(
    lens: &str,
    dirs: &[PathBuf],
    today: &str,
    engine: &dyn SqlEngine,
) -> Result<Vec<Brought>> {
    let all = crate::lenses::with(dirs)?;
    // a lens may be looked through from a thing - lens@thing - as an observatory is looked from somewhere:
    // the lens's {here} is where one stands
    let (lens, here) = lens
        .split_once('@')
        .map_or((lens, None), |(lens, here)| (lens, Some(here)));
    // a lens of that name - or else a table of that name, looked through as itself
    let itself = identity(lens, &all, dirs);
    let Some(found) = all.iter().find(|l| l.lens.id == lens).or(itself.as_ref()) else {
        bail!(
            "no lens or table is called {lens}: there are the lenses {}",
            all.iter()
                .map(|l| l.lens.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    };
    if found.focus.is_empty() {
        bail!("the lens {lens} has no focus yet");
    }
    // a surface, said by a lens: a lens that reads a plane lays out what another lens brings into focus.
    // It is looked through from that lens - surface@lens - and the plane it reads is that lens's, as rows;
    // looked through from nothing, it has nothing to lay out
    let plane = lays_out_a_plane(found).then(|| match here {
        Some(of) => brought(of, dirs, today, engine)
            .and_then(|parts| Ok(as_plane(&parts, &Named::of(dirs)?))),
        None => Ok(Vec::new()),
    });
    let plane = plane.transpose()?;
    let here = here.filter(|_| plane.is_none());
    // a lens that says nothing of where one stands is looked through from a place: what is in view is what
    // is in view from there - a star what it holds, a constellation its stars, an observatory all it looks at
    if let Some(here) = here.filter(|_| !says_here(found)) {
        let Some(from) = place_named(here, dirs)? else {
            bail!("the lens {lens} is looked through from a place, and {here} is no place in view - a lens that says {{here}} is looked through from any thing");
        };
        return brought(lens, &crate::board::in_view(&from).0, today, engine);
    }
    let stood;
    let found = match here {
        Some(here) => {
            stood = standing(found, here);
            &stood
        }
        None => found,
    };
    static CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let scratch = std::env::temp_dir().join(format!(
        "ocoeco-focus-{}-{}",
        std::process::id(),
        CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let projected = (|| -> Result<(Vec<Brought>, Vec<String>)> {
        let wide = widened(found, &all);
        let load = with_shapes(
            &tad::sql::load_script(
                &scratch,
                &gather(&wide, &all, dirs, &scratch, plane.as_deref(), here)?,
            ),
            &wide,
            today,
        );
        let (mut parts, mut doubts) = (Vec::new(), Vec::new());
        // every part's question is asked of the engine in one go, with what the lens gathered loaded
        // once: a part finds its answer by its place among them - or asks for itself, where they could
        // not all be answered together
        let asking: Vec<usize> = found
            .focus
            .iter()
            .enumerate()
            .filter(|(_, part)| {
                part.places.is_none() && !(part.named.is_some() && part.line.trim().is_empty())
            })
            .map(|(at, _)| at)
            .collect();
        let questions: Vec<String> = asking
            .iter()
            .map(|at| {
                found.focus[*at]
                    .sql
                    .replace("{today}", today)
                    .replace("{who}", &whose())
            })
            .collect();
        let mut answered: std::collections::BTreeMap<usize, Vec<Map<String, Value>>> =
            answers_to_all(engine, &load, &questions)
                .map(|all| asking.iter().copied().zip(all).collect())
                .unwrap_or_default();
        for (at, part) in found.focus.iter().enumerate() {
            // a part may be what another lens brings into focus, placed here - all of it, or the part
            // under one heading. The other lens gathers, shapes and lays out; this one only places.
            if let Some(other) = &part.places {
                // what is placed: one lens, or every lens in view ("*") - but this one, and any that places
                // every lens itself
                let placing: Vec<&Lens> = if other == "*" {
                    all.iter()
                        .filter(|l| {
                            // and no surface: a surface lays out a plane, it is not a part of one. Nor
                            // a lens that looks from a thing, which from nowhere has only how to stand
                            // to say. Nor a lens a part before this one has placed already: so a lens
                            // may say what comes first, and then every other
                            l.lens.id != lens
                                && !lays_out_a_plane(l)
                                // from a thing, every lens is every lens that looks from a thing;
                                // and from nowhere a lens that looks from a thing places none
                                && says_here(l) == here.is_some()
                                && (here.is_some() || !says_here(found))
                                && !l.focus.iter().any(|p| p.places.as_deref() == Some("*"))
                                && !found.focus[..at]
                                    .iter()
                                    .any(|p| p.places.as_deref() == Some(l.lens.id.as_str()))
                        })
                        .collect()
                } else {
                    all.iter().filter(|l| l.lens.id == *other).collect()
                };
                // a lens that is not in view from here cannot be placed: said, and the rest is shown - what
                // is in view depends on where one looks from, and that is not a fault of the lens
                if placing.is_empty() {
                    // every lens, where there is none to place, is nothing missing
                    if other != "*" {
                        parts.push(Brought {
                            title: Some(format!("{other}: not in view from here")),
                            ..Default::default()
                        });
                    }
                    continue;
                }
                for placed in placing {
                    thread_local!(static DEEP: std::cell::Cell<u8> = const { std::cell::Cell::new(0) });
                    if DEEP.get() > 8 {
                        bail!("the lens {lens} places {other}, which comes back to it: lenses placed in a ring");
                    }
                    DEEP.set(DEEP.get() + 1);
                    // a lens that looks from a thing, placed by one that stands on a thing, stands there
                    let from = match here {
                        Some(here) if says_here(placed) => format!("{}@{here}", placed.lens.id),
                        _ => placed.lens.id.clone(),
                    };
                    let theirs = brought(&from, dirs, today, engine);
                    DEEP.set(DEEP.get() - 1);
                    match &part.only {
                        // one shape of it, placed among this lens's own
                        Some(only) => parts.extend(
                            theirs?
                                .into_iter()
                                .filter(|p| {
                                    !p.title.as_deref().is_some_and(|t| t.starts_with(NOT_SOUND))
                                })
                                .filter(|p| p.title.as_deref().is_some_and(|t| named(t) == only)),
                        ),
                        // the whole of it: its plane, placed within this one, its own shape kept
                        None => {
                            let (within, nothing_because) = match theirs {
                                Ok(within) => (within, None),
                                // lenses placed in a ring are no lens failing to bring something: the arrangement cannot be
                                Err(why)
                                    if format!("{why:#}").contains("lenses placed in a ring") =>
                                {
                                    return Err(why)
                                }
                                Err(why) => (Vec::new(), Some(format!("{why:#}"))),
                            };
                            // from a thing, a lens that brings nothing from there has nothing to say
                            // of it, and is left out
                            if other == "*"
                                && here.is_some()
                                && nothing_because.is_none()
                                && within
                                    .iter()
                                    .all(|p| p.lines.is_empty() && p.within.is_empty())
                            {
                                continue;
                            }
                            parts.push(Brought {
                                title: Some(placed.lens.id.clone()),
                                about: Some(placed.lens.says.clone()),
                                within,
                                nothing_because,
                                of_every: other == "*",
                                ..Default::default()
                            });
                        }
                    }
                }
                continue;
            }
            // a shape named for the other parts, with no line of its own, is passed on through, not laid out
            if part.named.is_some() && part.line.trim().is_empty() {
                continue;
            }
            let all = match answered.remove(&at) {
                Some(all) => all,
                None => answers(
                    engine,
                    &load,
                    &part
                        .sql
                        .replace("{today}", today)
                        .replace("{who}", &whose()),
                )?,
            };
            // a part may be laid out once for each value of one of its columns - a red line, a driver,
            // a star - in the order the values first appear; otherwise it is laid out once
            let mut each: Vec<Vec<Map<String, Value>>> = Vec::new();
            match &part.per {
                Some(per) if !all.is_empty() => {
                    let mut seen: Vec<String> = Vec::new();
                    for row in all {
                        let value = row
                            .get(per)
                            .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_string))
                            .unwrap_or_default();
                        match seen.iter().position(|s| *s == value) {
                            Some(at) => each[at].push(row),
                            None => {
                                seen.push(value);
                                each.push(vec![row]);
                            }
                        }
                    }
                }
                _ => each.push(all),
            }
            for rows in each {
                if rows.is_empty() && !part.always {
                    continue;
                }
                // a line that names what its answers do not hold would be laid out with a hole in it
                if let Some(first) = rows.first() {
                    let heading = part
                        .title
                        .as_deref()
                        .map(named)
                        .unwrap_or("a part with no heading");
                    for name in asked(&part.line)
                        .into_iter()
                        .filter(|name| !first.contains_key(*name))
                    {
                        doubts.push(format!(
                            "{heading}: its line names {{{name}}}, which its answers do not hold"
                        ));
                    }
                }
                parts.push(Brought {
                    values: rows.clone(),
                    chain: part
                        .chain
                        .as_ref()
                        .and_then(|c| Some((c.first()?.clone(), c.get(1)?.clone()))),
                    title: part.title.as_ref().map(|t| {
                        rows.first()
                            .map_or_else(|| t.clone(), |row| laid_out(t, row))
                            .replace("{count}", &rows.len().to_string())
                    }),
                    lines: rows.iter().map(|row| laid_out(&part.line, row)).collect(),
                    urgent: part.urgent,
                    within: Vec::new(),
                    about: None,
                    nothing_because: None,
                    of_every: false,
                    per: part.per.clone().filter(|_| !rows.is_empty()),
                    keys: rows
                        .iter()
                        .map(|row| {
                            match (
                                row.get("star").and_then(Value::as_str),
                                row.get("id").and_then(Value::as_str),
                            ) {
                                (Some(star), Some(id)) => format!("{star}/{id}"),
                                _ => laid_out(&part.line, row),
                            }
                        })
                        .collect(),
                    measures: match &part.measure {
                        Some(column) => rows
                            .iter()
                            .map(|row| {
                                let so_much = match row.get(column) {
                                    Some(Value::Number(n)) => n.as_f64(),
                                    Some(Value::String(s)) => s.parse().ok(),
                                    _ => None,
                                };
                                // the whole it is of: the answer's own, when the lens names it; else the part's
                                let whole = part.whole.as_ref().and_then(|w| match row.get(w) {
                                    Some(Value::Number(n)) => n.as_f64(),
                                    Some(Value::String(s)) => s.parse().ok(),
                                    _ => None,
                                });
                                so_much.map(|v| (v, whole.or(part.of).unwrap_or(1.0)))
                            })
                            .collect(),
                        None => Vec::new(),
                    },
                });
            }
        }
        Ok((parts, doubts))
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    // the projection looks at itself before it reaches any surface: what is not sound becomes a part
    // of it - so every surface shows it, the voice says it, and the check fails; never silently
    let (mut parts, mut doubts) = projected?;
    doubts.extend(unsound(&parts));
    if !doubts.is_empty() {
        parts.push(Brought {
            title: Some(format!("{NOT_SOUND} ({})", doubts.len())),
            lines: doubts.clone(),
            keys: doubts,
            measures: Vec::new(),
            ..Default::default()
        });
    }
    Ok(parts)
}

/// The heading of the part a projection adds to itself when it is not sound.
pub const NOT_SOUND: &str = "this focus is not sound";

/// The names a line asks its answer for: each {name}.
fn asked(line: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        let name = &rest[open + 1..open + close];
        if !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            names.push(name);
        }
        rest = &rest[open + close + 1..];
    }
    names
}

/// What is not sound in a projection - what a lens brought into focus, still as structure, before any
/// surface lays it out. True of every projection, whatever the lens: each line has what it is about;
/// the same thing is not shown twice in one part; a measure lies within its whole. This is where a
/// focus can be checked: here, while it is still intelligible - not afterwards, in a surface's text.
pub fn unsound(parts: &[Brought]) -> Vec<String> {
    let mut doubts = Vec::new();
    for part in parts {
        let heading = part
            .title
            .as_deref()
            .map(named)
            .unwrap_or("a part with no heading");
        if part.keys.len() != part.lines.len() {
            doubts.push(format!(
                "{heading}: {} lines, but {} things they are about",
                part.lines.len(),
                part.keys.len()
            ));
        }
        if !part.measures.is_empty() && part.measures.len() != part.lines.len() {
            doubts.push(format!(
                "{heading}: {} lines, but {} measures",
                part.lines.len(),
                part.measures.len()
            ));
        }
        // a line that is about something of its own (a star's item), shown more than once
        let mut shown = std::collections::BTreeMap::new();
        for (key, _) in part
            .keys
            .iter()
            .zip(&part.lines)
            .filter(|(key, line)| key != line)
        {
            *shown.entry(key).or_insert(0usize) += 1;
        }
        for (key, times) in shown.into_iter().filter(|(_, times)| *times > 1) {
            doubts.push(format!("{heading}: {key} is shown {times} times"));
        }
        for (so_much, of) in part.measures.iter().flatten() {
            if *of <= 0.0 || *so_much < 0.0 || so_much > of {
                doubts.push(format!(
                    "{heading}: a measure of {so_much} of {of} is not within its whole"
                ));
            }
        }
    }
    doubts
}

/// What a lens takes its light from, when it reads a shape another lens names: that lens's tables and what
/// it reads, and its shapes, made ready before the lens's own - followed as far as shapes lead. The lens
/// returned gathers and shapes as this one does, with all of that; it is not asked of itself.
fn widened(lens: &Lens, all: &[Lens]) -> Lens {
    fn reach(
        lens: &Lens,
        all: &[Lens],
        seen: &mut Vec<String>,
        reads: &mut Vec<String>,
        before: &mut Vec<crate::lenses::Part>,
    ) {
        for read in &lens.lens.reads {
            let Some(giver) = all.iter().find(|g| {
                g.focus
                    .iter()
                    .any(|p| p.named.as_deref() == Some(read.as_str()))
            }) else {
                continue;
            };
            if seen.contains(&giver.lens.id) {
                continue;
            }
            seen.push(giver.lens.id.clone());
            // what the giver's shapes ask of comes first
            reach(giver, all, seen, reads, before);
            reads.extend(
                giver
                    .tables
                    .keys()
                    .cloned()
                    .chain(giver.lens.reads.iter().cloned()),
            );
            before.extend(
                giver
                    .focus
                    .iter()
                    .filter(|p| p.named.is_some() && p.places.is_none())
                    .cloned(),
            );
        }
    }
    let mut wide = lens.clone();
    let mut before = Vec::new();
    reach(
        lens,
        all,
        &mut vec![lens.lens.id.clone()],
        &mut wide.lens.reads,
        &mut before,
    );
    wide.lens.reads.sort();
    wide.lens.reads.dedup();
    before.append(&mut wide.focus);
    wide.focus = before;
    wide
}

/// The lens that names a shape, among these.
fn giver_of<'a>(shape: &str, all: &'a [Lens]) -> Option<&'a Lens> {
    all.iter()
        .find(|g| g.focus.iter().any(|p| p.named.as_deref() == Some(shape)))
}

/// The shapes a lens names, made ready for its parts to ask of: each named part as a view, in the order
/// the lens gives them - so a shape may ask of one named before it.
fn with_shapes(load: &str, lens: &Lens, today: &str) -> String {
    let mut script = load.to_string();
    for part in lens.focus.iter().filter(|p| p.places.is_none()) {
        if let Some(name) = &part.named {
            script.push_str(&format!(
                "CREATE OR REPLACE VIEW \"{name}\" AS {};\n",
                part.sql
                    .replace("{today}", today)
                    .replace("{who}", &whose())
                    .trim()
                    .trim_end_matches(';')
            ));
        }
    }
    script
}

/// Whether a query names a table: the name standing as a word of its own.
pub(crate) fn names_table(sql: &str, table: &str) -> bool {
    let word = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
    sql.match_indices(table).any(|(at, _)| {
        !word(sql[..at].chars().next_back()) && !word(sql[at + table.len()..].chars().next())
    })
}

/// The beam of a lens: not what it brings into focus, but how light travels through it. What enters -
/// each table it gathers, how many rows, from how many places; what each part does with it - what it asks
/// of, how many rows enter and how many answers pass, or which lens it places and how much of that comes
/// through; and where it goes on to - the lenses that place it, and the surfaces it can be laid out on.
/// It is a projection like any other, so any surface lays it out.
pub fn beam(
    lens: &str,
    dirs: &[PathBuf],
    today: &str,
    engine: &dyn SqlEngine,
) -> Result<Vec<Brought>> {
    let _one_look = crate::relations::one_look();
    let all = crate::lenses::with(dirs)?;
    // a lens looked through from somewhere - lens@thing, lens@place - is traced as it is looked through
    // from there: its beam is the beam of that look, not of a lens of another name
    let (lens, here) = lens
        .split_once('@')
        .map_or((lens, None), |(lens, here)| (lens, Some(here)));
    // a lens of that name - or else a table of that name, looked through as itself
    let itself = identity(lens, &all, dirs);
    let Some(found) = all.iter().find(|l| l.lens.id == lens).or(itself.as_ref()) else {
        bail!(
            "no lens or table is called {lens}: there are the lenses {}",
            all.iter()
                .map(|l| l.lens.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    };
    // from a place, what is in view is what is in view from there; from a thing, the lens says where
    // one stands
    if let Some(here) = here.filter(|_| !says_here(found)) {
        let Some(from) = place_named(here, dirs)? else {
            bail!("the lens {lens} is looked through from a place, and {here} is no place in view");
        };
        return beam(lens, &crate::board::in_view(&from).0, today, engine);
    }
    let stood;
    let found = match here {
        Some(here) => {
            stood = standing(found, here);
            &stood
        }
        None => found,
    };
    let scratch = std::env::temp_dir().join(format!("ocoeco-beam-{}-{lens}", std::process::id()));
    let traced = (|| -> Result<Vec<Brought>> {
        let wide = widened(found, &all);
        let load = with_shapes(
            &tad::sql::load_script(&scratch, &gather(&wide, &all, dirs, &scratch, None, here)?),
            &wide,
            today,
        );
        // light in: what the lens gathers, as it gathered it
        let mut tables: Vec<(String, usize, usize)> = Vec::new();
        for table in found
            .tables
            .keys()
            .chain(found.lens.reads.iter())
            .filter(|t| giver_of(t, &all).is_none())
        {
            let text =
                std::fs::read_to_string(scratch.join(format!("{table}.json"))).unwrap_or_default();
            let rows: Vec<Value> = text
                .lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect();
            let places: std::collections::BTreeSet<&str> = rows
                .iter()
                .filter_map(|r| r.get("star").and_then(Value::as_str))
                .collect();
            tables.push((table.clone(), rows.len(), places.len()));
        }
        let light_in = if tables.is_empty() {
            Brought {
                title: Some("light in (0)".into()),
                lines: vec!["nothing of its own: it only places what other lenses bring".into()],
                keys: vec!["-".into()],
                ..Default::default()
            }
        } else {
            Brought {
                title: Some(format!("light in ({})", tables.len())),
                // each table, and by which route it came - and as of when: a route must not hide that
                lines: tables
                    .iter()
                    .map(|(t, rows, places)| {
                        let _ = places;
                        if crate::lenses::OF_THE_LANGUAGE.contains(&t.as_str()) {
                            format!("{t} - {rows} rows made from the lenses in view, now")
                        } else if crate::relations::OF_THE_GRAPH.contains(&t.as_str()) {
                            format!("{t} - {rows} rows made from the links between what is in view, now")
                        } else if crate::kept::OF_THE_KEPT.contains(&t.as_str()) {
                            format!("{t} - {rows} rows made from every row in view, now")
                        } else if t.as_str() == IN_FULL {
                            format!("{t} - {rows} rows: the thing stood on, each field as it is kept")
                        } else if let Some(route) = all.iter().flat_map(|l| l.route.iter()).find(|r| r.table == *t) {
                            // read where the places are held, now - or, of what was observed, when it was
                            match dirs.iter().map(|dir| crate::routes::route(dir)).find(|(by, _)| *by == crate::routes::OBSERVED) {
                                Some((_, as_of)) => format!("{t} - {rows} rows read from {} when it was observed, {as_of}", route.from),
                                None => format!("{t} - {rows} rows read from {}, where {places} place(s) are held, now", route.from),
                            }
                        } else {
                            // what arrived by each route - and, where the same thing came by more than one, how
                            // many are in view: each once, as it last was
                            let arrived = crate::routes::rows_by_route(t, dirs);
                            match crate::routes::of_a_table(t, dirs).join("; ") {
                                came if came.is_empty() => format!("{t} - no rows, from anywhere in view"),
                                came if arrived != *rows => format!("{t} - {came} - {rows} in view, each once, as it last was"),
                                came => format!("{t} - {came}"),
                            }
                        }
                    })
                    .collect(),
                keys: tables.iter().map(|(t, _, _)| t.clone()).collect(),
                ..Default::default()
            }
        };
        // through the focus: what each part lets pass
        let mut through = Brought {
            title: Some(format!("through the focus ({})", found.focus.len())),
            ..Default::default()
        };
        // the shapes named so far: what passes through each, so that a part asking of a shape is seen to
        // take its light from it
        let mut shaped: Vec<(String, usize, usize)> = Vec::new();
        // a shape another lens names, read here, is light coming in from that lens
        let mut light_in = light_in;
        // and how long each took to gather, where that is worth saying: so a look says where its time
        // goes. Within one look the graph is made once - the table that asks for it first waits for it
        let took = TOOK.with(|took| took.borrow().clone());
        for (line, table) in light_in.lines.iter_mut().zip(&light_in.keys) {
            let all: u128 = took
                .iter()
                .filter(|(t, _)| t == table)
                .map(|(_, ms)| ms)
                .sum();
            if all >= WORTH_SAYING {
                line.push_str(&format!(" - gathered in {all} ms"));
                // the graph says where its own making went, stretch by stretch
                if crate::relations::OF_THE_GRAPH.contains(&table.as_str()) {
                    let said: Vec<String> = crate::relations::stretches()
                        .into_iter()
                        .filter(|(_, ms)| *ms >= 5)
                        .map(|(name, ms)| format!("{name} {ms}"))
                        .collect();
                    if !said.is_empty() {
                        line.push_str(&format!(": {}", said.join(", ")));
                    }
                }
            }
        }
        for read in &found.lens.reads {
            if let Some(giver) = giver_of(read, &all) {
                let rows = answers(engine, &load, &format!("SELECT * FROM \"{read}\""))?.len();
                if light_in.keys == ["-"] {
                    light_in.lines.clear();
                    light_in.keys.clear();
                }
                light_in.lines.push(format!(
                    "{read} - {rows} rows, shaped by the lens {}",
                    giver.lens.id
                ));
                light_in.keys.push(read.clone());
                shaped.push((read.clone(), rows, 0));
            }
        }
        light_in.title = Some(format!(
            "light in ({})",
            light_in.keys.iter().filter(|k| *k != "-").count()
        ));
        for (at, part) in found.focus.iter().enumerate() {
            // a heading laid out once for each value is named by what it is laid out per
            let heading = match (part.title.as_deref().map(named), &part.per) {
                (Some(title), Some(per)) if title.contains('{') => {
                    format!("laid out once for each {per}")
                }
                (Some(title), _) => title.to_string(),
                (None, _) => "a part with no heading".to_string(),
            };
            let (line, measure) = match &part.places {
                Some(other) => {
                    let came: usize = if all.iter().any(|l| l.lens.id == *other) {
                        brought(other, dirs, today, engine)?
                            .iter()
                            .filter(|p| {
                                !p.title.as_deref().is_some_and(|t| t.starts_with(NOT_SOUND))
                            })
                            .filter(|p| {
                                part.only.as_deref().is_none_or(|only| {
                                    p.title.as_deref().is_some_and(|t| named(t) == only)
                                })
                            })
                            .map(|p| p.lines.len())
                            .sum()
                    } else {
                        0
                    };
                    let which = part
                        .only
                        .as_deref()
                        .map(|only| format!(" - only {only:?}"))
                        .unwrap_or_default();
                    (
                        format!(
                            "{}. placed from {other}{which}: {came} line(s) come through",
                            at + 1
                        ),
                        None,
                    )
                }
                None => {
                    let asked: Vec<&(String, usize, usize)> = tables
                        .iter()
                        .chain(shaped.iter())
                        .filter(|(t, _, _)| names_table(&part.sql, t))
                        .collect();
                    let entering: usize = asked.iter().map(|(_, rows, _)| rows).sum();
                    let passing = answers(
                        engine,
                        &load,
                        &part
                            .sql
                            .replace("{today}", today)
                            .replace("{who}", &whose()),
                    )?
                    .len();
                    let of = asked
                        .iter()
                        .map(|(t, _, _)| t.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let shown = match &part.named {
                        // a named shape: what it makes passes on to the parts that ask of it
                        Some(name) => format!(
                            "{}. shapes {name} - asks of {of}: {entering} rows in, {passing} shaped",
                            at + 1
                        ),
                        None => format!(
                            "{}. {heading} - asks of {of}: {entering} rows in, {passing} through",
                            at + 1
                        ),
                    };
                    let measure = Some((passing as f64, entering.max(passing).max(1) as f64));
                    if let Some(name) = &part.named {
                        shaped.push((name.clone(), passing, 0));
                    }
                    (shown, measure)
                }
            };
            through.lines.push(line);
            through.measures.push(measure);
            through.keys.push(format!("{lens}/{}", at + 1));
        }
        if through.measures.iter().all(Option::is_none) {
            through.measures.clear();
        }
        // on, from here: the lenses that place this one, and the surfaces it is laid out on
        let placed_in: Vec<&str> = all
            .iter()
            .filter(|l| l.focus.iter().any(|p| p.places.as_deref() == Some(lens)))
            .map(|l| l.lens.id.as_str())
            .collect();
        let mut on = Brought {
            title: None,
            ..Default::default()
        };
        if !placed_in.is_empty() {
            on.lines
                .push(format!("placed in: {}", placed_in.join(", ")));
            on.keys.push("placed in".into());
        }
        on.lines.push(format!(
            "laid out on: {} - and in the terminal (ocoeco fulldome)",
            crate::dome::SURFACES.join(", ")
        ));
        on.keys.push("laid out on".into());
        on.title = Some(format!("on, from here ({})", on.lines.len()));
        Ok(vec![light_in, through, on])
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    traced
}

/// A heading without its count: "in progress (2)" is "in progress".
fn named(title: &str) -> &str {
    title.rsplit_once(" (").map_or(title, |(name, _)| name)
}

/// What is new in view since `before`: each line of a headed part that was not in that part before,
/// said under its heading; and what is no longer in view at all. A part with no heading - a total -
/// is not news, and a line whose words changed but which is about the same thing is not new.
pub fn news(before: &[Brought], now: &[Brought]) -> Vec<String> {
    let was_in = |name: &str| -> std::collections::BTreeSet<&String> {
        before
            .iter()
            .filter(|p| p.title.as_deref().map(named) == Some(name))
            .flat_map(|p| p.keys.iter())
            .collect()
    };
    let mut said = Vec::new();
    for part in now {
        let Some(title) = &part.title else { continue };
        let was = was_in(named(title));
        // what the lens says is urgent is marked so; the voice carries it urgently, deciding nothing
        let urgently = if part.urgent { "urgent: " } else { "" };
        for (key, line) in part.keys.iter().zip(&part.lines) {
            if !was.contains(key) {
                said.push(format!("{urgently}{}: {line}", named(title)));
            }
        }
    }
    let in_view: std::collections::BTreeSet<&String> = now
        .iter()
        .filter(|p| p.title.is_some())
        .flat_map(|p| p.keys.iter())
        .collect();
    // what is no longer in view is said as the line it was, under its heading
    let mut gone = std::collections::BTreeSet::new();
    for part in before.iter().filter(|p| p.title.is_some()) {
        let heading = named(part.title.as_deref().unwrap_or_default());
        for (key, line) in part.keys.iter().zip(&part.lines) {
            if !in_view.contains(key) && gone.insert(key) {
                said.push(format!("no longer in view: {heading}: {line}"));
            }
        }
    }
    said
}

/// What is new in view since the focus kept at `kept`, which then keeps this one. The first time
/// nothing is kept, so nothing is new: it only keeps.
pub fn since(kept: &Path, now: &[Brought]) -> Result<Vec<String>> {
    let before: Option<Vec<Brought>> = std::fs::read_to_string(kept)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    if let Some(dir) = kept.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(kept, serde_json::to_string(now)?)?;
    Ok(before.map(|before| news(&before, now)).unwrap_or_default())
}

/// What the lens brings into focus, laid out on the plain-text surface.
pub fn focus(lens: &str, dirs: &[PathBuf], today: &str, engine: &dyn SqlEngine) -> Result<String> {
    Ok(crate::dome::Text.lay_out(&brought(lens, dirs, today, engine)?))
}

/// What is not sound in the shape a lens gives what it brings into focus - checked with no data at all:
/// a query has its columns whether or not anything answers it, and those columns are the shape. Each
/// part's line may name only what its query gives, and so may its measure. A projection does not check
/// what it projects - that is the data's to be sound in, and observing's to have gathered whole - only
/// the shape it is projected in; so this is checked in the making, and need not be watched after.
pub fn shape(lens: &Lens, engine: &dyn SqlEngine) -> Result<Vec<String>> {
    shape_among(lens, &crate::lenses::lenses(), engine)
}

/// The same, of a lens a place brings: its shape is told among the lenses in view from that place - so
/// that it may read a table, or ask of a shape, another lens of the place gives.
pub fn shape_among(lens: &Lens, among: &[Lens], engine: &dyn SqlEngine) -> Result<Vec<String>> {
    static CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let scratch = std::env::temp_dir().join(format!(
        "ocoeco-shape-{}-{}",
        std::process::id(),
        CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let doubts = (|| -> Result<Vec<String>> {
        // the lens's tables, and those it reads, with nothing in them
        let wide = widened(lens, among);
        let load = with_shapes(
            &tad::sql::load_script(&scratch, &gather(&wide, among, &[], &scratch, None, None)?),
            &wide,
            "2026-01-01",
        );
        let mut doubts = Vec::new();
        for part in &lens.focus {
            // a part placed from another lens asks nothing here: what must hold is that the lens, and the
            // heading if one is named, are there - told for the lenses built in; one a place brings is
            // met when it is looked through
            if let Some(other) = &part.places {
                if let Some(theirs) = crate::lenses::lenses().iter().find(|l| l.lens.id == *other) {
                    if let Some(only) = part.only.as_deref().filter(|only| {
                        !theirs.focus.iter().any(|p| {
                            // a heading laid out for each value is known only when data passes
                            p.title
                                .as_deref()
                                .is_some_and(|t| named(t) == *only || named(t).contains('{'))
                        })
                    }) {
                        doubts.push(format!("it places the part {only:?} of the lens {other}, which has no part under that heading"));
                    }
                }
                continue;
            }
            if part.sql.trim().is_empty() {
                doubts.push(
                    "a part asks nothing: it has neither a query nor a lens it places".to_string(),
                );
                continue;
            }
            let heading = part
                .title
                .as_deref()
                .map(named)
                .unwrap_or("a part with no heading");
            if let Some(name) = part.named.as_deref().filter(|n| {
                crate::lenses::tables().contains_key(*n) || lens.tables.contains_key(*n)
            }) {
                doubts.push(format!(
                    "{heading}: it names a shape {name}, which is also the name of a table"
                ));
            }
            let described = answers(
                engine,
                &load,
                &format!(
                    "DESCRIBE {}",
                    part.sql
                        .replace("{today}", "2026-01-01")
                        .trim()
                        .trim_end_matches(';')
                ),
            )?;
            let columns: Vec<&str> = described
                .iter()
                .filter_map(|c| c.get("column_name").and_then(Value::as_str))
                .collect();
            for name in asked(&part.line)
                .into_iter()
                .filter(|name| !columns.contains(name))
            {
                doubts.push(format!("{heading}: its line names {{{name}}}, which its query does not give (it gives {})", columns.join(", ")));
            }
            if let Some(measure) = part.measure.as_deref().filter(|m| !columns.contains(m)) {
                doubts.push(format!(
                    "{heading}: its measure is {measure}, which its query does not give"
                ));
            }
            if let Some(per) = part.per.as_deref().filter(|p| !columns.contains(p)) {
                doubts.push(format!(
                    "{heading}: it is laid out per {per}, which its query does not give"
                ));
            }
            if let Some(chain) = &part.chain {
                if chain.len() != 2 {
                    doubts.push(format!(
                        "{heading}: a chain names two columns - from and to - not {}",
                        chain.len()
                    ));
                }
                for column in chain.iter().filter(|c| !columns.contains(&c.as_str())) {
                    doubts.push(format!(
                        "{heading}: its chain runs through {column}, which its query does not give"
                    ));
                }
            }
            if let Some(whole) = part.whole.as_deref().filter(|w| !columns.contains(w)) {
                doubts.push(format!(
                    "{heading}: its whole is {whole}, which its query does not give"
                ));
            }
            if part.measure.is_some() && part.whole.is_none() && part.of.is_none_or(|of| of <= 0.0)
            {
                doubts.push(format!(
                    "{heading}: it has a measure but not the whole it is of"
                ));
            }
        }
        Ok(doubts)
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    doubts
}

/// Every lens with a focus: whether its shape is sound, and whether it comes into focus over the place
/// at `dir` - what does not, and why.
pub fn every_lens_focuses(dir: &Path, engine: &dyn SqlEngine) -> Vec<String> {
    let _one_look = crate::relations::one_look();
    let mut problems: Vec<String> = crate::lenses::differing(dir)
        .into_iter()
        .map(|id| format!("the lens {id} kept here is no longer the one built into this ocoeco: what is looked through is not what is kept"))
        .collect();
    let all = match crate::lenses::with(&[dir.to_path_buf()]) {
        Ok(all) => all,
        Err(e) => return vec![format!("{e:#}")],
    };
    // what this place adds to a lens kept elsewhere is read only where that lens is in view
    problems.extend(crate::lenses::added_by(dir).into_iter().filter(|more| !all.iter().any(|l| l.lens.id == more.lens.id)).map(|more| {
        format!("this place adds to the lens {}, and no lens of that name is in view from here: what it adds is not read", more.lens.id)
    }));
    // a lens that only places others is not brought into focus again on its own: what it places is
    for lens in all
        .iter()
        .filter(|l| l.focus.iter().any(|p| p.places.is_none()))
    {
        match shape_among(lens, &all, engine) {
            Err(e) => problems.push(format!(
                "the shape of the lens {} cannot be told: {}",
                lens.lens.id,
                e.to_string().lines().next().unwrap_or_default()
            )),
            Ok(doubts) => problems.extend(doubts.into_iter().map(|doubt| {
                format!(
                    "the shape of the lens {} is not sound - {doubt}",
                    lens.lens.id
                )
            })),
        }
        match brought(&lens.lens.id, &[dir.to_path_buf()], "2026-01-01", engine) {
            Err(e) => problems.push(format!(
                "the lens {} cannot come into focus here: {}",
                lens.lens.id,
                e.to_string().lines().next().unwrap_or_default()
            )),
            Ok(parts) => {
                for part in parts
                    .iter()
                    .filter(|p| p.title.as_deref().is_some_and(|t| t.starts_with(NOT_SOUND)))
                {
                    problems.extend(part.lines.iter().map(|doubt| {
                        format!("the lens {} is not sound here - {doubt}", lens.lens.id)
                    }));
                }
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An engine that answers from what it is asked: one row for work in progress, none waiting.
    struct StandIn;
    impl SqlEngine for StandIn {
        fn run(&self, script: &str, _format: &str) -> Result<String> {
            assert!(
                script.contains("CREATE TABLE \"backlog\"") && script.contains("\"star\" VARCHAR"),
                "the gathered rows carry their star"
            );
            Ok(if script.contains("AS times") {
                r#"[{"state":"in progress - read off what was done","who":"claude","star":"r","id":"a","title":"A","next":" - next: x","times":2,"last":"2026-11-01 10:00"}]"#.into()
            } else if script.contains("count(DISTINCT star)") {
                r#"[{"open":2,"parked":0,"stars":1}]"#.into()
            } else {
                "[]".into()
            })
        }
    }

    #[test]
    fn what_is_new_in_view_is_said_once_and_a_changed_age_is_not_news() {
        let part = |title: &str, rows: &[(&str, &str)]| Brought {
            title: Some(title.into()),
            lines: rows.iter().map(|(_, l)| l.to_string()).collect(),
            keys: rows.iter().map(|(k, _)| k.to_string()).collect(),
            measures: vec![],
            ..Default::default()
        };
        let total = |line: &str| Brought {
            title: None,
            lines: vec![line.into()],
            keys: vec![line.into()],
            measures: vec![],
            ..Default::default()
        };
        let before = [
            part("in progress (1)", &[("r/a", "ann: r/a - A, 1 day(s)")]),
            part("next (2)", &[("r/b", "r/b - B"), ("r/c", "r/c - C")]),
            total("open 5"),
        ];
        let now = [
            part(
                "in progress (2)",
                &[("r/a", "ann: r/a - A, 2 day(s)"), ("r/b", "bob: r/b - B")],
            ),
            part("waiting (1)", &[("r/d", "r/d - D - waits on E")]),
            total("open 4"),
        ];
        assert_eq!(
            news(&before, &now),
            [
                "in progress: bob: r/b - B",
                "waiting: r/d - D - waits on E",
                "no longer in view: next: r/c - C"
            ]
        );
        assert!(news(&now, &now).is_empty());
        // what the lens says is urgent is marked so, for the voice to carry urgently
        let mut failing = part("failing on main (1)", &[("o/r:ci", "o/r - ci: failure")]);
        failing.urgent = true;
        assert_eq!(
            news(&now, &[failing]).first().map(String::as_str),
            Some("urgent: failing on main: o/r - ci: failure")
        );
        let kept =
            std::env::temp_dir().join(format!("ocoeco-since-{}/work.json", std::process::id()));
        let _ = std::fs::remove_file(&kept);
        assert!(
            since(&kept, &before).unwrap().is_empty(),
            "the first time only keeps"
        );
        assert_eq!(since(&kept, &now).unwrap().len(), 3);
        assert!(since(&kept, &now).unwrap().is_empty(), "said once");
    }

    #[test]
    fn a_projection_says_what_is_not_sound_in_it() {
        let part =
            |title: &str, rows: &[(&str, &str)], measures: Vec<Option<(f64, f64)>>| Brought {
                title: Some(title.into()),
                lines: rows.iter().map(|(_, l)| l.to_string()).collect(),
                keys: rows.iter().map(|(k, _)| k.to_string()).collect(),
                measures,
                ..Default::default()
            };
        let sound = [
            part(
                "in progress (2)",
                &[("r/a", "ann: r/a - A"), ("r/b", "bob: r/b - B")],
                vec![],
            ),
            part(
                "done lately (2)",
                &[("r - tidied", "r - tidied"), ("r - tidied", "r - tidied")],
                vec![],
            ),
            part(
                "reached (1)",
                &[("r/m", "r - M: 8 of 9")],
                vec![Some((8.0, 9.0))],
            ),
        ];
        assert!(
            unsound(&sound).is_empty(),
            "two glimpses with the same words are two glimpses: {:?}",
            unsound(&sound)
        );
        // what fed back into itself on 2026-11-12: the same run, three times over
        let fed_back = [part(
            "failing on main (3)",
            &[
                ("r/ci", "r - ci: failure"),
                ("r/ci", "r - ci: failure"),
                ("r/ci", "r - ci: failure"),
            ],
            vec![],
        )];
        assert_eq!(
            unsound(&fed_back),
            ["failing on main: r/ci is shown 3 times"]
        );
        let beyond = [part(
            "reached (1)",
            &[("r/m", "r - M: 12 of 9")],
            vec![Some((12.0, 9.0))],
        )];
        assert_eq!(
            unsound(&beyond),
            ["reached: a measure of 12 of 9 is not within its whole"]
        );
        let mut torn = part(
            "next (2)",
            &[("r/a", "r/a - A"), ("r/b", "r/b - B")],
            vec![Some((1.0, 2.0))],
        );
        torn.keys.pop();
        assert_eq!(
            unsound(&[torn]),
            [
                "next: 2 lines, but 1 things they are about",
                "next: 2 lines, but 1 measures"
            ]
        );
        assert_eq!(
            asked("{star}/{id} - {title}: {so_much} of 9 {not a name} {}"),
            ["star", "id", "title", "so_much"]
        );
    }

    #[test]
    fn a_shape_is_checked_with_no_data_at_all() {
        /// An engine that only tells a query's columns - as it can with nothing in any table.
        struct Columns;
        impl SqlEngine for Columns {
            fn run(&self, script: &str, _format: &str) -> Result<String> {
                assert!(
                    script.contains("DESCRIBE"),
                    "the shape is asked of the query, not of its answers"
                );
                Ok(
                    r#"[{"column_name":"star"},{"column_name":"id"},{"column_name":"so_much"}]"#
                        .into(),
                )
            }
        }
        let lens = |line: &str, more: &str| -> Lens {
            toml::from_str(&format!("[lens]\nid = \"t\"\nsays = \"a lens to test with\"\n[tables.things]\nkey = [\"id\"]\nfields = [{{ name = \"id\", type = \"string\" }}]\n[[focus]]\ntitle = \"things ({{count}})\"\nsql = \"SELECT 1\"\nline = \"{line}\"\n{more}")).unwrap()
        };
        assert!(shape(
            &lens("{star}/{id}", "measure = \"so_much\"\nof = 9\n"),
            &Columns
        )
        .unwrap()
        .is_empty());
        assert_eq!(shape(&lens("{star}/{id} - {titel}", ""), &Columns).unwrap(), ["things: its line names {titel}, which its query does not give (it gives star, id, so_much)"]);
        assert_eq!(
            shape(&lens("{id}", "measure = \"readiness\"\n"), &Columns).unwrap(),
            [
                "things: its measure is readiness, which its query does not give",
                "things: it has a measure but not the whole it is of"
            ]
        );
    }

    #[test]
    fn a_part_is_laid_out_once_for_each_red_line_under_its_own_name() {
        /// An engine that answers the lens of insight with three connections on two red lines.
        struct Lines;
        impl SqlEngine for Lines {
            fn run(&self, script: &str, _format: &str) -> Result<String> {
                Ok(if script.contains("FROM connections ORDER BY 1, seq") {
                    r#"[{"thread":"route","this":"a","how":"led to","that":"b","says":"first","known":"read"},
                        {"thread":"signs","this":"x","how":"repeats","that":"y","says":"same","known":"reasoned"},
                        {"thread":"route","this":"b","how":"needed","that":"c","says":"then","known":"recorded"}]"#.into()
                } else {
                    "[]".into()
                })
            }
        }
        let d = std::env::temp_dir().join(format!("ocoeco-per-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nconnections = \"data/connections.json\"\n").unwrap();
        std::fs::write(d.join("data/connections.json"), "").unwrap();
        let parts = brought("insight", &[d], "2026-11-13", &Lines).unwrap();
        let headed: Vec<(&str, usize)> = parts
            .iter()
            .filter_map(|p| Some((p.title.as_deref()?, p.lines.len())))
            .collect();
        assert_eq!(
            headed,
            [("route (2)", 2), ("signs (1)", 1)],
            "each red line under its own name, in the order first met"
        );
        assert_eq!(
            parts[0].lines,
            [
                "a - led to - b: first (read)",
                "b - needed - c: then (recorded)"
            ]
        );
        assert!(parts[0].chain.is_some(), "and each is still a chain");
    }

    #[test]
    fn a_lens_asks_of_a_shape_another_lens_names() {
        /// An engine that answers the taker's part - and finds the giver's shape made ready before it.
        struct Taken;
        impl SqlEngine for Taken {
            fn run(&self, script: &str, _format: &str) -> Result<String> {
                if script.trim_end().ends_with("FROM given;") {
                    let made = script
                        .find("CREATE OR REPLACE VIEW \"given\" AS SELECT 1 AS n")
                        .expect("the giver's shape is made ready");
                    assert!(
                        made < script.rfind("SELECT n FROM given").unwrap(),
                        "before the part that asks of it"
                    );
                    return Ok(r#"[{"n":"one"}]"#.into());
                }
                Ok("[]".into())
            }
        }
        let d = std::env::temp_dir().join(format!("ocoeco-taken-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("lenses")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n").unwrap();
        std::fs::write(d.join("lenses/giver.toml"), "[lens]\nid = \"giver\"\nsays = \"Names a shape, and shows nothing of it.\"\n[[focus]]\nnamed = \"given\"\nsql = \"SELECT 1 AS n\"\n").unwrap();
        std::fs::write(d.join("lenses/taker.toml"), "[lens]\nid = \"taker\"\nsays = \"Asks of a shape another lens names.\"\nreads = [\"given\"]\n[[focus]]\ntitle = \"taken ({count})\"\nsql = \"SELECT n FROM given\"\nline = \"{n}\"\n").unwrap();
        let places = [d.clone()];
        let parts = brought("taker", &places, "2026-11-14", &Taken).unwrap();
        assert_eq!(
            parts
                .iter()
                .map(|p| (p.title.clone().unwrap_or_default(), p.lines.clone()))
                .collect::<Vec<_>>(),
            [("taken (1)".to_string(), vec!["one".to_string()])]
        );
        let beam = beam("taker", &places, "2026-11-14", &Taken).unwrap();
        assert_eq!(
            beam[0].lines,
            ["given - 0 rows, shaped by the lens giver"],
            "light in: the shape, said as shaped by the lens that names it"
        );
    }

    #[test]
    fn a_lens_reads_a_table_another_lens_in_view_declares() {
        /// An engine that tells the columns of the part that asks of the notes, and answers it.
        struct One;
        impl SqlEngine for One {
            fn run(&self, script: &str, _format: &str) -> Result<String> {
                Ok(if script.contains("DESCRIBE") {
                    r#"[{"column_name":"n"}]"#.into()
                } else if script.contains("-- read") {
                    r#"[{"n":"one"}]"#.into()
                } else {
                    "[]".into()
                })
            }
        }
        let d = std::env::temp_dir().join(format!("ocoeco-read-of-another-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("lenses")).unwrap();
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nnotes = \"data/notes.json\"\n").unwrap();
        std::fs::write(
            d.join("data/notes.json"),
            "{\"id\":\"a\",\"n\":\"x\"}\n{\"id\":\"b\",\"n\":\"y\"}\n",
        )
        .unwrap();
        // one lens of the place declares the table, another only reads it, a third reads what nothing declares
        std::fs::write(d.join("lenses/keeper.toml"), "[lens]\nid = \"keeper\"\nsays = \"Declares the notes.\"\n[tables.notes]\nkey = [\"id\"]\nfields = [{ name = \"id\", type = \"string\", required = true }, { name = \"n\", type = \"string\" }]\n[[focus]]\ntitle = \"kept ({count})\"\nsql = \"SELECT n FROM notes\"\nline = \"{n}\"\n").unwrap();
        std::fs::write(d.join("lenses/reader.toml"), "[lens]\nid = \"reader\"\nsays = \"Reads the notes another lens declares.\"\nreads = [\"notes\"]\n[[focus]]\ntitle = \"read ({count})\"\nsql = \"SELECT n FROM notes -- read\"\nline = \"{n}\"\n").unwrap();
        std::fs::write(d.join("lenses/lost.toml"), "[lens]\nid = \"lost\"\nsays = \"Reads what nothing in view declares.\"\nreads = [\"kept-nowhere\"]\n[[focus]]\ntitle = \"read ({count})\"\nsql = \"SELECT 1 AS n\"\nline = \"{n}\"\n").unwrap();
        let places = [d.clone()];
        let light = beam("reader", &places, "2026-11-15", &One).unwrap();
        assert_eq!(
            light[0].lines,
            ["notes - 2 rows from 1 place(s) held here, as it is now"],
            "the table another lens declares comes in"
        );
        let parts = brought("reader", &places, "2026-11-15", &One).unwrap();
        assert_eq!(parts[0].lines, ["one"]);
        // its shape is told among the lenses in view from its place - among the program's own alone
        // nothing declares what it reads
        let all = crate::lenses::with(&places).unwrap();
        let reader = all.iter().find(|l| l.lens.id == "reader").unwrap();
        assert!(shape_among(reader, &all, &One).unwrap().is_empty());
        assert!(shape(reader, &One).is_err());
        let lacking = format!(
            "{:#}",
            brought("lost", &places, "2026-11-15", &One).unwrap_err()
        );
        assert!(
            lacking.contains("reads kept-nowhere") && lacking.contains("no lens in view"),
            "{lacking}"
        );
    }

    #[test]
    fn a_lens_looked_through_from_a_thing_is_given_that_thing_in_full() {
        /// An engine that answers nothing: what is looked at here is what the lens is given.
        struct Nothing;
        impl SqlEngine for Nothing {
            fn run(&self, _script: &str, _format: &str) -> Result<String> {
                Ok("[]".into())
            }
        }
        let d = std::env::temp_dir().join(format!("ocoeco-in-full-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("lenses")).unwrap();
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nnotes = \"data/notes.json\"\n").unwrap();
        std::fs::write(
            d.join("data/notes.json"),
            "{\"id\":\"a\",\"n\":\"x\",\"why\":\"for a reason\",\"left\":\"\"}\n{\"id\":\"b\",\"n\":\"y\"}\n",
        )
        .unwrap();
        std::fs::write(d.join("lenses/keeper.toml"), "[lens]\nid = \"keeper\"\nsays = \"Declares the notes.\"\n[tables.notes]\nkey = [\"id\"]\nextra_fields = true\nfields = [{ name = \"id\", type = \"string\", required = true }, { name = \"n\", type = \"string\" }]\n[[focus]]\ntitle = \"kept ({count})\"\nsql = \"SELECT n FROM notes\"\nline = \"{n}\"\n").unwrap();
        std::fs::write(d.join("lenses/whole.toml"), "[lens]\nid = \"whole\"\nsays = \"The thing stood on, in full.\"\nreads = [\"in_full\"]\n[[focus]]\ntitle = \"itself ({count})\"\nsql = \"SELECT field, value FROM in_full\"\nline = \"{field}: {value}\"\n").unwrap();
        let places = [d.clone()];
        let all = crate::lenses::with(&places).unwrap();
        // named by its place in full or by its star: each field that says something, as it is kept
        for name in ["o/r/notes:a", "r/notes:a"] {
            let said: Vec<(String, String)> = in_full(name, &all, &places)
                .iter()
                .map(|row| {
                    (
                        row["field"].as_str().unwrap().to_string(),
                        row["value"].as_str().unwrap().to_string(),
                    )
                })
                .collect();
            assert_eq!(
                said,
                [
                    ("n".to_string(), "x".to_string()),
                    ("why".to_string(), "for a reason".to_string())
                ],
                "{name}"
            );
        }
        assert!(in_full("r/notes:nothing-so-called", &all, &places).is_empty());
        // a lens that reads it is looked through from a thing, and is given that thing
        let whole = all.iter().find(|l| l.lens.id == "whole").unwrap();
        assert!(says_here(whole));
        let light = beam("whole@r/notes:a", &places, "2026-11-15", &Nothing).unwrap();
        assert_eq!(
            light[0].lines,
            ["in_full - 2 rows: the thing stood on, each field as it is kept"]
        );
    }

    #[test]
    fn every_answer_of_a_focus_is_its_own_and_those_of_what_is_placed_in_it() {
        let answer = |n: &str| Map::from_iter([("n".to_string(), Value::from(n))]);
        let placed = Brought {
            values: vec![answer("placed")],
            ..Default::default()
        };
        let parts = [
            Brought {
                values: vec![answer("own")],
                ..Default::default()
            },
            Brought {
                within: vec![placed],
                ..Default::default()
            },
        ];
        let said: Vec<&str> = every_answer(&parts)
            .iter()
            .filter_map(|a| a["n"].as_str())
            .collect();
        assert_eq!(said, ["own", "placed"]);
    }

    #[test]
    fn a_part_names_a_shape_that_other_parts_ask_of() {
        /// An engine that answers a part asking of the shape - and finds the shape made ready before it.
        struct Shaped;
        impl SqlEngine for Shaped {
            fn run(&self, script: &str, _format: &str) -> Result<String> {
                if script.trim_end().ends_with("FROM rests;") {
                    let made = script
                        .find("CREATE OR REPLACE VIEW \"rests\" AS SELECT 1 AS n")
                        .expect("the shape is made ready");
                    assert!(
                        made < script.rfind("SELECT n FROM rests").unwrap(),
                        "before the part that asks of it"
                    );
                    return Ok(r#"[{"n":"one"}]"#.into());
                }
                Ok("[]".into())
            }
        }
        let d = std::env::temp_dir().join(format!("ocoeco-shape-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("lenses")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n").unwrap();
        std::fs::write(d.join("lenses/shaped.toml"), "[lens]\nid = \"shaped\"\nsays = \"Names a shape, and asks of it.\"\n[[focus]]\nnamed = \"rests\"\nsql = \"SELECT 1 AS n\"\n[[focus]]\ntitle = \"asked ({count})\"\nsql = \"SELECT n FROM rests\"\nline = \"{n}\"\n").unwrap();
        let parts = brought("shaped", std::slice::from_ref(&d), "2026-11-14", &Shaped).unwrap();
        let laid: Vec<(&str, &[String])> = parts
            .iter()
            .map(|p| (p.title.as_deref().unwrap_or_default(), p.lines.as_slice()))
            .collect();
        assert_eq!(
            laid,
            [("asked (1)", &["one".to_string()][..])],
            "the shape is passed on through, not laid out; the part that asks of it is"
        );
    }

    #[test]
    fn a_beam_shows_how_light_travels_through_a_lens() {
        /// An engine that answers the one part that asks, with two answers.
        struct Two;
        impl SqlEngine for Two {
            fn run(&self, script: &str, _format: &str) -> Result<String> {
                Ok(if script.contains("-- kept") {
                    r#"[{"n":"one"},{"n":"two"}]"#.into()
                } else {
                    "[]".into()
                })
            }
        }
        let d = std::env::temp_dir().join(format!("ocoeco-beam-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("lenses")).unwrap();
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nnotes = \"data/notes.json\"\n").unwrap();
        std::fs::write(
            d.join("data/notes.json"),
            "{\"id\":\"a\",\"n\":\"x\"}\n{\"id\":\"b\",\"n\":\"y\"}\n{\"id\":\"c\",\"n\":\"z\"}\n",
        )
        .unwrap();
        std::fs::write(d.join("lenses/inner.toml"), "[lens]\nid = \"inner\"\nsays = \"Asks one thing of the notes.\"\n[tables.notes]\nkey = [\"id\"]\nfields = [{ name = \"id\", type = \"string\", required = true }, { name = \"n\", type = \"string\" }]\n[[focus]]\ntitle = \"kept ({count})\"\nsql = \"SELECT n FROM notes -- kept\"\nline = \"{n}\"\n").unwrap();
        std::fs::write(d.join("lenses/sheet.toml"), "[lens]\nid = \"sheet\"\nsays = \"Places the inner lens, and asks nothing.\"\n[[focus]]\nplaces = \"inner\"\nonly = \"kept\"\n").unwrap();
        let places = [d.clone()];
        let inner = beam("inner", &places, "2026-11-13", &Two).unwrap();
        let titles: Vec<&str> = inner
            .iter()
            .map(|p| p.title.as_deref().unwrap_or_default())
            .collect();
        assert_eq!(
            titles,
            ["light in (1)", "through the focus (1)", "on, from here (2)"]
        );
        assert_eq!(
            inner[0].lines,
            ["notes - 3 rows from 1 place(s) held here, as it is now"],
            "what enters, and from where"
        );
        assert_eq!(
            inner[1].lines,
            ["1. kept - asks of notes: 3 rows in, 2 through"],
            "what the focus lets pass"
        );
        assert_eq!(
            inner[1].measures,
            [Some((2.0, 3.0))],
            "and how much of the light that is"
        );
        assert_eq!(inner[2].lines[0], "placed in: sheet", "where it goes on to");
        assert!(
            unsound(&inner).is_empty(),
            "a beam is a sound projection: {:?}",
            unsound(&inner)
        );
        let sheet = beam("sheet", &places, "2026-11-13", &Two).unwrap();
        assert_eq!(
            sheet[0].lines,
            ["nothing of its own: it only places what other lenses bring"]
        );
        assert_eq!(
            sheet[1].lines,
            ["1. placed from inner - only \"kept\": 2 line(s) come through"]
        );
    }

    #[test]
    fn a_lens_of_lenses_places_what_other_lenses_bring_and_asks_nothing_itself() {
        /// An engine that answers each query by a word written in it.
        struct ByWord;
        impl SqlEngine for ByWord {
            fn run(&self, script: &str, _format: &str) -> Result<String> {
                Ok(if script.contains("-- kept") {
                    r#"[{"n":"one"}]"#.into()
                } else if script.contains("-- other") {
                    r#"[{"n":"two"}]"#.into()
                } else {
                    "[]".into()
                })
            }
        }
        let d = std::env::temp_dir().join(format!("ocoeco-sheet-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("lenses")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n").unwrap();
        let part = |title: &str, word: &str| {
            format!("[[focus]]\ntitle = \"{title} ({{count}})\"\nsql = \"SELECT 1 AS n -- {word}\"\nline = \"{{n}}\"\n")
        };
        std::fs::write(
            d.join("lenses/inner.toml"),
            format!(
                "[lens]\nid = \"inner\"\nsays = \"Asks two things of the data.\"\n{}{}",
                part("kept", "kept"),
                part("other", "other")
            ),
        )
        .unwrap();
        std::fs::write(d.join("lenses/sheet.toml"), "[lens]\nid = \"sheet\"\nsays = \"A sheet: places what the inner lens brings, and asks nothing.\"\n[[focus]]\nplaces = \"inner\"\n[[focus]]\nplaces = \"inner\"\nonly = \"kept\"\n").unwrap();
        let places = [d.clone()];
        let sheet = brought("sheet", &places, "2026-11-13", &ByWord).unwrap();
        // the whole of the inner lens, as a plane within the sheet - its own shape kept; then only its part
        // under one heading, among the sheet's own
        assert_eq!(sheet.len(), 2, "{sheet:?}");
        assert_eq!(
            (
                sheet[0].title.as_deref(),
                sheet[0].about.as_deref(),
                sheet[0]
                    .within
                    .iter()
                    .map(|p| p.title.clone().unwrap_or_default())
                    .collect::<Vec<_>>()
            ),
            (
                Some("inner"),
                Some("Asks two things of the data."),
                vec!["kept (1)".to_string(), "other (1)".to_string()]
            )
        );
        assert_eq!(
            (sheet[1].title.as_deref(), sheet[1].lines.as_slice()),
            (Some("kept (1)"), &["one".to_string()][..])
        );
        // a lens that is not in view from here is said to be so, and the rest is shown
        std::fs::write(d.join("lenses/far.toml"), "[lens]\nid = \"far\"\nsays = \"Places a lens that only another place brings.\"\n[[focus]]\nplaces = \"brought-elsewhere\"\n[[focus]]\nplaces = \"inner\"\nonly = \"kept\"\n").unwrap();
        let far = brought("far", &places, "2026-11-13", &ByWord).unwrap();
        assert_eq!(
            far.iter()
                .map(|p| p.title.as_deref().unwrap_or_default())
                .collect::<Vec<_>>(),
            ["brought-elsewhere: not in view from here", "kept (1)"]
        );
        // lenses placed in a ring are said, not followed for ever
        std::fs::write(d.join("lenses/ring.toml"), "[lens]\nid = \"ring\"\nsays = \"Places itself, which cannot be.\"\n[[focus]]\nplaces = \"ring\"\n").unwrap();
        assert!(brought("ring", &places, "2026-11-13", &ByWord)
            .unwrap_err()
            .to_string()
            .contains("placed in a ring"));
    }

    #[test]
    fn a_line_is_laid_out_from_an_answer() {
        let answer: Map<String, Value> =
            serde_json::from_str(r#"{"a":"one","n":3,"gone":null}"#).unwrap();
        assert_eq!(
            laid_out("{a} and {n}{gone}; {unknown} stays", &answer),
            "one and 3; {unknown} stays"
        );
    }

    #[test]
    fn a_focus_is_its_parts_and_an_empty_part_is_left_out() {
        let d = std::env::temp_dir().join(format!("ocoeco-focus-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nbacklog = \"data/backlog.json\"\n").unwrap();
        std::fs::write(
            d.join("data/backlog.json"),
            "{\"id\":\"a\",\"title\":\"A\",\"status\":\"in-progress\"}\n",
        )
        .unwrap();
        let text = focus("work", &[d], "2026-11-12", &StandIn).unwrap();
        assert_eq!(
            text,
            "in progress - read off what was done (1)\n  claude: r/a - A - next: x (2 time(s), the last at 2026-11-01 10:00)\nopen 2 and parked 0, across 1 stars\n"
        );
        assert!(focus("no-such-lens", &[], "2026-11-12", &StandIn)
            .unwrap_err()
            .to_string()
            .contains("no lens or table is called no-such-lens: there are the lenses "));
    }
}

#[cfg(test)]
mod the_identity {
    use super::*;

    /// An engine that answers the passthrough of the table notes with one row.
    struct Notes;
    impl SqlEngine for Notes {
        fn run(&self, script: &str, _format: &str) -> Result<String> {
            Ok(
                if script
                    .trim_end()
                    .ends_with("SELECT * FROM \"notes\" ORDER BY seq;")
                {
                    r#"[{"id":"a","star":"r","seq":1,"n":"x","empty":""}]"#.into()
                } else {
                    "[]".into()
                },
            )
        }
    }

    #[test]
    fn a_table_is_looked_through_as_itself_with_nothing_declared_but_its_name() {
        let d = std::env::temp_dir().join(format!("ocoeco-identity-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("lenses")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n").unwrap();
        std::fs::write(d.join("lenses/inner.toml"), "[lens]\nid = \"inner\"\nsays = \"Defines the notes.\"\n[tables.notes]\nkey = [\"id\"]\nfields = [{ name = \"id\", type = \"string\", required = true }, { name = \"n\", type = \"string\" }]\n").unwrap();
        let parts = brought("notes", std::slice::from_ref(&d), "2026-11-14", &Notes).unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(
            (parts[0].title.as_deref(), parts[0].lines.as_slice()),
            (Some("notes (1)"), &["r/a - n: x".to_string()][..]),
            "the row itself, every field as it is, an empty one left out"
        );
        assert!(brought(
            "nothing-of-this-name",
            std::slice::from_ref(&d),
            "2026-11-14",
            &Notes
        )
        .unwrap_err()
        .to_string()
        .contains("no lens or table is called"));
    }
}

#[cfg(test)]
mod from_a_thing {
    use super::*;

    #[test]
    fn a_lens_looked_through_from_a_thing_says_where_it_stands() {
        let lens: Lens = toml::from_str("[lens]\nid = \"around\"\nsays = \"From one thing.\"\n[[focus]]\ntitle = \"from {here} ({count})\"\nsql = \"SELECT 1 FROM edges WHERE \\\"from\\\" = '{here}'\"\nline = \"x\"\n").unwrap();
        let stood = standing(&lens, "o/r/notes:it's");
        assert_eq!(
            stood.focus[0].sql, "SELECT 1 FROM edges WHERE \"from\" = 'o/r/notes:it''s'",
            "where one stands, said safely"
        );
        assert_eq!(
            stood.focus[0].title.as_deref(),
            Some("from o/r/notes:it's ({count})")
        );
    }
}

#[cfg(test)]
mod a_table_no_lens_declares {
    use super::*;

    #[test]
    fn its_shape_is_read_off_the_rows_a_place_keeps() {
        let d = std::env::temp_dir().join(format!("ocoeco-read-off-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/r\"\nname = \"R: a star\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nplain = \"data/plain.json\"\n").unwrap();
        std::fs::write(
            d.join("data/plain.json"),
            "{\"id\":\"p\",\"n\":3,\"said\":\"x\"}\n{\"id\":\"q\",\"n\":2.5,\"sure\":true}\n",
        )
        .unwrap();
        let declared = read_off("plain", std::slice::from_ref(&d)).expect("a table a place keeps");
        let kinds: Vec<(&str, &str)> = declared["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| (f["name"].as_str().unwrap(), f["type"].as_str().unwrap()))
            .collect();
        assert_eq!(
            kinds,
            [
                ("id", "string"),
                ("n", "number"),
                ("said", "string"),
                ("sure", "boolean")
            ],
            "a whole number and a number together are a number"
        );
        assert!(read_off("kept-by-no-place", std::slice::from_ref(&d)).is_none());
        assert!(
            identity("plain", &[], std::slice::from_ref(&d)).is_some(),
            "and it is looked through as itself"
        );
    }
}

#[cfg(test)]
mod a_surface_said_by_a_lens {
    use super::*;

    #[test]
    fn a_page_closes_what_it_opens_folds_what_follows_and_stands_like_parts_side_by_side() {
        let d = std::env::temp_dir()
            .join(format!("ocoeco-as-page-{}", std::process::id()))
            .join("o/p");
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::create_dir_all(d.join("lenses")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/p\"\nname = \"P: a place\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nbacklog = \"data/backlog.json\"\nproblems = \"data/problems.json\"\n").unwrap();
        std::fs::write(d.join("data/backlog.json"), "{\"id\":\"a\",\"title\":\"A\",\"status\":\"in-progress\",\"driver\":\"x\",\"opened\":\"2026-01-01\"}\n{\"id\":\"b\",\"title\":\"B\",\"status\":\"next\",\"opened\":\"2026-01-01\"}\n").unwrap();
        std::fs::write(d.join("data/problems.json"), "{\"id\":\"s\",\"kind\":\"lacking\",\"what\":\"it lacks\",\"date\":\"2026-01-01\",\"by\":\"x\"}\n").unwrap();
        // the place says what it shows first, and keeps a lens whose part is laid out once for each status
        std::fs::write(
            d.join("lenses/first.toml"),
            "[lens]\nid = \"first\"\nsays = \"What this place shows first.\"\n\n[[focus]]\nplaces = \"signs\"\n\n[[focus]]\nplaces = \"*\"\n",
        )
        .unwrap();
        std::fs::write(
            d.join("lenses/by-status.toml"),
            "[lens]\nid = \"by-status\"\nsays = \"Items, by their status.\"\nreads = [\"backlog\"]\n\n[[focus]]\ntitle = \"{status}\"\nsql = \"SELECT status, title FROM backlog ORDER BY status, id\"\nline = \"{title}\"\nper = \"status\"\n",
        )
        .unwrap();
        // held where there is an engine to ask: where there is none - the check on GitHub - it steps aside
        let Ok(engine) = tad::sql::DuckDb::find() else {
            eprintln!("no engine here: a page is held to closing what it opens where there is one");
            return;
        };
        let dirs = [d.clone()];
        let page = |lens: &str| {
            crate::dome::surface("lines", 80).unwrap().lay_out(
                &brought(&format!("as-page@{lens}"), &dirs, "2026-01-02", &engine).unwrap(),
            )
        };
        let times = |page: &str, piece: &str| page.matches(piece).count();
        // whatever a lens brings, the page closes each piece it opens
        for lens in [
            "fulldome",
            "first",
            "by-status",
            "work",
            "signs",
            "purpose",
            "insight",
        ] {
            let page = page(lens);
            for (opens, closes) in [
                ("<details", "</details>"),
                ("<summary>", "</summary>"),
                ("<section", "</section>"),
                ("<div", "</div>"),
                ("<ul>", "</ul>"),
                ("<li", "</li>"),
            ] {
                assert_eq!(
                    times(&page, opens),
                    times(&page, closes),
                    "{lens}: every {opens} is closed"
                );
            }
        }
        // what a place put first, by name, is open; what came as one of every other is folded
        let first = page("first");
        assert_eq!(
            times(&first, "<details class=\"lens\" id=\"signs\" open>"),
            1,
            "the lens the place put first is a card, open"
        );
        assert_eq!(times(&first, "\" open>"), 1, "and every other is folded");
        assert!(times(&first, "<details class=\"lens\"") > 5);
        assert_eq!(times(&page("fulldome"), "\" open>"), 0);
        // parts laid out once for each of something, each short, stand side by side
        let by_status = page("by-status");
        assert_eq!(times(&by_status, "<div class=\"each\">"), 1);
        assert_eq!(times(&by_status, "<section class=\"part"), 2);
    }

    #[test]
    fn a_plane_laid_out_by_the_lens_is_what_the_text_surface_gives() {
        let d = std::env::temp_dir()
            .join(format!("ocoeco-as-text-{}", std::process::id()))
            .join("o/p");
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/p\"\nname = \"P: a place\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nbacklog = \"data/backlog.json\"\nproblems = \"data/problems.json\"\n").unwrap();
        std::fs::write(d.join("data/backlog.json"), "{\"id\":\"a\",\"title\":\"A\",\"status\":\"in-progress\",\"driver\":\"x\",\"opened\":\"2026-01-01\"}\n{\"id\":\"b\",\"title\":\"B\",\"status\":\"next\",\"opened\":\"2026-01-01\"}\n").unwrap();
        std::fs::write(d.join("data/problems.json"), "{\"id\":\"s\",\"kind\":\"lacking\",\"what\":\"it lacks\",\"date\":\"2026-01-01\",\"by\":\"x\"}\n").unwrap();
        // held where there is an engine to ask: where there is none - the check on GitHub - it steps aside
        let Ok(engine) = tad::sql::DuckDb::find() else {
            eprintln!("no engine here: a surface said by a lens is held equal to the program's where there is one");
            return;
        };
        let dirs = [d.clone()];
        let on = |surface: &str, lens: &str| {
            crate::dome::surface(surface, 80)
                .unwrap()
                .lay_out(&brought(lens, &dirs, "2026-01-02", &engine).unwrap())
        };
        for lens in ["work", "signs", "graph", "insight", "fulldome"] {
            let by_the_program = on("text", lens);
            assert!(
                !by_the_program.is_empty(),
                "the lens {lens} brings something"
            );
            assert_eq!(
                on("lines", &format!("as-text@{lens}")),
                by_the_program,
                "the lens {lens}, laid out by as-text"
            );
        }
        // looked through from nothing, a surface has nothing to lay out - and that is no failure
        assert!(on("lines", "as-text").is_empty());
        // and a surface is not a part of the whole: the fulldome places every lens but the surfaces
        assert!(!on("text", "fulldome").contains("as-text:"));
    }
}

#[cfg(test)]
mod what_a_line_is_about {
    use super::*;

    #[test]
    fn a_plane_says_the_thing_each_line_is_about_where_its_answer_names_one() {
        let d = std::env::temp_dir()
            .join(format!("ocoeco-about-{}", std::process::id()))
            .join("o/p");
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(d.join("oco.toml"), "schema = 1\n[project]\nid = \"o/p\"\nname = \"P: a place\"\nkind = \"tool\"\nparent = \"o\"\n[exports]\nbacklog = \"data/backlog.json\"\nproblems = \"data/problems.json\"\n").unwrap();
        std::fs::write(
            d.join("data/backlog.json"),
            "{\"id\":\"a\",\"title\":\"A\"}\n{\"id\":\"both\",\"title\":\"B\"}\n",
        )
        .unwrap();
        std::fs::write(d.join("data/problems.json"), "{\"id\":\"s\",\"what\":\"it lacks\"}\n{\"id\":\"both\",\"what\":\"two of one name\"}\n").unwrap();
        let named = Named::of(std::slice::from_ref(&d)).unwrap();
        let answer = |pairs: &[(&str, &str)]| {
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), Value::from(*v)))
                .collect::<Map<String, Value>>()
        };
        // the value an answer calls thing; or any value that is a thing's name
        assert_eq!(
            named.thing(&answer(&[("thing", "p/backlog:a")])).as_deref(),
            Some("p/backlog:a")
        );
        assert_eq!(
            named
                .thing(&answer(&[("root", "p/problems:s"), ("says", "words")]))
                .as_deref(),
            Some("p/problems:s")
        );
        // or its star and id, where they are one thing only - and not where they could be two
        assert_eq!(
            named
                .thing(&answer(&[("star", "p"), ("id", "s")]))
                .as_deref(),
            Some("p/problems:s")
        );
        assert_eq!(named.thing(&answer(&[("star", "p"), ("id", "both")])), None);
        assert_eq!(
            named.thing(&answer(&[("says", "p/backlog:a is spoken of here")])),
            None,
            "words that speak of a thing do not name it"
        );
        // a thing's whole name names it too - and every name that names it alone is known
        assert_eq!(
            named
                .thing(&answer(&[("of_thing", "o/p/backlog:a")]))
                .as_deref(),
            Some("o/p/backlog:a")
        );
        assert_eq!(
            named.names_of("p/backlog:a"),
            ["o/p/backlog:a", "p/backlog:a"]
        );
        assert_eq!(
            named.names_of("o/p/backlog:a"),
            ["o/p/backlog:a", "p/backlog:a"]
        );
        assert!(named.names_of("p/backlog:nothing").is_empty());
        // and the plane carries it, line by line
        let part = Brought {
            title: Some("showing (1)".into()),
            lines: vec!["p/s - it lacks".into(), "a line about nothing".into()],
            values: vec![
                answer(&[("star", "p"), ("id", "s")]),
                answer(&[("said", "x")]),
            ],
            ..Default::default()
        };
        let plane = as_plane(&[part], &named);
        let about: Vec<Option<&str>> = plane
            .iter()
            .map(|row| row.get("thing").and_then(|t| t.as_str()))
            .collect();
        assert_eq!(
            about,
            [None, Some("p/problems:s"), None],
            "a heading, a line about a thing, a line about none"
        );
        // and a mark for each thing, in the order they first appear: what one says to stand there, where
        // one cannot point
        let marks: Vec<Option<u64>> = plane
            .iter()
            .map(|row| row.get("mark").and_then(|m| m.as_u64()))
            .collect();
        assert_eq!(marks, [None, Some(1), None]);
    }
}

#[cfg(test)]
mod asked_in_one_go {
    use super::*;

    /// An engine that answers each question it is given with one row saying which it was - and keeps
    /// count of how often it was run.
    struct Counting(std::cell::Cell<usize>);
    impl SqlEngine for Counting {
        fn run(&self, script: &str, _format: &str) -> Result<String> {
            self.0.set(self.0.get() + 1);
            Ok(script
                .lines()
                .filter(|line| line.starts_with("SELECT"))
                .map(|line| {
                    match line
                        .strip_prefix("SELECT '")
                        .and_then(|rest| rest.split_once("' AS between_parts"))
                    {
                        Some((between, _)) => format!("[{{\"between_parts\":\"{between}\"}}]\n"),
                        None if line.contains("nothing") => String::new(),
                        None => format!(
                            "[{{\"asked\":\"{}\"}},\n{{\"asked\":\"again\"}}]\n",
                            line.trim_end_matches(';')
                        ),
                    }
                })
                .collect())
        }
    }

    #[test]
    fn the_parts_of_a_lens_are_asked_in_one_go_and_each_finds_its_own_answer() {
        let engine = Counting(std::cell::Cell::new(0));
        let questions = [
            "SELECT one".to_string(),
            "SELECT nothing".to_string(),
            "SELECT three;".to_string(),
        ];
        let all = answers_to_all(&engine, "CREATE TABLE t AS SELECT 1;\n", &questions).unwrap();
        assert_eq!(engine.0.get(), 1, "the engine is run once for all of them");
        assert_eq!(all.len(), 3);
        assert_eq!(all[0][0]["asked"], "SELECT one");
        assert_eq!(all[0].len(), 2, "every row of an answer is its own");
        assert!(
            all[1].is_empty(),
            "a part with no answer has none - and the next is still its own"
        );
        assert_eq!(all[2][0]["asked"], "SELECT three");
        // one question alone is asked as it always was
        assert!(answers_to_all(&engine, "", &questions[..1]).is_none());
        // and an engine that answers some other way is not believed: each part then asks for itself
        struct Otherwise;
        impl SqlEngine for Otherwise {
            fn run(&self, _script: &str, _format: &str) -> Result<String> {
                Ok("[{\"asked\":\"whatever\"}]\n".into())
            }
        }
        assert!(answers_to_all(&Otherwise, "", &questions).is_none());
    }
}

#[cfg(test)]
mod how_a_part_came_to_be_placed {
    use super::*;

    #[test]
    fn a_plane_says_which_lens_came_as_one_of_every_and_what_a_part_is_laid_out_once_for() {
        let of_a_star = |name: &str| Brought {
            title: Some(name.into()),
            lines: vec!["why it exists".into()],
            per: Some("star".into()),
            ..Default::default()
        };
        let placed = |name: &str, of_every: bool| Brought {
            title: Some(name.into()),
            about: Some("what it says of itself".into()),
            of_every,
            within: vec![Brought {
                title: Some("a part laid out once".into()),
                lines: vec!["a line".into()],
                ..Default::default()
            }],
            ..Default::default()
        };
        let plane = as_plane(
            &[
                of_a_star("a"),
                of_a_star("b"),
                placed("signs", false),
                placed("work", true),
            ],
            &Named::default(),
        );
        let said = |kind: &str, field: &str| -> Vec<Value> {
            plane
                .iter()
                .filter(|row| row["kind"] == kind)
                .map(|row| row.get(field).cloned().unwrap_or(Value::Null))
                .collect()
        };
        assert_eq!(
            said("lens", "of_every"),
            [Value::from(false), Value::from(true)],
            "what a place put first by name, and what came as one of every other"
        );
        assert_eq!(
            said("heading", "per"),
            [
                Value::from("star"),
                Value::from("star"),
                Value::Null,
                Value::Null
            ],
            "a part laid out once for each star says so; a part laid out once says nothing of it"
        );
    }
}

#[cfg(test)]
mod a_heading_leads_to_what_it_is_for {
    use super::*;

    #[test]
    fn a_part_laid_out_for_each_star_is_headed_by_that_star_and_leads_to_its_page() {
        let mut named = Named::default();
        named
            .whole
            .insert("o/a/places:a".into(), "a/places:a".into());
        named
            .short
            .insert("a/places:a".into(), Some("o/a/places:a".into()));
        let laid_for = |column: &str, value: &str| Brought {
            title: Some(value.into()),
            lines: vec!["a line".into()],
            values: vec![Map::from_iter([(column.to_string(), Value::from(value))])],
            per: Some(column.into()),
            ..Default::default()
        };
        pages_are(std::collections::BTreeMap::from([(
            "a/places:a".to_string(),
            "places/o/a.html".to_string(),
        )]));
        let plane = as_plane(&[laid_for("star", "a"), laid_for("status", "next")], &named);
        pages_are(Default::default());
        let headings: Vec<(Option<&str>, Option<&str>)> = plane
            .iter()
            .filter(|row| row["kind"] == "heading")
            .map(|row| {
                (
                    row.get("thing").and_then(|t| t.as_str()),
                    row.get("page").and_then(|p| p.as_str()),
                )
            })
            .collect();
        assert_eq!(
            headings,
            [(Some("a/places:a"), Some("places/o/a.html")), (None, None)],
            "a star's part is headed by its place, and leads to its page; a state names no thing"
        );
    }
}
