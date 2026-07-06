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

/// Probe pandoc, vale, Harper CLI, and in-process harper-core.
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

    out
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
        let statuses = collect_statuses();
        let names: Vec<_> = statuses.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"pandoc"), "{names:?}");
        assert!(names.contains(&"vale"), "{names:?}");
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
