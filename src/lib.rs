//! # canonic
//!
//! Versioned markdown canned-response corpus for the cluster / Support Jira work.
//!
//! The CLI is a thin front end. Library consumers can call the same engines:
//!
//! | Module | Role |
//! |--------|------|
//! | [`corpus`] | Walk and load `resp-*.md` responses |
//! | [`check`] | Quality gate (prefix, sop, closings) |
//! | [`convert`] | Pandoc markdown ↔ Jira wiki markup |
//! | [`index`] | Tantivy BM25 search and near-duplicate pairs |
//! | [`lint`] | Vale CLI + in-process Harper |
//! | [`doctor`] | Tooling probes |
//! | [`jira_import`] | Free REST probe, read import, explicit comment POST |
//! | [`scaffold`] | New `resp-` templates and promote import → responses |
//! | [`tui`] | Interactive ratatui corpus browser (`canonic tui`) |
//!
//! ## Example
//!
//! ```no_run
//! use canonic::{default_corpus_dir, walk_responses, check_responses};
//!
//! let dir = default_corpus_dir();
//! let responses = walk_responses(&dir).expect("read corpus");
//! let report = check_responses(&responses);
//! assert!(report.ok() || !report.findings.is_empty());
//! ```
//!
//! Markdown under `corpus/responses/` remains the source of truth. Jira is a
//! publication surface: convert for paste-in, import only as drafts under
//! `corpus/imports/`, then [`scaffold::promote_to_corpus`] after review.

#![doc(html_logo_url = "https://raw.githubusercontent.com/HaoZeke/canonic/main/docs/source/_static/mark.svg")]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/HaoZeke/canonic/main/docs/source/_static/favicon.svg")]

pub mod check;
pub mod convert;
pub mod corpus;
pub mod doctor;
pub mod index;
pub mod jira_import;
pub mod lint;
pub mod scaffold;
pub mod tui;

pub use check::{check_corpus, check_responses, format_check_report, CheckReport, REQUIRED_PREFIX};
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
pub use tui::{run_tui, App as TuiApp};
