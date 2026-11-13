//! Focus: a lens brings data into view. A lens's focus is declared as data (lenses/*.toml): parts, each
//! a query over the lens's tables and a line saying how one answer is laid out. The data of every place
//! asked for is gathered - each row with its star - and the queries are run by tad's SQL engine. Nothing
//! is made: the same data, brought into focus; and what is seen is laid out as plain text - the first
//! of the ways a focus may be laid out.

use crate::dome::Dome;
use crate::join::rows;
use crate::lenses::Lens;
use crate::mark::Mark;
use anyhow::{bail, Context, Result};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use tad::sql::SqlEngine;

/// Gathers the lens's tables from the places at `dirs` into `into`, each row with its star and seq -
/// its place in the order gathered - and returns the declaration of what was gathered.
fn gather(lens: &Lens, dirs: &[PathBuf], into: &Path) -> Result<tad::Declaration> {
    std::fs::create_dir_all(into)?;
    let mut tables = toml::Table::new();
    // its own tables, and those it reads from other lenses: one lens defines a piece of data, many read it
    let all = crate::lenses::tables();
    let read: Vec<(String, toml::Value)> = lens
        .lens
        .reads
        .iter()
        .filter_map(|t| all.get(t).map(|d| (t.clone(), d.clone())))
        .collect();
    for (table, declared) in lens
        .tables
        .iter()
        .map(|(t, d)| (t.clone(), d.clone()))
        .chain(read)
    {
        let (table, declared) = (&table, &declared);
        let (mut text, mut seq) = (String::new(), 0u64);
        // the lenses themselves, as rows: kept by no place - they are the language, read as data
        let own = crate::lenses::OF_THE_LANGUAGE.contains(&table.as_str());
        if own {
            for row in crate::lenses::as_rows(dirs)?
                .remove(table.as_str())
                .unwrap_or_default()
            {
                seq += 1;
                let mut row = row;
                row.insert("seq".into(), seq.into());
                text.push_str(&serde_json::to_string(&row)?);
                text.push('\n');
            }
        }
        for dir in dirs.iter().filter(|_| !own) {
            // what was observed is a place too: a folder of tables whose every row names its project
            if !dir.join("oco.toml").is_file() && dir.join("projects.json").is_file() {
                for mut fields in crate::harvest::read_rows(&dir.join(format!("{table}.json"))) {
                    let project = fields
                        .get("project")
                        .and_then(|p| p.as_str())
                        .unwrap_or_default()
                        .to_string();
                    seq += 1;
                    fields.insert(
                        "star".into(),
                        project.rsplit('/').next().unwrap_or(&project).into(),
                    );
                    fields.insert("seq".into(), seq.into());
                    text.push_str(&serde_json::to_string(&fields)?);
                    text.push('\n');
                }
                continue;
            }
            let Ok(mark) = std::fs::read_to_string(dir.join("oco.toml"))
                .map_err(anyhow::Error::from)
                .and_then(|t| Mark::parse(&t))
            else {
                continue;
            };
            let star = mark
                .project
                .id
                .rsplit('/')
                .next()
                .unwrap_or(&mark.project.id)
                .to_string();
            for mut row in rows(dir, &mark, table) {
                let Some(fields) = row.as_object_mut() else {
                    continue;
                };
                seq += 1;
                fields.insert("star".into(), star.clone().into());
                fields.insert("seq".into(), seq.into());
                text.push_str(&serde_json::to_string(&row)?);
                text.push('\n');
            }
        }
        std::fs::write(into.join(format!("{table}.json")), text)?;
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
pub fn brought(
    lens: &str,
    dirs: &[PathBuf],
    today: &str,
    engine: &dyn SqlEngine,
) -> Result<Vec<Brought>> {
    let all = crate::lenses::with(dirs)?;
    let Some(found) = all.iter().find(|l| l.lens.id == lens) else {
        bail!(
            "no lens is called {lens}: there are {}",
            all.iter()
                .map(|l| l.lens.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    };
    if found.focus.is_empty() {
        bail!("the lens {lens} has no focus yet");
    }
    static CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let scratch = std::env::temp_dir().join(format!(
        "ocoeco-focus-{}-{}",
        std::process::id(),
        CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let projected = (|| -> Result<(Vec<Brought>, Vec<String>)> {
        let load = tad::sql::load_script(&scratch, &gather(found, dirs, &scratch)?);
        let (mut parts, mut doubts) = (Vec::new(), Vec::new());
        for part in &found.focus {
            // a part may be what another lens brings into focus, placed here - all of it, or the part
            // under one heading. The other lens gathers, shapes and lays out; this one only places.
            if let Some(other) = &part.through {
                thread_local!(static DEEP: std::cell::Cell<u8> = const { std::cell::Cell::new(0) });
                if DEEP.get() > 8 {
                    bail!("the lens {lens} looks through {other}, which comes back to it: lenses placed in a ring");
                }
                DEEP.set(DEEP.get() + 1);
                let theirs = brought(other, dirs, today, engine);
                DEEP.set(DEEP.get() - 1);
                parts.extend(
                    theirs?
                        .into_iter()
                        .filter(|p| !p.title.as_deref().is_some_and(|t| t.starts_with(NOT_SOUND)))
                        .filter(|p| {
                            part.only.as_deref().is_none_or(|only| {
                                p.title.as_deref().is_some_and(|t| named(t) == only)
                            })
                        }),
                );
                continue;
            }
            let all = answers(engine, &load, &part.sql.replace("{today}", today))?;
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
        for (key, line) in part.keys.iter().zip(&part.lines) {
            if !was.contains(key) {
                said.push(format!("{}: {line}", named(title)));
            }
        }
    }
    let in_view: std::collections::BTreeSet<&String> = now
        .iter()
        .filter(|p| p.title.is_some())
        .flat_map(|p| p.keys.iter())
        .collect();
    let mut gone = std::collections::BTreeSet::new();
    for key in before
        .iter()
        .filter(|p| p.title.is_some())
        .flat_map(|p| p.keys.iter())
    {
        if !in_view.contains(key) && gone.insert(key) {
            said.push(format!("no longer in view: {key}"));
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
    static CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let scratch = std::env::temp_dir().join(format!(
        "ocoeco-shape-{}-{}",
        std::process::id(),
        CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let doubts = (|| -> Result<Vec<String>> {
        // the lens's tables, and those it reads, with nothing in them
        let load = tad::sql::load_script(&scratch, &gather(lens, &[], &scratch)?);
        let mut doubts = Vec::new();
        for part in &lens.focus {
            // a part placed from another lens asks nothing here: what must hold is that the lens, and the
            // heading if one is named, are there - told for the lenses built in; one a place brings is
            // met when it is looked through
            if let Some(other) = &part.through {
                if let Some(theirs) = crate::lenses::lenses().iter().find(|l| l.lens.id == *other) {
                    if let Some(only) = part.only.as_deref().filter(|only| {
                        !theirs
                            .focus
                            .iter()
                            .any(|p| p.title.as_deref().map(named) == Some(only))
                    }) {
                        doubts.push(format!("it places the part {only:?} of the lens {other}, which has no part under that heading"));
                    }
                }
                continue;
            }
            if part.sql.trim().is_empty() {
                doubts.push(
                    "a part asks nothing: it has neither a query nor a lens to look through"
                        .to_string(),
                );
                continue;
            }
            let heading = part
                .title
                .as_deref()
                .map(named)
                .unwrap_or("a part with no heading");
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
    let mut problems: Vec<String> = crate::lenses::differing(dir)
        .into_iter()
        .map(|id| format!("the lens {id} kept here is no longer the one built into this ocoeco: what is looked through is not what is kept"))
        .collect();
    let all = match crate::lenses::with(&[dir.to_path_buf()]) {
        Ok(all) => all,
        Err(e) => return vec![format!("{e:#}")],
    };
    for lens in all.iter().filter(|l| !l.focus.is_empty()) {
        match shape(lens, engine) {
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
            Ok(
                if script.contains("status = 'in-progress' ORDER BY 1, seq") {
                    r#"[{"driver":"claude","star":"r","id":"a","title":"A","next":" - next: x","age":null}]"#.into()
                } else if script.contains("count(DISTINCT star)") {
                    r#"[{"open":2,"parked":0,"stars":1}]"#.into()
                } else {
                    "[]".into()
                },
            )
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
                "no longer in view: r/c"
            ]
        );
        assert!(news(&now, &now).is_empty());
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
        std::fs::write(d.join("lenses/sheet.toml"), "[lens]\nid = \"sheet\"\nsays = \"A sheet: places what the inner lens brings, and asks nothing.\"\n[[focus]]\nthrough = \"inner\"\n[[focus]]\nthrough = \"inner\"\nonly = \"kept\"\n").unwrap();
        let places = [d.clone()];
        let sheet = brought("sheet", &places, "2026-11-13", &ByWord).unwrap();
        let laid: Vec<(&str, &str)> = sheet
            .iter()
            .map(|p| (p.title.as_deref().unwrap_or_default(), p.lines[0].as_str()))
            .collect();
        assert_eq!(
            laid,
            [
                ("kept (1)", "one"),
                ("other (1)", "two"),
                ("kept (1)", "one")
            ],
            "all of the inner lens, then only its part under one heading"
        );
        // lenses placed in a ring are said, not followed for ever
        std::fs::write(d.join("lenses/ring.toml"), "[lens]\nid = \"ring\"\nsays = \"Looks through itself, which cannot be.\"\n[[focus]]\nthrough = \"ring\"\n").unwrap();
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
            "in progress (1)\n  claude: r/a - A - next: x\nopen 2 and parked 0, across 1 stars\n"
        );
        assert!(focus("no-such-lens", &[], "2026-11-12", &StandIn)
            .unwrap_err()
            .to_string()
            .contains("no lens is called no-such-lens: there are "));
    }
}
