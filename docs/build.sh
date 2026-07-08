#!/usr/bin/env bash
# Build the canonic Sphinx/Shibuya site + rustdoc API.
# Prose source: docs/orgmode/*.org → ox-rst → docs/source/*.rst (untracked).
# Usage (from repo root or this directory):
#   ./docs/build.sh
# Optional: CANONIC_DOC_VENV=path  CANONIC_SKIP_RUSTDOC=1  CANONIC_SKIP_ORG_EXPORT=1
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

echo "==> 2/3 sphinx-build (Shibuya) → $BUILD"
rm -rf "$BUILD"
sphinx-build -b html -n "$SRC" "$BUILD" 2>&1

if [[ "${CANONIC_SKIP_RUSTDOC:-0}" != "1" ]]; then
  echo "==> 3/3 cargo doc → $BUILD/rustdoc"
  if command -v cargo >/dev/null 2>&1; then
    cargo doc --no-deps --document-private-items -q
    rm -rf "$BUILD/rustdoc"
    # target/doc holds the rustdoc tree (crate + deps stubs when --no-deps)
    cp -a "$ROOT/target/doc" "$BUILD/rustdoc"
    # convenience redirect
    cat > "$BUILD/rustdoc/index.html" <<'HTML'
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8"/>
  <meta http-equiv="refresh" content="0; url=canonic/index.html"/>
  <title>canonic rustdoc</title>
  <link rel="canonical" href="canonic/index.html"/>
</head>
<body>
  <p><a href="canonic/index.html">canonic rustdoc</a></p>
</body>
</html>
HTML
    echo "    rustdoc: $BUILD/rustdoc/canonic/index.html"
  else
    echo "    skip rustdoc: cargo not on PATH"
  fi
else
  echo "==> 3/3 skip rustdoc (CANONIC_SKIP_RUSTDOC=1)"
fi

echo ""
echo "OK: open $BUILD/index.html"
echo "    rustdoc: $BUILD/rustdoc/canonic/index.html"
echo "    python3 -m http.server -d $BUILD 8000   # optional"
