//! End-to-end canned-response loop: scaffold, check, promote, convert, corpus gate.
//!
//! Drives the real `canonic` binary. Import path reuses the free REST fixture.

use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_canonic"))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run_ok(args: &[&str], envs: &[(&str, &str)]) -> String {
    let mut cmd = Command::new(bin());
    cmd.current_dir(repo_root()).args(args);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd.output().expect("run canonic");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        out.status.success(),
        "canonic {args:?} failed\nstdout={stdout}\nstderr={stderr}"
    );
    stdout
}

#[test]
fn scaffold_new_is_check_clean_via_cli() {
    let tmp = repo_root().join("target/test-tmp/canned-loop-scaffold");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let stdout = run_ok(
        &[
            "new",
            "Scratch queue policy",
            "--id",
            "resp-scratch-queue-policy",
            "--tags",
            "queue,scratch",
            "--out",
            tmp.to_str().unwrap(),
        ],
        &[],
    );
    assert!(stdout.contains("scaffolded"), "{stdout}");
    let path = tmp.join("resp-scratch-queue-policy.md");
    assert!(path.is_file(), "missing {path:?}");
    let body = fs::read_to_string(&path).unwrap();
    assert!(body.contains("prefix: resp"), "{body}");
    assert!(body.contains("id: resp-scratch-queue-policy"), "{body}");
    assert!(body.contains("Support Team"), "{body}");

    let check = run_ok(&["check", "--corpus", tmp.to_str().unwrap()], &[]);
    assert!(
        check.contains("All quality checks passed") || check.contains("0 finding"),
        "{check}"
    );
}

#[test]
fn promote_moves_check_clean_draft_into_responses_style_dir() {
    let root = repo_root().join("target/test-tmp/canned-loop-promote");
    let imports = root.join("imports");
    let responses = root.join("responses");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&imports).unwrap();
    fs::create_dir_all(&responses).unwrap();

    run_ok(
        &[
            "new",
            "Module load order",
            "--id",
            "resp-module-load-order",
            "--out",
            imports.to_str().unwrap(),
        ],
        &[],
    );
    let draft = imports.join("resp-module-load-order.md");
    // Edit draft body slightly (still check-clean).
    let mut text = fs::read_to_string(&draft).unwrap();
    text = text.replace(
        "Replace this paragraph with the shared advisor answer. Keep the team sign-off below.",
        "Load compiler modules before MPI modules when building software.",
    );
    fs::write(&draft, text).unwrap();

    let stdout = run_ok(
        &[
            "promote",
            draft.to_str().unwrap(),
            "--corpus",
            responses.to_str().unwrap(),
        ],
        &[],
    );
    assert!(stdout.contains("promoted"), "{stdout}");
    let published = responses.join("resp-module-load-order.md");
    assert!(published.is_file());
    assert!(draft.is_file(), "import draft remains for human cleanup");

    let list = run_ok(&["list", "--corpus", responses.to_str().unwrap()], &[]);
    assert!(list.contains("resp-module-load-order"), "{list}");
    let check = run_ok(&["check", "--corpus", responses.to_str().unwrap()], &[]);
    assert!(
        check.contains("All quality checks passed") || check.contains("0 finding"),
        "{check}"
    );
}

#[test]
fn published_sample_corpus_is_non_empty_and_check_clean() {
    let corpus = repo_root().join("corpus/responses");
    let list = run_ok(&["list", "--corpus", corpus.to_str().unwrap()], &[]);
    assert!(
        list.contains("resp-project-space-is-not-a-backup"),
        "seeded sample missing from list: {list}"
    );
    let check = run_ok(&["check", "--corpus", corpus.to_str().unwrap()], &[]);
    assert!(
        check.contains("All quality checks passed") || check.contains("0 finding"),
        "{check}"
    );
}

#[test]
fn convert_or_dry_run_renders_sample_when_pandoc_present() {
    let sample = repo_root().join("corpus/responses/resp-project-space-is-not-a-backup.md");
    assert!(sample.is_file());
    let doctor = Command::new(bin())
        .current_dir(repo_root())
        .args(["doctor"])
        .output()
        .expect("doctor");
    let doctor_out = format!(
        "{}{}",
        String::from_utf8_lossy(&doctor.stdout),
        String::from_utf8_lossy(&doctor.stderr)
    );
    if !doctor_out.to_lowercase().contains("pandoc")
        || doctor_out.contains("pandoc: missing")
        || doctor_out.contains("pandoc missing")
    {
        // Still assert convert fails clearly if pandoc missing — or skip.
        let out = Command::new(bin())
            .current_dir(repo_root())
            .args(["convert", sample.to_str().unwrap()])
            .output()
            .expect("convert");
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            assert!(
                err.contains("pandoc"),
                "expected pandoc missing message, got {err}"
            );
            return;
        }
    }
    let stdout = run_ok(&["convert", sample.to_str().unwrap()], &[]);
    assert!(!stdout.trim().is_empty(), "convert produced empty body");
    // jira writer typically emits wiki-ish markup without markdown # headings
    assert!(
        stdout.contains("project space")
            || stdout.contains("Project space")
            || stdout.contains("backup")
            || stdout.contains("h1.")
            || stdout.contains("*"),
        "unexpected convert output: {stdout}"
    );
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .unwrap()
        .port()
}

fn health_ok(base: &str) -> bool {
    Command::new("curl")
        .args(["-sf", &format!("{base}/health")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

struct Fixture {
    child: Child,
    port: u16,
}

impl Fixture {
    fn start() -> Option<Self> {
        let port = free_port();
        let script = repo_root().join("scripts/jira-fixture/server.py");
        if !script.is_file() {
            return None;
        }
        let child = Command::new("python3")
            .arg(&script)
            .env("CANONIC_JIRA_FIXTURE_PORT", port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let base = format!("http://127.0.0.1:{port}");
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if health_ok(&base) {
                return Some(Self { child, port });
            }
            thread::sleep(Duration::from_millis(50));
        }
        let mut c = child;
        let _ = c.kill();
        None
    }

    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn import_fixture_then_promote_edited_draft() {
    let Some(fx) = Fixture::start() else {
        eprintln!("skip: jira fixture unavailable");
        return;
    };
    let root = repo_root().join("target/test-tmp/canned-loop-import-promote");
    let imports = root.join("imports");
    let responses = root.join("responses");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&imports).unwrap();
    fs::create_dir_all(&responses).unwrap();

    let base = fx.base_url();
    let envs = [
        ("JIRA_BASE_URL", base.as_str()),
        ("JIRA_EMAIL", "advisor"),
        ("JIRA_API_TOKEN", "canonic-test"),
    ];
    // Avoid inheriting a real AUTH_HEADER from the builder host.
    let mut cmd = Command::new(bin());
    cmd.current_dir(repo_root())
        .env_remove("JIRA_AUTH_HEADER")
        .envs(envs.iter().copied())
        .args([
            "import-jira",
            "project = HSP AND labels = canned-response",
            "--out",
            imports.to_str().unwrap(),
        ]);
    let out = cmd.output().expect("import-jira");
    assert!(
        out.status.success(),
        "import failed: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // Pick one draft and rewrite it into a curated, check-clean response.
    let draft = fs::read_dir(&imports)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().map(|x| x == "md").unwrap_or(false))
        .expect("at least one import draft");
    let id = draft.file_stem().unwrap().to_string_lossy().to_string();
    // Use scaffold-shaped body with the imported id so promote keeps the id.
    let curated = format!(
        "---\nid: {id}\ntitle: Curated from fixture\nprefix: resp\ntags: [imported]\nsop: none\n---\n\n# Curated from fixture\n\nHello,\n\nThis answer was reviewed after import from the free REST fixture.\n\nRegards,\nSupport Team\n"
    );
    fs::write(&draft, curated).unwrap();

    let promote = run_ok(
        &[
            "promote",
            draft.to_str().unwrap(),
            "--corpus",
            responses.to_str().unwrap(),
        ],
        &[],
    );
    assert!(promote.contains("promoted"), "{promote}");
    let list = run_ok(&["list", "--corpus", responses.to_str().unwrap()], &[]);
    assert!(list.contains(&id), "{list}");
    let check = run_ok(&["check", "--corpus", responses.to_str().unwrap()], &[]);
    assert!(
        check.contains("All quality checks passed") || check.contains("0 finding"),
        "{check}"
    );
}

#[test]
fn skill_and_ci_document_corpus_gate() {
    let skill = repo_root().join(".agents/skills/canonic-canned-loop/SKILL.md");
    assert!(skill.is_file(), "missing skill at {}", skill.display());
    let skill_body = fs::read_to_string(&skill).unwrap();
    for needle in [
        "import-jira",
        "promote",
        "new",
        "check",
        "lint",
        "convert",
        "jira-comment",
        "no bulk",
    ] {
        assert!(
            skill_body.to_lowercase().contains(&needle.to_lowercase())
                || skill_body.contains(needle),
            "skill missing `{needle}`"
        );
    }
    // "no bulk" appears as "No bulk" / "bulk library"
    assert!(
        skill_body.to_lowercase().contains("bulk"),
        "skill should ban bulk sync"
    );

    let ci = fs::read_to_string(repo_root().join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("canonic check") || ci.contains("./target/release/canonic check"));
    assert!(ci.contains("lint --engine harper") || ci.contains("lint"));
    assert!(ci.contains("corpus/responses"));
}

/// Helper so rustc does not warn if Path import is only used in comments on some targets.
#[allow(dead_code)]
fn _touch(p: &Path) -> bool {
    p.exists()
}
