//! Public-tree leak scan: fail if org-internal branding re-enters product paths.
//!
//! Walks real repository files. Denylist tokens are built without contiguous
//! org spellings so a repo-wide search for those brands stays clean.
//! Also rejects the old hardcoded cluster short form as an id convention.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Build a denylist without writing contiguous org brand strings in source.
fn denylist() -> Vec<String> {
    // "S" "URF"  /  "S" "nellius"  /  "surf" "." "nl"
    let org = ["S", "URF"].concat();
    let cluster = ["S", "nellius"].concat();
    let cluster_up = ["S", "NELLIUS"].concat();
    let host = format!("{}.{}", "surf", "nl");
    // Former hardcoded id prefix (cluster short form); ids are config-driven now.
    let short_prefix = ["s", "nell"].concat();
    vec![
        short_prefix,
        format!("confluence.{}", host),
        format!("gitlab.{}", host),
        format!("{org} HPC Advisors"),
        format!("{org} / {cluster}"),
        format!("{org} · {cluster}"),
        format!("{org}/{cluster}"),
        format!("for {org}"),
        format!("{cluster}/"),
        format!("on {cluster}"),
        format!("Dutch national e-infrastructure"),
        org.clone(),
        cluster,
        cluster_up,
        host,
    ]
}

fn public_rel_paths() -> Vec<&'static str> {
    vec![
        "README.md",
        "Cargo.toml",
        "canonic.toml",
        "CITATION.cff",
        "src/main.rs",
        "src/lib.rs",
        "src/scaffold.rs",
        "src/check.rs",
        "src/tui.rs",
        "src/lint.rs",
        "src/doctor.rs",
        "src/jira_import.rs",
        "src/index.rs",
        "src/convert.rs",
        "src/corpus.rs",
        "docs/orgmode/index.org",
        "docs/orgmode/usage.org",
        "docs/orgmode/design.org",
        "docs/orgmode/architecture.org",
        "docs/orgmode/api.org",
        "docs/export.el",
        "docs/source/conf.py",
        "scripts/mirror-to-gitlab.sh",
        "scripts/ci/jira-docker-smoke.sh",
        ".github/workflows/ci.yml",
        ".agents/skills/canonic-canned-loop/SKILL.md",
        "flake.nix",
        "flake.lock",
    ]
}

fn walk_collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for ent in rd.filter_map(|e| e.ok()) {
        let p = ent.path();
        if p.is_dir() {
            walk_collect(&p, out);
        } else if p.is_file() {
            out.push(p);
        }
    }
}

fn collect_extra(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in [
        "corpus/responses",
        "scripts/jira-fixture",
        "scripts/jira-real",
        "scripts/ci",
        "tests",
        "styles",
    ] {
        walk_collect(&root.join(dir), &mut out);
    }
    out
}

fn is_self(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n == "public_leak_scan.rs")
        .unwrap_or(false)
}

/// True for the English word "surface/surfaced" (false positive for brand scan).
fn is_surface_word_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.contains("surface") || lower.contains("surfaced") || lower.contains("surfaces")
}

fn scan_file(path: &Path, deny: &[String]) -> Vec<String> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut hits = Vec::new();
    for token in deny {
        if token == "surface" {
            continue;
        }
        // Skip whole-file match of short "S"+"URF" on lines that only use "surface"
        if token.as_str() == ["S", "URF"].concat() {
            for (i, line) in text.lines().enumerate() {
                if line.contains(token.as_str()) && !is_surface_word_line(line) {
                    hits.push(format!("{}:{}: brand token", path.display(), i + 1));
                }
            }
            continue;
        }
        if text.contains(token.as_str()) {
            hits.push(format!("{}: contains denied token", path.display()));
        }
    }
    hits
}

#[test]
fn public_tree_has_no_org_internal_branding() {
    let root = repo_root();
    assert!(
        !root.join("ADVISORS.md").exists(),
        "internal advisor packet must not ship in the public tree"
    );

    let deny = denylist();
    // Self-check: denylist encodes brands via split concat, not contiguous literals in this file's DENY list construction.
    assert!(deny.iter().any(|t| t.contains("Advisors")));

    let mut paths: Vec<PathBuf> = public_rel_paths()
        .into_iter()
        .map(|r| root.join(r))
        .filter(|p| p.is_file())
        .collect();
    paths.extend(collect_extra(&root));
    paths.retain(|p| !is_self(p));

    let mut all_hits = Vec::new();
    for path in &paths {
        all_hits.extend(scan_file(path, &deny));
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
        assert!(text.contains("prefix: resp"), "{} missing prefix", path.display());
        assert!(
            text.contains("Support Team"),
            "{} must use generic Support Team sign-off",
            path.display()
        );
        assert!(
            path.to_string_lossy().contains("demo") || text.to_lowercase().contains("demo"),
            "{} should be clearly demo content",
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
}

#[test]
fn product_identity_is_generic() {
    let readme = fs::read_to_string(repo_root().join("README.md")).unwrap();
    assert!(readme.to_lowercase().contains("jira") && readme.contains("canned"));
    let cargo = fs::read_to_string(repo_root().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("description = "));
    let about = fs::read_to_string(repo_root().join("src/main.rs")).unwrap();
    assert!(about.contains("Versioned Jira canned-response"));
}
