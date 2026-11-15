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
        /// Observe the places held under ROOT (owner/repository, two levels down) instead of searching
        #[arg(long)]
        root: Option<String>,
        /// Observe only what has run (runs), not the places' tables
        #[arg(long)]
        only: Option<String>,
        /// Observe by holding: bring each place found to hand under DIR, as it is on GitHub now, and read it
        /// there as any place held is read - and what a route reads with it
        #[arg(long)]
        hold: Option<std::path::PathBuf>,
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
        /// What it is for: canon, picture, conveyor, device-lab, tool, research (or lens, the older word) -
        /// or, for a place that gathers, constellation or observatory
        #[arg(long)]
        kind: String,
        /// A short name for navigation
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value = "open-commons-observatory")]
        parent: String,
        /// What this place gathers (repeatable): a star as owner/repository, or every star of an owner
        /// as owner/* - for a constellation or an observatory
        #[arg(long = "gathers")]
        gathers: Vec<String>,
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
    /// Take a row back: it is no longer kept here - refused where something still points at it. The
    /// inverse of entering one
    TakeBack {
        /// The table: decisions, problems, ...
        table: String,
        /// The id of the row to take back
        id: String,
        /// The place it is taken back from
        #[arg(long, default_value = ".")]
        dir: std::path::PathBuf,
    },
    /// Move rows of a table from this place to another, every link kept true: links to them - here,
    /// there, and in every place under ROOT - are rewritten to where they now are
    Move {
        /// The table: decisions, research, ...
        table: String,
        /// The ids of the rows to move
        #[arg(required = true)]
        ids: Vec<String>,
        /// The place they move to
        #[arg(long)]
        to: std::path::PathBuf,
        /// Rewrite links in every place under ROOT too (two levels down: owner/repository)
        #[arg(long)]
        root: Option<String>,
        /// The place they move from
        #[arg(long, default_value = ".")]
        dir: std::path::PathBuf,
    },
    /// The fulldome, in the terminal: every lens, and what each brings into focus - to move through
    Fulldome {
        /// Every place under ROOT together (two levels down: owner/repository)
        #[arg(long)]
        root: Option<String>,
        /// What was observed: a folder ocoeco observe wrote, every row naming its project
        #[arg(long)]
        observed: Option<std::path::PathBuf>,
        /// The width of the screen surface, in columns
        #[arg(long, default_value_t = 80)]
        width: usize,
        /// Lay the whole fulldome out on a surface - text or screen, or a lens that says one, as as-page says
        /// the page - instead of opening it
        #[arg(long)]
        surface: Option<String>,
        /// The place, when no ROOT is given
        #[arg(long, default_value = ".")]
        dir: std::path::PathBuf,
    },
    Focus {
        /// The lens: work, ...
        lens: String,
        /// Bring every place under ROOT into focus together (two levels down: owner/repository)
        #[arg(long)]
        root: Option<String>,
        /// The place to bring into focus, when no ROOT is given
        #[arg(long, default_value = ".")]
        dir: std::path::PathBuf,
        /// The width of the screen surface, in columns
        #[arg(long, default_value_t = 80)]
        width: usize,
        /// The surface to lay it out on: text, screen or json - or a lens that says a surface, as as-page says
        /// the page
        #[arg(long, default_value = "text")]
        surface: String,
        /// What was observed: a folder ocoeco observe wrote, every row naming its project
        #[arg(long)]
        observed: Option<std::path::PathBuf>,
        /// Show the lens's beam instead: how light travels through it - what enters, what each part lets
        /// pass, and where it goes on to
        #[arg(long)]
        beam: bool,
        /// Say only what is new in view since the focus kept in FILE, and keep this one there
        #[arg(long)]
        since: Option<std::path::PathBuf>,
        /// Do not end: show it again as what it shows changes - on a screen the whole anew, elsewhere
        /// only what newly came into view. Idle, it waits without load
        #[arg(long)]
        follow: bool,
        /// Look from there in passing: without standing there - no step is left on the trail. For a look
        /// that is not a driver's attention: a comparison, a timing
        #[arg(long)]
        passing: bool,
    },
    /// Lay a site out: every page the lens says there is, each on the surface, written under OUT. A site
    /// is said by a lens - each row of what it brings names a page: where it is written (path), what it
    /// looks through (looks), and the thing it is the page of (of_thing), so that lines about it lead there
    Site {
        /// The lens that says which pages there are
        lens: String,
        /// The surface each page is laid out on: a lens that says a surface
        #[arg(long, default_value = "as-page")]
        surface: String,
        /// Where the pages are written
        #[arg(long)]
        out: std::path::PathBuf,
        /// What an observatory observed, instead of the places held here
        #[arg(long)]
        observed: Option<std::path::PathBuf>,
        /// Every place under ROOT (two levels down: owner/repository)
        #[arg(long)]
        root: Option<String>,
        #[arg(long, default_value = ".")]
        dir: std::path::PathBuf,
    },
    /// What may come next on a typed line, from a blank: read off the language and the rows that exist.
    /// The last word is the one being typed (give "" for a blank). With --shell, answers as a shell asks -
    /// complete -C 'ocoeco fits --shell' ocoeco - from COMP_LINE and COMP_POINT, one word to a line
    Fits {
        /// Answer as a shell asks: the typed line from COMP_LINE and COMP_POINT, the words alone
        #[arg(long)]
        shell: bool,
        /// The place stood in, whose rows a link may point at
        #[arg(long, default_value = ".")]
        dir: std::path::PathBuf,
        /// What is typed after ocoeco, the last being the word being typed
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        words: Vec<String>,
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
    // a look cut short by whoever reads it - piped to something that takes its first lines and stops -
    // ends there, quietly: that the reader stopped reading is no failure of the look
    let as_before = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |why| {
        if why.to_string().contains("failed printing to stdout") {
            std::process::exit(0);
        }
        as_before(why);
    }));
    // what writes rows - glimpse, in both its forms, and note - is guarded: a write that would add a link
    // pointing nowhere is refused, and the place's tables are put back as they were
    let said: Vec<String> = std::env::args().collect();
    if matches!(said.get(1).map(String::as_str), Some("glimpse" | "note")) && said.len() > 2 {
        let dir = said
            .iter()
            .position(|a| a == "--dir")
            .and_then(|i| said.get(i + 1))
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| ".".into());
        if dir.join("oco.toml").is_file() {
            return ocoeco::relations::guarded(&dir, run);
        }
    }
    run()
}

fn run() -> Result<()> {
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
    // said with no command - nothing at all, or words that name none: the window, where there is no
    // terminal to move in. One is shown where one stands, and says a mark, a lens, back or up
    {
        use clap::CommandFactory;
        let names_a_command = |word: &str| {
            word.starts_with('-')
                || word == "help"
                || Cli::command()
                    .get_subcommands()
                    .any(|c| c.get_name() == word)
        };
        if !said.get(1).is_some_and(|word| names_a_command(word)) {
            let today = std::process::Command::new("date")
                .arg("+%F")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            let who = std::env::var("OCOECO_BY")
                .or_else(|_| std::env::var("USER"))
                .unwrap_or_else(|_| "someone".into());
            let engine = tad::sql::DuckDb::find()?;
            print!(
                "{}",
                ocoeco::window::said(&said[1..], std::path::Path::new("."), &who, &today, &engine)?
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
            gathers,
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
            if !gathers.is_empty() && !ocoeco::mark::PLACES.contains(&j.kind.as_str()) {
                anyhow::bail!("only a place that gathers - a constellation or an observatory - says what it gathers; this is a {}", j.kind);
            }
            let written = join::init(&dir, &j, &today)?;
            // a place that gathers says so in its mark, from its first day
            let mark_at = dir.join("oco.toml");
            let mark = std::fs::read_to_string(&mark_at)?;
            if !gathers.is_empty() && !mark.contains("\n[place]") {
                let named: Vec<String> = gathers.iter().map(|g| format!("{g:?}")).collect();
                let with_place = format!("{mark}\n# What place this is, and what it gathers: a star by name, or every star of an owner (owner/*).\n[place]\nis = \"{}\"\ngathers = [{}]\n", j.kind, named.join(", "));
                ocoeco::mark::Mark::parse(&with_place)?;
                std::fs::write(&mark_at, with_place)?;
                println!("written: what it gathers ({})", gathers.join(", "));
            }
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
            // no action given: a record named by its id is counted as it is; with neither, it is chosen
            // from the list in the terminal, so it is never typed twice - and where there is no terminal
            // to choose in, that is said, not left to a window that cannot open
            let chosen = match (action, id.is_some()) {
                (Some(a), _) => Some(pick::Pick {
                    id: id.clone(),
                    action: a,
                    new,
                    minutes,
                    load,
                }),
                (None, true) => Some(pick::Pick {
                    id: id.clone(),
                    action: String::new(),
                    new,
                    minutes,
                    load,
                }),
                (None, false) if !std::io::IsTerminal::is_terminal(&std::io::stdout()) => {
                    anyhow::bail!("say what was done by hand - ocoeco manual \"ACTION\" - or which record to count, --id ID: the list to choose from needs a terminal, and there is none here")
                }
                (None, false) => pick::run(&dir)?,
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
            // where it was made: the things stood on since the last record of work - those held in places
            // kept beside this one - as links from this place; a place stood on is where one looked from,
            // not a row
            let own = std::fs::read_to_string(dir.join("oco.toml"))
                .ok()
                .and_then(|t| ocoeco::mark::Mark::parse(&t).ok())
                .map(|m| m.project.id);
            let beside: Vec<String> = std::fs::canonicalize(&dir)
                .ok()
                .and_then(|d| {
                    d.parent()
                        .and_then(|p| p.parent())
                        .map(ocoeco::board::projects)
                })
                .unwrap_or_default()
                .iter()
                .filter_map(|d| {
                    std::fs::read_to_string(d.join("oco.toml"))
                        .ok()
                        .and_then(|t| ocoeco::mark::Mark::parse(&t).ok())
                        .map(|m| m.project.id)
                })
                .collect();
            let trail = ocoeco::trail::taken(&by);
            // what is there now, among the places kept beside this one: a record of work was made
            // standing on what is there when it is made - a thing since gone, or named anew, is not
            // pointed at
            let there: std::collections::BTreeSet<String> = if trail.is_empty() {
                Default::default()
            } else {
                let dirs = std::fs::canonicalize(&dir)
                    .ok()
                    .and_then(|d| {
                        d.parent()
                            .and_then(|p| p.parent())
                            .map(ocoeco::board::projects)
                    })
                    .unwrap_or_default();
                ocoeco::relations::as_graph(&dirs)
                    .map(|mut graph| graph.remove("things").unwrap_or_default())
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|thing| {
                        thing
                            .get("id")
                            .and_then(|id| id.as_str())
                            .map(str::to_string)
                    })
                    .collect()
            };
            let stood_on: Vec<String> = trail
                .into_iter()
                .filter(|thing| {
                    !thing.contains("/places:")
                        && there.contains(thing)
                        && beside
                            .iter()
                            .any(|place| thing.starts_with(&format!("{place}/")))
                })
                .map(|thing| {
                    match own
                        .as_deref()
                        .and_then(|own| thing.strip_prefix(&format!("{own}/")))
                    {
                        Some(local) => local.to_string(),
                        None => thing,
                    }
                })
                .collect();
            let g = glimpse::Glimpse {
                summary,
                kind,
                breaking,
                advances,
                minutes,
                load,
                by,
                stood_on,
            };
            println!("{}", glimpse::glimpse(&dir, &g, &now, &id)?);
        }
        Cmd::Commit { staged, dir } => println!("{}", glimpse::commit(&dir, !staged)?),
        Cmd::TakeBack { table, id, dir } => {
            // taking a row back is guarded as entering one is: what pointed somewhere still points there
            println!(
                "{}",
                ocoeco::relations::guarded(&dir, || ocoeco::note::take_back(&dir, &table, &id))?
            );
        }
        Cmd::Move {
            table,
            ids,
            to,
            root,
            dir,
        } => {
            let mut others = std::collections::BTreeMap::new();
            if let Some(root) = root {
                for place in ocoeco::board::projects(&std::path::PathBuf::from(root.replacen(
                    '~',
                    &std::env::var("HOME").unwrap_or_default(),
                    1,
                ))) {
                    if let Ok(mark) = std::fs::read_to_string(place.join("oco.toml"))
                        .map_err(anyhow::Error::from)
                        .and_then(|t| ocoeco::mark::Mark::parse(&t))
                    {
                        others.insert(mark.project.id, place);
                    }
                }
            }
            let moved = ocoeco::moving::move_rows(&dir, &to, &table, &ids, &others)?;
            println!("moved {} row(s) of {table} to {}", moved.rows, to.display());
            for (place, links) in &moved.rewritten {
                println!("  {links} link(s) rewritten in {place}");
            }
        }
        Cmd::Fulldome {
            root,
            dir,
            observed,
            surface,
            width,
        } => {
            // an entry can be made only where one place is stood in: it has somewhere to go
            let single = observed.is_none() && root.is_none();
            let (places, scale) = match (observed, root) {
                (Some(observed), _) => (
                    ocoeco::board::observed_from(&dir, observed),
                    "every place observed".to_string(),
                ),
                (None, Some(root)) => (
                    ocoeco::board::projects(&std::path::PathBuf::from(root.replacen(
                        '~',
                        &std::env::var("HOME").unwrap_or_default(),
                        1,
                    ))),
                    format!("every place under {root}"),
                ),
                // the same at every scale: a star shows itself, a place that gathers shows what it gathers
                (None, None) => ocoeco::board::in_view(&dir),
            };
            let today = std::process::Command::new("date")
                .arg("+%F")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            let engine = tad::sql::DuckDb::find()?;
            // the whole looked through: the one the place one looks from says for itself, and where it
            // says none, every lens in view as they come
            let whole =
                ocoeco::lenses::whole_said_at(&dir).unwrap_or_else(|| "fulldome".to_string());
            if let Some(surface) = surface {
                // the whole fulldome, laid out on a surface: the plane-lens that places every lens in view,
                // each keeping its own shape within it - laid out as any plane is, in the surface's frame
                // the page is said by a lens: its old name still asks for it
                let surface = if surface == "html" {
                    "as-page".to_string()
                } else {
                    surface
                };
                // a surface said by a lens lays the whole out itself
                if !ocoeco::dome::SURFACES.contains(&surface.as_str())
                    && ocoeco::lenses::with(&places)?
                        .iter()
                        .any(|l| l.lens.id == surface && ocoeco::focus::lays_out_a_plane(l))
                {
                    let laid = ocoeco::focus::brought(
                        &format!("{surface}@{whole}"),
                        &places,
                        &today,
                        &engine,
                    )?;
                    // a surface may say {scale} in its frame: what is looked at, and when - filled here
                    print!(
                        "{}",
                        ocoeco::dome::surface("lines", width)?
                            .lay_out(&laid)
                            .replace(
                                "{scale}",
                                &ocoeco::dome::escaped(&format!("{scale}, {today}"))
                            )
                            // what the page is called: what the place one looks from is called
                            .replace(
                                "{title}",
                                &ocoeco::dome::escaped(
                                    &ocoeco::lenses::called_at(&dir)
                                        .unwrap_or_else(|| "The fulldome".to_string()),
                                ),
                            )
                            // and what it says of itself, under that: what the whole says it shows
                            .replacen(
                                "{says}",
                                &ocoeco::dome::escaped(
                                    &ocoeco::lenses::with(&places)?
                                        .iter()
                                        .find(|l| l.lens.id == whole)
                                        .map(|l| l.lens.says.clone())
                                        .unwrap_or_default(),
                                ),
                                1,
                            )
                    );
                    return Ok(());
                }
                let plane = ocoeco::focus::brought(&whole, &places, &today, &engine)?;
                print!(
                    "{}",
                    ocoeco::dome::surface(&surface, width)?
                        .whole(&format!("{scale}, {today}"), &plane)
                );
                return Ok(());
            }
            ocoeco::fulldome::open(
                scale,
                single,
                ocoeco::lenses::with(&places)?,
                ocoeco::lenses::whole_said_at(&dir),
                |lens, looking| {
                    // standing on a thing in the dome is a step on the driver's trail, as it is from a shell
                    if let Some(thing) = ocoeco::focus::stood_on(lens, &places) {
                        let who = std::env::var("OCOECO_BY")
                            .or_else(|_| std::env::var("USER"))
                            .unwrap_or_else(|_| "someone".into());
                        ocoeco::trail::step(&who, &thing);
                    }
                    let found = ocoeco::lenses::with(&places)
                        .unwrap_or_default()
                        .into_iter()
                        .find(|l| l.lens.id == lens);
                    match (looking, found) {
                        (ocoeco::fulldome::Looking::Glimpse, Some(found)) => {
                            ocoeco::entries::of(&found)
                        }
                        (ocoeco::fulldome::Looking::Beam, _) => {
                            ocoeco::focus::beam(lens, &places, &today, &engine)
                                .map(|parts| {
                                    ocoeco::dome::surface("text", 80)
                                        .map(|s| s.lay_out(&parts))
                                        .unwrap_or_default()
                                })
                                .unwrap_or_else(|e| format!("{e:#}"))
                        }
                        _ => ocoeco::focus::focus(lens, &places, &today, &engine)
                            .unwrap_or_else(|e| format!("{e:#}")),
                    }
                },
                |entry, args| {
                    // an entry made from the fulldome goes the way ocoeco glimpse goes: guarded, into this place
                    let mut all = args.to_vec();
                    all.extend(["--dir".to_string(), dir.display().to_string()]);
                    let who = std::env::var("OCOECO_BY")
                        .or_else(|_| std::env::var("USER"))
                        .unwrap_or_default();
                    ocoeco::relations::guarded(&dir, || {
                        ocoeco::entries::enter(entry, &all, &today, &who)
                    })
                },
                // which lines of what is looked through are about a thing, and which: as the plane says -
                // the same rows any surface is given - so the window finds nothing by itself
                |lens| {
                    let (Ok(parts), Ok(named)) = (
                        ocoeco::focus::brought(lens, &places, &today, &engine),
                        ocoeco::focus::Named::of(&places),
                    ) else {
                        return Vec::new();
                    };
                    let mut line = 0usize;
                    ocoeco::focus::as_plane(&parts, &named)
                        .iter()
                        .filter_map(|row| {
                            // a lens placed within is laid out after an empty line: two lines of the text
                            line += if row["kind"] == "lens" { 2 } else { 1 };
                            row.get("thing")
                                .and_then(|t| t.as_str())
                                .map(|thing| (line - 1, thing.to_string()))
                        })
                        .collect()
                },
            )?;
        }
        Cmd::Focus {
            lens,
            root,
            dir,
            surface,
            width,
            observed,
            since,
            beam,
            follow,
            passing,
        } => {
            let places = match (observed, root) {
                (Some(observed), _) => ocoeco::board::observed_from(&dir, observed),
                (None, Some(root)) => ocoeco::board::projects(&std::path::PathBuf::from(
                    root.replacen('~', &std::env::var("HOME").unwrap_or_default(), 1),
                )),
                (None, None) => ocoeco::board::in_view(&dir).0,
            };
            // a look that does not end: the same look, asked again whenever what is in view has changed -
            // the files the places keep, what git has of them, where the drivers stand. On a screen the
            // whole is shown anew; elsewhere, only the lines that newly came into view. Idle, it only waits
            if follow {
                use std::io::{IsTerminal, Write};
                let again: Vec<String> = std::env::args()
                    .skip(1)
                    .filter(|a| a != "--follow")
                    .collect();
                let on_a_screen = std::io::stdout().is_terminal();
                let (mut seen, mut shown): (Option<u128>, String) = (None, String::new());
                loop {
                    let now = Some(as_it_stands(&places));
                    if now != seen {
                        seen = now;
                        let asked = std::process::Command::new(std::env::current_exe()?)
                            .args(&again)
                            .output()?;
                        let said = String::from_utf8_lossy(&asked.stdout).to_string();
                        if said != shown {
                            if on_a_screen {
                                print!("\x1b[2J\x1b[H{said}");
                            } else {
                                let before: std::collections::BTreeSet<&str> =
                                    shown.lines().collect();
                                for line in said.lines().filter(|line| !before.contains(line)) {
                                    println!("{line}");
                                }
                            }
                            std::io::stdout().flush()?;
                            shown = said;
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
            let today = std::process::Command::new("date")
                .arg("+%F")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            let engine = tad::sql::DuckDb::find()?;
            // a lens that says where one stands, asked for from nowhere, is looked through from where the
            // driver stands: the last thing stood on, on whatever surface that was
            let who = std::env::var("OCOECO_BY")
                .or_else(|_| std::env::var("USER"))
                .unwrap_or_else(|_| "someone".into());
            let lens = match ocoeco::trail::stands(&who) {
                Some(thing)
                    if !lens.contains('@') && ocoeco::focus::stands_somewhere(&lens, &places) =>
                {
                    format!("{lens}@{thing}")
                }
                _ => lens,
            };
            // the page is said by a lens: its old name still asks for it
            let surface = if surface == "html" {
                "as-page".to_string()
            } else {
                surface
            };
            // a surface may be said by a lens: then that lens is looked through, from the lens asked for,
            // and what it brings is lines laid out already
            let said_by_a_lens = !ocoeco::dome::SURFACES.contains(&surface.as_str())
                && ocoeco::lenses::with(&places)?
                    .iter()
                    .any(|l| l.lens.id == surface && ocoeco::focus::lays_out_a_plane(l));
            let (parts, surface) = if beam {
                (
                    ocoeco::focus::beam(&lens, &places, &today, &engine)?,
                    surface,
                )
            } else if said_by_a_lens {
                (
                    ocoeco::focus::brought(&format!("{surface}@{lens}"), &places, &today, &engine)?,
                    "lines".to_string(),
                )
            } else {
                (
                    ocoeco::focus::brought(&lens, &places, &today, &engine)?,
                    surface,
                )
            };
            // looking through a lens from a thing is a step on the driver's trail: the work, seen where it
            // happens - a record of work reads from it where it was made
            if let Some(thing) = ocoeco::focus::stood_on(&lens, &places).filter(|_| !passing) {
                let who = std::env::var("OCOECO_BY")
                    .or_else(|_| std::env::var("USER"))
                    .unwrap_or_else(|_| "someone".into());
                ocoeco::trail::step(&who, &thing);
            }
            match since {
                // only what is new in view since last time
                Some(kept) => {
                    for new in ocoeco::focus::since(&kept, &parts)? {
                        println!("{new}");
                    }
                }
                None => {
                    let laid = ocoeco::dome::surface(&surface, width)?.lay_out(&parts);
                    // a surface said by a lens may say {scale} in its frame: here, the lens and the day
                    print!(
                        "{}",
                        if said_by_a_lens {
                            laid.replace(
                                "{scale}",
                                &ocoeco::dome::escaped(&format!("{lens}, {today}")),
                            )
                            .replace("{title}", &ocoeco::dome::escaped(&lens))
                            // and what the page says of itself: what the lens looked through says it shows
                            .replacen(
                                "{says}",
                                &ocoeco::dome::escaped(
                                    &ocoeco::lenses::with(&places)?
                                        .iter()
                                        .find(|l| {
                                            Some(l.lens.id.as_str()) == lens.split('@').next()
                                        })
                                        .map(|l| l.lens.says.clone())
                                        .unwrap_or_default(),
                                ),
                                1,
                            )
                        } else {
                            laid
                        }
                    );
                }
            }
        }
        Cmd::Site {
            lens,
            surface,
            out,
            observed,
            root,
            dir,
        } => {
            let places = match (observed, root) {
                (Some(observed), _) => ocoeco::board::observed_from(&dir, observed),
                (None, Some(root)) => ocoeco::board::projects(&std::path::PathBuf::from(
                    root.replacen('~', &std::env::var("HOME").unwrap_or_default(), 1),
                )),
                (None, None) => ocoeco::board::in_view(&dir).0,
            };
            let today = std::process::Command::new("date")
                .arg("+%F")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            let engine = tad::sql::DuckDb::find()?;
            // laying a site out is one look: what is in view is the same for every page of it
            let _one_look = ocoeco::relations::one_look();
            // the pages there are: each row of what the lens brings into focus that names where a page is
            // written and what it looks through
            let said = |row: &serde_json::Map<String, serde_json::Value>, key: &str| {
                row.get(key)
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .trim()
                    .to_string()
            };
            // what the lens brings into focus is asked once, and what a lens placed in it brings is read
            // as its own: so a place may offer pages of what it holds, and an observatory place them
            let of_the_site = ocoeco::focus::brought(&lens, &places, &today, &engine)?;
            let rows = ocoeco::focus::every_answer(&of_the_site);
            let pages: Vec<(String, String, String, String)> = rows
                .iter()
                .copied()
                .filter(|row| !said(row, "path").is_empty() && !said(row, "looks").is_empty())
                .map(|row| {
                    (
                        said(row, "path"),
                        said(row, "looks"),
                        said(row, "scale"),
                        said(row, "of_thing"),
                    )
                })
                .collect();
            if pages.is_empty() {
                anyhow::bail!("the lens {lens} says no pages: each row of what it brings into focus names where a page is written (path) and what it looks through (looks)");
            }
            if let Some((path, ..)) = pages.iter().find(|(path, ..)| {
                path.starts_with('/') || path.split('/').any(|step| step == "..")
            }) {
                anyhow::bail!(
                    "a page is written under where the site is laid out: {path} leads out of it"
                );
            }
            // which things have a page: a line about one then leads to it - by every name that names
            // that thing alone. A name that is the name of several things leads nowhere
            let named = ocoeco::focus::Named::of(&places)?;
            ocoeco::focus::pages_are(
                pages
                    .iter()
                    .filter(|(_, _, _, of)| !of.is_empty())
                    .flat_map(|(path, _, _, of)| {
                        named
                            .names_of(of)
                            .into_iter()
                            .map(move |name| (name, path.clone()))
                    })
                    .collect(),
            );
            // and no two pages are written in one place
            let mut written = std::collections::BTreeSet::new();
            if let Some((path, ..)) = pages
                .iter()
                .find(|(path, ..)| !written.insert(path.clone()))
            {
                anyhow::bail!(
                    "two pages of the site are written at {path}: each page has a place of its own"
                );
            }
            // and what each page is headed by: what its row calls its title - or, saying none, what the
            // place one looks from is called
            let titles: std::collections::BTreeMap<String, String> = rows
                .iter()
                .copied()
                .map(|row| (said(row, "path"), said(row, "title")))
                .filter(|(_, title)| !title.is_empty())
                .collect();
            let called =
                ocoeco::lenses::called_at(&dir).unwrap_or_else(|| "The fulldome".to_string());
            // and what each page says of itself, under its title: what its row says
            let saying: std::collections::BTreeMap<String, String> = rows
                .iter()
                .copied()
                .map(|row| (said(row, "path"), said(row, "says")))
                .collect();
            for (path, looks, scale, _) in &pages {
                let laid = ocoeco::focus::brought(
                    &format!("{surface}@{looks}"),
                    &places,
                    &today,
                    &engine,
                )?;
                let what = if scale.is_empty() { looks } else { scale };
                let text = ocoeco::dome::surface("lines", 80)?.lay_out(&laid).replace(
                    "{scale}",
                    &ocoeco::dome::escaped(&format!("{what}, {today}")),
                );
                let text = text.replace(
                    "{title}",
                    &ocoeco::dome::escaped(
                        titles.get(path).map_or(called.as_str(), String::as_str),
                    ),
                );
                let text = text.replacen(
                    "{says}",
                    &ocoeco::dome::escaped(saying.get(path).map_or("", String::as_str)),
                    1,
                );
                let file = out.join(path);
                if let Some(folder) = file.parent() {
                    std::fs::create_dir_all(folder)?;
                }
                std::fs::write(&file, text)?;
            }
            ocoeco::focus::pages_are(Default::default());
            println!(
                "{} page(s) laid out on {surface} -> {}",
                pages.len(),
                out.display()
            );
        }
        Cmd::Fits { shell, dir, words } => {
            use clap::CommandFactory;
            // the program's own commands, as their declaration says: name, what each is, its flags, its
            // positionals - read, not written again
            let commands: Vec<ocoeco::fits::Command> = Cli::command()
                .get_subcommands()
                .map(|c| ocoeco::fits::Command {
                    name: c.get_name().to_string(),
                    is: c.get_about().map(|a| a.to_string()).unwrap_or_default(),
                    flags: c
                        .get_arguments()
                        .filter_map(|a| {
                            Some((
                                a.get_long()?.to_string(),
                                a.get_help().map(|h| h.to_string()).unwrap_or_default(),
                                a.get_action().takes_values(),
                            ))
                        })
                        .collect(),
                    positionals: c
                        .get_positionals()
                        .map(|a| a.get_id().to_string())
                        .collect(),
                })
                .collect();
            let words = if shell {
                let line = std::env::var("COMP_LINE").unwrap_or_default();
                let point = std::env::var("COMP_POINT")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(line.chars().count());
                ocoeco::fits::typed_line(&line, point)
            } else if words.is_empty() {
                vec![String::new()]
            } else {
                words
            };
            // a shell splits a word at a colon and at an @: it is answered with what comes after the last one typed
            let typing = words.last().cloned().unwrap_or_default();
            let cut = typing.rfind([':', '@']).map_or(0, |at| at + 1);
            for f in ocoeco::fits::after(&words, &dir, &commands) {
                match (shell, f.is.is_empty()) {
                    (true, _) if f.word.is_empty() => {}
                    (true, _) => println!("{}", &f.word[cut.min(f.word.len())..]),
                    (false, true) => println!("{}", f.word),
                    (false, false) => println!("{}  - {}", f.word, f.is),
                }
            }
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
            // checking a place is one look: the graph is made once for all that is checked of it
            let _one_look = ocoeco::relations::one_look();
            let (well, mut problems) = join::check(&dir)?;
            // a place's mark and an older lens.toml beside it say the same of what is gathered
            if let (Ok(mark), Ok(old)) = (
                std::fs::read_to_string(dir.join("oco.toml"))
                    .map_err(anyhow::Error::from)
                    .and_then(|t| ocoeco::mark::Mark::parse(&t)),
                std::fs::read_to_string(dir.join("lens.toml")),
            ) {
                problems.extend(mark.against_family(&old));
            }
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
            // and nothing this place keeps holds what is kept out: seen here, before it lands
            problems.extend(ocoeco::kept::showing(&dir));
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
        Cmd::Observe {
            out,
            root,
            only,
            hold,
        } => {
            // a place observes what its mark says it gathers, and itself - not whatever is found. A
            // place whose mark names nothing gathered (the first observatory) observes all, as before.
            let own = std::fs::read_to_string("oco.toml")
                .ok()
                .and_then(|t| ocoeco::mark::Mark::parse(&t).ok())
                .filter(|m| m.place.as_ref().is_some_and(|p| !p.gathers.is_empty()));
            // observing by holding: what is found on GitHub is brought to hand - each place as it is there
            // now - and read from there as any place held is read. What is observed is then what is held, as
            // of when it was brought; and what a route reads is observed with it.
            let root = match (root, &hold) {
                (None, Some(hold)) => {
                    let there = match &own {
                        Some(own) => {
                            let mut named = own
                                .place
                                .as_ref()
                                .map(|p| p.gathers.clone())
                                .unwrap_or_default();
                            named.push(own.project.id.clone());
                            gh.gathered(&named)?
                        }
                        None => <discover::GitHub as discover::Outlet>::find(&gh, None)?,
                    };
                    // only what carries a mark is a place: each is asked for its mark before anything is
                    // brought - an owner may have what is no place at all, and far larger than any
                    let places: Vec<discover::Found> = discover::marked(there, &gh)
                        .into_iter()
                        .filter(|c| c.mark.is_some())
                        .map(|c| c.found)
                        .collect();
                    for (place, why) in discover::hold(&places, hold) {
                        eprintln!("{place} could not be brought to hand: {why}");
                    }
                    Some(hold.display().to_string())
                }
                (root, _) => root,
            };
            let held = root.map(|r| {
                discover::Held::new(std::path::PathBuf::from(r.replacen(
                    '~',
                    &std::env::var("HOME").unwrap_or_default(),
                    1,
                )))
            });
            let found = match (&held, &own) {
                (Some(held), _) => discover::discover(&[(held, held)], None)?,
                // and what it gathers is found by asking for it - every repository of an owner it names,
                // each star it names, and itself - taking what carries a mark: no label, no search
                (None, Some(own)) => {
                    let mut named = own
                        .place
                        .as_ref()
                        .map(|p| p.gathers.clone())
                        .unwrap_or_default();
                    named.push(own.project.id.clone());
                    discover::marked(gh.gathered(&named)?, &gh)
                }
                (None, None) => discover::discover(&[(&gh, &gh)], None)?,
            };
            let found: Vec<_> = match &own {
                Some(own) => found
                    .into_iter()
                    .filter(|c| own.gathers(&c.found.repo) || c.found.repo == own.project.id)
                    .collect(),
                None => found,
            };
            let mut h = match (only.as_deref(), &held) {
                (Some("runs"), _) => harvest::Harvest {
                    projects: Vec::new(),
                    tables: Default::default(),
                    problems: Vec::new(),
                },
                (Some(other), _) => anyhow::bail!("--only knows runs, not {other}"),
                (None, Some(held)) => harvest::harvest(&found, held),
                (None, None) => harvest::harvest(&found, &gh),
            };
            // and what the routes read, where the places are held to be read
            if only.is_none() && held.is_some() {
                harvest::observe_routes(&found, &mut h);
            }
            harvest::observe_runs(&found, &gh, &mut h);
            match only {
                Some(_) => harvest::write_part(&mut h, &out)?,
                None => harvest::write(&mut h, &out)?,
            }
            println!(
                "{} projects, {} tables ({} rows), {} problems -> {}",
                found.len(),
                h.tables.len(),
                h.tables.values().map(Vec::len).sum::<usize>(),
                h.problems.len(),
                out.display()
            );
        }
    }
    Ok(())
}

/// What is in view, as it stands now, in one number: every file the places keep their tables in, what git
/// has of each place, and where the drivers stand - each by when it last changed and how long it is. Two
/// numbers that differ mean something in view has changed; reading it is cheap, so waiting costs nothing.
fn as_it_stands(places: &[std::path::PathBuf]) -> u128 {
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    for place in places {
        match std::fs::read_to_string(place.join("oco.toml"))
            .ok()
            .and_then(|text| ocoeco::mark::Mark::parse(&text).ok())
        {
            Some(mark) => {
                files.extend(
                    mark.exports
                        .values()
                        .map(|path| place.join(path.split('@').next().unwrap_or(path))),
                );
                files.extend(
                    ["oco.toml", ".git/HEAD", ".git/index", ".git/FETCH_HEAD"]
                        .map(|f| place.join(f)),
                );
            }
            // what was observed: a folder of tables
            None => files.extend(
                std::fs::read_dir(place)
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.ok().map(|e| e.path())),
            ),
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let trail = std::path::Path::new(&home).join(".local/share/ocoeco/trail");
        files.extend(
            std::fs::read_dir(trail)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok().map(|e| e.path())),
        );
    }
    files
        .iter()
        .filter_map(|file| std::fs::metadata(file).ok())
        .fold(0u128, |all, meta| {
            let changed = meta
                .modified()
                .ok()
                .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            all.wrapping_mul(1_000_003)
                .wrapping_add(changed)
                .wrapping_add(meta.len() as u128)
        })
}
