//! Layered project configuration (file-first, no env for app settings).
//!
//! Built with [`figment`] (the usual Rust 12-factor style layerer):
//!
//! 1. Struct defaults ([`DEFAULT_PREFIX`], empty Jira block)
//! 2. Discovered `canonic.toml` (walk up from cwd), or `--config PATH`
//! 3. Optional `canonic.local.toml` beside the discovered file (gitignored secrets)
//! 4. Explicit CLI overrides (`--prefix`)
//!
//! Jira credentials live under `[jira]` in those TOML files — not `JIRA_*` env vars.

use anyhow::{bail, Context, Result};
use figment::{
    providers::{Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};

/// Default shared library prefix when no config/CLI override is set.
pub const DEFAULT_PREFIX: &str = "resp";

/// Free-tier Jira connection settings (from `canonic.toml` / `canonic.local.toml`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JiraSettings {
    /// Instance base URL, e.g. `https://your-instance.atlassian.net`.
    #[serde(default)]
    pub base_url: Option<String>,
    /// Cloud Free: account email for Basic auth with `api_token`.
    #[serde(default)]
    pub email: Option<String>,
    /// Cloud Free API token (with `email`).
    #[serde(default)]
    pub api_token: Option<String>,
    /// Raw `Authorization` header value (e.g. `Bearer …` for Server/DC). Wins over email/token.
    #[serde(default)]
    pub auth_header: Option<String>,
}

impl JiraSettings {
    /// True when enough fields are present to attempt a connection.
    pub fn is_configured(&self) -> bool {
        let base = self.base_url.as_deref().map(str::trim).unwrap_or("");
        if base.is_empty() {
            return false;
        }
        if self
            .auth_header
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_some()
        {
            return true;
        }
        let email = self.email.as_deref().map(str::trim).unwrap_or("");
        let token = self.api_token.as_deref().map(str::trim).unwrap_or("");
        !email.is_empty() && !token.is_empty()
    }
}

/// Loaded canonic project settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicConfig {
    /// Shared id / front-matter prefix (e.g. `resp` → `resp-topic-slug.md`).
    #[serde(default = "default_prefix_string")]
    pub prefix: String,
    /// Optional free Jira REST settings.
    #[serde(default)]
    pub jira: JiraSettings,
    /// Path the primary TOML was loaded from (not serialized; set after load).
    #[serde(skip)]
    pub source_path: Option<PathBuf>,
}

fn default_prefix_string() -> String {
    DEFAULT_PREFIX.to_string()
}

impl Default for CanonicConfig {
    fn default() -> Self {
        Self {
            prefix: DEFAULT_PREFIX.to_string(),
            jira: JiraSettings::default(),
            source_path: None,
        }
    }
}

impl CanonicConfig {
    /// Validate and normalize in place.
    pub fn validate(mut self) -> Result<Self> {
        self.prefix = normalize_prefix(&self.prefix)?;
        if let Some(ref mut u) = self.jira.base_url {
            *u = u.trim().trim_end_matches('/').to_string();
            if u.is_empty() {
                self.jira.base_url = None;
            }
        }
        Ok(self)
    }
}

/// ASCII letter/digit/`-` only; no leading/trailing `-`; non-empty.
pub fn normalize_prefix(raw: &str) -> Result<String> {
    let p = raw.trim();
    if p.is_empty() {
        bail!("prefix must be non-empty");
    }
    if p.starts_with('-') || p.ends_with('-') {
        bail!("prefix must not start or end with '-' (got `{p}`)");
    }
    if !p
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        bail!(
            "prefix must be lowercase ascii letters, digits, or '-' (got `{p}`)"
        );
    }
    Ok(p.to_string())
}

/// Discover `canonic.toml` walking up from `start` (or cwd).
pub fn find_config_path(start: Option<&Path>) -> Option<PathBuf> {
    let mut dir = start
        .map(Path::to_path_buf)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    loop {
        let candidate = dir.join("canonic.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

/// Optional CLI overrides merged last (highest priority after files).
#[derive(Debug, Clone, Default, Serialize)]
struct CliOverrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    prefix: Option<String>,
}

/// Load layered config: defaults → `canonic.toml` → `canonic.local.toml` → CLI.
pub fn load_config(
    explicit: Option<&Path>,
    cli_prefix: Option<&str>,
) -> Result<CanonicConfig> {
    let primary = if let Some(p) = explicit {
        if !p.is_file() {
            bail!("config file not found: {}", p.display());
        }
        Some(p.to_path_buf())
    } else {
        find_config_path(None)
    };

    let mut figment = Figment::new().merge(Serialized::defaults(CanonicConfig::default()));

    if let Some(ref path) = primary {
        figment = figment.merge(Toml::file(path));
        let local = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("canonic.local.toml");
        if local.is_file() {
            figment = figment.merge(Toml::file(&local));
        }
    }

    if let Some(p) = cli_prefix {
        figment = figment.merge(Serialized::defaults(CliOverrides {
            prefix: Some(p.to_string()),
        }));
    }

    let mut cfg: CanonicConfig = figment
        .extract()
        .with_context(|| {
            if let Some(ref p) = primary {
                format!("load canonic config from {}", p.display())
            } else {
                "load canonic config (defaults only; no canonic.toml found)".into()
            }
        })?;
    cfg.source_path = primary;
    cfg.validate()
}

/// Convenience: prefix after a full load (defaults + files + optional CLI).
pub fn resolve_prefix(
    cli_prefix: Option<&str>,
    config: &CanonicConfig,
) -> Result<String> {
    if let Some(p) = cli_prefix {
        return normalize_prefix(p);
    }
    normalize_prefix(&config.prefix)
}

/// Load a single TOML file as the sole file layer (tests / tooling).
pub fn load_config_file(path: &Path) -> Result<CanonicConfig> {
    load_config(Some(path), None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{tempdir, NamedTempFile};

    #[test]
    fn normalize_rejects_bad_prefixes() {
        assert!(normalize_prefix("").is_err());
        assert!(normalize_prefix("-x").is_err());
        assert!(normalize_prefix("X").is_err());
        assert!(normalize_prefix("has space").is_err());
        assert_eq!(normalize_prefix("resp").unwrap(), "resp");
        assert_eq!(normalize_prefix("team-a").unwrap(), "team-a");
    }

    #[test]
    fn load_toml_prefix() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "prefix = \"acme\"").unwrap();
        let cfg = load_config_file(f.path()).unwrap();
        assert_eq!(cfg.prefix, "acme");
    }

    #[test]
    fn cli_prefix_overrides_file() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "prefix = \"from-file\"").unwrap();
        let cfg = load_config(Some(f.path()), Some("cli")).unwrap();
        assert_eq!(cfg.prefix, "cli");
    }

    #[test]
    fn local_toml_overrides_primary_jira() {
        let dir = tempdir().unwrap();
        let primary = dir.path().join("canonic.toml");
        let local = dir.path().join("canonic.local.toml");
        std::fs::write(
            &primary,
            "prefix = \"resp\"\n\n[jira]\nbase_url = \"https://primary.example\"\n",
        )
        .unwrap();
        std::fs::write(
            &local,
            "[jira]\nbase_url = \"https://local.example\"\nemail = \"a@b.c\"\napi_token = \"tok\"\n",
        )
        .unwrap();
        let cfg = load_config(Some(&primary), None).unwrap();
        assert_eq!(cfg.prefix, "resp");
        assert_eq!(
            cfg.jira.base_url.as_deref(),
            Some("https://local.example")
        );
        assert!(cfg.jira.is_configured());
    }

    #[test]
    fn resolve_cli_wins() {
        let cfg = CanonicConfig {
            prefix: "from-file".into(),
            ..Default::default()
        };
        assert_eq!(resolve_prefix(Some("cli"), &cfg).unwrap(), "cli");
    }
}
