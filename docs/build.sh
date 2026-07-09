#!/usr/bin/env bash
# Build the canonic Sphinx/Shibuya site with embedded Rust API docs.
# Prose: docs/orgmode/*.org → ox-rst → docs/source/*.rst (untracked).
# Rust API: sphinxcontrib-rust → docs/source/crates/** (untracked, Shibuya HTML).
# Usage (from repo root or this directory):
#   ./docs/build.sh
# Optional: CANONIC_DOC_VENV=path  CANONIC_SKIP_ORG_EXPORT=1
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VENV="${CANONIC_DOC_VENV:-$ROOT/.venv-docs}"
REQ="$ROOT/docs/requirements.txt"
SRC="$ROOT/docs/source"
BUILD="$ROOT/docs/build"
DOCS="$ROOT/docs"

if [[ "${CANONIC_SKIP_ORG_EXPORT:-0}" != "1" ]]; then
  echo "==> 0/3 orgmode → RST (emacs ox-rst)"
  if ! command -v emacs >/dev/null 2>&1; then
    echo "error: emacs required to export docs/orgmode → docs/source/*.rst" >&2
    echo "       install emacs, or set CANONIC_SKIP_ORG_EXPORT=1 if RST already present" >&2
    exit 1
  fi
  (
    cd "$DOCS"
    emacs --batch --load export.el
  )
else
  echo "==> 0/3 skip org export (CANONIC_SKIP_ORG_EXPORT=1)"
fi

echo "==> 1/3 ensure Python doc deps in $VENV"
if [[ ! -d "$VENV" ]]; then
  python3 -m venv "$VENV"
fi
# shellcheck disable=SC1091
source "$VENV/bin/activate"
python -m pip install -q --upgrade pip
python -m pip install -q -r "$REQ"

if ! command -v sphinx-rustdocgen >/dev/null 2>&1; then
  echo "==> install sphinx-rustdocgen (needed by sphinxcontrib-rust)"
  if ! command -v cargo >/dev/null 2>&1; then
    echo "error: cargo required to install sphinx-rustdocgen" >&2
    exit 1
  fi
  cargo install sphinx-rustdocgen --locked
fi

echo "==> 2/3 sphinx-build (Shibuya + embedded Rust API) → $BUILD"
rm -rf "$BUILD"
# sphinxcontrib-rust regenerates docs/source/crates/ during this step
sphinx-build -b html -n "$SRC" "$BUILD" 2>&1

echo ""
echo "OK: open $BUILD/index.html"
echo "    Rust API: $BUILD/api.html (and $BUILD/crates/…)"
echo "    python3 -m http.server -d $BUILD 8000   # optional"
