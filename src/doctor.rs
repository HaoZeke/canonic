//! Health check for external and in-process tooling.

use crate::convert;
use crate::lint::{binary_available, harper_binary_name, harper_core_available};
use serde::Serialize;

/// One line of doctor status for a tool.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ToolStatus {
    pub name: String,
    pub present: bool,
    pub detail: String,
}

impl ToolStatus {
    pub fn line(&self) -> String {
        let mark = if self.present { "ok" } else { "MISSING" };
        format!("{}: {} — {}", self.name, mark, self.detail)
    }
}

/// Probe pandoc, vale, Harper CLI, in-process harper-core, and optional free Jira.
pub fn collect_statuses() -> Vec<ToolStatus> {
    let mut out = Vec::new();

    let pandoc = convert::tool_available();
    out.push(ToolStatus {
        name: "pandoc".into(),
        present: pandoc,
        detail: if pandoc {
            convert::pandoc_version_line().unwrap_or_else(|| "present on PATH".into())
        } else {
            "not installed or not on PATH (required for `canonic convert`)".into()
        },
    });

    let vale = binary_available("vale");
    out.push(ToolStatus {
        name: "vale".into(),
        present: vale,
        detail: if vale {
            "present on PATH (used by `canonic lint --engine vale|all`)".into()
        } else {
            "not installed or not on PATH (optional; style lint skipped when missing)".into()
        },
    });

    match harper_binary_name() {
        Some(bin) => out.push(ToolStatus {
            name: "harper-cli".into(),
            present: true,
            detail: format!("{bin} present on PATH (optional CLI fallback)"),
        }),
        None => out.push(ToolStatus {
            name: "harper-cli".into(),
            present: false,
            detail: "no harper-cli/harper/harperls on PATH (optional; in-process harper-core still used)"
                .into(),
        }),
    }

    let hc = harper_core_available();
    out.push(ToolStatus {
        name: "harper-core".into(),
        present: hc,
        detail: if hc {
            format!(
                "in-process grammar engine linked (v{})",
                harper_core::core_version()
            )
        } else {
            "in-process harper-core not linked in this build".into()
        },
    });

    // Optional free-tier Jira: only when env is configured (never fails doctor critical).
    out.push(jira_env_status());

    out
}

/// Report free Jira env + quick probe when `JIRA_BASE_URL` is set.
fn jira_env_status() -> ToolStatus {
    if std::env::var_os("JIRA_BASE_URL").is_none() {
        return ToolStatus {
            name: "jira".into(),
            present: false,
            detail: "JIRA_BASE_URL unset (optional; set for jira-probe / import-jira / jira-comment)"
                .into(),
        };
    }
    match crate::jira_import::JiraConfig::from_env() {
        Ok(cfg) => match crate::jira_import::probe_jira(&cfg) {
            Ok(probe) => {
                let fmt = match probe.comment_format {
                    crate::jira_import::CommentBodyFormat::Adf => "ADF/v3",
                    crate::jira_import::CommentBodyFormat::Wiki => "wiki/v2",
                    crate::jira_import::CommentBodyFormat::Auto => "auto",
                };
                ToolStatus {
                    name: "jira".into(),
                    present: true,
                    detail: format!(
                        "ok free platform REST — {} ({fmt}; no Marketplace apps)",
                        probe.display_name
                    ),
                }
            }
            Err(e) => ToolStatus {
                name: "jira".into(),
                present: false,
                detail: format!("JIRA_BASE_URL set but probe failed: {e:#}"),
            },
        },
        Err(e) => ToolStatus {
            name: "jira".into(),
            present: false,
            detail: format!("Jira env incomplete: {e:#}"),
        },
    }
}

/// Human-readable multi-line doctor report (never empty of named tools).
pub fn format_doctor(statuses: &[ToolStatus]) -> String {
    let mut s = String::from("canonic doctor\n");
    for st in statuses {
        s.push_str(&st.line());
        s.push('\n');
    }
    s
}

/// Critical tools for the convert workflow: currently pandoc.
pub fn critical_missing(statuses: &[ToolStatus]) -> Vec<&ToolStatus> {
    statuses
        .iter()
        .filter(|s| s.name == "pandoc" && !s.present)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_names_pandoc_vale_and_harper() {
        // Isolate optional Jira probe from ambient builder env.
        let prev = std::env::var_os("JIRA_BASE_URL");
        std::env::remove_var("JIRA_BASE_URL");
        let statuses = collect_statuses();
        if let Some(v) = prev {
            std::env::set_var("JIRA_BASE_URL", v);
        }
        let names: Vec<_> = statuses.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"pandoc"), "{names:?}");
        assert!(names.contains(&"vale"), "{names:?}");
        assert!(names.contains(&"jira"), "{names:?}");
        assert!(
            names.iter().any(|n| n.contains("harper")),
            "expected harper status, got {names:?}"
        );
        let text = format_doctor(&statuses);
        assert!(text.contains("pandoc:"));
        assert!(text.contains("vale:"));
        assert!(text.contains("harper"));
        // Explicit ok or MISSING, never silent blank report
        assert!(
            text.contains("ok") || text.contains("MISSING"),
            "expected explicit status markers: {text}"
        );
    }

    #[test]
    fn format_doctor_marks_missing_tools() {
        let statuses = vec![
            ToolStatus {
                name: "pandoc".into(),
                present: false,
                detail: "not on PATH".into(),
            },
            ToolStatus {
                name: "vale".into(),
                present: true,
                detail: "present".into(),
            },
        ];
        let text = format_doctor(&statuses);
        assert!(text.contains("pandoc: MISSING"));
        assert!(text.contains("vale: ok"));
        assert_eq!(critical_missing(&statuses).len(), 1);
    }
}
