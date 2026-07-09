//! # canonic
//!
//! Versioned markdown canned-response corpus for Jira support workflows.
//!
//! The CLI is a thin front end. Library consumers can call the same engines:
//!
//! | Module | Role |
//! |--------|------|
//! | [`config`] | `canonic.toml` / `--prefix` shared id prefix |
//! | [`corpus`] | Walk and load `{prefix}-*.md` responses |
//! | [`check`] | Quality gate (prefix, sop, closings) |
//! | [`convert`] | Pandoc markdown ↔ Jira wiki markup |
//! | [`index`] | Tantivy BM25 search and near-duplicate pairs |
//! | [`lint`] | Vale CLI + in-process Harper |
//! | [`doctor`] | Tooling probes |
//! | [`jira_import`] | Free REST probe, read import, explicit comment POST |
//! | [`scaffold`] | New templates and promote import → responses |
//! | [`tui`] | Interactive ratatui corpus browser (`canonic tui`) |
//!
//! ## Example
//!
//! ```no_run
//! use canonic::{default_corpus_dir, walk_responses, check_responses, DEFAULT_PREFIX};
//!
//! let dir = default_corpus_dir();
//! let responses = walk_responses(&dir).expect("read corpus");
//! let report = check_responses(&responses, DEFAULT_PREFIX);
//! assert!(report.ok() || !report.findings.is_empty());
//! ```
//!
//! Markdown under `corpus/responses/` remains the source of truth. The shared
//! id prefix is **user-chosen** in `canonic.toml` (optional `--prefix` CLI
//! override; default `resp`). Jira settings live under `[jira]` in the same
//! files (prefer `canonic.local.toml` for tokens). Jira is a publication
//! surface: convert for paste-in, import only as drafts under
//! `corpus/imports/`, then [`scaffold::promote_to_corpus`] after review.

// Logo/favicon for the site live under docs/source/_static (Shibuya). Do not use
// #![doc(html_*)] list-form attrs — sphinx-rustdocgen only accepts #[doc = "…"].

pub mod check;
pub mod config;
pub mod convert;
pub mod corpus;
pub mod doctor;
pub mod index;
pub mod jira_import;
pub mod lint;
pub mod scaffold;
pub mod tui;

pub use check::{check_corpus, check_responses, format_check_report, CheckReport};
pub use config::{
    find_config_path, load_config, load_config_file, normalize_prefix, resolve_prefix,
    CanonicConfig, JiraSettings, DEFAULT_PREFIX,
};
pub use convert::{
    convert_jira_to_markdown, convert_markdown_to_jira, convert_path_to_jira,
    tool_available as pandoc_available,
};
pub use corpus::{default_corpus_dir, load_response, walk_responses, CannedResponse};
pub use doctor::{collect_statuses, critical_missing, format_doctor, ToolStatus};
pub use index::{
    default_index_dir, find_duplicates, find_duplicates_jaccard, jaccard_similarity, reindex,
    search, self_query_for, tokenize, DedupePair, IndexDoc, SearchHit,
};
pub use jira_import::{
    comment_body_to_text, default_import_dir, format_probe, import_jira, is_cloud_host,
    plain_text_to_adf, post_comment_from_markdown, post_comment_from_markdown_with_format,
    post_issue_comment, post_issue_comment_with_format, probe_jira, CommentBodyFormat, JiraAuth,
    JiraConfig, JiraProbe, PostedComment,
};
pub use lint::{
    format_report, lint_paths, lint_text_harper_inprocess, LintEngine, LintFinding, LintReport,
};
pub use scaffold::{
    check_response_path, promote_to_corpus, resolve_response_id, scaffold_markdown, write_scaffold,
    ScaffoldOptions, TEAM_SIGN_OFF,
};
pub use tui::{run_tui, run_tui_with_prefix, App as TuiApp};
