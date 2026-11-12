//! ocoeco: the observatory's information ecosystem - the mark, discovery and the harvest
//! (see SPEC.md).
//!
//!   ocoeco discover [--kind KIND] [--json]   every marked project, found and confirmed by its mark
//!   ocoeco harvest --out DIR                  their shared tables, each row with its provenance
//!   ocoeco gather --repo OWNER/REPO --out DIR its activity, releases and CI - data made by work
//!   ocoeco init --id OWNER/REPO --name NAME --kind KIND   what a repository needs to join, written here
//!   ocoeco check                               this repository as the harvest would read it

use anyhow::Result;
use clap::{Parser, Subcommand};
use ocoeco::{discover, gather, glimpse, harvest, join, manual, note, pick};

#[derive(Parser)]
#[command(
    version,
    about = "The observatory's information ecosystem: the mark, discovery, the harvest"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Find every marked project (GitHub: the oco-project topic), each confirmed by its oco.toml
    Discover {
        /// Only projects of this kind
        #[arg(long)]
        kind: Option<String>,
        /// As JSON
        #[arg(long)]
        json: bool,
    },
    /// Read every confirmed project's shared tables into DIR: each row with its project, merged, canonical
    /// Observe every place: gather their tables and what has run there (once called harvest)
    #[command(alias = "harvest")]
    Observe {
        #[arg(long)]
        out: std::path::PathBuf,
    },
    /// Gather a repository's activity, releases and CI into DIR - data made by work
    Gather {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        out: std::path::PathBuf,
    },
    /// Write what this repository needs to join: its mark, the gatherer's caller, a place for its records
    Init {
        /// owner/repo
        #[arg(long)]
        id: String,
        /// "Name: what it is for"
        #[arg(long)]
        name: String,
        /// observatory, canon, lens, picture, conveyor, device-lab, tool or research
        #[arg(long)]
        kind: String,
        /// A short name for navigation
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value = "open-commons-observatory")]
        parent: String,
        #[arg(long, default_value = ".")]
        dir: std::path::PathBuf,
    },
    /// Manual work: record a small action done by hand - bumps it if done before, and keeps when
    Manual {
        /// The action: a verb and its object, at most 12 words
        action: Option<String>,
        /// Bump this record (by its id) instead of matching the words
        #[arg(long)]
        id: Option<String>,
        /// Add it as new even though it resembles a record
        #[arg(long)]
        new: bool,
        /// About how many minutes it takes by hand
        #[arg(long)]
        minutes: Option<i64>,
        /// How draining it was this time, 0-10 (Borg CR10: 0 nothing, 2 easy, 5 hard, 10 maximal)
        #[arg(long)]
        load: Option<i64>,
        #[arg(long, default_value = ".")]
        dir: std::path::PathBuf,
    },
    /// Record: add or update one row of this project's own table, checked against the shared format
    Note {
        /// The table: backlog, decisions, concepts, workings, session_log, idea, milestones, ...
        table: String,
        /// The row's id
        #[arg(long)]
        id: String,
        /// FIELD=VALUE to set (repeatable)
        #[arg(long = "set")]
        set: Vec<String>,
        /// FIELD=VALUE to append to the field's text (repeatable)
        #[arg(long = "add")]
        add: Vec<String>,
        #[arg(long, default_value = ".")]
        dir: std::path::PathBuf,
    },
    /// A glimpse: say what you just did, while it is fresh - one line; ocoeco supplies the rest
    Glimpse {
        /// What was done, in one line (at most 88 characters: it can become a commit title)
        summary: String,
        /// feat, fix, docs, refactor, perf, test, build, ci, chore, style or revert
        #[arg(long, default_value = "docs")]
        kind: String,
        /// What it breaks, if anything
        #[arg(long)]
        breaking: Option<String>,
        /// What it advances (backlog:ID or milestone:ID, repeatable); by default worked out from what was written
        #[arg(long = "advances")]
        advances: Vec<String>,
        /// Minutes it took
        #[arg(long)]
        minutes: Option<String>,
        /// How hard it was, 0-10
        #[arg(long)]
        load: Option<String>,
        /// Who looked (default: OCOECO_BY, else USER)
        #[arg(long)]
        by: Option<String>,
        #[arg(long, default_value = ".")]
        dir: std::path::PathBuf,
    },
    /// Commit what changed, with a message rendered from the glimpses not yet committed
    Commit {
        /// Commit only what is staged, instead of everything changed
        #[arg(long)]
        staged: bool,
        #[arg(long, default_value = ".")]
        dir: std::path::PathBuf,
    },
    /// Bring a lens into focus: what it shows of this place, or of every place under ROOT
    Focus {
        /// The lens: work, ...
        lens: String,
        /// Bring every place under ROOT into focus together (two levels down: owner/repository)
        #[arg(long)]
        root: Option<String>,
        /// The place to bring into focus, when no ROOT is given
        #[arg(long, default_value = ".")]
        dir: std::path::PathBuf,
    },
    /// The board: every marked project's backlog under ROOT as one flow - in progress, waiting, next,
    /// and what needs attention
    Board {
        /// Where the projects are kept, two levels down (owner/repository)
        #[arg(long, default_value = "~/Projects")]
        root: String,
    },
    /// Read this repository as the harvest would: its mark and every table it shares
    Check {
        #[arg(long, default_value = ".")]
        dir: std::path::PathBuf,
        /// Also check the links into other stars, found under ROOT two levels down (owner/repository)
        #[arg(long)]
        root: Option<String>,
    },
}

fn main() -> Result<()> {
    // `ocoeco glimpse` alone, or with an entry a lens declares, is answered from the lenses; anything
    // else - the record of work as it is done among it - goes on as before
    let said: Vec<String> = std::env::args().collect();
    if said.get(1).map(String::as_str) == Some("glimpse") {
        if said.len() == 2 {
            print!("{}", ocoeco::entries::composed());
            return Ok(());
        }
        if ocoeco::entries::is_entry(&said[2]) {
            let today = std::process::Command::new("date")
                .arg("+%F")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            let who = std::env::var("OCOECO_BY")
                .or_else(|_| std::env::var("USER"))
                .unwrap_or_default();
            println!(
                "{}",
                ocoeco::entries::enter(&said[2], &said[3..], &today, &who)?
            );
            return Ok(());
        }
    }
    let gh = discover::GitHub::new();
    match Cli::parse().cmd {
        Cmd::Init {
            id,
            name,
            kind,
            title,
            parent,
            dir,
        } => {
            let today = gather::day(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64 / 86_400)
                    .unwrap_or(0),
            );
            let j = join::Join {
                id,
                name,
                title,
                kind,
                parent,
            };
            let written = join::init(&dir, &j, &today)?;
            for f in &written {
                println!("written: {f}");
            }
            if written.is_empty() {
                println!("nothing to write: everything is already here");
            }
            println!("next: give the repository the topics oco-project and oco-kind-{} (the observatory finds it by them),", j.kind);
            println!("      write its backlog and milestones in oco/, and run `ocoeco check`.");
        }
        Cmd::Manual {
            action,
            id,
            new,
            minutes,
            load,
            dir,
        } => {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let now = format!(
                "{}T{:02}:{:02}:{:02}Z",
                gather::day(secs / 86_400),
                secs % 86_400 / 3_600,
                secs % 3_600 / 60,
                secs % 60
            );
            // no action given: choose it from the list in the terminal, so it is never typed twice
            let chosen = match action {
                Some(a) => Some(pick::Pick {
                    id: id.clone(),
                    action: a,
                    new,
                    minutes,
                    load,
                }),
                None => pick::run(&dir)?,
            };
            match chosen {
                Some(p) => println!(
                    "{}",
                    manual::manual(
                        &dir,
                        &p.action,
                        p.id.as_deref(),
                        p.new,
                        p.minutes,
                        p.load,
                        &now
                    )?
                ),
                None => println!("nothing recorded"),
            }
        }
        Cmd::Note {
            table,
            id,
            set,
            add,
            dir,
        } => {
            let today = gather::day(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64 / 86_400)
                    .unwrap_or(0),
            );
            let mut changes = Vec::new();
            for s in &set {
                changes.push(note::parse_change(s, false)?);
            }
            for s in &add {
                changes.push(note::parse_change(s, true)?);
            }
            println!("{}", note::note(&dir, &table, &id, &changes, &today)?);
            glimpse::remember(&dir, &table, &id)?;
        }
        Cmd::Glimpse {
            summary,
            kind,
            breaking,
            advances,
            minutes,
            load,
            by,
            dir,
        } => {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let (now, id) = glimpse::now_and_id(secs);
            let by = by
                .or_else(|| std::env::var("OCOECO_BY").ok())
                .or_else(|| std::env::var("USER").ok())
                .unwrap_or_else(|| "someone".into());
            let g = glimpse::Glimpse {
                summary,
                kind,
                breaking,
                advances,
                minutes,
                load,
                by,
            };
            println!("{}", glimpse::glimpse(&dir, &g, &now, &id)?);
        }
        Cmd::Commit { staged, dir } => println!("{}", glimpse::commit(&dir, !staged)?),
        Cmd::Focus { lens, root, dir } => {
            let places =
                match root {
                    Some(root) => ocoeco::board::projects(&std::path::PathBuf::from(
                        root.replacen('~', &std::env::var("HOME").unwrap_or_default(), 1),
                    )),
                    None => vec![dir],
                };
            let today = std::process::Command::new("date")
                .arg("+%F")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            print!(
                "{}",
                ocoeco::focus::focus(&lens, &places, &today, &tad::sql::DuckDb::find()?)?
            );
        }
        Cmd::Board { root } => {
            // the board is the work lens, focused on every place under the root
            let home = std::env::var("HOME").unwrap_or_default();
            let places =
                ocoeco::board::projects(&std::path::PathBuf::from(root.replacen('~', &home, 1)));
            let today = std::process::Command::new("date")
                .arg("+%F")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            print!(
                "{}",
                ocoeco::focus::focus("work", &places, &today, &tad::sql::DuckDb::find()?)?
            );
        }
        Cmd::Check { dir, root } => {
            let (well, mut problems) = join::check(&dir)?;
            // the links: every one must lead somewhere - within this star, and with --root into others
            let mut others = std::collections::BTreeMap::new();
            if let Some(root) = root {
                let home = std::env::var("HOME").unwrap_or_default();
                for place in
                    ocoeco::board::projects(&std::path::PathBuf::from(root.replacen('~', &home, 1)))
                {
                    if let Ok(mark) = std::fs::read_to_string(place.join("oco.toml"))
                        .map_err(anyhow::Error::from)
                        .and_then(|t| ocoeco::mark::Mark::parse(&t))
                    {
                        others.insert(mark.project.id, place);
                    }
                }
            }
            problems.extend(ocoeco::relations::broken(&dir, &others)?);
            // and every lens must come into focus over this place: a lens that cannot is not landed
            if let Ok(engine) = tad::sql::DuckDb::find() {
                problems.extend(ocoeco::focus::every_lens_focuses(&dir, &engine));
            }
            for w in &well {
                println!("ok   {w}");
            }
            for p in &problems {
                println!("FAIL {p}");
            }
            if !problems.is_empty() {
                anyhow::bail!("{} problems: the harvest would report them", problems.len());
            }
            let (told, lacks) = join::essence(&dir)?;
            println!(
                "essence: {told} of 4 told{}",
                if lacks.is_empty() {
                    ""
                } else {
                    " - still to tell:"
                }
            );
            for l in &lacks {
                println!("     - {l}");
            }
            for k in join::unexported(&dir)? {
                println!("kept but not exported (the harvest never sees it): {k}");
            }
            let (words, clashes) = join::vocabulary(&dir)?;
            println!("vocabulary: {words} words of its own, beside ocoeco's shared words");
            for c in &clashes {
                println!("     - {c}");
            }
            println!("all is well: the harvest would read this repository");
        }
        Cmd::Gather { repo, out } => {
            let [a, r, c] = gather::gather(&gh, &repo, &out)?;
            println!(
                "{repo}: {a} days of activity, {r} releases, {c} workflows -> {}",
                out.display()
            );
        }
        Cmd::Discover { kind, json } => {
            let found = discover::discover(&[(&gh, &gh)], kind.as_deref())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&found)?);
            } else {
                for c in &found {
                    match (&c.mark, &c.problem) {
                        (Some(m), _) => println!(
                            "{:<44} {:<11} parent {:<26} {} tables",
                            c.found.repo,
                            m.project.kind,
                            m.project.parent,
                            m.exports.len()
                        ),
                        (None, Some(p)) => println!("{:<44} NOT A MARK: {p}", c.found.repo),
                        _ => {}
                    }
                }
                println!(
                    "{} found, {} confirmed",
                    found.len(),
                    found.iter().filter(|c| c.mark.is_some()).count()
                );
            }
        }
        Cmd::Observe { out } => {
            let found = discover::discover(&[(&gh, &gh)], None)?;
            let mut h = harvest::harvest(&found, &gh);
            harvest::observe_runs(&found, &gh, &mut h);
            harvest::write(&mut h, &out)?;
            println!(
                "{} projects, {} tables ({} rows), {} problems -> {}",
                h.projects.len(),
                h.tables.len(),
                h.tables.values().map(Vec::len).sum::<usize>(),
                h.problems.len(),
                out.display()
            );
        }
    }
    Ok(())
}
