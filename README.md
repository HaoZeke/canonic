# canonic

**Canonical** canned-response corpus for a Jira instance: version-controlled **markdown** is the source of truth. Convert to Jira/Confluence wiki markup with **pandoc**, lint with **Vale** and **Harper** (in-process `harper-core`, optional CLI), and search the corpus with a local **Heed** store ranked by **BM25**.

## Requirements

| Tool | Role |
|------|------|
| [Rust](https://rustup.rs/) (1.75+) | Build the CLI |
| [pandoc](https://pandoc.org/) | `convert` — markdown → `jira` writer markup (check with `canonic doctor`) |
| [Vale](https://vale.sh/) | `lint --engine vale` — prose style (optional) |
| [Harper](https://writewithharper.com/) | Grammar via **in-process `harper-core`** (always linked); optional `harper-cli` / `harper` on `PATH` as an extra pass |

Missing optional tools produce an **explicit** message; the CLI does not panic or silently no-op. Harper grammar does **not** require a binary on `PATH`.

## Corpus layout

```
corpus/responses/
  password-reset.md
  vpn-access.md
  license-renewal.md
```

Each file may use optional YAML front matter:

```markdown
---
id: password-reset
title: Password reset self-service
tags: [account, password]
---

# Password reset self-service

Body in normal Markdown…
```

The markdown tree is the **source of truth**. The local BM25 index under `.canonic-index/` is generated and gitignored.

## Build

```bash
cargo build --release
```

## Usage

```bash
# Tool health (pandoc / vale / harper-cli / harper-core)
canonic doctor

# List responses
canonic list
canonic list --corpus corpus/responses

# Convert one file (or whole corpus) to Jira wiki markup via pandoc
canonic convert corpus/responses/password-reset.md
canonic convert --write                    # writes *.jira.txt beside sources
canonic convert --corpus corpus/responses

# Lint: Vale (subprocess if present) + Harper (in-process harper-core)
canonic lint
canonic lint --engine vale
canonic lint --engine harper
canonic lint --json

# Rebuild Heed index and BM25 search
canonic reindex
canonic search "wireguard vpn"
canonic search "password reset" -n 5
```

### Doctor exit codes

- `0` — pandoc present (convert workflow unblocked); other tools may still be `MISSING`
- `1` — pandoc missing (critical for `convert`)

### Harper: in-process vs CLI

- **Default / primary:** `harper-core` linked into the binary — `canonic lint --engine harper` works with no Harper install.
- **Optional:** if `harper-cli`, `harper`, or `harperls` is on `PATH`, that CLI is also invoked and its findings are merged.

## Design notes

- **BM25** (lexical relevance), not neural embeddings.
- Subprocess integration for **pandoc** and **Vale**; **Harper** primarily in-process.
- Index/search logic is unit-tested without external tools.

## License

MIT
