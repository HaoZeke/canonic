//! Heed-backed canned-response store with BM25 search.

use crate::corpus::{walk_responses, CannedResponse};
use anyhow::Result;
use heed::types::{Bytes, Str};
use heed::{Database, Env, EnvOpenOptions};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const MAP_SIZE: usize = 64 * 1024 * 1024; // 64 MiB enough for canned-response corpora
const BM25_K1: f64 = 1.5;
const BM25_B: f64 = 0.75;

/// One indexed canned response (persisted as JSON in Heed).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexDoc {
    pub id: String,
    pub title: String,
    pub content: String,
    pub path: String,
    pub tags: Vec<String>,
}

/// A ranked hit from BM25 over the local index.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub id: String,
    pub score: f64,
    pub title: String,
    pub snippet: String,
    pub path: PathBuf,
}

/// Default index directory (gitignored) under the working tree.
pub fn default_index_dir() -> PathBuf {
    PathBuf::from(".canonic-index")
}

/// Tokenize for BM25: lowercase, split on non-alphanumeric.
pub fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|s| s.to_string())
        .collect()
}

impl From<&CannedResponse> for IndexDoc {
    fn from(r: &CannedResponse) -> Self {
        IndexDoc {
            id: r.id.clone(),
            title: r.title.clone(),
            content: r.content.clone(),
            path: r.path.display().to_string(),
            tags: r.tags.clone(),
        }
    }
}

fn open_env(index_dir: &Path) -> heed::Result<Env> {
    fs::create_dir_all(index_dir).ok();
    unsafe {
        EnvOpenOptions::new()
            .map_size(MAP_SIZE)
            .max_dbs(4)
            .open(index_dir)
    }
}

type DocDb = Database<Str, Bytes>;

fn open_docs(env: &Env) -> heed::Result<DocDb> {
    let mut wtxn = env.write_txn()?;
    let db = env.create_database(&mut wtxn, Some("docs"))?;
    wtxn.commit()?;
    Ok(db)
}

/// Reindex all markdown responses under `corpus_dir` into Heed at `index_dir`.
/// Returns number of documents written.
pub fn reindex(corpus_dir: &Path, index_dir: &Path) -> Result<usize> {
    let responses = walk_responses(corpus_dir)?;
    let env = open_env(index_dir)?;
    let db = open_docs(&env)?;
    let mut wtxn = env.write_txn()?;

    let keys: Vec<String> = db
        .iter(&wtxn)?
        .filter_map(|r| r.ok().map(|(k, _)| k.to_string()))
        .collect();
    for k in keys {
        db.delete(&mut wtxn, &k)?;
    }

    let mut n = 0usize;
    for r in responses {
        let doc = IndexDoc::from(&r);
        let bytes = serde_json::to_vec(&doc)?;
        db.put(&mut wtxn, &doc.id, &bytes)?;
        n += 1;
    }
    wtxn.commit()?;
    Ok(n)
}

/// Load all docs from the Heed store.
pub fn load_docs(index_dir: &Path) -> Result<Vec<IndexDoc>> {
    if !index_dir.exists() {
        return Ok(vec![]);
    }
    let env = open_env(index_dir)?;
    let db = open_docs(&env)?;
    let rtxn = env.read_txn()?;
    let mut docs = Vec::new();
    for item in db.iter(&rtxn)? {
        let (_k, v) = item?;
        let doc: IndexDoc = serde_json::from_slice(v)?;
        docs.push(doc);
    }
    Ok(docs)
}

/// BM25 score of query tokens against a document token list.
pub fn bm25_score(
    query_tokens: &[String],
    doc_tokens: &[String],
    avgdl: f64,
    idf: &HashMap<String, f64>,
) -> f64 {
    if query_tokens.is_empty() || doc_tokens.is_empty() || avgdl <= 0.0 {
        return 0.0;
    }
    let mut tf: HashMap<&str, f64> = HashMap::new();
    for t in doc_tokens {
        *tf.entry(t.as_str()).or_insert(0.0) += 1.0;
    }
    let dl = doc_tokens.len() as f64;
    let mut score = 0.0;
    for qt in query_tokens {
        let f = *tf.get(qt.as_str()).unwrap_or(&0.0);
        if f == 0.0 {
            continue;
        }
        let idf_t = *idf.get(qt.as_str()).unwrap_or(&0.0);
        let denom = f + BM25_K1 * (1.0 - BM25_B + BM25_B * (dl / avgdl));
        score += idf_t * (f * (BM25_K1 + 1.0)) / denom;
    }
    score
}

fn build_idf(docs: &[Vec<String>]) -> (HashMap<String, f64>, f64) {
    let n = docs.len() as f64;
    let mut df: HashMap<String, f64> = HashMap::new();
    let mut total_len = 0.0;
    for d in docs {
        total_len += d.len() as f64;
        let mut seen = std::collections::HashSet::new();
        for t in d {
            if seen.insert(t.as_str()) {
                *df.entry(t.clone()).or_insert(0.0) += 1.0;
            }
        }
    }
    let avgdl = if n > 0.0 { total_len / n } else { 0.0 };
    let mut idf = HashMap::new();
    for (term, dfi) in df {
        // Robertson-Sparck Jones idf with +0.5 smoothing.
        let val = ((n - dfi + 0.5) / (dfi + 0.5) + 1.0).ln();
        idf.insert(term, val.max(0.0));
    }
    (idf, avgdl)
}

fn doc_tokens(doc: &IndexDoc) -> Vec<String> {
    let mut t = tokenize(&doc.id);
    t.extend(tokenize(&doc.title));
    t.extend(tokenize(&doc.content));
    for tag in &doc.tags {
        t.extend(tokenize(tag));
    }
    t
}

/// Search the Heed-backed corpus with BM25. Returns hits sorted by score desc.
pub fn search(index_dir: &Path, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
    let docs = load_docs(index_dir)?;
    search_docs(&docs, query, limit)
}

/// Pure BM25 over an in-memory doc list (unit-tested without Heed I/O).
pub fn search_docs(docs: &[IndexDoc], query: &str, limit: usize) -> Result<Vec<SearchHit>> {
    let q_tokens = tokenize(query);
    if q_tokens.is_empty() {
        return Ok(vec![]);
    }
    let tokenized: Vec<Vec<String>> = docs.iter().map(doc_tokens).collect();
    let (idf, avgdl) = build_idf(&tokenized);
    let mut hits: Vec<SearchHit> = docs
        .iter()
        .zip(tokenized.iter())
        .filter_map(|(doc, toks)| {
            let score = bm25_score(&q_tokens, toks, avgdl, &idf);
            if score <= 0.0 {
                return None;
            }
            Some(SearchHit {
                id: doc.id.clone(),
                score,
                title: doc.title.clone(),
                snippet: snippet_for(&doc.content, &q_tokens),
                path: PathBuf::from(&doc.path),
            })
        })
        .collect();
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits.truncate(limit);
    Ok(hits)
}

fn snippet_for(text: &str, query_tokens: &[String]) -> String {
    let flat = text.replace('\n', " ");
    if flat.is_empty() {
        return String::new();
    }
    let lower = flat.to_lowercase();
    let mut pos = None;
    for qt in query_tokens {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn tokenize_splits_and_lowercases() {
        assert_eq!(
            tokenize("Hello, VPN-Access!"),
            vec!["hello", "vpn", "access"]
        );
    }

    #[test]
    fn bm25_ranks_relevant_doc_first() {
        let docs = vec![
            IndexDoc {
                id: "password-reset".into(),
                title: "Password reset self-service".into(),
                content: "use the self-service portal to reset your account password".into(),
                path: "password-reset.md".into(),
                tags: vec!["password".into()],
            },
            IndexDoc {
                id: "vpn-access".into(),
                title: "Corporate VPN onboarding".into(),
                content: "install wireguard profile corporate for remote network access vpn_dns_failure"
                    .into(),
                path: "vpn-access.md".into(),
                tags: vec!["vpn".into()],
            },
            IndexDoc {
                id: "license-renewal".into(),
                title: "Software license renewal".into(),
                content: "procurement request with vendor quote and LICENSE_RUSH subject".into(),
                path: "license-renewal.md".into(),
                tags: vec!["license".into()],
            },
        ];
        let hits = search_docs(&docs, "wireguard vpn_dns_failure corporate", 5).unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].id, "vpn-access");
        assert!(hits[0].score > 0.0);
        // Relevant should outrank password-reset for this query
        if hits.len() > 1 {
            assert!(hits[0].score >= hits[1].score);
        }
    }

    #[test]
    fn reindex_and_search_roundtrip_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let corpus = dir.path().join("responses");
        fs::create_dir_all(&corpus).unwrap();
        let mut hit = fs::File::create(corpus.join("hit.md")).unwrap();
        writeln!(
            hit,
            "---\nid: unique-hit\ntitle: Unique Hit\n---\n\nunique_zxqword appears only here\n"
        )
        .unwrap();
        fs::write(
            corpus.join("miss.md"),
            "---\nid: miss\ntitle: Miss\n---\n\ntotally unrelated cooking recipes\n",
        )
        .unwrap();

        let idx = dir.path().join("index");
        let n = reindex(&corpus, &idx).unwrap();
        assert_eq!(n, 2);
        let hits = search(&idx, "unique_zxqword", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "unique-hit");
        assert!(hits[0].score > 0.0);
    }

    #[test]
    fn empty_query_returns_no_hits() {
        let docs = vec![IndexDoc {
            id: "x".into(),
            title: "X".into(),
            content: "hello".into(),
            path: "x.md".into(),
            tags: vec![],
        }];
        let hits = search_docs(&docs, "   ", 5).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn zero_limit_returns_no_hits() {
        let docs = vec![IndexDoc {
            id: "x".into(),
            title: "X".into(),
            content: "hello searchable content".into(),
            path: "x.md".into(),
            tags: vec![],
        }];
        let hits = search_docs(&docs, "searchable", 0).unwrap();
        assert!(hits.is_empty());
    }
}
