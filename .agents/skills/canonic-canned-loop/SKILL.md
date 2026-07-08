---
name: canonic-canned-loop
description: >-
  Day-to-day canned-response loop with canonic: probe free Jira,
  import drafts, scaffold or promote into corpus/responses, edit, check/lint,
  reindex/search/dedupe, convert or explicit jira-comment, and rely on CI.
  Use when curating shared resp- prefix markdown, importing Jira comments as review
  drafts, or publishing one-shot free-tier comments. No bulk auto-sync and no
  Marketplace apps.
---

# canonic canned-response loop

Markdown under `corpus/responses/` is the source of truth. Jira is a publication
surface. Free platform REST only (Cloud Free API token or Server/DC PAT).

## Constraints (do not break)

- **No bulk library push** into Jira. `jira-comment` is one file → one issue.
- **No Marketplace / JSM canned-response admin APIs** — native REST only.
- **Imports are drafts**: `import-jira` writes under `corpus/imports/` (or `--out`),
  never silently overwrites `corpus/responses/`.
- **Promote only after review**: edit drafts, run `check`, then `promote`.
- Secrets via env (`JIRA_BASE_URL`, `JIRA_EMAIL` + `JIRA_API_TOKEN`, or
  `JIRA_AUTH_HEADER`). Never commit tokens.

## Env

```bash
export JIRA_BASE_URL=https://your-instance.atlassian.net   # or Server/DC base
export JIRA_EMAIL=you@example.org
export JIRA_API_TOKEN=...                                  # Cloud API token
# Server/DC PAT alternative:
# export JIRA_AUTH_HEADER="Bearer <pat>"
```

## Day-to-day loop

### 1. Probe free REST

```bash
canonic doctor
canonic jira-probe
```

### 2. Pull snippets as review drafts

```bash
canonic import-jira "project = HSP AND labels = canned-response" --dry-run
canonic import-jira "project = HSP AND labels = canned-response"
# drafts land in corpus/imports/ by default
```

Fixture smoke (no live Jira):

```bash
# optional: scripts/jira-fixture/server.py + env pointing at it
canonic import-jira "project = HSP AND labels = canned-response" --out /tmp/imports
```

### 3. Scaffold a new response (template)

```bash
canonic new "Project space is not a backup" --tags storage,project-space
canonic new "Queue limits" --id resp-queue-limits --sop none --out corpus/responses
```

### 4. Edit, then quality gate

```bash
# edit the markdown (imports or responses)
canonic check
canonic lint --engine harper          # in-process harper-core; Vale optional
canonic list
canonic tui                           # browse / filter / convert preview (no Jira POST)
```

### 5. Promote import → published corpus

```bash
canonic promote corpus/imports/resp-some-topic-hsp-101.md
canonic check
canonic reindex
canonic search "project space"
canonic dedupe --reindex --threshold 1.0
```

### 6. Render / publish (human-gated)

```bash
canonic convert corpus/responses/resp-project-space-is-not-a-backup.md
canonic jira-comment --issue HSP-101 \
  corpus/responses/resp-project-space-is-not-a-backup.md --dry-run
# explicit write only when reviewed:
canonic jira-comment --issue HSP-101 \
  corpus/responses/resp-project-space-is-not-a-backup.md
```

Cloud Free uses ADF on `/rest/api/3`; Server/DC uses wiki on `/rest/api/2`.
Override with `--body-format wiki|adf` when needed.

## CI

GitHub Actions runs `cargo test --locked --all-targets`, then corpus `check` and
in-process Harper lint on `corpus/responses/`, plus a convert smoke when pandoc
is installed. Keep published responses check-clean so CI stays green.

## TUI keys (quick)

| Key | Action |
|-----|--------|
| `j`/`k` | Move · `/` filter · `C` check · `c` convert preview |
| `l` lint · `r` reindex · `s` search · `q` quit |

Full usage: `docs/source/usage.rst` and README **Usage** section.

## Agent checklist

1. Prefer `canonic new` over hand-copying front matter.
2. Never write import drafts straight into `corpus/responses/` without `promote`.
3. Run `check` (and `lint --engine harper`) before convert or `jira-comment`.
4. Use `canonic tui` for browse/filter/convert preview — it does **not** post to Jira.
5. Do not invent bulk sync or paid Jira extensions.
6. Leave production tickets out of git until a human promotes a reviewed draft.
