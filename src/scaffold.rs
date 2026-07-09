//! Scaffold new responses and promote import drafts into the published corpus.

use crate::check::{check_responses, CheckReport};
use crate::config::DEFAULT_PREFIX;
use crate::corpus::load_response;
use crate::jira_import::slugify;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Default team closing used by scaffolds (must pass personal-signature checks).
pub const TEAM_SIGN_OFF: &str = "Regards,\nSupport Team\n";

/// Options for rendering a new canned-response markdown file.
#[derive(Debug, Clone)]
pub struct ScaffoldOptions {
    /// Human title (also used for the H1 and default id slug).
    pub title: String,
    /// Explicit id (`{prefix}-…`); when `None`, derived as `{prefix}-<slugify(title)>`.
    pub id: Option<String>,
    /// Shared library prefix (from canonic.toml / --prefix).
    pub prefix: String,
    /// Front-matter `sop` (Confluence URL or `none`).
    pub sop: String,
    /// Optional tags (without brackets).
    pub tags: Vec<String>,
    /// Optional body paragraphs (between greeting and sign-off). Empty → placeholder.
    pub body: Option<String>,
}

impl Default for ScaffoldOptions {
    fn default() -> Self {
        Self {
            title: String::new(),
            id: None,
            prefix: DEFAULT_PREFIX.to_string(),
            sop: "none".into(),
            tags: Vec::new(),
            body: None,
        }
    }
}

/// Resolve a stable `{prefix}-` id from options (or title).
pub fn resolve_response_id(opts: &ScaffoldOptions) -> Result<String> {
    let prefix = opts.prefix.trim();
    if prefix.is_empty() {
        bail!("prefix must be non-empty");
    }
    let raw = if let Some(ref id) = opts.id {
        id.trim().to_string()
    } else {
        let slug = slugify(&opts.title);
        format!("{prefix}-{slug}")
    };
    if raw.is_empty() {
        bail!("response id is empty");
    }
    let want = format!("{prefix}-");
    if !raw.starts_with(&want) {
        bail!(
            "id must start with `{prefix}-` (got `{raw}`); pass --id or a title that slugifies cleanly"
        );
    }
    let rest = &raw[want.len()..];
    if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        bail!("id `{raw}` contains characters that are not safe for a markdown filename stem");
    }
    Ok(raw)
}

/// Render check-clean markdown for a new response (pure; no IO).
pub fn scaffold_markdown(opts: &ScaffoldOptions) -> Result<String> {
    if opts.title.trim().is_empty() {
        bail!("title must be non-empty");
    }
    let prefix = opts.prefix.trim();
    let id = resolve_response_id(opts)?;
    let sop = opts.sop.trim();
    if sop.is_empty() {
        bail!("sop must be non-empty (use the literal `none` if there is no SOP URL)");
    }
    let tags = if opts.tags.is_empty() {
        "[]".to_string()
    } else {
        format!(
            "[{}]",
            opts.tags
                .iter()
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let body = opts
        .body
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(
            "Replace this paragraph with the shared team answer. Keep the team sign-off below.",
        );
    let title = opts.title.trim();
    Ok(format!(
        "---\nid: {id}\ntitle: {title}\nprefix: {prefix}\ntags: {tags}\nsop: {sop}\n---\n\n# {title}\n\nHello,\n\n{body}\n\n{TEAM_SIGN_OFF}"
    ))
}

/// Write a scaffolded response under `dir` as `{id}.md`. Refuses to overwrite unless `force`.
pub fn write_scaffold(dir: &Path, opts: &ScaffoldOptions, force: bool) -> Result<PathBuf> {
    let id = resolve_response_id(opts)?;
    let text = scaffold_markdown(opts)?;
    fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let path = dir.join(format!("{id}.md"));
    if path.exists() && !force {
        bail!(
            "refusing to overwrite {} (pass --force to replace)",
            path.display()
        );
    }
    fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

/// Quality-check a single on-disk markdown response (real check rules).
pub fn check_response_path(path: &Path, prefix: &str) -> Result<CheckReport> {
    let doc = load_response(path)?;
    Ok(check_responses(&[doc], prefix))
}

/// Promote an import draft into the published corpus after `check` against `prefix`.
pub fn promote_to_corpus(
    src: &Path,
    dest_dir: &Path,
    force: bool,
    prefix: &str,
) -> Result<PathBuf> {
    if !src.is_file() {
        bail!("promote source is not a file: {}", src.display());
    }
    let report = check_response_path(src, prefix)?;
    if !report.ok() {
        let detail = report
            .findings
            .iter()
            .map(|f| format!("  [{}] {}", f.code, f.message))
            .collect::<Vec<_>>()
            .join("\n");
        bail!(
            "refuse to promote {}: quality check failed ({} finding(s)):\n{detail}\nEdit the draft until `canonic check` is clean, then promote again.",
            src.display(),
            report.findings.len()
        );
    }
    let doc = load_response(src)?;
    let dest = dest_dir.join(format!("{}.md", doc.id));
    if dest.exists() && !force {
        bail!(
            "refusing to overwrite published response {} (pass --force to replace)",
            dest.display()
        );
    }
    fs::create_dir_all(dest_dir).with_context(|| format!("create {}", dest_dir.display()))?;
    fs::copy(src, &dest).with_context(|| {
        format!(
            "copy {} → {}",
            src.display(),
            dest.display()
        )
    })?;
    let after = check_response_path(&dest, prefix)?;
    if !after.ok() {
        let _ = fs::remove_file(&dest);
        bail!(
            "promoted file failed quality check after copy: {:?}",
            after.findings
        );
    }
    Ok(dest)
}

#[cfg(test)]
fn from_markdown_for_test(path: &Path, text: &str) -> Result<crate::corpus::CannedResponse> {
    fs::write(path, text)?;
    load_response(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::check_responses;
    use crate::config::DEFAULT_PREFIX;
    use tempfile::tempdir;

    #[test]
    fn scaffold_markdown_is_check_clean() {
        let opts = ScaffoldOptions {
            title: "Project space is not a backup".into(),
            id: None,
            prefix: DEFAULT_PREFIX.into(),
            sop: "none".into(),
            tags: vec!["storage".into()],
            body: Some(
                "Shared project storage is for active working data, not long-term backup."
                    .into(),
            ),
        };
        let md = scaffold_markdown(&opts).unwrap();
        assert!(md.contains("prefix: resp"));
        assert!(md.contains("id: resp-project-space-is-not-a-backup"));
        assert!(md.contains("Support Team"));
        let dir = tempdir().unwrap();
        let path = dir.path().join("resp-project-space-is-not-a-backup.md");
        let doc = from_markdown_for_test(&path, &md).unwrap();
        let report = check_responses(&[doc], DEFAULT_PREFIX);
        assert!(report.ok(), "{:?}", report.findings);
    }

    #[test]
    fn resolve_id_rejects_wrong_prefix() {
        let opts = ScaffoldOptions {
            title: "X".into(),
            id: Some("personal-foo".into()),
            prefix: DEFAULT_PREFIX.into(),
            ..Default::default()
        };
        assert!(resolve_response_id(&opts).is_err());
    }

    #[test]
    fn custom_prefix_scaffold() {
        let opts = ScaffoldOptions {
            title: "Queue limits".into(),
            id: None,
            prefix: "acme".into(),
            sop: "none".into(),
            tags: vec![],
            body: Some("Use the documented partition limits.".into()),
        };
        let dir = tempdir().unwrap();
        let path = write_scaffold(dir.path(), &opts, false).unwrap();
        assert!(path.ends_with("acme-queue-limits.md"));
        let report = check_response_path(&path, "acme").unwrap();
        assert!(report.ok(), "{:?}", report.findings);
    }

    #[test]
    fn promote_rejects_dirty_draft() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("password-reset.md");
        fs::write(&src, "# no front matter\n\nRegards,\nAlice\n").unwrap();
        let dest = dir.path().join("out");
        let err = promote_to_corpus(&src, &dest, false, DEFAULT_PREFIX).unwrap_err();
        assert!(
            err.to_string().contains("quality check failed")
                || err.to_string().contains("refuse"),
            "{err}"
        );
    }

    #[test]
    fn promote_copies_clean_draft() {
        let dir = tempdir().unwrap();
        let opts = ScaffoldOptions {
            title: "Module environments".into(),
            id: Some("resp-module-environments".into()),
            prefix: DEFAULT_PREFIX.into(),
            sop: "none".into(),
            tags: vec!["software".into()],
            body: Some("Load the module system before running jobs.".into()),
        };
        let imports = dir.path().join("imports");
        let responses = dir.path().join("responses");
        let src = write_scaffold(&imports, &opts, false).unwrap();
        let dest = promote_to_corpus(&src, &responses, false, DEFAULT_PREFIX).unwrap();
        assert!(dest.exists());
        assert!(src.exists(), "source draft kept for human cleanup");
        assert_eq!(
            dest.file_name().unwrap().to_str().unwrap(),
            "resp-module-environments.md"
        );
        assert!(check_response_path(&dest, DEFAULT_PREFIX).unwrap().ok());
    }
}
