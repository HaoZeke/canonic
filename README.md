# canonic

**Canonical** canned-response corpus for the generic Jira Jira work: version-controlled **markdown** under a shared **`resp-`** prefix is the source of truth. Convert with **pandoc**, enforce **quality checks** before migration, lint with **Vale** / **Harper**, and **search / dedupe** with a local **Tantivy** index (BM25).

## Requirements

| Tool | Role |
|------|------|
| [Rust](https://rustup.rs/) (1.75+) | Build the CLI |
| [pandoc](https://pandoc.org/) | `convert` — markdown → `jira` writer |
| [Vale](https://vale.sh/) | optional style lint |
| Harper | **in-process `harper-core`** (linked); optional CLI on `PATH` |

## Build

```bash
cargo build --release
```

## Tutorial: your first canned response

`corpus/responses/` starts empty (only a `.gitkeep` placeholder), so every response your team publishes comes from a reviewed answer. This walks through adding one.

1. Create `corpus/responses/resp-example-topic.md`:

   ```markdown
   ---
   id: resp-example-topic
   title: Example topic
   prefix: resp
   tags: [example]
   sop: none
   ---

   # Example topic

   Replace this with the real advisor answer.

   Regards,
   Support Team
   ```

2. Validate the front matter and closing:

   ```bash
   cargo run -- check
   ```

3. Index it and search:

   ```bash
   cargo run -- reindex
   cargo run -- search "example topic"
   ```

4. Convert to Jira wiki markup (requires pandoc):

   ```bash
   cargo run -- convert corpus/responses/resp-example-topic.md
   ```

Delete `resp-example-topic.md` once you commit a real response under its own id; it exists only to teach the format.

## Corpus layout (`resp` prefix)

```
corpus/responses/
  resp-<topic-slug>.md
  ...
```

Front matter convention (enforced by `canonic check`):

```markdown
---
id: resp-<topic-slug>
title: Human-readable title
prefix: resp
tags: [tag-one, tag-two]
sop: none
---
```

- `id` and filename stem must match and start with `resp-`
- `prefix: resp` required (shared advisor library; no personal prefixes)
- `sop:` required — Confluence URL or literal `none`
- Closings must be team-generic (e.g. `Support Team`), not personal names

`.gitignore` excludes the Tantivy index generated under `.canonic-index/`.

## Usage

```bash
canonic doctor
canonic list
canonic check                          # quality gate (exit 1 on findings)
canonic convert corpus/responses/resp-example-topic.md
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

### Mirroring for team MR review

`scripts/mirror-to-gitlab.sh` pushes the current branch to a second remote (added or updated on each run) so a team can open merge requests on a self-hosted GitLab that GitHub does not mirror to automatically:

```bash
CANONIC_GITLAB_REMOTE=git@gitlab.example:group/canonic.git scripts/mirror-to-gitlab.sh
```

Pass a branch name as the first argument to override the current branch.

## Design notes

- **Tantivy BM25** for search and near-duplicate discovery (better fit for curation/dedupe than a hand-rolled store).
- Markdown remains the source of truth; Jira is a publication surface. `canonic convert` produces the wiki markup, a human pastes or imports it — there is no live Jira API sync.
- Quality checks implement the meeting rule: **review before migration**, shared `resp` prefix only.

## Citation

See `CITATION.cff`, or use GitHub's "Cite this repository" button.

## License

MIT — see `LICENSE`.
