//! Canonic: versioned markdown canned responses for Jira, with convert/lint/search/dedupe.

pub mod check;
pub mod convert;
pub mod corpus;
pub mod doctor;
pub mod index;
pub mod lint;

pub use check::{check_corpus, check_responses, format_check_report, CheckReport, REQUIRED_PREFIX};
pub use convert::{
    convert_markdown_to_jira, convert_path_to_jira, tool_available as pandoc_available,
};
pub use corpus::{default_corpus_dir, load_response, walk_responses, CannedResponse};
pub use doctor::{collect_statuses, critical_missing, format_doctor, ToolStatus};
pub use index::{
    default_index_dir, find_duplicates, find_duplicates_jaccard, jaccard_similarity, reindex,
    search, self_query_for, tokenize, DedupePair, IndexDoc, SearchHit,
};
pub use lint::{
    format_report, lint_paths, lint_text_harper_inprocess, LintEngine, LintFinding, LintReport,
};
