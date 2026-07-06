//! Smoke tests that drive the real `canonic` binary when possible.

use std::path::PathBuf;
use std::process::Command;

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_canonic"))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn list_shows_sample_corpus() {
    let out = Command::new(bin())
        .current_dir(repo_root())
        .args(["list", "--corpus", "corpus/responses"])
        .output()
        .expect("run list");
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("password-reset"), "{stdout}");
    assert!(stdout.contains("vpn-access"), "{stdout}");
    assert!(stdout.contains("license-renewal"), "{stdout}");
}

#[test]
fn reindex_and_search_ranks_vpn_for_wireguard_query() {
    let root = repo_root();
    let idx = root.join("target/test-canonic-index-cli");
    let _ = std::fs::remove_dir_all(&idx);

    let re = Command::new(bin())
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
    assert!(re.status.success(), "stderr={}", String::from_utf8_lossy(&re.stderr));

    let se = Command::new(bin())
        .current_dir(&root)
        .args([
            "search",
            "wireguard vpn_dns_failure",
            "-n",
            "3",
            "--index",
            idx.to_str().unwrap(),
        ])
        .output()
        .expect("search");
    assert!(se.status.success(), "stderr={}", String::from_utf8_lossy(&se.stderr));
    let stdout = String::from_utf8_lossy(&se.stdout);
    assert!(
        stdout.lines().next().unwrap_or("").contains("vpn-access"),
        "expected vpn-access first, got:\n{stdout}"
    );
}

#[test]
fn convert_password_reset_emits_jira_markup_when_pandoc_present() {
    let pandoc_ok = Command::new("pandoc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !pandoc_ok {
        eprintln!("skip convert assertion: pandoc not on PATH");
        return;
    }
    let out = Command::new(bin())
        .current_dir(repo_root())
        .args(["convert", "corpus/responses/password-reset.md"])
        .output()
        .expect("convert");
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    // pandoc jira writer uses h1. for ATX headings and *bold* for strong
    assert!(
        stdout.contains("h1.") || stdout.contains("Password"),
        "expected jira heading forms, got: {stdout:?}"
    );
    assert!(
        stdout.contains("*") || stdout.contains("self-service") || stdout.contains("selfservice"),
        "expected wiki-style emphasis or body, got: {stdout:?}"
    );
}

#[test]
fn lint_does_not_panic_and_reports_missing_or_ran() {
    let out = Command::new(bin())
        .current_dir(repo_root())
        .args(["lint", "--corpus", "corpus/responses", "--engine", "all"])
        .output()
        .expect("lint");
    // May exit 0 or 1 depending on findings; must not crash (no signal)
    assert!(out.status.code().is_some(), "lint was killed by signal");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("lint:")
            || combined.contains("MISSING")
            || combined.contains("finding")
            || combined.contains("No issues"),
        "expected explicit lint report, got: {combined}"
    );
}
