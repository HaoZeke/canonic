# canonic

**Canonical** canned-response corpus for a Jira instance: version-controlled **markdown** is the source of truth. Convert to Jira/Confluence wiki markup with **pandoc**, lint with **Vale** and **Harper**, and search the corpus with a local **Heed** store ranked by **BM25**.

## Requirements

| Tool | Role |
|------|------|
| [Rust](https://rustup.rs/) (1.75+) | Build the CLI |
| [pandoc](https://pandoc.org/) | `convert` — markdown → `jira` writer markup |
| [Vale](https://vale.sh/) | `lint` — prose style (optional but recommended) |
| [Harper](https://writewithharper.com/) CLI (`harper-cli` / `harper`) | `lint` — grammar (optional) |

Missing optional tools produce an **explicit** error/message; the CLI does not panic or silently no-op.

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
# List responses
canonic list
canonic list --corpus corpus/responses

# Convert one file (or whole corpus) to Jira wiki markup via pandoc
canonic convert corpus/responses/password-reset.md
canonic convert --write                    # writes *.jira.txt beside sources
canonic convert --corpus corpus/responses

# Lint with Vale and Harper
canonic lint
canonic lint --engine vale
canonic lint --engine harper
canonic lint --json

# Rebuild Heed index and BM25 search
canonic reindex
canonic search "wireguard vpn"
canonic search "password reset" -n 5
```

## Design notes

- **BM25** (lexical relevance), not neural embeddings.
- Subprocess integration for **pandoc** and **Vale**; Harper via CLI when on `PATH`.
- Index/search logic is unit-tested without external tools; convert/lint invoke real binaries when present.

## License

MIT
