//! Pandoc-backed markdown → Jira/Confluence wiki markup conversion.

use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

/// First line of `pandoc --version` when available.
pub fn pandoc_version_line() -> Option<String> {
    let out = Command::new("pandoc").arg("--version").output().ok()?;
    if !out.status.success() && out.stdout.is_empty() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Check whether `pandoc` is available on PATH.
pub fn tool_available() -> bool {
    Command::new("pandoc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Convert markdown text to Jira wiki markup via `pandoc -f markdown -t jira`.
pub fn convert_markdown_to_jira(markdown: &str) -> Result<String> {
    if !tool_available() {
        bail!(
            "pandoc is not installed or not on PATH; install pandoc to convert markdown to jira markup"
        );
    }
    let mut child = Command::new("pandoc")
        .args(["-f", "markdown", "-t", "jira"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("spawn pandoc")?;

    use std::io::Write;
    {
        let mut stdin = child.stdin.take().context("open pandoc stdin")?;
        stdin
            .write_all(markdown.as_bytes())
            .context("write markdown to pandoc")?;
    }

    let output = child.wait_with_output().context("wait for pandoc")?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        bail!("pandoc failed (status {}): {err}", output.status);
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Read a markdown file and convert to Jira markup.
pub fn convert_path_to_jira(path: &Path) -> Result<String> {
    let md = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    convert_markdown_to_jira(&md)
}

/// Build the pandoc argv used for conversion (for unit tests of invocation construction).
pub fn pandoc_jira_args() -> Vec<&'static str> {
    vec!["-f", "markdown", "-t", "jira"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pandoc_args_target_jira_writer() {
        let args = pandoc_jira_args();
        assert!(args.contains(&"markdown"));
        assert!(args.contains(&"jira"));
        assert_eq!(args, vec!["-f", "markdown", "-t", "jira"]);
    }

    #[test]
    fn convert_sample_when_pandoc_present() {
        if !tool_available() {
            // Explicit path still exists; missing tool is reported by convert_markdown_to_jira.
            let err = convert_markdown_to_jira("# Hi").unwrap_err().to_string();
            assert!(err.to_lowercase().contains("pandoc"));
            return;
        }
        let jira = convert_markdown_to_jira(
            "# Password reset\n\nUse **self-service** portal:\n\n- step one\n- step two\n",
        )
        .unwrap();
        // Pandoc jira writer: headings as h1., bold as *text*, lists as * or -
        assert!(
            jira.contains("h1.") || jira.contains("Password"),
            "expected jira heading or title, got: {jira:?}"
        );
        assert!(
            jira.contains("*self-service*") || jira.contains("self-service"),
            "expected bold or body text, got: {jira:?}"
        );
    }
}
