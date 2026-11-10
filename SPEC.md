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
[exports]                          # the tables it shares: name = path in the repository
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

Today's gatherers (to become standard parts of ocoeco): the linux-phone-oco/docs feeds (wiki claims,
kernel activity, shell releases - weekly), and devlab's test results (written into test_results and
test_runs after every run).
