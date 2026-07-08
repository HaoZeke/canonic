# canonic

<p align="center">
  <img src="docs/source/_static/logo.svg" width="280" alt="canonic logo" />
</p>

**Canonical** canned-response corpus for the generic Jira Jira work: version-controlled **markdown** under a shared **`resp-`** prefix is the source of truth. Convert with **pandoc**, enforce **quality checks** before migration, lint with **Vale** / **Harper**, and **search / dedupe** with a local **Tantivy** index (BM25).

## Docs site (Shibuya)

Sphinx + [Shibuya](https://shibuya.lepture.com/) themed HTML lives under `docs/`. Build from the repo root:

```bash
./docs/build.sh
# open docs/build/index.html
# optional local server:
python3 -m http.server -d docs/build 8000
```

The script creates `.venv-docs` if needed, installs `docs/requirements.txt` (Sphinx, Shibuya, sphinx-design, sphinx-copybutton), runs `sphinx-build`, then `cargo doc` and copies rustdoc into `docs/build/rustdoc/` (open `docs/build/rustdoc/canonic/index.html` or use the **Rust API** nav link). Branding assets live in `docs/source/_static/` (logos, favicon, mark, architecture/module diagrams). Set `CANONIC_SKIP_RUSTDOC=1` to skip the rustdoc step.

## Install

Primary path for a public clone (Rust 1.85+; matches locked clap / harper-core):

```bash
cargo install --git https://github.com/HaoZeke/canonic --locked
canonic --help
canonic doctor
```

From a checkout:

```bash
cargo build --release
./target/release/canonic --help
```

| Tool | Role |
|------|------|
| [Rust](https://rustup.rs/) (1.85+) | Build / install the CLI |
| [pandoc](https://pandoc.org/) | `convert` — markdown → `jira` writer |
| [Vale](https://vale.sh/) | optional style lint |
| Harper | **in-process `harper-core`** (linked); optional CLI on `PATH` |

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

JIRA_BASE_URL=https://your-instance.atlassian.net JIRA_EMAIL=you@example.org JIRA_API_TOKEN=... \
  canonic import-jira "project = HSP AND labels = canned-response" --dry-run
```

### Dedupe

`dedupe` rebuilds or reuses the Tantivy index, then for each response runs a self-query (title + content terms) and reports other documents that rank above `--threshold`. Pair reasons include the Tantivy score and a content **Jaccard** similarity for a second opinion. Use a high threshold to list only strong near-copies when curating the library before a Jira migration.

### Free Jira REST (no paid Marketplace apps)

canonic talks only to **native Jira platform REST** (Cloud Free API tokens or Server/Data Center PAT). It does **not** use paid Marketplace apps, ScriptRunner, or Service Desk “canned response admin” product APIs.

```bash
# Probe connectivity + identity (GET /rest/api/2/myself)
JIRA_BASE_URL=https://your-instance.atlassian.net \
JIRA_EMAIL=you@example.org JIRA_API_TOKEN=... \
  canonic jira-probe

# Import existing issue comments as review drafts (never auto-writes corpus/responses/)
canonic import-jira "project = HSP AND labels = canned-response" --dry-run
canonic import-jira "project = HSP AND labels = canned-response"

# Explicit write: convert one markdown file with pandoc jira and POST as an issue comment
canonic jira-comment --issue HSP-101 corpus/responses/resp-example-topic.md --dry-run
canonic jira-comment --issue HSP-101 corpus/responses/resp-example-topic.md
```

- **Read:** `import-jira` → `GET /search` + `GET /issue/{key}/comment` → drafts under `corpus/imports/`.
- **Write:** `jira-comment` → `POST /issue/{key}/comment` with pandoc `jira` wiki body only. No unattended bulk library sync; review-before-migrate still applies.
- Bodies use free-compatible wiki markup from pandoc’s `jira` writer (not ADF-only Marketplace formatters).

Optional developer smoke (not required for normal install). Passwords in these scripts are **disposable fixture-only** defaults for local containers — never production credentials.

**Live smoke (official Atlassian Jira Software):** host with podman/docker and a few GB free RAM:

```bash
./scripts/jira-real/run-import-smoke.sh
```

Pulls `atlassian/jira-software:9.12.15`, classic-setup with Atlassian’s published short-lived **developer timebomb** license (Server/Data Center is trial/paid; the key is for local testing only), seeds `HSP` issues with `canned-response` labels and wiki-markup comments, runs `canonic import-jira`, then removes the container/volume.

**Faster fixture** (REST-only stand-in):

```bash
./scripts/jira-fixture/run-import-smoke.sh
```

Authentication reads from the environment:

- `JIRA_BASE_URL` — required, e.g. `https://your-instance.atlassian.net`.
- `JIRA_EMAIL` + `JIRA_API_TOKEN` — Basic auth (the Jira Cloud convention).
- `JIRA_AUTH_HEADER` — a raw `Authorization` header instead, e.g. `Bearer <personal-access-token>` for Jira Server/Data Center. Takes precedence over `JIRA_EMAIL`/`JIRA_API_TOKEN` when set.

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
- Markdown remains the source of truth; Jira is a publication surface. `canonic convert` produces the wiki markup for a human to paste in; `canonic import-jira` reads existing issue comments back out as drafts. Neither direction writes to Jira automatically.
- Quality checks implement the meeting rule: **review before migration**, shared `resp` prefix only.

## Citation

See `CITATION.cff`, or use GitHub's "Cite this repository" button.

## License

MIT — see `LICENSE`.
