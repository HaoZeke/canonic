#!/usr/bin/env python3
"""Post-process sphinx-rustdocgen RST so re-export links resolve.

sphinx-rustdocgen emits list items like::

    * :rust:any:``canonic::check::CheckReport``

(note the *double* backticks). Nested ``rust:crate`` / ``rust:module`` bodies
do not interpret that role, so HTML shows a literal ``:rust:any:`` prefix next
to a non-linked code span.

This rewrites those bullets to plain docutils hyperlinks that target the
generated module page anchors, e.g.::

    * `canonic::check::CheckReport <check.html#canonic-check-CheckReport>`_

Module overview tables are left alone: grid tables need fixed column widths,
and the Modules toctree already links every module page.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

# Generator uses double backticks (literal code); single-backtick role form is
# also accepted for robustness.
ANY_ROLE = re.compile(
    r"(?P<indent>[ \t]*)\* :rust:any:`{1,2}(?P<path>[^`]+)`{1,2}"
)


def anchor_for(path: str) -> str:
    return path.replace("::", "-")


def module_page(path: str, crate: str) -> str | None:
    """Sibling HTML page for a path like canonic::check::CheckReport → check.html"""
    parts = path.split("::")
    if len(parts) < 2 or parts[0] != crate:
        return None
    return f"{parts[1]}.html"


def fix_text(text: str, crate: str) -> str:
    def any_repl(m: re.Match[str]) -> str:
        indent = m.group("indent")
        path = m.group("path")
        page = module_page(path, crate)
        if not page:
            return m.group(0)
        anc = anchor_for(path)
        return f"{indent}* `{path} <{page}#{anc}>`_"

    return ANY_ROLE.sub(any_repl, text)


def fix_crates_tree(crates_root: Path) -> int:
    """Rewrite re-export xrefs under *crates_root*. Returns files changed."""
    if not crates_root.is_dir():
        return 0
    n = 0
    for crate_dir in sorted(p for p in crates_root.iterdir() if p.is_dir()):
        crate = crate_dir.name
        for rst in crate_dir.glob("*.rst"):
            raw = rst.read_text(encoding="utf-8")
            fixed = fix_text(raw, crate)
            if fixed != raw:
                rst.write_text(fixed, encoding="utf-8")
                n += 1
    return n


def main(crates_root: Path) -> int:
    if not crates_root.is_dir():
        print(f"skip: no crates dir at {crates_root}", file=sys.stderr)
        return 0
    n = fix_crates_tree(crates_root)
    print(f"fix_rust_xrefs: updated {n} file(s)")
    return 0


if __name__ == "__main__":
    root = (
        Path(sys.argv[1])
        if len(sys.argv) > 1
        else Path(__file__).resolve().parent / "source" / "crates"
    )
    raise SystemExit(main(root))
