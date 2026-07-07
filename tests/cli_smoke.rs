//! Smoke tests that drive the real `canonic` binary against synthetic fixtures.
//!
//! Fixtures are generated per test rather than read from `corpus/responses/`,
//! so these tests stay green regardless of what the team has actually published.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_canonic"))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Write a minimal well-formed `resp-` response fixture and return its path.
fn write_response(dir: &Path, slug: &str, title: &str, body: &str) -> PathBuf {
    let id = format!("resp-{slug}");
    let path = dir.join(format!("{id}.md"));
    fs::write(
        &path,
        format!(
            "---\nid: {id}\ntitle: {title}\nprefix: resp\nsop: none\n---\n\n# {title}\n\n{body}\n\nRegards,\nSupport Team\n"
        ),
    )
    .unwrap();
    path
}

#[test]
fn list_shows_corpus_entries() {
    let dir = tempfile::tempdir().unwrap();
    write_response(dir.path(), "alpha-topic", "Alpha topic", "Alpha body text.");
    write_response(dir.path(), "beta-topic", "Beta topic", "Beta body text.");

    let out = Command::new(bin())
        .current_dir(repo_root())
        .args(["list", "--corpus", dir.path().to_str().unwrap()])
        .output()
        .expect("run list");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("resp-alpha-topic"), "{stdout}");
    assert!(stdout.contains("resp-beta-topic"), "{stdout}");
}

#[test]
fn check_passes_on_well_formed_corpus() {
    let dir = tempfile::tempdir().unwrap();
    write_response(dir.path(), "alpha-topic", "Alpha topic", "Alpha body text.");

    let out = Command::new(bin())
        .current_dir(repo_root())
        .args(["check", "--corpus", dir.path().to_str().unwrap()])
        .output()
        .expect("check");
    assert!(
        out.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("All quality checks passed") || stdout.contains("0 finding"),
        "{stdout}"
    );
}

#[test]
fn check_fails_on_missing_prefix_and_sop() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("password-reset.md"),
        "---\nid: password-reset\n---\n\nBody\n",
    )
    .unwrap();

    let out = Command::new(bin())
        .current_dir(repo_root())
        .args(["check", "--corpus", dir.path().to_str().unwrap()])
        .output()
        .expect("check");
    assert!(!out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("id-prefix"), "{stdout}");
    assert!(stdout.contains("sop-field"), "{stdout}");
}

#[test]
fn reindex_search_and_dedupe_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    write_response(
        dir.path(),
        "project-space-not-backup",
        "Project space is not a backup",
        "Project space is not a backup or archive. Use tape for long-term retention.",
    );
    write_response(
        dir.path(),
        "small-compute-sbu",
        "SBU calculation",
        "Small compute needs an SBU calculation for GPU and CPU hours.",
    );
    let idx = dir.path().join("index");

    let re = Command::new(bin())
        .current_dir(repo_root())
        .args([
            "reindex",
            "--corpus",
            dir.path().to_str().unwrap(),
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
        .current_dir(repo_root())
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
        .current_dir(repo_root())
        .args([
            "dedupe",
            "--corpus",
            dir.path().to_str().unwrap(),
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
    let dstdout = String::from_utf8_lossy(&de.stdout);
    assert!(
        dstdout.contains("no near-duplicate") || dstdout.contains("↔") || dstdout.starts_with("["),
        "{dstdout}"
    );
}

#[test]
fn convert_sample_when_pandoc_present() {
    let pandoc_ok = Command::new("pandoc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !pandoc_ok {
        eprintln!("skip convert: pandoc missing");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let path = write_response(
        dir.path(),
        "project-space-not-backup",
        "Project space is not a backup",
        "Project space is **not** a backup or archive.",
    );

    let out = Command::new(bin())
        .current_dir(repo_root())
        .args(["convert", path.to_str().unwrap()])
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
