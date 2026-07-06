//! Canonic: versioned markdown canned responses for Jira, with convert/lint/search.

pub mod convert;
pub mod corpus;
pub mod index;
pub mod lint;

pub use convert::{convert_markdown_to_jira, convert_path_to_jira, tool_available as pandoc_available};
pub use corpus::{default_corpus_dir, load_response, walk_responses, CannedResponse};
pub use index::{
    bm25_score, default_index_dir, reindex, search, search_docs, tokenize, IndexDoc, SearchHit,
};
pub use lint::{lint_paths, LintEngine, LintReport, LintFinding};
