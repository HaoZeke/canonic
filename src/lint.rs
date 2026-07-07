//! Vale and Harper lint for the markdown corpus (CLI + in-process harper-core).

use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;

/// Which external (or in-process) lint engine to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum LintEngine {
    /// Run Vale (if present) and Harper (in-process, plus CLI if present).
    All,
    /// Run only Vale.
    Vale,
    /// Run Harper via in-process `harper-core` (and optional CLI if on PATH).
    Harper,
}

/// One lint finding from any engine.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LintFinding {
    pub engine: String,
    pub path: String,
    pub message: String,
    pub severity: String,
    pub line: Option<u32>,
}

/// Aggregated report from one or more engines.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LintReport {
    pub findings: Vec<LintFinding>,
    /// Engines that ran successfully.
    pub ran: Vec<String>,
    /// Engines that were requested but missing (explicit messages).
    pub missing: Vec<String>,
}

impl LintReport {
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty() && self.missing.is_empty()
    }

    pub fn summary_line(&self) -> String {
        format!(
            "lint: {} finding(s); ran={:?}; missing={:?}",
            self.findings.len(),
            self.ran,
            self.missing
        )
    }
}

/// Check whether a binary is on PATH and responds successfully to `--version` or `--help`.
pub fn binary_available(name: &str) -> bool {
    for flag in ["--version", "--help", "version"] {
        if let Ok(out) = Command::new(name).arg(flag).output() {
            if out.status.success()
                || !out.stdout.is_empty()
                || String::from_utf8_lossy(&out.stderr)
                    .to_lowercase()
                    .contains(name)
            {
                return true;
            }
        }
    }
    false
}

/// In-process harper-core is always linked in this build.
pub fn harper_core_available() -> bool {
    true
}

/// Construct the Vale command line for the given paths (tested without running Vale).
pub fn vale_command_args(paths: &[PathBuf]) -> Vec<String> {
    let mut args = vec!["--output=JSON".to_string(), "--no-exit".to_string()];
    for p in paths {
        args.push(p.display().to_string());
    }
    args
}

/// Construct Harper CLI args (best-effort across harper-cli naming).
pub fn harper_command_args(paths: &[PathBuf]) -> Vec<String> {
    let mut args = Vec::new();
    for p in paths {
        args.push(p.display().to_string());
    }
    args
}

/// Resolve which binary name to use for Harper CLI (optional fallback).
pub fn harper_binary_name() -> Option<&'static str> {
    ["harper-cli", "harper", "harperls"]
        .into_iter()
        .find(|name| binary_available(name))
}

/// Lint the given paths with the selected engine(s).
/// Missing *optional* tools produce explicit entries in `report.missing` and do **not** panic.
/// Harper always uses in-process `harper-core` when the Harper engine is selected.
pub fn lint_paths(paths: &[PathBuf], engine: LintEngine) -> Result<LintReport> {
    if paths.is_empty() {
        bail!("no paths to lint");
    }
    let mut report = LintReport {
        findings: Vec::new(),
        ran: Vec::new(),
        missing: Vec::new(),
    };

    let want_vale = matches!(engine, LintEngine::All | LintEngine::Vale);
    let want_harper = matches!(engine, LintEngine::All | LintEngine::Harper);

    if want_vale {
        if binary_available("vale") {
            match run_vale(paths) {
                Ok(mut findings) => {
                    report.ran.push("vale".into());
                    report.findings.append(&mut findings);
                }
                Err(e) => {
                    report.findings.push(LintFinding {
                        engine: "vale".into(),
                        path: String::new(),
                        message: format!("vale invocation failed: {e}"),
                        severity: "error".into(),
                        line: None,
                    });
                    report.ran.push("vale".into());
                }
            }
        } else {
            report.missing.push(
                "vale is not installed or not on PATH; install Vale to lint prose style".into(),
            );
        }
    }

    if want_harper {
        // Primary: in-process harper-core (no PATH dependency).
        match run_harper_inprocess(paths) {
            Ok(mut findings) => {
                report.ran.push("harper-core".into());
                report.findings.append(&mut findings);
            }
            Err(e) => {
                report.findings.push(LintFinding {
                    engine: "harper-core".into(),
                    path: String::new(),
                    message: format!("harper-core lint failed: {e}"),
                    severity: "error".into(),
                    line: None,
                });
                report.ran.push("harper-core".into());
            }
        }
        // Optional: also surface CLI findings when a binary is present.
        if let Some(bin) = harper_binary_name() {
            match run_harper_cli(bin, paths) {
                Ok(mut findings) => {
                    report.ran.push(bin.into());
                    report.findings.append(&mut findings);
                }
                Err(e) => {
                    report.findings.push(LintFinding {
                        engine: bin.into(),
                        path: String::new(),
                        message: format!("harper CLI invocation failed: {e}"),
                        severity: "error".into(),
                        line: None,
                    });
                    report.ran.push(bin.into());
                }
            }
        }
    }

    Ok(report)
}

/// Lint a single text buffer with in-process harper-core (unit-testable pure path).
pub fn lint_text_harper_inprocess(text: &str) -> Vec<LintFinding> {
    use harper_core::linting::{LintGroup, Linter};
    use harper_core::spell::FstDictionary;
    use harper_core::{Dialect, Document};

    let mut grammar = LintGroup::new_curated(FstDictionary::curated(), Dialect::American);
    let mut findings = Vec::new();
    for (start_line, paragraph) in prose_paragraphs(text) {
        if paragraph.trim().is_empty() {
            continue;
        }
        let doc = Document::new_plain_english_curated(&paragraph);
        for lint in grammar.lint(&doc) {
            let snippet = doc.get_span_content_str(&lint.span);
            let kind = format!("{:?}", lint.lint_kind);
            findings.push(LintFinding {
                engine: "harper-core".into(),
                path: String::new(),
                message: format!("[{kind}] '{snippet}' — {}", lint.message),
                severity: "suggestion".into(),
                line: Some(start_line as u32),
            });
        }
    }
    findings
}

fn run_harper_inprocess(paths: &[PathBuf]) -> Result<Vec<LintFinding>> {
    let mut all = Vec::new();
    for path in paths {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read {} for harper-core", path.display()))?;
        let display = path.display().to_string();
        for mut f in lint_text_harper_inprocess(&text) {
            f.path = display.clone();
            all.push(f);
        }
    }
    Ok(all)
}

/// Yield (1-based start line, paragraph text), skipping front matter, headings, fences, lists markers lightly.
fn prose_paragraphs(text: &str) -> Vec<(usize, String)> {
    let mut body = text;
    // Strip YAML front matter if present.
    if let Some(rest) = body.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            body = rest[end + 4..].trim_start_matches(['\r', '\n']);
        }
    }
    let mut paragraphs = Vec::new();
    let mut buf = String::new();
    let mut buf_start = 0usize;
    let mut in_fence = false;
    // Map body lines back to original line numbers: count skipped front-matter lines.
    let line_offset = text[..text.len() - body.len()].lines().count();

    for (idx, raw) in body.lines().enumerate() {
        let line_no = line_offset + idx + 1;
        let line = raw.trim();
        if line.starts_with("```") {
            in_fence = !in_fence;
            if !buf.is_empty() {
                paragraphs.push((buf_start, std::mem::take(&mut buf)));
            }
            continue;
        }
        if in_fence {
            continue;
        }
        let skip = line.is_empty()
            || line.starts_with('#')
            || line.starts_with('|')
            || line.starts_with("---");
        if skip {
            if !buf.is_empty() {
                paragraphs.push((buf_start, std::mem::take(&mut buf)));
            }
            continue;
        }
        let body_line = line
            .trim_start_matches(['-', '+', '*', '#'])
            .trim_start_matches(|c: char| c.is_ascii_digit())
            .trim_start_matches(['.', ')'])
            .trim();
        if body_line.is_empty() {
            if !buf.is_empty() {
                paragraphs.push((buf_start, std::mem::take(&mut buf)));
            }
            continue;
        }
        if buf.is_empty() {
            buf_start = line_no;
        } else {
            buf.push(' ');
        }
        buf.push_str(body_line);
    }
    if !buf.is_empty() {
        paragraphs.push((buf_start, buf));
    }
    paragraphs
}

fn run_vale(paths: &[PathBuf]) -> Result<Vec<LintFinding>> {
    let args = vale_command_args(paths);
    let output = Command::new("vale")
        .args(&args)
        .output()
        .context("spawn vale")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&stdout) {
        let mut findings = Vec::new();
        for (path, alerts) in map {
            if let Some(arr) = alerts.as_array() {
                for a in arr {
                    let message = a
                        .get("Message")
                        .or_else(|| a.get("message"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("vale alert")
                        .to_string();
                    let severity = a
                        .get("Severity")
                        .or_else(|| a.get("severity"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("suggestion")
                        .to_string();
                    let line = a
                        .get("Line")
                        .or_else(|| a.get("line"))
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32);
                    findings.push(LintFinding {
                        engine: "vale".into(),
                        path: path.clone(),
                        message,
                        severity,
                        line,
                    });
                }
            }
        }
        return Ok(findings);
    }

    let mut findings = Vec::new();
    let combined = format!("{stdout}{stderr}");
    if !combined.trim().is_empty() && !output.status.success() {
        for line in combined.lines().take(50) {
            if line.trim().is_empty() {
                continue;
            }
            findings.push(LintFinding {
                engine: "vale".into(),
                path: paths
                    .first()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
                message: line.to_string(),
                severity: "info".into(),
                line: None,
            });
        }
    }
    Ok(findings)
}

fn run_harper_cli(bin: &str, paths: &[PathBuf]) -> Result<Vec<LintFinding>> {
    let args = harper_command_args(paths);
    let output = Command::new(bin)
        .args(&args)
        .output()
        .with_context(|| format!("spawn {bin}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    let mut findings = Vec::new();

    if !output.status.success() || combined.to_lowercase().contains("error") {
        for line in combined.lines().take(100) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            findings.push(LintFinding {
                engine: bin.into(),
                path: paths
                    .first()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
                message: line.to_string(),
                severity: "suggestion".into(),
                line: None,
            });
        }
    }
    Ok(findings)
}

/// Format a human-readable report to a string.
pub fn format_report(report: &LintReport) -> String {
    let mut out = String::new();
    out.push_str(&report.summary_line());
    out.push('\n');
    for m in &report.missing {
        out.push_str("MISSING: ");
        out.push_str(m);
        out.push('\n');
    }
    for f in &report.findings {
        let line = f.line.map(|n| format!(":{n}")).unwrap_or_default();
        out.push_str(&format!(
            "[{}] {}{}: {} ({})\n",
            f.engine, f.path, line, f.message, f.severity
        ));
    }
    if report.findings.is_empty() && report.missing.is_empty() {
        out.push_str("No issues found.\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vale_args_include_json_and_paths() {
        let paths = vec![PathBuf::from("corpus/responses/a.md")];
        let args = vale_command_args(&paths);
        assert!(args.iter().any(|a| a.contains("JSON")));
        assert!(args.iter().any(|a| a.ends_with("a.md")));
    }

    #[test]
    fn harper_args_pass_paths() {
        let paths = vec![PathBuf::from("x.md"), PathBuf::from("y.md")];
        let args = harper_command_args(&paths);
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "x.md");
    }

    #[test]
    fn harper_inprocess_flags_classic_article_error() {
        // Documented harper-core example sentence.
        let findings = lint_text_harper_inprocess("This is an test.");
        assert!(
            !findings.is_empty(),
            "expected at least one grammar finding for 'This is an test.': {findings:?}"
        );
        assert!(findings.iter().all(|f| f.engine == "harper-core"));
    }

    #[test]
    fn lint_paths_harper_runs_inprocess_without_cli() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("bad.md");
        std::fs::write(&p, "This is an test of the grammar engine.\n").unwrap();
        let report = lint_paths(&[p], LintEngine::Harper).unwrap();
        assert!(
            report.ran.iter().any(|r| r == "harper-core"),
            "expected harper-core in ran: {:?}",
            report.ran
        );
        // Must not claim MISSING harper when in-process works
        assert!(
            report
                .missing
                .iter()
                .all(|m| !m.to_lowercase().contains("harper")),
            "unexpected missing harper: {:?}",
            report.missing
        );
    }

    #[test]
    fn missing_vale_still_explicit_when_all() {
        let paths = vec![PathBuf::from("Cargo.toml")];
        let report = lint_paths(&paths, LintEngine::All).unwrap();
        // harper-core always runs
        assert!(
            report.ran.iter().any(|r| r == "harper-core"),
            "ran={:?}",
            report.ran
        );
        // If vale missing, message is explicit
        for m in &report.missing {
            assert!(
                m.contains("not installed") || m.contains("PATH"),
                "missing message should be explicit: {m}"
            );
        }
    }

    #[test]
    fn format_report_mentions_missing() {
        let report = LintReport {
            findings: vec![],
            ran: vec![],
            missing: vec!["vale is not installed or not on PATH; install Vale".into()],
        };
        let s = format_report(&report);
        assert!(s.contains("MISSING:"));
        assert!(s.contains("vale"));
    }
}
