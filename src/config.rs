//! Project config: shared response id prefix and related defaults.
//!
//! Resolution order for `prefix`:
//! 1. CLI `--prefix`
//! 2. env `CANONIC_PREFIX`
//! 3. `prefix` in `canonic.toml` (or `--config` path)
//! 4. [`DEFAULT_PREFIX`] (`resp`)

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Default shared library prefix when no config/env/CLI override is set.
pub const DEFAULT_PREFIX: &str = "resp";

/// Loaded canonic project settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicConfig {
    /// Shared id / front-matter prefix (e.g. `resp` → `resp-topic-slug.md`).
    #[serde(default = "default_prefix_string")]
    pub prefix: String,
}

fn default_prefix_string() -> String {
    DEFAULT_PREFIX.to_string()
}

impl Default for CanonicConfig {
    fn default() -> Self {
        Self {
            prefix: DEFAULT_PREFIX.to_string(),
        }
    }
}

impl CanonicConfig {
    /// Validate and normalize prefix in place.
    pub fn validate(mut self) -> Result<Self> {
        self.prefix = normalize_prefix(&self.prefix)?;
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

/// Load TOML from an explicit path.
pub fn load_config_file(path: &Path) -> Result<CanonicConfig> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("read config {}", path.display()))?;
    let cfg: CanonicConfig = toml::from_str(&text)
        .with_context(|| format!("parse config {}", path.display()))?;
    cfg.validate()
}

/// Discover `canonic.toml` from `start` walking up, or cwd.
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

/// Resolve config: optional explicit file, else discover, else defaults.
pub fn load_config(explicit: Option<&Path>) -> Result<CanonicConfig> {
    if let Some(p) = explicit {
        return load_config_file(p);
    }
    if let Some(p) = find_config_path(None) {
        return load_config_file(&p);
    }
    Ok(CanonicConfig::default())
}

/// Final prefix: CLI override → env → config file → default.
pub fn resolve_prefix(
    cli_prefix: Option<&str>,
    config: &CanonicConfig,
) -> Result<String> {
    if let Some(p) = cli_prefix {
        return normalize_prefix(p);
    }
    if let Ok(env_p) = env::var("CANONIC_PREFIX") {
        if !env_p.trim().is_empty() {
            return normalize_prefix(&env_p);
        }
    }
    normalize_prefix(&config.prefix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

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
    fn resolve_cli_wins() {
        let cfg = CanonicConfig {
            prefix: "from-file".into(),
        };
        assert_eq!(resolve_prefix(Some("cli"), &cfg).unwrap(), "cli");
    }
}
