#!/usr/bin/env python3
"""Extract #+begin_export rst blocks from docs/orgmode into docs/source.

canonic prose is authored as full RST export blocks inside org files (same
as the hand-written RST era). This avoids relying on ox-rst when Emacs/Org
on CI is too old or MELPA is flaky. Prefer `emacs --batch --load export.el`
when available; build.sh falls back here on export failure.
"""
from __future__ import annotations

import re
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
ORG = ROOT / "orgmode"
SRC = ROOT / "source"

BLOCK = re.compile(
    r"^[ \t]*#\+begin_export[ \t]+rst[ \t]*\n(.*?)^[ \t]*#\+end_export[ \t]*$",
    re.M | re.S | re.I,
)


def main() -> int:
    if not ORG.is_dir():
        print(f"error: missing {ORG}", file=sys.stderr)
        return 1
    SRC.mkdir(parents=True, exist_ok=True)
    n = 0
    for org in sorted(ORG.rglob("*.org")):
        text = org.read_text(encoding="utf-8")
        blocks = [m.group(1).strip("\n") for m in BLOCK.finditer(text)]
        if not blocks:
            continue
        rel = org.relative_to(ORG).with_suffix(".rst")
        out = SRC / rel
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text("\n\n".join(blocks).rstrip() + "\n", encoding="utf-8")
        n += 1
        print(f"  wrote {out.relative_to(ROOT)}")
    # copy image attachments next to org sources
    for img in ORG.rglob("*"):
        if img.suffix.lower() in {".svg", ".png", ".jpg", ".jpeg", ".gif"}:
            rel = img.relative_to(ORG)
            dest = SRC / rel
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(img, dest)
    print(f"mkrst: {n} org file(s) → RST")
    return 0 if n else 1


if __name__ == "__main__":
    raise SystemExit(main())
