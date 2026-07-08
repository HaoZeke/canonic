//! Corpus readiness: multiple curated resp- responses, SOP field, workflow docs.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_canonic"))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    fs::read_to_string(repo_root().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

#[test]
fn published_corpus_has_multiple_resp_responses() {
    let dir = repo_root().join("corpus/responses");
    let mut md: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "md").unwrap_or(false))
        .collect();
    md.sort();
    assert!(
        md.len() >= 2,
        "need ≥2 published resp- responses, got {}",
        md.len()
    );
    let mut saw_non_none_sop = false;
    let mut saw_none_sop = false;
    for path in &md {
        let text = fs::read_to_string(path).unwrap();
        assert!(
            text.contains("prefix: resp"),
            "{} missing prefix: resp",
            path.display()
        );
        assert!(
            text.contains("Support Team"),
            "{} missing team sign-off",
            path.display()
        );
        if text.lines().any(|l| l.starts_with("sop:") && !l.contains("none")) {
            saw_non_none_sop = true;
        }
        if text.lines().any(|l| l.trim() == "sop: none") {
            saw_none_sop = true;
        }
    }
    assert!(
        saw_non_none_sop,
        "at least one published response should set sop to a non-none value"
    );
    assert!(
        saw_none_sop,
        "at least one response may use sop: none so the convention is demonstrated"
    );

    let out = Command::new(bin())
        .current_dir(repo_root())
        .args(["list", "--corpus", dir.to_str().unwrap()])
        .output()
        .expect("list");
    assert!(out.status.success(), "list failed");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let listed = stdout.lines().filter(|l| l.starts_with("resp-")).count();
    assert!(listed >= 2, "list should show ≥2 responses: {stdout}");

    let check = Command::new(bin())
        .current_dir(repo_root())
        .args(["check", "--corpus", dir.to_str().unwrap()])
        .output()
        .expect("check");
    assert!(
        check.status.success(),
        "check must pass: {}",
        String::from_utf8_lossy(&check.stdout)
    );
}

#[test]
fn imports_gitignored_and_mirror_script_exists() {
    let gi = read(".gitignore");
    assert!(
        gi.contains("corpus/imports") || gi.contains("imports/"),
        "imports must stay out of published git history"
    );
    let mirror = repo_root().join("scripts/mirror-to-gitlab.sh");
    assert!(mirror.is_file(), "missing {}", mirror.display());
    let sh = fs::read_to_string(&mirror).unwrap();
    assert!(sh.contains("CANONIC_GITLAB_REMOTE"));

    let usage = read("docs/source/usage.rst");
    assert!(usage.contains("mirror-to-gitlab") || usage.contains("GitLab"));
    assert!(
        usage.to_lowercase().contains("bulk"),
        "workflow must forbid bulk Jira sync"
    );
    // Never ship internal colleague packets in the public tree.
    assert!(
        !repo_root().join("ADVISORS.md").exists(),
        "ADVISORS.md must not exist in the public repo"
    );
}

#[test]
fn reindex_search_finds_project_space_topic() {
    let root = repo_root();
    let idx = root.join("target/test-tmp/colleague-ready-index");
    let _ = fs::remove_dir_all(&idx);
    let reindex = Command::new(bin())
        .current_dir(&root)
        .args([
            "reindex",
            "--corpus",
            "corpus/responses",
            "--index",
            idx.to_str().unwrap(),
        ])
        .output()
        .expect("reindex");
    assert!(
        reindex.status.success(),
        "reindex: {}",
        String::from_utf8_lossy(&reindex.stderr)
    );
    let search = Command::new(bin())
        .current_dir(&root)
        .args([
            "search",
            "project space backup",
            "--index",
            idx.to_str().unwrap(),
            "-n",
            "5",
        ])
        .output()
        .expect("search");
    assert!(search.status.success());
    let stdout = String::from_utf8_lossy(&search.stdout);
    assert!(
        stdout.contains("resp-project-space") || stdout.contains("backup"),
        "expected project-space hit: {stdout}"
    );
}
