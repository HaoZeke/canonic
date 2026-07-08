//! Public-tree leak scan: fail if org-internal branding re-enters product paths.
//!
//! Drives a real walk of the repository (not a reimplemented product path).
//! The technical id prefix `resp-` is allowed as a **naming convention** only;
//! product marketing must not brand Jira/org hosts.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Tokens that must not appear in public product identity / samples / docs.
const DENY: &[&str] = &[
    "confluence.example.com",
    "gitlab.example.com",
    "Support Team",
    "Dutch national e-infrastructure",
    "Jira",
    "markdown corpus",
    "Jira",
    "for Support",
    "Demo/",
    "on the cluster",
    "ADVISORS.md",
];

/// Paths relative to repo root that are scanned (files only).
fn public_globs() -> Vec<&'static str> {
    vec![
        "README.md",
        "Cargo.toml",
        "CITATION.cff",
        "ADVISORS.md", // must not exist; scanned if present
        "src/main.rs",
        "src/lib.rs",
        "src/scaffold.rs",
        "src/check.rs",
        "src/tui.rs",
        "src/lint.rs",
        "docs/source/index.rst",
        "docs/source/usage.rst",
        "docs/source/design.rst",
        "docs/source/architecture.rst",
        "docs/source/api.rst",
        "docs/source/conf.py",
        "scripts/mirror-to-gitlab.sh",
        ".github/workflows/ci.yml",
        ".agents/skills/canonic-canned-loop/SKILL.md",
    ]
}

fn collect_corpus_and_scripts(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in ["corpus/responses", "scripts/jira-fixture", "scripts/jira-real"] {
        let d = root.join(dir);
        if !d.is_dir() {
            continue;
        }
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            let Ok(rd) = fs::read_dir(dir) else { return };
            for ent in rd.filter_map(|e| e.ok()) {
                let p = ent.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.is_file() {
                    out.push(p);
                }
            }
        }
        walk(&d, &mut out);
    }
    out
}

fn scan_file(path: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut hits = Vec::new();
    for token in DENY {
        if text.contains(token) {
            hits.push(format!("{}: contains {:?}", path.display(), token));
        }
    }
    // Standalone product branding: "Demo" as a word (not inside resp- ids).
    for (i, line) in text.lines().enumerate() {
        if line.contains("Demo") || line.contains("DEMO") {
            // allow mention only in comments about the prefix convention naming history? No — deny all.
            hits.push(format!("{}:{}: Demo branding", path.display(), i + 1));
        }
        // Support as org brand (not "surface")
        if line.contains("Support") {
            let lower = line.to_lowercase();
            if lower.contains("surface") || lower.contains("surfaced") {
                continue;
            }
            hits.push(format!("{}:{}: Support branding", path.display(), i + 1));
        }
    }
    hits
}

#[test]
fn public_tree_has_no_org_internal_branding() {
    let root = repo_root();
    assert!(
        !root.join("ADVISORS.md").exists(),
        "ADVISORS.md must not ship in the public tree"
    );

    let mut paths: Vec<PathBuf> = public_globs()
        .into_iter()
        .map(|r| root.join(r))
        .filter(|p| p.is_file())
        .collect();
    paths.extend(collect_corpus_and_scripts(&root));

    // Also scan unit-test fixture strings in src that ship as source (samples).
    // Do not scan this file: it contains the denylist strings by definition.
    for rel in [
        "tests/cli_smoke.rs",
        "tests/canned_loop.rs",
        "tests/jira_free_rest.rs",
        "tests/public_metadata.rs",
        "tests/branding_assets.rs",
    ] {
        let p = root.join(rel);
        if p.is_file() {
            paths.push(p);
        }
    }

    let mut all_hits = Vec::new();
    for path in &paths {
        all_hits.extend(scan_file(path));
    }

    assert!(
        all_hits.is_empty(),
        "public leak scan failed ({} hit(s)):\n{}",
        all_hits.len(),
        all_hits.join("\n")
    );
}

#[test]
fn published_corpus_is_demo_and_check_clean() {
    let root = repo_root();
    let dir = root.join("corpus/responses");
    let md: Vec<_> = fs::read_dir(&dir)
        .expect("corpus/responses")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "md").unwrap_or(false))
        .collect();
    assert!(!md.is_empty(), "need at least one published demo response for CI");

    for path in &md {
        let text = fs::read_to_string(path).unwrap();
        assert!(
            text.contains("prefix: resp"),
            "{} missing prefix",
            path.display()
        );
        assert!(
            text.contains("Support Team"),
            "{} must use generic Support Team sign-off",
            path.display()
        );
        assert!(
            !text.contains("example.com"),
            "{} must not use *.example.com URLs",
            path.display()
        );
        assert!(
            text.to_lowercase().contains("demo") || path.to_string_lossy().contains("demo"),
            "{} should be clearly demo/fictional content",
            path.display()
        );
    }

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_canonic"));
    let check = std::process::Command::new(&bin)
        .current_dir(&root)
        .args(["check", "--corpus", dir.to_str().unwrap()])
        .output()
        .expect("check");
    assert!(
        check.status.success(),
        "check: {}",
        String::from_utf8_lossy(&check.stdout)
    );
    let list = std::process::Command::new(&bin)
        .current_dir(&root)
        .args(["list", "--corpus", dir.to_str().unwrap()])
        .output()
        .expect("list");
    assert!(list.status.success());
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(
        stdout.lines().filter(|l| l.starts_with("resp-")).count() >= 1,
        "list: {stdout}"
    );
}

#[test]
fn product_identity_is_generic() {
    let readme = fs::read_to_string(repo_root().join("README.md")).unwrap();
    assert!(
        readme.to_lowercase().contains("jira") && readme.contains("canned"),
        "README must describe the generic Jira canned-response tool"
    );
    assert!(!readme.contains("Dutch national"));
    let cargo = fs::read_to_string(repo_root().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("description = "));
    assert!(!cargo.contains("Jira"));
    let about = fs::read_to_string(repo_root().join("src/main.rs")).unwrap();
    assert!(about.contains("Versioned Jira canned-response"));
    assert!(!about.contains("Jira canned"));
}
