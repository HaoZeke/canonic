//! Vale and Harper lint invocation for the markdown corpus.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Which external (or in-process) lint engine to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum LintEngine {
    /// Run both Vale and Harper when available.
    All,
    /// Run only Vale.
    Vale,
    /// Run only Harper (`harper-cli` / `harper` on PATH).
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
            if out.status.success() || !out.stdout.is_empty() || !out.stderr.is_empty() {
                // Some CLIs exit non-zero on --version but still print; treat as present if we got any output.
                if out.status.success()
                    || !out.stdout.is_empty()
                    || String::from_utf8_lossy(&out.stderr).to_lowercase().contains(name)
                {
                    return true;
                }
            }
        }
    }
    // Last resort: `which`-like via `command -v` not available; try bare spawn of --help
    Command::new(name)
        .arg("--help")
        .output()
        .map(|o| o.status.success() || !o.stdout.is_empty() || !o.stderr.is_empty())
        .unwrap_or(false)
}

/// Construct the Vale command line for the given paths (tested without running Vale).
pub fn vale_command_args(paths: &[PathBuf]) -> Vec<String> {
    let mut args = vec![
        "--output=JSON".to_string(),
        "--no-exit".to_string(),
    ];
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

/// Resolve which binary name to use for Harper.
pub fn harper_binary_name() -> Option<&'static str> {
    for name in ["harper-cli", "harper", "harperls"] {
        if binary_available(name) {
            return Some(name);
        }
    }
    None
}

/// Lint the given paths with the selected engine(s).
/// Missing tools produce explicit entries in `report.missing` and do **not** panic.
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
        if let Some(bin) = harper_binary_name() {
            match run_harper(bin, paths) {
                Ok(mut findings) => {
                    report.ran.push(bin.into());
                    report.findings.append(&mut findings);
                }
                Err(e) => {
                    report.findings.push(LintFinding {
                        engine: bin.into(),
                        path: String::new(),
                        message: format!("harper invocation failed: {e}"),
                        severity: "error".into(),
                        line: None,
                    });
                    report.ran.push(bin.into());
                }
            }
        } else {
            report.missing.push(
                "harper is not installed or not on PATH (tried harper-cli, harper, harperls); install Harper for grammar checks"
                    .into(),
            );
        }
    }

    // If every requested engine is missing, still succeed with an explicit report (non-panic).
    Ok(report)
}

fn run_vale(paths: &[PathBuf]) -> Result<Vec<LintFinding>> {
    let args = vale_command_args(paths);
    let output = Command::new("vale")
        .args(&args)
        .output()
        .context("spawn vale")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Vale JSON: object keyed by file path -> array of alerts
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

    // Non-JSON fallback: treat non-empty human output as a single informational finding set
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

fn run_harper(bin: &str, paths: &[PathBuf]) -> Result<Vec<LintFinding>> {
    let args = harper_command_args(paths);
    let output = Command::new(bin)
        .args(&args)
        .output()
        .with_context(|| format!("spawn {bin}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    let mut findings = Vec::new();

    // Harper CLI output formats vary; capture non-empty lines as findings when it reports issues.
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
    // Clean exit with no output → zero findings (success)
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
        let line = f
            .line
            .map(|n| format!(":{n}"))
            .unwrap_or_default();
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

/// Helper for tests: assert vale args include JSON output mode.
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
    fn missing_tools_produce_explicit_messages() {
        // Use a path that exists for the empty-check; engines may or may not be present.
        let paths = vec![PathBuf::from("Cargo.toml")];
        // Force both engines; if missing, we get messages rather than panic.
        let report = lint_paths(&paths, LintEngine::All).unwrap();
        // At least one of: ran something, or reported missing.
        assert!(
            !report.ran.is_empty() || !report.missing.is_empty(),
            "expected ran or missing entries: {report:?}"
        );
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
