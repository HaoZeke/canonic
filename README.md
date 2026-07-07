# canonic

**Canonical** canned-response corpus for Jira Jira work: version-controlled **markdown** under a shared **`resp-`** prefix is the source of truth. Convert with **pandoc**, enforce **quality checks** before migration, lint with **Vale** / **Harper**, and **search / dedupe** with a local **Tantivy** index (BM25).

## Requirements

| Tool | Role |
|------|------|
| [Rust](https://rustup.rs/) (1.75+) | Build the CLI |
| [pandoc](https://pandoc.org/) | `convert` — markdown → `jira` writer |
| [Vale](https://vale.sh/) | optional style lint |
| Harper | **in-process `harper-core`** (linked); optional CLI on `PATH` |

## Corpus layout (Support `resp` prefix)

```
corpus/responses/
  resp-project-space-not-backup.md
  resp-small-compute-sbu-calculation.md
  ...
```

Front matter convention (enforced by `canonic check`):

```markdown
---
id: resp-project-space-not-backup
title: Project space is not a backup or archive
prefix: resp
tags: [storage, project-space]
sop: none
---
```

- `id` and filename stem must match and start with `resp-`
- `prefix: resp` required (shared advisor library; no personal prefixes)
- `sop:` required — Confluence URL or literal `none`
- Closings must be team-generic (e.g. `Support Team`), not personal names

Samples are drafted from the 2026-07-06 team onboarding / HPC advisors meeting (project space, SBU math, top-up/extension, local-facilities data collection, GPFS triage, account permission).

The Tantivy index under `.canonic-index/` is generated and gitignored.

## Build

```bash
cargo build --release
```

## Usage

```bash
canonic doctor
canonic list
canonic check                          # quality gate (exit 1 on findings)
canonic convert corpus/responses/resp-project-space-not-backup.md
canonic lint --engine harper

canonic reindex
canonic search "project space backup"
canonic dedupe --reindex --threshold 1.0
canonic dedupe --threshold 0.5 --json
```

### Dedupe

`dedupe` rebuilds or reuses the Tantivy index, then for each response runs a self-query (title + content terms) and reports other documents that rank above `--threshold`. Pair reasons include the Tantivy score and a content **Jaccard** similarity for a second opinion. Use a high threshold to list only strong near-copies when curating the library before a Jira migration.

### Doctor / check exit codes

- `doctor`: `1` if pandoc missing (convert blocked)
- `check`: `1` if any quality finding

## Design notes

- **Tantivy BM25** for search and near-duplicate discovery (better fit for curation/dedupe than a hand-rolled store).
- Markdown remains the source of truth; Jira is a publication surface (API sync still future work).
- Quality checks implement the meeting rule: **review before migration**, shared `resp` prefix only.

## License

MIT
