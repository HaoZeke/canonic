#!/usr/bin/env bash
# Build the canonic Sphinx/Shibuya site.
# Usage (from repo root or this directory):
#   ./docs/build.sh
# Optional: CANONIC_DOC_VENV=path
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VENV="${CANONIC_DOC_VENV:-$ROOT/.venv-docs}"
REQ="$ROOT/docs/requirements.txt"
SRC="$ROOT/docs/source"
BUILD="$ROOT/docs/build"

echo "==> 1/2 ensure Python doc deps in $VENV"
if [[ ! -d "$VENV" ]]; then
  python3 -m venv "$VENV"
fi
# shellcheck disable=SC1091
source "$VENV/bin/activate"
python -m pip install -q --upgrade pip
python -m pip install -q -r "$REQ"

echo "==> 2/2 sphinx-build (Shibuya) → $BUILD"
rm -rf "$BUILD"
sphinx-build -b html -n "$SRC" "$BUILD" 2>&1

echo ""
echo "OK: open $BUILD/index.html"
echo "    python3 -m http.server -d $BUILD 8000   # optional"
