//! Load and walk version-controlled markdown canned responses.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// One canned response derived from a markdown file under the corpus.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CannedResponse {
    /// Stable id (front matter `id` or file stem).
    pub id: String,
    /// Display title (front matter `title` or first heading / id).
    pub title: String,
    /// Full markdown body (including optional front matter as stored on disk).
    pub body: String,
    /// Body with YAML front matter stripped, used for search/indexing.
    pub content: String,
    /// Absolute or relative path to the source markdown file.
    pub path: PathBuf,
    /// Optional tags from front matter.
    pub tags: Vec<String>,
}

/// Default corpus directory relative to the current working directory / repo root.
pub fn default_corpus_dir() -> PathBuf {
    PathBuf::from("corpus/responses")
}

/// Walk `root` for `*.md` files and load each as a [`CannedResponse`].
pub fn walk_responses(root: &Path) -> Result<Vec<CannedResponse>> {
    if !root.exists() {
        bail!("corpus directory does not exist: {}", root.display());
    }
    let mut out = Vec::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        out.push(load_response(path)?);
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

/// Load a single markdown file into a [`CannedResponse`].
pub fn load_response(path: &Path) -> Result<CannedResponse> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("read canned response {}", path.display()))?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let (fm, content) = split_front_matter(&raw);
    let id = fm
        .get("id")
        .cloned()
        .unwrap_or_else(|| stem.clone());
    let title = fm
        .get("title")
        .cloned()
        .or_else(|| first_heading(&content))
        .unwrap_or_else(|| id.clone());
    let tags = fm
        .get("tags")
        .map(|s| parse_tags(s))
        .unwrap_or_default();

    Ok(CannedResponse {
        id,
        title,
        body: raw,
        content,
        path: path.to_path_buf(),
        tags,
    })
}

/// Minimal YAML front-matter parser for `key: value` and simple list tags.
/// Not a full YAML implementation — enough for our corpus convention.
fn split_front_matter(raw: &str) -> (std::collections::HashMap<String, String>, String) {
    let mut map = std::collections::HashMap::new();
    let trimmed = raw.trim_start_matches('\u{feff}');
    if !trimmed.starts_with("---") {
        return (map, raw.to_string());
    }
    let rest = &trimmed[3..];
    let rest = rest.trim_start_matches(['\r', '\n']);
    if let Some(end) = rest.find("\n---") {
        let block = &rest[..end];
        let body = rest[end + 4..].trim_start_matches(['\r', '\n']).to_string();
        for line in block.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once(':') {
                let key = k.trim().to_string();
                let val = v.trim().to_string();
                map.insert(key, val);
            }
        }
        return (map, body);
    }
    (map, raw.to_string())
}

fn parse_tags(s: &str) -> Vec<String> {
    let s = s.trim();
    if s.starts_with('[') && s.ends_with(']') {
        s[1..s.len() - 1]
            .split(',')
            .map(|t| t.trim().trim_matches('"').trim_matches('\'').to_string())
            .filter(|t| !t.is_empty())
            .collect()
    } else if s.is_empty() {
        vec![]
    } else {
        vec![s.to_string()]
    }
}

fn first_heading(content: &str) -> Option<String> {
    for line in content.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix('#') {
            let title = rest.trim_start_matches('#').trim();
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn load_parses_front_matter_and_body() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sample.md");
        let mut f = fs::File::create(&p).unwrap();
        writeln!(
            f,
            "---\nid: sample-id\ntitle: Sample Title\ntags: [a, b]\n---\n\n# Heading\n\nBody text.\n"
        )
        .unwrap();
        let r = load_response(&p).unwrap();
        assert_eq!(r.id, "sample-id");
        assert_eq!(r.title, "Sample Title");
        assert_eq!(r.tags, vec!["a", "b"]);
        assert!(r.content.contains("Body text"));
        assert!(!r.content.contains("sample-id"));
    }

    #[test]
    fn walk_finds_markdown_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("one.md"), "# One\n\nalpha\n").unwrap();
        fs::write(dir.path().join("two.md"), "# Two\n\nbeta\n").unwrap();
        fs::write(dir.path().join("skip.txt"), "nope").unwrap();
        let docs = walk_responses(dir.path()).unwrap();
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].id, "one");
        assert_eq!(docs[1].id, "two");
    }
}
