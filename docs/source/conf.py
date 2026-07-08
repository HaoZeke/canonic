"""Sphinx configuration for the canonic project site (Shibuya theme)."""

from __future__ import annotations

from pathlib import Path

_DOCS = Path(__file__).resolve().parent
_ROOT = _DOCS.parent.parent

project = "canonic"
copyright = "2026, Rohit Goswami"
author = "Rohit Goswami"
release = "0.1.0"
version = "0.1"

extensions = [
    "sphinx_copybutton",
    "sphinx_design",
]

templates_path = ["_templates"]
exclude_patterns: list[str] = ["crates", "rustdoc"]

html_theme = "shibuya"
html_static_path = ["_static"]
html_favicon = "_static/favicon.svg"
html_logo = "_static/logo.svg"
html_title = "canonic"
html_baseurl = "https://canonic.rgoswami.me/"
html_css_files = ["custom.css"]
# cargo doc output is copied next to Sphinx HTML by docs/build.sh
html_extra_path: list[str] = []

html_context = {
    "source_type": "github",
    "source_user": "HaoZeke",
    "source_repo": "canonic",
    "source_version": "main",
    "source_docs_path": "/docs/source/",
}

html_theme_options = {
    "accent_color": "teal",
    "light_logo": "_static/logo.svg",
    "dark_logo": "_static/logo-dark.svg",
    "github_url": "https://github.com/HaoZeke/canonic",
    "dark_code": True,
    "globaltoc_expand_depth": 1,
    "nav_links": [
        {"title": "Usage", "url": "usage"},
        {"title": "Design", "url": "design"},
        {"title": "Architecture", "url": "architecture"},
        {"title": "Rust API", "url": "api"},
    ],
}

html_sidebars = {
    "**": [
        "sidebars/localtoc.html",
        "sidebars/repo-stats.html",
        "sidebars/edit-this-page.html",
    ],
}

html_meta = {
    "description": (
        "canonic — versioned Jira canned-response corpus: "
        "markdown under resp-, pandoc convert, quality checks, Tantivy search."
    ),
}

copybutton_prompt_text = r">>> |\.\.\. |\$ |In \[\d*\]: | {2,5}\.\.\.: | {5,8}: "
copybutton_prompt_is_regexp = True
