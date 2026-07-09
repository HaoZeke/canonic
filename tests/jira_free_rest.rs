//! Integration: free-tier Jira REST against the in-repo Python fixture.
//!
//! Spawns `scripts/jira-fixture/server.py` on an ephemeral port (no Marketplace
//! apps). Drives the real `canonic` binary for probe, import, and comment write.
//! Jira credentials come from a temp `canonic.toml` (file config, not env).

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

fn health_ok(base: &str) -> bool {
    Command::new("curl")
        .args(["-sf", &format!("{base}/health")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral")
        .local_addr()
        .expect("local_addr")
        .port()
}

/// Write a temp canonic.toml with [jira] for the fixture (no env).
fn write_jira_toml(path: &Path, base_url: &str) {
    let text = format!(
        "prefix = \"resp\"\n\n[jira]\nbase_url = \"{base_url}\"\nemail = \"advisor\"\napi_token = \"canonic-test\"\n"
    );
    std::fs::write(path, text).expect("write canonic.toml");
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
fn free_rest_probe_import_and_comment_via_fixture() {
    let Some(fx) = Fixture::start() else {
        eprintln!("skip: could not start jira-fixture (python3?)");
        return;
    };
    let base = fx.base_url();
    let bin = bin();
    let tmp = repo_root().join("target/test-tmp/jira-free-rest");
    let _ = std::fs::create_dir_all(&tmp);
    let cfg = tmp.join("canonic.toml");
    write_jira_toml(&cfg, &base);
    let cfg_s = cfg.to_str().unwrap();

    // Probe
    let out = Command::new(&bin)
        .args(["--config", cfg_s, "jira-probe"])
        .output()
        .expect("run jira-probe");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "jira-probe failed: {stdout}{stderr}"
    );
    assert!(stdout.contains("free REST"), "{stdout}");
    assert!(stdout.contains("Fixture Advisor") || stdout.contains("advisor"), "{stdout}");

    // Import dry-run
    let out_dir = tmp.join("imports");
    let _ = std::fs::remove_dir_all(&out_dir);
    let out = Command::new(&bin)
        .args([
            "--config",
            cfg_s,
            "import-jira",
            "project = HSP AND labels = canned-response",
            "--out",
            out_dir.to_str().unwrap(),
            "--dry-run",
        ])
        .output()
        .expect("import dry-run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "import dry-run: {stdout}");
    assert!(stdout.contains("hsp-101") || stdout.contains("would import"), "{stdout}");
    assert!(!stdout.to_lowercase().contains("hsp-103"));

    // Real import
    let out = Command::new(&bin)
        .args([
            "--config",
            cfg_s,
            "import-jira",
            "project = HSP AND labels = canned-response",
            "--out",
            out_dir.to_str().unwrap(),
        ])
        .output()
        .expect("import");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "import: {stdout}");
    let drafts: Vec<_> = std::fs::read_dir(&out_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
        .collect();
    assert_eq!(drafts.len(), 3, "expected 3 drafts");
    let draft_101 = drafts
        .iter()
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.contains("hsp-101"))
                .unwrap_or(false)
        })
        .expect("draft for HSP-101");
    let sample = std::fs::read_to_string(&draft_101).expect("draft");
    assert!(sample.contains("prefix: resp"));
    assert!(sample.contains("imported from HSP-101"));

    // Comment write (wiki — localhost fixture)
    let md = tmp.join("resp-post.md");
    std::fs::write(
        &md,
        "---\nid: resp-post\ntitle: Post\nprefix: resp\nsop: none\n---\n\n# Post\n\nUse *self-service*.\n\nRegards,\nSupport Team\n",
    )
    .unwrap();
    let out = Command::new(&bin)
        .args([
            "--config",
            cfg_s,
            "jira-comment",
            "--issue",
            "HSP-101",
            "--body-format",
            "wiki",
            md.to_str().unwrap(),
        ])
        .output()
        .expect("jira-comment");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "jira-comment failed: {stdout}{stderr}"
    );
    assert!(stdout.contains("posted comment"), "{stdout}");
}
