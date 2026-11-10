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
| `backlog` | id, title, status (open, in-progress, done), note, opened | the work ahead and done |
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
