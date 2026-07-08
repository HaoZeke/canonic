//! Structural checks for public open-source distribution metadata.

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    fs::read_to_string(repo_root().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
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
    assert!(cargo.contains("rust-version = \"1.75\""));
    assert!(cargo.contains("keywords = ["));
    assert!(cargo.contains("categories = ["));
    assert!(cargo.contains("readme = \"README.md\""));
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
    // versions agree
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
    // no private builder hostnames in public docs
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
}

#[test]
fn gitignore_covers_agent_and_docs_artifacts() {
    let gi = read(".gitignore");
    assert!(gi.contains(".claude/") || gi.contains(".claude"));
    assert!(gi.contains("docs/build/") || gi.contains("docs/build"));
    assert!(gi.contains(".venv-docs") || gi.contains("venv"));
    assert!(gi.contains("/target") || gi.contains("target"));
}
