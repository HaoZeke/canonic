//! Tantivy-backed full-text index for search and near-duplicate detection.

use crate::corpus::{walk_responses, CannedResponse};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{
    Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, Value, STORED, STRING,
};
use tantivy::{doc, Index, IndexWriter, ReloadPolicy, TantivyDocument};

/// One document stored in the search index.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexDoc {
    pub id: String,
    pub title: String,
    pub content: String,
    pub path: String,
    pub tags: Vec<String>,
    pub sop: Option<String>,
}

/// A ranked hit from Tantivy BM25 search.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub id: String,
    pub score: f64,
    pub title: String,
    pub snippet: String,
    pub path: PathBuf,
}

/// A pair of near-duplicate responses for curation.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DedupePair {
    pub left_id: String,
    pub right_id: String,
    pub score: f64,
    pub left_path: String,
    pub right_path: String,
    pub reason: String,
}

/// Default index directory (gitignored) under the working tree.
pub fn default_index_dir() -> PathBuf {
    PathBuf::from(".canonic-index")
}

struct Fields {
    id: Field,
    title: Field,
    body: Field,
    path: Field,
    tags: Field,
    sop: Field,
}

fn build_schema() -> (Schema, Fields) {
    let mut builder = Schema::builder();
    let id = builder.add_text_field("id", STRING | STORED);
    let text_indexing = TextFieldIndexing::default()
        .set_tokenizer("default")
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let text_opts = TextOptions::default()
        .set_indexing_options(text_indexing)
        .set_stored();
    let title = builder.add_text_field("title", text_opts.clone());
    let body = builder.add_text_field("body", text_opts.clone());
    let path = builder.add_text_field("path", STRING | STORED);
    let tags = builder.add_text_field("tags", text_opts);
    let sop = builder.add_text_field("sop", STRING | STORED);
    let schema = builder.build();
    (
        schema,
        Fields {
            id,
            title,
            body,
            path,
            tags,
            sop,
        },
    )
}

impl From<&CannedResponse> for IndexDoc {
    fn from(r: &CannedResponse) -> Self {
        IndexDoc {
            id: r.id.clone(),
            title: r.title.clone(),
            content: r.content.clone(),
            path: r.path.display().to_string(),
            tags: r.tags.clone(),
            sop: r.sop.clone(),
        }
    }
}

/// Tokenize for pure lexical helpers (tests / jaccard).
pub fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Jaccard similarity over token sets (pure; used as a second signal for dedupe tests).
pub fn jaccard_similarity(a: &str, b: &str) -> f64 {
    let ta: HashSet<_> = tokenize(a).into_iter().collect();
    let tb: HashSet<_> = tokenize(b).into_iter().collect();
    if ta.is_empty() && tb.is_empty() {
        return 1.0;
    }
    if ta.is_empty() || tb.is_empty() {
        return 0.0;
    }
    let inter = ta.intersection(&tb).count() as f64;
    let union = ta.union(&tb).count() as f64;
    inter / union
}

/// Reindex all markdown responses under `corpus_dir` into Tantivy at `index_dir`.
/// Returns number of documents written.
pub fn reindex(corpus_dir: &Path, index_dir: &Path) -> Result<usize> {
    let responses = walk_responses(corpus_dir)?;
    if index_dir.exists() {
        fs::remove_dir_all(index_dir)
            .with_context(|| format!("clear old index {}", index_dir.display()))?;
    }
    fs::create_dir_all(index_dir)?;
    let (schema, fields) = build_schema();
    let index = Index::create_in_dir(index_dir, schema)?;
    let mut writer: IndexWriter = index.writer(50_000_000)?;
    let mut n = 0usize;
    for r in &responses {
        let tags = r.tags.join(" ");
        let sop = r.sop.clone().unwrap_or_default();
        writer.add_document(doc!(
            fields.id => r.id.as_str(),
            fields.title => r.title.as_str(),
            fields.body => r.content.as_str(),
            fields.path => r.path.display().to_string(),
            fields.tags => tags.as_str(),
            fields.sop => sop.as_str(),
        ))?;
        n += 1;
    }
    writer.commit()?;
    Ok(n)
}

fn open_index(index_dir: &Path) -> Result<(Index, Fields)> {
    if !index_dir.exists() {
        bail!("index not found at {}", index_dir.display());
    }
    let index = Index::open_in_dir(index_dir)
        .with_context(|| format!("open tantivy index {}", index_dir.display()))?;
    let schema = index.schema();
    let fields = Fields {
        id: schema.get_field("id")?,
        title: schema.get_field("title")?,
        body: schema.get_field("body")?,
        path: schema.get_field("path")?,
        tags: schema.get_field("tags")?,
        sop: schema.get_field("sop")?,
    };
    Ok((index, fields))
}

fn field_text(doc: &TantivyDocument, field: Field) -> String {
    doc.get_first(field)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Strip characters that break Tantivy's query parser; keep alphanumeric terms.
pub fn sanitize_query(query: &str) -> String {
    tokenize(query).join(" ")
}

/// Search the Tantivy index with BM25. Returns hits sorted by score desc.
pub fn search(index_dir: &Path, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
    let query = sanitize_query(query);
    if query.is_empty() || limit == 0 {
        return Ok(vec![]);
    }
    let (index, fields) = open_index(index_dir)?;
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()?;
    let searcher = reader.searcher();
    let parser = QueryParser::for_index(&index, vec![fields.title, fields.body, fields.tags]);
    let q = parser
        .parse_query(&query)
        .with_context(|| format!("parse query {query:?}"))?;
    let top = searcher.search(&q, &TopDocs::with_limit(limit))?;
    let mut hits = Vec::new();
    for (score, addr) in top {
        let retrieved: TantivyDocument = searcher.doc(addr)?;
        let id = field_text(&retrieved, fields.id);
        let title = field_text(&retrieved, fields.title);
        let body = field_text(&retrieved, fields.body);
        let path = field_text(&retrieved, fields.path);
        hits.push(SearchHit {
            id,
            score: score as f64,
            title,
            snippet: snippet_for(&body, &query),
            path: PathBuf::from(path),
        });
    }
    Ok(hits)
}

fn snippet_for(text: &str, query: &str) -> String {
    let flat = text.replace('\n', " ");
    if flat.is_empty() {
        return String::new();
    }
    let lower = flat.to_lowercase();
    let mut pos = None;
    for qt in tokenize(query) {
        if let Some(i) = lower.find(qt.as_str()) {
            pos = Some(i);
            break;
        }
    }
    let match_char = pos.map(|i| lower[..i].chars().count()).unwrap_or(0);
    let start_char = match_char.saturating_sub(40);
    let total_chars = flat.chars().count();
    let end_char = (start_char + 120).min(total_chars);
    let mut s: String = flat
        .chars()
        .skip(start_char)
        .take(end_char - start_char)
        .collect();
    if start_char > 0 {
        s = format!("...{s}");
    }
    if end_char < total_chars {
        s.push_str("...");
    }
    s
}

/// Build a short self-query for near-duplicate search from a response.
pub fn self_query_for(doc: &CannedResponse) -> String {
    let mut parts = vec![doc.title.clone()];
    let words: Vec<_> = doc
        .content
        .split_whitespace()
        .filter(|w| w.len() > 3)
        .take(40)
        .map(|s| s.trim_matches(|c: char| !c.is_alphanumeric() && c != '-'))
        .filter(|s| !s.is_empty())
        .collect();
    parts.push(words.join(" "));
    parts.join(" ")
}

/// Find near-duplicate pairs using Tantivy self-queries plus optional Jaccard floor.
///
/// For each document, query the index with a content-derived query and flag other
/// documents that rank above `score_threshold` (excluding self). Pairs are unique
/// and ordered by score desc.
pub fn find_duplicates(
    index_dir: &Path,
    corpus_dir: &Path,
    score_threshold: f64,
    per_doc_limit: usize,
) -> Result<Vec<DedupePair>> {
    let docs = walk_responses(corpus_dir)?;
    let mut pairs: Vec<DedupePair> = Vec::new();
    let mut seen_keys: HashSet<(String, String)> = HashSet::new();

    for doc in &docs {
        let q = self_query_for(doc);
        if q.trim().is_empty() {
            continue;
        }
        let hits = search(index_dir, &q, per_doc_limit.max(2))?;
        for hit in hits {
            if hit.id == doc.id {
                continue;
            }
            if hit.score < score_threshold {
                continue;
            }
            let (a, b) = if doc.id <= hit.id {
                (doc.id.clone(), hit.id.clone())
            } else {
                (hit.id.clone(), doc.id.clone())
            };
            if !seen_keys.insert((a.clone(), b.clone())) {
                continue;
            }
            let other = docs.iter().find(|d| d.id == hit.id);
            let jacc = other
                .map(|o| jaccard_similarity(&doc.content, &o.content))
                .unwrap_or(0.0);
            pairs.push(DedupePair {
                left_id: a,
                right_id: b,
                score: hit.score,
                left_path: doc.path.display().to_string(),
                right_path: hit.path.display().to_string(),
                reason: format!(
                    "tantivy self-query hit score={:.3}; content jaccard={jacc:.3}",
                    hit.score
                ),
            });
        }
    }
    pairs.sort_by(|x, y| {
        y.score
            .partial_cmp(&x.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(pairs)
}

/// Pure near-dup detection on an in-memory list via Jaccard (no Tantivy I/O).
pub fn find_duplicates_jaccard(docs: &[CannedResponse], threshold: f64) -> Vec<DedupePair> {
    let mut pairs = Vec::new();
    for i in 0..docs.len() {
        for j in (i + 1)..docs.len() {
            let score = jaccard_similarity(&docs[i].content, &docs[j].content);
            if score >= threshold {
                pairs.push(DedupePair {
                    left_id: docs[i].id.clone(),
                    right_id: docs[j].id.clone(),
                    score,
                    left_path: docs[i].path.display().to_string(),
                    right_path: docs[j].path.display().to_string(),
                    reason: format!("content jaccard={score:.3}"),
                });
            }
        }
    }
    pairs.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn tokenize_splits_and_lowercases() {
        assert_eq!(
            tokenize("Hello, Project-Space!"),
            vec!["hello", "project", "space"]
        );
    }

    #[test]
    fn jaccard_high_for_near_duplicates() {
        let a = "project space is not a backup or archive on demo";
        let b = "project space is not a backup archive for demo users";
        let c = "small compute request needs an sbu calculation for gpu nodes";
        assert!(jaccard_similarity(a, b) > jaccard_similarity(a, c));
        assert!(jaccard_similarity(a, b) > 0.3);
    }

    #[test]
    fn reindex_and_search_ranks_project_space_query() {
        let dir = tempfile::tempdir().unwrap();
        let corpus = dir.path().join("responses");
        fs::create_dir_all(&corpus).unwrap();
        fs::write(
            corpus.join("resp-project-space-not-backup.md"),
            "---\nid: resp-project-space-not-backup\ntitle: Project space is not a backup\nprefix: resp\nsop: none\n---\n\nProject space is not a backup or archive. Use tape for long-term retention.\n",
        )
        .unwrap();
        fs::write(
            corpus.join("resp-small-compute-sbu-calculation.md"),
            "---\nid: resp-small-compute-sbu-calculation\ntitle: SBU calculation\nprefix: resp\nsop: none\n---\n\nSmall compute needs an SBU calculation for GPU and CPU hours.\n",
        )
        .unwrap();

        let idx = dir.path().join("index");
        let n = reindex(&corpus, &idx).unwrap();
        assert_eq!(n, 2);
        let hits = search(&idx, "project space backup archive tape", 5).unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].id, "resp-project-space-not-backup");
        assert!(hits[0].score > 0.0);
    }

    #[test]
    fn find_duplicates_flags_near_copy() {
        let dir = tempfile::tempdir().unwrap();
        let corpus = dir.path().join("responses");
        fs::create_dir_all(&corpus).unwrap();
        let body = "Project space on the cluster is user-managed working storage and not a backup system for research data that must be archived to tape.";
        fs::write(
            corpus.join("resp-a.md"),
            format!("---\nid: resp-a\ntitle: Project space note A\nprefix: resp\nsop: none\n---\n\n{body}\n"),
        )
        .unwrap();
        fs::write(
            corpus.join("resp-b.md"),
            format!("---\nid: resp-b\ntitle: Project space note B\nprefix: resp\nsop: none\n---\n\n{body} Please confirm your backup plan.\n"),
        )
        .unwrap();
        fs::write(
            corpus.join("resp-c.md"),
            "---\nid: resp-c\ntitle: Unrelated SBU math\nprefix: resp\nsop: none\n---\n\nGPU hours and thin node SBU rates for small compute grants.\n",
        )
        .unwrap();

        let idx = dir.path().join("index");
        reindex(&corpus, &idx).unwrap();
        // Low threshold so near-copies surface; unrelated should not pair with them at high score.
        let pairs = find_duplicates(&idx, &corpus, 0.1, 5).unwrap();
        assert!(
            pairs.iter().any(|p| {
                (p.left_id == "resp-a" && p.right_id == "resp-b")
                    || (p.left_id == "resp-b" && p.right_id == "resp-a")
            }),
            "expected a/b pair, got {pairs:?}"
        );
    }

    #[test]
    fn jaccard_dedupe_pure_path() {
        let docs = vec![
            CannedResponse {
                id: "resp-a".into(),
                title: "A".into(),
                prefix: Some("resp".into()),
                sop: Some("none".into()),
                body: String::new(),
                content: "alpha beta gamma delta shared tokens for test".into(),
                path: PathBuf::from("a.md"),
                tags: vec![],
            },
            CannedResponse {
                id: "resp-b".into(),
                title: "B".into(),
                prefix: Some("resp".into()),
                sop: Some("none".into()),
                body: String::new(),
                content: "alpha beta gamma delta shared tokens for test again".into(),
                path: PathBuf::from("b.md"),
                tags: vec![],
            },
            CannedResponse {
                id: "resp-c".into(),
                title: "C".into(),
                prefix: Some("resp".into()),
                sop: Some("none".into()),
                body: String::new(),
                content: "completely different vocabulary about licensing".into(),
                path: PathBuf::from("c.md"),
                tags: vec![],
            },
        ];
        let pairs = find_duplicates_jaccard(&docs, 0.5);
        assert_eq!(pairs.len(), 1);
        assert!(pairs[0].left_id == "resp-a" || pairs[0].right_id == "resp-a");
    }

    #[test]
    fn empty_query_returns_no_hits() {
        let dir = tempfile::tempdir().unwrap();
        let corpus = dir.path().join("responses");
        fs::create_dir_all(&corpus).unwrap();
        let mut f = fs::File::create(corpus.join("resp-x.md")).unwrap();
        writeln!(
            f,
            "---\nid: resp-x\ntitle: X\nprefix: resp\nsop: none\n---\n\nhello searchable\n"
        )
        .unwrap();
        let idx = dir.path().join("index");
        reindex(&corpus, &idx).unwrap();
        assert!(search(&idx, "   ", 5).unwrap().is_empty());
        assert!(search(&idx, "searchable", 0).unwrap().is_empty());
    }
}
