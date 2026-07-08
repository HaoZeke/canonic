//! Structural checks for Shibuya branding assets and Sphinx wiring.
//!
//! These assert the shipped docs identity files exist with non-trivial geometry
//! and that conf.py configures Shibuya the way the theme expects.

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn static_dir() -> PathBuf {
    repo_root().join("docs/source/_static")
}

fn assert_svg_nontrivial(name: &str) {
    let path = static_dir().join(name);
    let meta = fs::metadata(&path).unwrap_or_else(|e| panic!("missing {name}: {e}"));
    assert!(meta.len() > 400, "{name} too small ({} bytes)", meta.len());
    let body = fs::read_to_string(&path).unwrap();
    assert!(
        body.contains("<svg") && (body.contains("<path") || body.contains("<rect")),
        "{name} lacks SVG geometry"
    );
    assert!(
        body.to_lowercase().contains("canonic") || name == "mark.svg",
        "{name} should reference canonic identity (aria-label or wordmark)"
    );
}

#[test]
fn logo_and_favicon_svgs_are_real_artwork() {
    for name in ["logo.svg", "logo-dark.svg", "favicon.svg", "mark.svg"] {
        assert_svg_nontrivial(name);
    }
    let light = fs::read_to_string(static_dir().join("logo.svg")).unwrap();
    let dark = fs::read_to_string(static_dir().join("logo-dark.svg")).unwrap();
    // Light wordmark uses deep teal text; dark uses mint text — distinct treatments.
    assert!(
        light.contains("#115E59") && dark.contains("#CCFBF1"),
        "light/dark logos must use distinct wordmark fills for theme surfaces"
    );
    assert_ne!(light, dark, "light and dark logos must differ");
}

#[test]
fn sphinx_conf_wires_shibuya_branding() {
    let conf = fs::read_to_string(repo_root().join("docs/source/conf.py")).unwrap();
    assert!(conf.contains("html_theme = \"shibuya\""), "html_theme must be shibuya");
    assert!(conf.contains("html_favicon"), "favicon must be set");
    assert!(conf.contains("html_logo") || conf.contains("light_logo"), "logo path required");
    assert!(conf.contains("light_logo") && conf.contains("dark_logo"), "Shibuya light/dark logos");
    assert!(conf.contains("accent_color"), "accent_color for Shibuya chrome");
    assert!(
        conf.contains("HaoZeke") && conf.contains("canonic"),
        "GitHub/source context for this repo"
    );
    assert!(
        conf.contains("github_url") || conf.contains("source_repo"),
        "source/github context keys"
    );
}

#[test]
fn readme_embeds_logo_and_docs_build() {
    let readme = fs::read_to_string(repo_root().join("README.md")).unwrap();
    assert!(
        readme.contains("docs/source/_static/logo.svg"),
        "README must embed shipped logo path"
    );
    assert!(
        readme.contains("alt=\"canonic logo\"") || readme.contains("alt=\"canonic\""),
        "README logo needs canonic alt text"
    );
    assert!(
        readme.contains("./docs/build.sh") || readme.contains("docs/build.sh"),
        "README must document the real docs build command"
    );
    assert!(
        readme.to_lowercase().contains("shibuya"),
        "README should name the Shibuya docs site"
    );
}

#[test]
fn docs_requirements_and_build_entry_exist() {
    let req = fs::read_to_string(repo_root().join("docs/requirements.txt")).unwrap();
    assert!(req.contains("sphinx") && req.contains("shibuya"));
    let build = repo_root().join("docs/build.sh");
    assert!(build.is_file(), "docs/build.sh missing");
    let script = fs::read_to_string(&build).unwrap();
    assert!(script.contains("sphinx-build"));
    assert!(script.contains("shibuya") || script.contains("Shibuya"));
}
