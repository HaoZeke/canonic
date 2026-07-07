//! Smoke tests that drive the real `canonic` binary.

use std::path::PathBuf;
use std::process::Command;

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_canonic"))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn list_shows_resp_corpus() {
    let out = Command::new(bin())
        .current_dir(repo_root())
        .args(["list", "--corpus", "corpus/responses"])
        .output()
        .expect("run list");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("resp-project-space-not-backup"), "{stdout}");
    assert!(stdout.contains("resp-small-compute-sbu-calculation"), "{stdout}");
    assert!(!stdout.contains("password-reset"), "{stdout}");
}

#[test]
fn check_passes_on_shipped_resp_corpus() {
    let out = Command::new(bin())
        .current_dir(repo_root())
        .args(["check", "--corpus", "corpus/responses"])
        .output()
        .expect("check");
    assert!(
        out.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("All quality checks passed") || stdout.contains("0 finding"), "{stdout}");
}

#[test]
fn reindex_search_and_dedupe_roundtrip() {
    let root = repo_root();
    let idx = root.join("target/test-canonic-tantivy-cli");
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
    assert!(
        re.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&re.stderr)
    );

    let se = Command::new(bin())
        .current_dir(&root)
        .args([
            "search",
            "project space backup archive tape",
            "-n",
            "3",
            "--index",
            idx.to_str().unwrap(),
        ])
        .output()
        .expect("search");
    assert!(
        se.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&se.stderr)
    );
    let stdout = String::from_utf8_lossy(&se.stdout);
    assert!(
        stdout
            .lines()
            .next()
            .unwrap_or("")
            .contains("resp-project-space-not-backup"),
        "expected project-space first, got:\n{stdout}"
    );

    let de = Command::new(bin())
        .current_dir(&root)
        .args([
            "dedupe",
            "--corpus",
            "corpus/responses",
            "--index",
            idx.to_str().unwrap(),
            "--threshold",
            "50",
        ])
        .output()
        .expect("dedupe");
    assert!(
        de.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&de.stderr)
    );
    // Distinct topics at high threshold: expect no pairs or only empty message
    let dstdout = String::from_utf8_lossy(&de.stdout);
    assert!(
        dstdout.contains("no near-duplicate") || dstdout.contains("↔") || dstdout.starts_with("["),
        "{dstdout}"
    );
}

#[test]
fn convert_resp_sample_when_pandoc_present() {
    let pandoc_ok = Command::new("pandoc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !pandoc_ok {
        eprintln!("skip convert: pandoc missing");
        return;
    }
    let out = Command::new(bin())
        .current_dir(repo_root())
        .args([
            "convert",
            "corpus/responses/resp-project-space-not-backup.md",
        ])
        .output()
        .expect("convert");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("h1.") || stdout.contains("Project space"),
        "{stdout:?}"
    );
}

#[test]
fn doctor_reports_tools() {
    let out = Command::new(bin())
        .current_dir(repo_root())
        .args(["doctor"])
        .output()
        .expect("doctor");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("pandoc:"), "{stdout}");
    assert!(stdout.contains("harper-core:"), "{stdout}");
}
