# ocoeco - the specification

ocoeco is the information ecosystem of the Open Commons Observatory: a principle, and the formats and
protocols that make it work. This is their specification. Version: 1.

## 1. The principle

A project's data is produced by its development and its functioning - commits, builds, tests,
releases, measurements - never kept by hand. Where a person must decide or judge (a plan, a decision,
a hand-checked result), that is data too, written once where the work happens, never copied
elsewhere. Data made by work is as true as the work.

## 2. The format: tad (text as data)

Every table is text: one JSON object per line (JSON lines), UTF-8, one file per table, kept in git.

**Full tad** - a data folder that is a DuckDB `EXPORT DATABASE (FORMAT json)`:
- `schema.sql` declares every table, its column types and constraints (keys, checks);
- `load.sql` names the file of each table (`COPY table FROM 'file'`) - the authority on what is loaded;
- each line holds only keys the schema knows, and every column appears on some line;
- importing the folder enforces the types and constraints;
- the **canonical form** is exactly the bytes DuckDB writes when it re-exports the folder, so a diff
  shows only real change; a canonical folder re-exports to itself;
- `checks/*.sql` are invariants: each query must return no rows.

**Light tad** - JSON lines without a schema, for small tables a project shares (its milestones, its
backlog): one object per line, an `id` on every row, rows ordered by `id`, compact separators. A light
table is promoted to full tad when it grows or other data depends on its shape.

## 3. The mark

A repository is part of the picture by its mark: `oco.toml` at its root, and the GitHub topics
`oco-project` and `oco-kind-<kind>`.

```toml
schema = 1
[project]
id = "owner/repo"
name = "Name: what it is for"      # before a colon: the title; after it: the goal
title = "Short name"               # optional: the name in navigation
kind = "tool"                      # observatory | canon | picture | conveyor | device-lab | tool | research
order = 2                          # optional: its place among its siblings (lower first, then by name)
parent = "owner-or-project"
[exports]                          # the tables it shares: name = path, or path@branch
backlog = "data/backlog.json"
```

A `picture` is a grand project (its own tab and views); `observatory` and `canon` are meta.

## 4. The standard tables

Shared tables use standard names so they join across projects; more columns are welcome.

| table | columns | what it says |
|---|---|---|
| `idea` | id, ord, kind (lead, problem, idea, picture, limit, start), heading, text | why it exists, for anyone - an ordered piece of writing |
| `backlog` | id, title, status (open, in-progress, done), note, opened | the work ahead and done |
| `vocabulary` | id, term, means, not, see | the words it speaks and exactly what they mean - and do not |
| `manual_work` | id, action, count, first, last, status (manual, automated, deliberate), automated_by, minutes | the small actions done by hand, and how often |
| `workings` | id, what, kind (tool, check, gatherer, test, practice), serves, how, why, evidence, uses | how each step is delivered and proven |
| `milestones` | id, subject, ord, title, horizon (working, perfection, ongoing), readiness (1-9), criteria, why | the path |
| `session_log` | id, entry_date, title, done, considered, rejected | how the work went |
| `concepts` | id, name, summary, why | the ideas |
| `decisions` | id, date, decision, why, rejected | what was chosen, and what was not |
| `practices` | id, practice, why, for | how to work |
| `test_runs` | id, device_id, run_at, kernel, shell, passed, failed, skipped | what was measured |
| `support_status` | device_id, component_id, status, notes | what works on which device |
| `feature_support` | feature_id, platform_id, status, note | features against platforms |
| `landscape` | id, name, project, what, for_us, link, checked | the projects around |

## 5. Propagation

**Discovery** is a port: an outlet finds candidates on one platform, a source reads their files. On
GitHub, one search of the topic index (`topic:oco-project`, narrowed by `topic:oco-kind-<kind>`) lists
every marked repository; each is confirmed by its `oco.toml`. Other platforms are further outlets.

**The harvest** reads every confirmed project's exported tables, adds a `project` column (its id) to
every row - its provenance - and merges the tables by name, written canonically (by project, then id).
What cannot be read or does not parse is listed in `problems`, never silently dropped. It runs on a
schedule; the picture keeps itself current.

## 6. Chaining

One project's data may be read by another's - with the provenance of every row kept. Milestones chain
by their **criteria**, a space-separated list of:

| criterion | reads |
|---|---|
| `feature:ID@PLATFORM` | that feature's status on that platform (`*` for every feature) |
| `component:ID@DEVICE` | that part's status on that device (`*` for every part) |
| `tests@DEVICE` | the device's latest test run: works when nothing failed |
| `backlog:ID` | done is works, in progress is partly, open is not yet |

Anything not measured is **unknown** - never counted as failure. A milestone is done when every
criterion works; readiness is earned in order along the path.

## 7. Gatherers

A gatherer turns work into data: a hook or a CI job that reads a real source - commits, CI results,
test runs, releases, a project's own files - and writes rows into a table, canonically, on a schedule
or an event. It declares its table, its source and when it runs. A row a gatherer writes is never
edited by hand; to change it, change the source.

Gathered data lives on a branch of its own, `oco-data` (a mark names it: `oco/gathered/ci.json@oco-data`):
the default branch stays the history of the work, a protected branch is never touched, and the
gatherer's commits are not themselves counted. A heartbeat counts people's and agents' work: commits
made by bots and workflows are left out.

Today's gatherers (to become standard parts of ocoeco): the linux-phone-oco/docs feeds (wiki claims,
kernel activity, shell releases - weekly), and devlab's test results (written into test_results and
test_runs after every run).

## 8. Lenses

The observatory looks through **lenses**. A lens is a repository of its own, marked with kind `lens`,
that declares a family of projects and the views that show it - as data, in `lens.toml`. The
observatory discovers lenses as it discovers projects and renders every lens with the same engine;
anyone can make a lens of their own family of projects, or fork one, without touching the observatory.

```toml
schema = 1

[lens]
title = "Linux Phones"
goal = "phones running Linux, as good as Android - and how close we are"

[family]                              # the projects this lens gathers - any of:
owners = ["linux-phone-oco"]          # every marked project these owners have
topics = []                           # every marked project with one of these topics
projects = ["creation-guidelines/lab-agent"]   # and these, by id

[[view]]
id = "how-close"
title = "How close we are"
kind = "path"                         # the path of a subject, with what the work says
subject = "linux-phones"

[[view]]
id = "features"
title = "Features"
kind = "matrix"                       # rows x columns of statuses, columns ranked best first
table = "feature_support"
rows = "features"                     # the table naming the rows (id, name, category)
row_key = "feature_id"
columns = "platforms"                 # the table naming the columns (id, name)
column_key = "platform_id"
```

**The view vocabulary** - the components every lens is drawn from:

| kind | shows |
|---|---|
| `path` | a subject's milestones on the three horizons, readiness earned in order, what the work says |
| `matrix` | statuses of rows against columns, rows grouped by category, columns ranked best first |
| `families` | a matrix whose columns are groups of another table (devices by their SoC) |
| `runs` | test runs as bars of passed, failed and skipped |
| `backlog` | the family's backlog: open, in progress, done |
| `log` | the family's session logs |
| `ideas` | the family's concepts and decisions |
| `heartbeat` | commits per day, releases and CI - made by work |
| `cards` | any table as cards: a title field, a text field, a link field |

The vocabulary grows only from what real lenses need.

**Curated and discovered.** The observatory curates: its own `curated` table (lens id, order) puts
lenses in its navigation. Every other lens it finds is listed among the discovered ones, rendered the
same way. The observatory's own lenses are curated - and built exactly like everyone else's.

## 9. Languages

Text comes in two kinds, and each is translated where it belongs.

**The interface** (headings, navigation, legends) belongs to the face that renders it, and is kept in
message catalogs - Project Fluent (`i18n/<language>.ftl`), which handles each language's grammar
(Russian plural forms: 1 коммит, 2 коммита, 5 коммитов).

**The data** belongs to the projects. Its translations never change the source tables: a project keeps
them in its own `translations` table, harvested like any other:

| column | what it says |
|---|---|
| id | `<table>:<row>:<column>:<language>` |
| table, row, column | the text translated |
| lang | the language (a BCP 47 tag: `ru`, `uk`, `de`) |
| text | the translation |
| by | `human` or `machine` - an agent's translation is never passed off as a person's |

A project's source language is its mark's `lang` (default `en`). Anything not translated falls back to
the source: a partly translated page is still a whole page.

**The site** is the same static files once per language, under `/<language>/` with identical paths;
the root keeps the old addresses as redirects to the default language. Every page declares its
`lang` and its translations (`hreflang`), and offers the others by their own names (English ·
Русский) - never flags, which stand for countries, not languages.

**Whose translation.** A translation applies only to rows of the project that wrote it: each project
translates its own data, in its own repository; a lens translates only its own text (in the `lenses`
table the column is the path in `lens.toml`: `lens.title`, `view.features.title`). Nobody translates
another project's data by hand - the lenses use what the projects' own translations give.

## 10. Joining

A repository joins in three steps, the first two done by one command:

```sh
ocoeco init --id OWNER/REPO --name "Name: what it is for" --kind tool   # the mark, the gatherer's caller, oco/ for its records
ocoeco check                                                              # the repository, as the harvest would read it
```

then its topics on GitHub, `oco-project` and `oco-kind-<kind>`, by which the observatory finds it.
`init` never overwrites what is there. After that the repository keeps its own backlog, milestones
and session log in `oco/` - its path fills from them - and its work is gathered daily.

## 11. A project, whole

The shared tables are declared as data in `language.toml`, a tad declaration (creation-guidelines/tad-engine): each table's fields in order, their types, the required ones, the allowed values, and the field a new row is dated by. Each table is a minimum that a project may extend with fields of its own. `ocoeco note` checks the row it writes with tad's `check_row`, and keeps every other line exactly as it was.

Work is recorded where it is fresh, as glimpses: `ocoeco glimpse "what was done"` at any moment - one typed line, the rest supplied (every row `ocoeco note` wrote since the last glimpse, the backlog items and milestones they serve, who and when). A glimpse needs no git. `ocoeco commit` is git's side: it renders a commit from the glimpses not yet committed - the weightiest kind as the title, every glimpse in the body, a `Glimpse:` trailer for each. Git depends on ocoeco, never the reverse.

A project tells four things about itself, each in its own table, each leading to the next:

1. **Its idea** - why it exists, written by the project for anyone: it starts from the reader's own
   experience, names the problem, says the idea, gives a picture, admits its limits, and says where it
   stands. An ordered piece of writing (`idea`), not a list of terms; a lens gathers and shows it and
   never writes it.
2. **Its path** - what it must reach: milestones on three horizons, each with criteria (`milestones`).
3. **Its workings** - how each step is delivered and proven (`workings`): what serves which milestone,
   how, why we care, and what evidence it produces. A working may be a tool, a check, a gatherer, a
   test or a practice, and may point at another project (`uses`) - a shared tool is a project that
   others' workings name.
4. **Its work** - what actually happened: the backlog, the log, and what was gathered.

How a project works is therefore not a manifest beside it but part of it, tied to its goals - and the
observatory is one more project that tells the same four things.

`ocoeco check` reads a repository's essence too and says, as advice that never fails the check, how
many of the four are told and what is still to tell (`essence: N of 4 told`): an idea with no opening,
no limit or no word on where it stands; a path not set; a step nothing delivers; a working that
serves a step the repository does not have.

A project's rows are recorded with `ocoeco note TABLE --id ID --set FIELD=VALUE [--add FIELD=TEXT]`:
one row added or updated, checked against this format, every other line left exactly as it was.

A project's **vocabulary** is the words it speaks: each with what it means in plain words, what it is
not, and related words. ocoeco defines the shared words in its own `vocabulary` table, compiled into
the `ocoeco` tool; a project adds its own beside them, and `ocoeco check` names any shared word it
redefines. A vocabulary translates jargon into plain words; it is not a licence for more of it.

A project's **manual work** is a ledger of the small actions done by hand - one action per record, a verb
and its object, a dozen words at most - with how often each was done. `ocoeco manual "ACTION"` bumps the
record it matches or adds one; when the words only resemble a record it names it, and the person
chooses (`--id` to bump, `--new` to add). Past three, a manual action is saturated: it asks for a tool.
A record automated by a tool says so, and a bump after that asks why it was done by hand again; a
record kept by hand deliberately (looking, listening, deciding) never asks to be automated.

Each manual work record keeps **minutes** (in all), **load** (how draining it is, 0-10, after Borg's CR10
scale of perceived exertion: 0 nothing, 2 easy, 5 hard, 10 maximal) and **effort** - minutes weighted by
load, where 5 counts as one. A manual action done three times, or one past 30 effort-minutes, is
saturated: a short heavy chore can outrank a long light one. Looking, listening and deciding are
counted like any other action - visual and audio checks, and rules written once, automate them too.

A project's **sources** are the prior art it stands on - what people already learned, and what is
taken from it (`| sources | id, title, by, year, url, takes |`). A concept or a decision cites them in
its `sources` field (ids, comma-separated), so research travels with the ideas that rest on it.

**Research** is the look around before acting: the question asked, what was found in plain words,
the sources it rests on, and when (`| research | id, question, found, sources, date |`). It is kept
even where no outside source exists - comparing one's own work with another's is research too.

The chain runs **research → decisions → steps**: a decision cites the research and sources it rests
on (`research`, `sources`) and names the milestones or workings it is about (`about`, ids
comma-separated). The observatory shows each decision as a small badge beside the steps it concerns,
so a reader sees at the step what was chosen, why, what was rejected, and what it rests on.

Every record and bump also appends an **event** to `manual_events` (`| manual_events | id, record, at,
minutes, load, effort, change |`): the exact time, this time's minutes, load and effort, and - when it
was recorded on landing - the change it belonged to. The record keeps the totals; the events keep the
when, so the views set manual work beside the commits and releases of the same days.

A word is chosen like any decision: a vocabulary entry names the decision that chose it (`decided`,
decision ids), and that decision records the words rejected and cites its research - so the
vocabulary shows, beside each word, why this word and not another.
