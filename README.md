# ocoeco

The information ecosystem of the [Open Commons Observatory](https://open-commons-observatory.github.io/) -
its backbone, both a principle and a tool.

**The principle:** a project's data is produced by its development and its functioning - commits,
builds, tests, releases, measurements - never kept by hand. Data made by work is as true as the work.

**The tool:** the working system of that ecosystem -
- **the formats and protocols**: tad (text as data) is the format ocoeco speaks - tables as JSON lines
  with a schema, a canonical form and verification - and the mark (`oco.toml` and its topics) that
  makes a repository part of the picture;
- **gatherers** that turn real activity into data, as hooks and CI jobs;
- **chaining**: one project's data feeding another's, every row carrying where it came from;
- **propagation**: discovery of marked projects and the harvest of their data into the observatory.

The specification - the principle, the tad format, the mark, the standard tables, propagation, chaining and gatherers - is [SPEC.md](SPEC.md).

## Joining

```sh
cargo install --git https://github.com/open-commons-observatory/ocoeco ocoeco
ocoeco init --id OWNER/REPO --name "Name: what it is for" --kind tool
ocoeco check
```

and the topics `oco-project` and `oco-kind-<kind>` on GitHub. See [SPEC.md](SPEC.md), section 10.

ocoeco's own state is kept the same way: [`data/`](data) holds its backlog, milestones and concepts,
which the observatory shows as its path. tad-engine (creation-guidelines) stays as it is, its formats
unchanged; ocoeco is the working system from now on.
