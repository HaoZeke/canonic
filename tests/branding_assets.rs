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

#[test]
fn landing_page_has_product_ux_structure() {
    let index = fs::read_to_string(repo_root().join("docs/source/index.rst")).unwrap();
    assert!(index.contains("cn-hero"), "landing hero class");
    assert!(index.contains("cn-hero-cta") || index.contains("cn-btn"), "hero CTAs");
    assert!(index.contains("cn-flow"), "workflow flow strip");
    assert!(index.contains("grid-item-card"), "sphinx-design cards");
    assert!(index.contains("cn-steps"), "numbered first-response steps");
    let css = fs::read_to_string(static_dir().join("custom.css")).unwrap();
    assert!(css.len() > 1500, "custom.css should be a real brand layer");
    assert!(css.contains(".cn-hero") && css.contains(".cn-flow") && css.contains(".cn-btn"));
    let usage = fs::read_to_string(repo_root().join("docs/source/usage.rst")).unwrap();
    assert!(usage.contains("cn-page-intro") && usage.contains("cn-cmd-list"));
}

#[test]
fn architecture_and_api_pages_ship_visuals_and_rustdoc_link() {
    let arch = fs::read_to_string(repo_root().join("docs/source/architecture.rst")).unwrap();
    assert!(arch.contains("architecture.svg"), "architecture page needs diagram");
    assert!(arch.contains("modules.svg"), "architecture page needs module map");
    let api = fs::read_to_string(repo_root().join("docs/source/api.rst")).unwrap();
    assert!(api.contains("rustdoc/canonic/index.html"), "API page must link shipped rustdoc");
    assert!(api.contains("cn-mod-grid") || api.contains("Module overview"));
    let arch_svg = static_dir().join("architecture.svg");
    let mod_svg = static_dir().join("modules.svg");
    assert!(arch_svg.is_file() && fs::metadata(&arch_svg).unwrap().len() > 400);
    assert!(mod_svg.is_file() && fs::metadata(&mod_svg).unwrap().len() > 400);
    let build = fs::read_to_string(repo_root().join("docs/build.sh")).unwrap();
    assert!(build.contains("cargo doc"), "docs build must generate rustdoc");
    assert!(build.contains("rustdoc"), "docs build copies rustdoc into site output");
    let conf = fs::read_to_string(repo_root().join("docs/source/conf.py")).unwrap();
    assert!(conf.contains("dark_code"), "Shibuya dark_code for code UX");
    assert!(conf.contains("architecture") && conf.contains("api"));
}
