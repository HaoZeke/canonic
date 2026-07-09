//! Corpus quality checks for a configured shared id prefix.

use crate::corpus::{walk_responses, CannedResponse};
use anyhow::Result;
use regex::Regex;
use serde::Serialize;
use std::path::Path;
use std::sync::OnceLock;

/// One quality finding.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CheckFinding {
    pub path: String,
    pub id: String,
    pub code: String,
    pub message: String,
}

/// Aggregated quality report.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CheckReport {
    pub findings: Vec<CheckFinding>,
    pub checked: usize,
}

impl CheckReport {
    pub fn ok(&self) -> bool {
        self.findings.is_empty()
    }

    pub fn summary_line(&self) -> String {
        format!(
            "check: {} file(s), {} finding(s)",
            self.checked,
            self.findings.len()
        )
    }
}

/// Run all quality rules over a corpus directory using `prefix` (from config/CLI).
pub fn check_corpus(root: &Path, prefix: &str) -> Result<CheckReport> {
    let docs = walk_responses(root)?;
    Ok(check_responses(&docs, prefix))
}

/// Pure quality rules over already-loaded responses (unit-testable).
pub fn check_responses(docs: &[CannedResponse], prefix: &str) -> CheckReport {
    let mut findings = Vec::new();
    for doc in docs {
        findings.extend(check_one(doc, prefix));
    }
    // Duplicate ids across corpus
    let mut seen: std::collections::HashMap<&str, &Path> = std::collections::HashMap::new();
    for doc in docs {
        if let Some(prev) = seen.insert(doc.id.as_str(), doc.path.as_path()) {
            findings.push(CheckFinding {
                path: doc.path.display().to_string(),
                id: doc.id.clone(),
                code: "duplicate-id".into(),
                message: format!(
                    "id `{}` also used by {}",
                    doc.id,
                    prev.display()
                ),
            });
        }
    }
    CheckReport {
        findings,
        checked: docs.len(),
    }
}

fn check_one(doc: &CannedResponse, prefix: &str) -> Vec<CheckFinding> {
    let mut out = Vec::new();
    let path = doc.path.display().to_string();
    let id_prefix = format!("{prefix}-");

    if !doc.id.starts_with(&id_prefix) {
        out.push(finding(
            &path,
            &doc.id,
            "id-prefix",
            format!(
                "id must start with `{prefix}-` (shared library prefix from canonic.toml / --prefix); got `{}`",
                doc.id
            ),
        ));
    }

    let stem = doc
        .path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if stem != doc.id {
        out.push(finding(
            &path,
            &doc.id,
            "id-filename",
            format!("filename stem `{stem}` must equal id `{}`", doc.id),
        ));
    }

    match doc.prefix.as_deref() {
        Some(p) if p == prefix => {}
        Some(p) => out.push(finding(
            &path,
            &doc.id,
            "prefix-field",
            format!("front matter `prefix` must be `{prefix}`; got `{p}`"),
        )),
        None => out.push(finding(
            &path,
            &doc.id,
            "prefix-field",
            format!("front matter `prefix: {prefix}` is required"),
        )),
    }

    match doc.sop.as_deref() {
        Some(s) if !s.trim().is_empty() => {}
        _ => out.push(finding(
            &path,
            &doc.id,
            "sop-field",
            "front matter `sop` is required (Confluence URL or the literal `none`)".into(),
        )),
    }

    if doc.title.trim().is_empty() {
        out.push(finding(
            &path,
            &doc.id,
            "title",
            "title must be non-empty".into(),
        ));
    }

    if let Some(msg) = personal_signature_hit(&doc.content) {
        out.push(finding(
            &path,
            &doc.id,
            "personal-signature",
            format!("possible personal sign-off; use shared team closing ({msg})"),
        ));
    }

    out
}

fn finding(path: &str, id: &str, code: &str, message: String) -> CheckFinding {
    CheckFinding {
        path: path.into(),
        id: id.into(),
        code: code.into(),
        message,
    }
}

fn personal_signature_hit(content: &str) -> Option<&'static str> {
    // Reject common personal closings that should be team-generic for shared responses.
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r"(?im)^\s*(cheers|thanks,?\s*$|best,?\s*$|yours,?\s*$|sincerely,?\s*$)\s*$|^\s*[-–—]\s*[A-Z][a-z]{1,12}\s*$|^\s*(John|Jane|Manuel|Alice|Bob)\s*$",
        )
        .expect("signature regex")
    });
    if re.is_match(content) {
        return Some("matched personal closing/name pattern");
    }
    let lines: Vec<&str> = content.lines().map(|l| l.trim()).collect();
    for w in lines.windows(2) {
        if w[0].eq_ignore_ascii_case("regards,") || w[0].eq_ignore_ascii_case("regards") {
            let next = w[1];
            if !next.is_empty()
                && !next.to_lowercase().contains("support")
                && !next.to_lowercase().contains("team")
                && next.split_whitespace().count() <= 2
                && next
                    .chars()
                    .all(|c| c.is_alphabetic() || c.is_whitespace() || c == '-')
            {
                return Some("Regards followed by personal name instead of team");
            }
        }
    }
    None
}

/// Format a human-readable check report.
pub fn format_check_report(report: &CheckReport) -> String {
    let mut s = report.summary_line();
    s.push('\n');
    for f in &report.findings {
        s.push_str(&format!(
            "[{}] {} ({}): {}\n",
            f.code, f.path, f.id, f.message
        ));
    }
    if report.ok() {
        s.push_str("All quality checks passed.\n");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DEFAULT_PREFIX;
    use crate::corpus::CannedResponse;
    use std::path::PathBuf;

    fn doc(id: &str, prefix: Option<&str>, sop: Option<&str>, content: &str) -> CannedResponse {
        CannedResponse {
            id: id.into(),
            title: "Title".into(),
            prefix: prefix.map(str::to_string),
            sop: sop.map(str::to_string),
            body: content.into(),
            content: content.into(),
            path: PathBuf::from(format!("{id}.md")),
            tags: vec![],
        }
    }

    #[test]
    fn accepts_well_formed_response() {
        let d = doc(
            "resp-project-space-not-backup",
            Some(DEFAULT_PREFIX),
            Some("none"),
            "Hello,\n\nBody.\n\nRegards,\nSupport Team\n",
        );
        let r = check_responses(&[d], DEFAULT_PREFIX);
        assert!(r.ok(), "{:?}", r.findings);
    }

    #[test]
    fn honors_custom_prefix() {
        let d = doc(
            "acme-topic",
            Some("acme"),
            Some("none"),
            "Hello,\n\nBody.\n\nRegards,\nSupport Team\n",
        );
        let r = check_responses(&[d], "acme");
        assert!(r.ok(), "{:?}", r.findings);
    }

    #[test]
    fn rejects_missing_prefix_and_bad_id() {
        let d = doc("password-reset", None, None, "Body\n");
        let r = check_responses(&[d], DEFAULT_PREFIX);
        let codes: Vec<_> = r.findings.iter().map(|f| f.code.as_str()).collect();
        assert!(codes.contains(&"id-prefix"), "{codes:?}");
        assert!(codes.contains(&"prefix-field"), "{codes:?}");
        assert!(codes.contains(&"sop-field"), "{codes:?}");
    }

    #[test]
    fn rejects_personal_signoff_name_after_regards() {
        let d = doc(
            "resp-foo",
            Some(DEFAULT_PREFIX),
            Some("none"),
            "Hello.\n\nRegards,\nAlice\n",
        );
        let r = check_responses(&[d], DEFAULT_PREFIX);
        assert!(
            r.findings.iter().any(|f| f.code == "personal-signature"),
            "{:?}",
            r.findings
        );
    }

    #[test]
    fn rejects_duplicate_ids() {
        let a = doc(
            "resp-a",
            Some(DEFAULT_PREFIX),
            Some("none"),
            "x\n\nRegards,\nSupport Team\n",
        );
        let mut b = a.clone();
        b.path = PathBuf::from("other/resp-a.md");
        let r = check_responses(&[a, b], DEFAULT_PREFIX);
        assert!(r.findings.iter().any(|f| f.code == "duplicate-id"));
    }
}
