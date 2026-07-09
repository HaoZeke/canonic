#!/usr/bin/env bash
# Drive the One Good Tutorial path against a real canonic binary.
#
# Usage (repo root):
#   cargo build --release --locked
#   ./scripts/tutorial-run.sh ./target/release/canonic
#   ./scripts/tutorial-run.sh --capture ./target/release/canonic
#     → writes docs/source/_generated/tutorial-session.txt for Sphinx
#       literalinclude (measured CLI output, not hand-typed).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

CAPTURE=0
BIN=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --capture) CAPTURE=1; shift ;;
    -h|--help)
      sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      BIN="$1"
      shift
      ;;
  esac
done

if [[ -z "$BIN" ]]; then
  if [[ -x "$ROOT/target/release/canonic" ]]; then
    BIN="$ROOT/target/release/canonic"
  elif command -v canonic >/dev/null 2>&1; then
    BIN="$(command -v canonic)"
  else
    echo "usage: $0 [--capture] /path/to/canonic" >&2
    exit 2
  fi
fi
if [[ ! -x "$BIN" ]]; then
  echo "error: not executable: $BIN" >&2
  exit 2
fi

DEMO_REL="corpus/responses/resp-demo-shared-quota.md"
DEMO="$ROOT/$DEMO_REL"
test -f "$DEMO" || { echo "error: missing $DEMO" >&2; exit 1; }
test -f "$ROOT/canonic.toml" || { echo "error: missing canonic.toml" >&2; exit 1; }

GEN_DIR="$ROOT/docs/source/_generated"
SESSION="$GEN_DIR/tutorial-session.txt"

# Stabilize volatile fields so committed captures stay reviewable.
stabilize() {
  # score floats, absolute index paths, host-specific doctor lines we omit
  sed -E \
    -e 's/score=[0-9]+(\.[0-9]+)?/score=…/g' \
    -e 's#into .canonic-index/?#into .canonic-index/#g' \
    -e 's#/home/[^/]+/[^[:space:]]+canonic/#./#g'
}

echo "==> doctor"
"$BIN" doctor >/dev/null || true

echo "==> list"
LIST_OUT="$("$BIN" list)"
echo "$LIST_OUT"
echo "$LIST_OUT" | grep -q 'resp-demo-shared-quota' || {
  echo "error: list missing resp-demo-shared-quota" >&2
  exit 1
}

echo "==> check"
CHECK_OUT="$("$BIN" check)"
echo "$CHECK_OUT"
echo "$CHECK_OUT" | grep -q '0 finding' || {
  echo "error: check not clean" >&2
  exit 1
}

echo "==> reindex + search"
# Per-run index dir (mktemp) so parallel cargo tests never share a Tantivy lock
# or delete each other's writer files via EXIT traps.
if [[ -n "${CANONIC_TUTORIAL_INDEX:-}" ]]; then
  INDEX_DIR="$CANONIC_TUTORIAL_INDEX"
  mkdir -p "$INDEX_DIR"
  CLEAN_INDEX=0
else
  INDEX_DIR="$(mktemp -d "${TMPDIR:-/tmp}/canonic-tutorial-index.XXXXXX")"
  CLEAN_INDEX=1
fi
cleanup_index() {
  if [[ "$CLEAN_INDEX" -eq 1 && -d "$INDEX_DIR" ]]; then
    rm -rf "$INDEX_DIR"
  fi
}
trap cleanup_index EXIT
REINDEX_OUT="$("$BIN" reindex --index "$INDEX_DIR")"
echo "$REINDEX_OUT"
# Normalize printed path for committed captures
REINDEX_OUT_STABLE="$(echo "$REINDEX_OUT" | sed "s|$INDEX_DIR|.canonic-index|g")"
SEARCH_OUT="$("$BIN" search "shared quota" -n 3 --index "$INDEX_DIR")"
echo "$SEARCH_OUT"
echo "$SEARCH_OUT" | grep -q 'resp-demo-shared-quota' || {
  echo "error: search missed resp-demo-shared-quota" >&2
  exit 1
}

CONV_OUT=""
if command -v pandoc >/dev/null 2>&1; then
  echo "==> convert (pandoc present)"
  CONV_OUT="$("$BIN" convert "$DEMO")"
  echo "$CONV_OUT" | head -n 12
  test -n "$(echo "$CONV_OUT" | tr -d '[:space:]')" || {
    echo "error: convert empty" >&2
    exit 1
  }
  echo "$CONV_OUT" | grep -qi 'Support Team\|shared\|quota\|Demo' || {
    echo "error: convert body missing expected demo content" >&2
    exit 1
  }
else
  echo "==> convert skipped (pandoc not on PATH)"
fi

if [[ "$CAPTURE" -eq 1 ]]; then
  mkdir -p "$GEN_DIR"
  {
    echo "\$ canonic list"
    echo "$LIST_OUT"
    echo
    echo "\$ canonic check"
    echo "$CHECK_OUT"
    echo
    echo "\$ canonic reindex && canonic search \"shared quota\" -n 3"
    echo "$REINDEX_OUT_STABLE" | stabilize
    echo "$SEARCH_OUT" | stabilize
    if [[ -n "$CONV_OUT" ]]; then
      echo
      echo "\$ canonic convert $DEMO_REL"
      # Keep convert short and stable for the page
      echo "$CONV_OUT" | head -n 16
      if [[ "$(echo "$CONV_OUT" | wc -l)" -gt 16 ]]; then
        echo "…"
      fi
    else
      echo
      echo "# convert skipped: pandoc not on PATH when this session was captured"
    fi
  } | stabilize >"$SESSION"
  echo "wrote $SESSION"
fi

echo "OK: tutorial path passed with $BIN"
