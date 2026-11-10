//! ocoeco: the observatory's information ecosystem - the mark, discovery and the harvest
//! (see SPEC.md).
//!
//!   ocoeco discover [--kind KIND] [--json]   every marked project, found and confirmed by its mark
//!   ocoeco harvest --out DIR                  their shared tables, each row with its provenance
//!   ocoeco gather --repo OWNER/REPO --out DIR its activity, releases and CI - data made by work

use anyhow::Result;
use clap::{Parser, Subcommand};
use ocoeco::{discover, gather, harvest};

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
}

fn main() -> Result<()> {
    let gh = discover::GitHub::new();
    match Cli::parse().cmd {
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
