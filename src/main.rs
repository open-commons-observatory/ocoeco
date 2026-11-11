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
use ocoeco::{discover, gather, harvest, join, note};

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
    Harvest {
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
    /// Read this repository as the harvest would: its mark and every table it shares
    Check {
        #[arg(long, default_value = ".")]
        dir: std::path::PathBuf,
    },
}

fn main() -> Result<()> {
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
        }
        Cmd::Check { dir } => {
            let (well, problems) = join::check(&dir)?;
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
        Cmd::Harvest { out } => {
            let found = discover::discover(&[(&gh, &gh)], None)?;
            let mut h = harvest::harvest(&found, &gh);
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
