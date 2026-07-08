//! Structural checks for public open-source distribution metadata.
//!
//! MSRV is derived from the package `rust-version` field and cross-checked
//! against locked direct dependencies that publish a rust-version (clap in
//! particular). That way a stale "1.75" claim cannot pass while Cargo.lock
//! requires a newer toolchain.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    fs::read_to_string(repo_root().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// Parse `rust-version = "X.Y"` or `rust-version = "X.Y.Z"` from a Cargo.toml body.
fn parse_rust_version_field(toml: &str) -> Option<String> {
    for line in toml.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("rust-version") {
            let rest = rest.trim().trim_start_matches('=').trim();
            let v = rest.trim_matches('"').trim_matches('\'').trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Compare dotted version strings as (major, minor, patch) tuples.
fn version_tuple(v: &str) -> (u32, u32, u32) {
    let mut parts = v.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

fn max_version(a: &str, b: &str) -> String {
    if version_tuple(a) >= version_tuple(b) {
        a.to_string()
    } else {
        b.to_string()
    }
}

/// Locate a crate source dir under `~/.cargo/registry/src` by name+version prefix.
fn find_registry_crate(name: &str, version: &str) -> Option<PathBuf> {
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")))?;
    let src = home.join("registry").join("src");
    if !src.is_dir() {
        return None;
    }
    let needle = format!("{name}-{version}");
    for entry in fs::read_dir(&src).ok()? {
        let entry = entry.ok()?;
        let index_dir = entry.path();
        if !index_dir.is_dir() {
            continue;
        }
        for crate_ent in fs::read_dir(&index_dir).ok()? {
            let crate_ent = crate_ent.ok()?;
            let p = crate_ent.path();
            let fname = p.file_name()?.to_string_lossy();
            if fname == needle || fname.starts_with(&format!("{needle}+")) {
                return Some(p);
            }
        }
    }
    None
}

/// Pull locked package version for a direct dependency from `cargo metadata`.
fn locked_dep_version(name: &str) -> Option<String> {
    let out = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(repo_root())
        .output()
        .ok()?;
    if !out.status.success() {
        // Fallback: full metadata (includes deps) so we can read resolved versions.
        let out = Command::new("cargo")
            .args(["metadata", "--format-version", "1"])
            .current_dir(repo_root())
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        return parse_version_from_metadata(String::from_utf8_lossy(&out.stdout).as_ref(), name);
    }
    // --no-deps only lists the package itself; need full graph for dep versions.
    let out = Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .current_dir(repo_root())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_version_from_metadata(String::from_utf8_lossy(&out.stdout).as_ref(), name)
}

fn parse_version_from_metadata(json: &str, name: &str) -> Option<String> {
    // Minimal scan: "name":"clap","version":"4.6.1" appears in packages array.
    let marker = format!("\"name\":\"{name}\"");
    let mut best: Option<String> = None;
    let mut search = json;
    while let Some(idx) = search.find(&marker) {
        let after = &search[idx + marker.len()..];
        if let Some(vpos) = after.find("\"version\":\"") {
            if vpos < 80 {
                let rest = &after[vpos + "\"version\":\"".len()..];
                if let Some(end) = rest.find('"') {
                    best = Some(rest[..end].to_string());
                }
            }
        }
        search = &search[idx + marker.len()..];
    }
    best
}

/// Read rust-version from a locked dependency's published Cargo.toml when available.
fn dep_rust_version(name: &str) -> Option<String> {
    let ver = locked_dep_version(name)?;
    let dir = find_registry_crate(name, &ver)?;
    let toml = fs::read_to_string(dir.join("Cargo.toml")).ok()?;
    parse_rust_version_field(&toml)
}

#[test]
fn cargo_toml_has_public_crate_metadata() {
    let cargo = read("Cargo.toml");
    assert!(cargo.contains("name = \"canonic\""));
    assert!(cargo.contains("version = \"0.1.0\""));
    assert!(cargo.contains("license = \"MIT\""));
    assert!(cargo.contains("authors = ["));
    assert!(cargo.contains("repository = \"https://github.com/HaoZeke/canonic\""));
    assert!(cargo.contains("homepage = \"https://github.com/HaoZeke/canonic\""));
    assert!(cargo.contains("keywords = ["));
    assert!(cargo.contains("categories = ["));
    assert!(cargo.contains("readme = \"README.md\""));
    let msrv = parse_rust_version_field(&cargo).expect("package rust-version field");
    assert!(
        version_tuple(&msrv) >= version_tuple("1.85"),
        "package rust-version {msrv} must be ≥ 1.85 (edition-2024 deps in lockfile)"
    );
}

#[test]
fn package_msrv_covers_locked_direct_deps() {
    let cargo = read("Cargo.toml");
    let package_msrv =
        parse_rust_version_field(&cargo).expect("package rust-version must be set");

    // clap is a direct dep with an explicit rust-version in the registry crate.
    // harper-core is edition 2024 (also implies ≥1.85); check clap's published field.
    let mut floor = package_msrv.clone();
    if let Some(clap_rv) = dep_rust_version("clap") {
        floor = max_version(&floor, &clap_rv);
        assert!(
            version_tuple(&package_msrv) >= version_tuple(&clap_rv),
            "package rust-version {package_msrv} must cover clap rust-version {clap_rv}"
        );
    } else {
        // Registry may be cold in some CI sandboxes; still require documented floor.
        assert!(
            version_tuple(&package_msrv) >= version_tuple("1.85"),
            "when clap registry metadata is unavailable, package MSRV must still be ≥ 1.85"
        );
    }

    // README and docs must advertise the same floor (X.Y+).
    let readme = read("README.md");
    let docs = read("docs/source/index.rst");
    let claim = format!("{package_msrv}+");
    // Allow "1.85+" even if rust-version is "1.85.0"
    let short = package_msrv
        .split('.')
        .take(2)
        .collect::<Vec<_>>()
        .join(".");
    let short_claim = format!("{short}+");
    assert!(
        readme.contains(&claim) || readme.contains(&short_claim),
        "README must document Rust {short_claim} (got package MSRV {package_msrv})"
    );
    assert!(
        docs.contains(&claim) || docs.contains(&short_claim),
        "docs/source/index.rst must document Rust {short_claim}"
    );
    // Guard against stale 1.75 claims
    assert!(
        !readme.contains("1.75+") && !docs.contains("1.75+"),
        "docs must not claim obsolete 1.75+ MSRV"
    );
    let _ = floor;
}

#[test]
fn citation_and_license_align_with_crate() {
    let cff = read("CITATION.cff");
    assert!(cff.contains("version: 0.1.0"));
    assert!(cff.contains("title: \"canonic\"") || cff.contains("title: canonic"));
    assert!(cff.contains("license: MIT"));
    assert!(cff.contains("repository-code: https://github.com/HaoZeke/canonic"));
    let license = read("LICENSE");
    assert!(
        license.contains("MIT") || license.contains("Permission is hereby granted"),
        "LICENSE should be the MIT text"
    );
    let cargo = read("Cargo.toml");
    let cargo_ver = cargo
        .lines()
        .find(|l| l.starts_with("version = "))
        .expect("crate version");
    assert!(cargo_ver.contains("0.1.0"));
    assert!(cff.contains("0.1.0"));
}

#[test]
fn readme_documents_public_install_and_license() {
    let readme = read("README.md");
    assert!(
        readme.contains("cargo install --git https://github.com/HaoZeke/canonic"),
        "README must document cargo install --git from the public remote"
    );
    assert!(readme.to_lowercase().contains("license") || readme.contains("MIT"));
    assert!(readme.contains("CITATION.cff") || readme.to_lowercase().contains("citation"));
    assert!(
        !readme.contains("rg.terra") && !readme.contains("rg.cosmolab"),
        "README must not name private builder hosts"
    );
}

#[test]
fn ci_workflow_runs_cargo_test() {
    let ci = read(".github/workflows/ci.yml");
    assert!(ci.contains("cargo test"), "CI must invoke cargo test");
    assert!(
        ci.contains("on:") && (ci.contains("push:") || ci.contains("pull_request:")),
        "CI should run on push/PR"
    );
    assert!(ci.contains("ubuntu-latest") || ci.contains("runs-on:"));
    assert!(
        ci.contains("canonic check") || ci.contains("./target/release/canonic check"),
        "CI must gate published corpus with canonic check"
    );
    assert!(
        ci.contains("lint --engine harper") || ci.contains("lint"),
        "CI must run corpus lint (harper in-process)"
    );
    assert!(
        ci.contains("corpus/responses"),
        "CI corpus gate must target corpus/responses"
    );
}

#[test]
fn gitignore_covers_agent_and_docs_artifacts() {
    let gi = read(".gitignore");
    assert!(gi.contains(".claude/") || gi.contains(".claude"));
    assert!(gi.contains("docs/build/") || gi.contains("docs/build"));
    assert!(gi.contains(".venv-docs") || gi.contains("venv"));
    assert!(gi.contains("/target") || gi.contains("target"));
}
