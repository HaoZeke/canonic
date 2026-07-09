# canonic

[![Documentation](https://img.shields.io/badge/docs-canonic.rgoswami.me-blue)](https://canonic.rgoswami.me)
[![Release](https://img.shields.io/github/v/release/HaoZeke/canonic)](https://github.com/HaoZeke/canonic/releases)

<p align="center">
  <img src="docs/source/_static/logo.svg" width="280" alt="canonic logo" />
</p>

**canonic** is a versioned **Jira canned-response** corpus tool: markdown under a **shared id prefix you choose** in `canonic.toml` (optional `--prefix`; default `resp`) is the source of truth. Convert with **pandoc**, enforce **quality checks**, lint with **Vale** / **Harper**, and **search / dedupe** with a local **Tantivy** index (BM25).


## Configuration

Settings are layered with [figment](https://docs.rs/figment) (file-first (no application environment variables)):

1. Built-in defaults (`prefix = "resp"`)
2. `canonic.toml` (walk-up from the working directory)
3. `canonic.local.toml` beside it (gitignored - tokens / machine overrides)
4. CLI flags for one-shot overrides (`--config`, `--prefix`)

```toml
# canonic.toml
prefix = "resp"

# Optional free Jira REST - put secrets in canonic.local.toml instead
# [jira]
# base_url = "https://your-instance.atlassian.net"
# email = "you@example.org"
# api_token = "..."
# # auth_header = "Bearer <pat>"   # Server/DC alternative
```

```bash
canonic check
canonic --prefix acme new "Topic title"   # one-shot prefix override
canonic --config /path/to/canonic.toml jira-probe
```

## Tutorial

Walk the included demos from list through convert: **[docs/orgmode/tutorial.org](docs/orgmode/tutorial.org)** (HTML: [Tutorial](https://canonic.rgoswami.me/tutorial.html)). Reproduce with:

```bash
cargo build --release --locked
./scripts/tutorial-run.sh ./target/release/canonic
./scripts/tutorial-run.sh --capture ./target/release/canonic  # refresh docs session
```

## Docs site (Shibuya)

Live site: **https://canonic.rgoswami.me** (Cloudflare Pages + Antics).

Sphinx + [Shibuya](https://shibuya.lepture.com/) themed HTML lives under `docs/`. Build from the repo root:

```bash
./docs/build.sh
# open docs/build/index.html
# optional local server:
python3 -m http.server -d docs/build 8000
```

Prose is authored in **org-mode** under `docs/orgmode/` (same pattern as nimvault/meetrec). `./docs/build.sh` runs Emacs `ox-rst` (`docs/export.el`) → untracked `docs/source/*.rst`, installs `docs/requirements.txt` (Sphinx, Shibuya, **sphinxcontrib-rust**, postprocess), and `sphinx-build` embeds the Rust API into the same Shibuya tree via `sphinx-rustdocgen` (rgpot pattern - not a side `cargo doc` tree). Needs `sphinx-rustdocgen` on `PATH` (`cargo install sphinx-rustdocgen`). Branding assets live in `docs/source/_static/`. Set `CANONIC_SKIP_ORG_EXPORT=1` only if RST is already exported.

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
| [pandoc](https://pandoc.org/) | `convert` - markdown → `jira` writer |
| [Vale](https://vale.sh/) | optional style lint |
| Harper | **in-process `harper-core`** (linked); optional CLI on `PATH` |

## Tutorial: your first canned response

Published answers live under `corpus/responses/` (a seeded sample includes so `list` / `check` / convert work on a clean clone). Prefer the scaffold CLI over hand-copying front matter.

1. Scaffold a check-clean draft (id = `resp-` + slug of the title):

   ```bash
   canonic new "Example topic" --tags example
   # → corpus/responses/resp-example-topic.md
   ```

2. Edit the body, then validate:

   ```bash
   canonic check
   canonic lint --engine harper
   ```

3. Index and search:

   ```bash
   canonic reindex
   canonic search "example topic"
   ```

4. Convert to Jira wiki markup (requires pandoc), or post explicitly:

   ```bash
   canonic convert corpus/responses/resp-example-topic.md
   canonic jira-comment --issue HSP-101 corpus/responses/resp-example-topic.md --dry-run
   ```

   An included demo also lives at `corpus/responses/resp-demo-shared-quota.md` for
   `list` / CI without scaffolding first.

**Import → review → promote:** pull existing Jira comments as drafts (never auto-published), edit, then promote:

```bash
canonic import-jira "project = HSP AND labels = canned-response"
# edit corpus/imports/resp-….md until check-clean
canonic promote corpus/imports/resp-….md
canonic check
```

Agent day-to-day loop: install the in-repo skill at `.agents/skills/canonic-canned-loop/` (see that `SKILL.md`).

### Team review (optional GitLab mirror)

Published wording should land via merge request. If you review on GitLab while the primary remote is elsewhere:

```bash
export CANONIC_GITLAB_REMOTE=git@gitlab.example.com:your-group/canonic.git
scripts/mirror-to-gitlab.sh
```

Import drafts stay under `corpus/imports/` (gitignored) until `canonic promote`. There is **no** bulk auto-sync of the library into Jira.

## Corpus layout (configured prefix prefix)

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
- `prefix: resp` required (shared library convention; no personal prefixes)
- `sop:` required - Confluence/service-desk wiki URL, or literal `none` when no SOP page exists yet
- Closings must be team-generic (e.g. `Support Team`), not personal names

`.gitignore` excludes the Tantivy index under `.canonic-index/` and review drafts under `corpus/imports/`.

## Usage

```bash
canonic doctor
canonic list                           # includes included resp-demo-*.md samples
canonic tui                            # interactive ratatui browser
canonic convert corpus/responses/resp-demo-shared-quota.md
canonic check                          # quality check (exit 1 on findings)
canonic lint --engine harper           # in-process harper-core (CI uses this)

canonic new "Example topic" --tags example
# → corpus/responses/resp-example-topic.md  (edit, then check again)

canonic reindex
canonic search "shared quota"
canonic dedupe --reindex --threshold 1.0
canonic dedupe --threshold 0.5 --json

# with [jira] in canonic.local.toml:
canonic import-jira "project = HSP AND labels = canned-response" --dry-run
canonic promote corpus/imports/resp-some-topic-hsp-101.md
```

| Command | Purpose |
|---------|---------|
| `doctor` | Tooling + optional `[jira]` probe |
| `tui` | Browse / filter / check / convert-preview (never posts to Jira) |
| `list` / `new` / `promote` | Inventory, scaffold, import→responses after check |
| `check` / `lint` | Quality check + Harper (CI uses `--engine harper`) |
| `convert` | Markdown → Jira wiki (pandoc) |
| `reindex` / `search` / `dedupe` | Local Tantivy BM25 + near-duplicates |
| `jira-probe` / `import-jira` / `jira-comment` | Free platform REST only (no Marketplace apps) |

Full recipes, keybindings, exit codes, and the free REST map: **[docs/orgmode/usage.org](docs/orgmode/usage.org)** (HTML via `./docs/build.sh` → Usage).

### Interactive TUI

```bash
canonic tui
canonic tui --corpus corpus/responses
```

| Key | Action |
|-----|--------|
| `j`/`k` or arrows | Move selection |
| `/` | Filter by id, title, tags, body |
| `C` | Check whole corpus |
| `c` | Convert selection → jira wiki **preview** (pandoc) |
| `l` | Lint selection (harper-core) |
| `r` / `s` | Rebuild index / search |
| `R` | Reload from disk |
| `?` | Help · `q` quit |

The TUI never posts to Jira; use `jira-comment` for an explicit one-shot write.

### Dedupe

`dedupe` rebuilds or reuses the Tantivy index, then for each response runs a self-query (title + content terms) and reports other documents that rank above `--threshold`. Pair reasons include the Tantivy score and a content **Jaccard** similarity for a second opinion. Use a high threshold to list only strong near-copies when curating the library before a Jira migration.

### Free Jira REST (no paid Marketplace apps)

Mapped to **official Atlassian platform REST** (Cloud Free API tokens or Server/Data Center PAT). No Marketplace apps, ScriptRunner, or JSM canned-response admin product APIs.

| canonic | Free platform endpoint | Host notes |
|---------|------------------------|------------|
| `jira-probe` | `GET /rest/api/2/myself` (+ `serverInfo`) | Cloud + Server; reports wiki vs ADF write path |
| `import-jira` | `GET …/search` (API/2, then API/3 + `/search/jql` fallback) · `GET …/issue/{key}/comment` (API/2 then 3) | Free-tier only; no Marketplace apps |
| `jira-comment` | **Server/DC:** `POST /rest/api/2/issue/{key}/comment` wiki `{"body":"…"}` · **Cloud Free:** `POST /rest/api/3/issue/{key}/comment` minimal **ADF** body | Auto from host (`*.atlassian.net` → ADF) |
| `doctor` | optional probe when `[jira]` is configured | Optional when unset |

```bash
# canonic.local.toml (gitignored):
#   [jira]
#   base_url = "https://your-instance.atlassian.net"
#   email = "you@example.org"
#   api_token = "..."
#   # auth_header = "Bearer <pat>"   # Server/DC

canonic jira-probe

# Import existing issue comments as review drafts (never auto-writes corpus/responses/)
canonic import-jira "project = HSP AND labels = canned-response" --dry-run
canonic import-jira "project = HSP AND labels = canned-response"
# edit corpus/imports/resp-….md, then:
canonic promote corpus/imports/resp-….md
canonic check

# Explicit write: pandoc jira → free REST comment (Cloud ADF / Server wiki)
canonic convert corpus/responses/resp-demo-shared-quota.md
canonic jira-comment --issue HSP-101 corpus/responses/resp-demo-shared-quota.md --dry-run
canonic jira-comment --issue HSP-101 corpus/responses/resp-demo-shared-quota.md
canonic jira-comment --issue HSP-101 PATH.md --body-format wiki   # force Server/DC
canonic jira-comment --issue HSP-101 PATH.md --body-format adf    # force Cloud ADF
```

- Import reads wiki **or** Cloud ADF comment bodies (ADF flattened to text, then pandoc `jira`→markdown when applicable).
- Write is **one file → one issue comment**, human-gated - **not bulk library sync**.
- `canonic doctor` reports free Jira status only when `[jira]` is configured (probe failure does not fail the critical path).

Optional developer smoke (not required for normal install). Passwords in these scripts are **disposable fixture-only** defaults for local containers, - never production credentials.

**Live smoke (official Atlassian Jira Software):** host with podman/docker and a few gigabytes of free RAM:

```bash
./scripts/jira-real/run-import-smoke.sh
```

Pulls `atlassian/jira-software:9.12.15`, classic-setup with Atlassian’s published short-lived **developer time-bomb** license (Server/Data Center is trial/paid; the key is for local testing only), seeds `HSP` issues with `canned-response` labels and wiki-markup comments, runs `canonic import-jira`, then removes the container/volume.


### Jira smoke tests (Docker / Nix)

Free-tier REST fixture (no Marketplace apps):

```bash
# local / CI helper (podman or docker)
cargo build --release
./scripts/ci/jira-docker-smoke.sh
```

Optional Nix image for the same fixture:

```bash
nix --extra-experimental-features 'nix-command flakes' build .#jira-fixture-image
docker load < result
./scripts/ci/jira-docker-smoke.sh
```

Official Atlassian Jira Software (heavy; needs several gigabytes of RAM, developer time-bomb license):

```bash
./scripts/jira-real/run-import-smoke.sh
```

**Faster fixture** (REST-only stand-in):

```bash
./scripts/jira-fixture/run-import-smoke.sh
```

Authentication is file config under `[jira]` in `canonic.toml` / `canonic.local.toml`:

- `base_url` - required, e.g. `https://your-instance.atlassian.net`.
- `email` + `api_token` - Basic auth (Jira Cloud Free convention).
- `auth_header` - raw `Authorization` header (e.g. `Bearer <pat>` for Server/DC); wins over email/token.

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
- Quality checks implement the meeting rule: **review before migration**, shared configured prefix only.

## Citation

See `CITATION.cff`, or use GitHub's "Cite this repository" button.

## License

MIT - see `LICENSE`.
