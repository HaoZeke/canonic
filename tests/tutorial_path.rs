//! Tutorial page structure + runnable path against the real canonic binary.
//!
//! The One Good Tutorial must stay wired in the org docs tree and executable
//! via `scripts/tutorial-run.sh` (list/check/reindex/search; convert if pandoc).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_canonic"))
}

#[test]
fn tutorial_org_is_one_good_path() {
    let root = repo_root();
    let org = fs::read_to_string(root.join("docs/orgmode/tutorial.org")).expect("tutorial.org");
    assert!(
        org.contains("end result") || org.contains("literalinclude"),
        "tutorial must show the outcome early"
    );
    assert!(
        org.contains("_generated/tutorial-session.txt")
            || org.contains("literalinclude:: _generated/tutorial-session.txt"),
        "tutorial end-state must literalinclude measured session capture"
    );
    for cmd in [
        "canonic list",
        "canonic check",
        "canonic reindex",
        "canonic search",
        "canonic convert",
        "canonic doctor",
    ] {
        assert!(
            org.contains(cmd) || org.contains("--capture"),
            "tutorial missing command: {cmd}"
        );
    }
    assert!(
        org.contains("resp-demo-shared-quota"),
        "tutorial must use shipped demo path"
    );
    assert!(
        org.contains(":doc:`usage`") || org.contains("usage"),
        "tutorial must link out to usage/reference"
    );
    assert!(
        !org.to_lowercase().contains("learning objective"),
        "no learning-objectives framing"
    );
    assert!(
        !org.to_lowercase().contains("exercise"),
        "no exercise blocks"
    );
    // no hard-coded cluster short form
    let banned = ["s", "nell"].concat();
    assert!(
        !org.to_lowercase().contains(&banned),
        "tutorial must not hard-code the old short-form prefix"
    );
    // RST titles: no &amp; in grid-style titles if any
    assert!(!org.contains("grid-item-card::") || !org.contains("&amp;"));

    let index = fs::read_to_string(root.join("docs/orgmode/index.org")).unwrap();
    assert!(
        index.contains("tutorial") && index.contains(":link: tutorial"),
        "index must link the tutorial in the docs map"
    );
    assert!(
        index.contains("   tutorial\n") || index.contains("\n   tutorial\n"),
        "toctree must include tutorial"
    );

    let conf = fs::read_to_string(root.join("docs/source/conf.py")).unwrap();
    assert!(
        conf.contains("\"tutorial\"") || conf.contains("'tutorial'"),
        "Shibuya nav must include tutorial"
    );

    let script = root.join("scripts/tutorial-run.sh");
    assert!(script.is_file(), "missing scripts/tutorial-run.sh");
    let body = fs::read_to_string(&script).unwrap();
    assert!(body.contains("canonic") || body.contains("$BIN"));
    assert!(body.contains("resp-demo-shared-quota"));
    assert!(body.contains("reindex") && body.contains("search") && body.contains("check"));
}

#[test]
fn tutorial_capture_writes_measured_session() {
    let root = repo_root();
    let script = root.join("scripts/tutorial-run.sh");
    let bin = bin();
    let out = Command::new(&script)
        .args(["--capture", bin.to_str().unwrap()])
        .current_dir(&root)
        .output()
        .expect("tutorial-run --capture");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "capture failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let session = root.join("docs/source/_generated/tutorial-session.txt");
    assert!(session.is_file(), "missing {session:?}");
    let body = fs::read_to_string(&session).unwrap();
    assert!(body.contains("$ canonic list"), "session missing list header: {body}");
    assert!(
        body.contains("resp-demo-shared-quota"),
        "session missing demo id: {body}"
    );
    assert!(body.contains("$ canonic check"), "session missing check");
    assert!(body.contains("0 finding"), "session check not clean: {body}");
    assert!(
        body.contains("shared quota") || body.contains("resp-demo-shared-quota"),
        "session missing search/demo content"
    );
    // committed session must stay present for Sphinx without a binary
    let org = fs::read_to_string(root.join("docs/orgmode/tutorial.org")).unwrap();
    assert!(org.contains("_generated/tutorial-session.txt"));
}

#[test]
fn tutorial_run_script_drives_real_binary() {

    let root = repo_root();
    let script = root.join("scripts/tutorial-run.sh");
    let bin = bin();
    assert!(bin.is_file(), "missing canonic test binary");

    let out = Command::new(&script)
        .arg(&bin)
        .current_dir(&root)
        .output()
        .expect("run tutorial-run.sh");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "tutorial-run.sh failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("resp-demo-shared-quota"),
        "expected demo id in tutorial output: {stdout}"
    );
    assert!(
        stdout.contains("OK: tutorial path passed") || stdout.contains("0 finding"),
        "expected success marker: {stdout}"
    );
}
