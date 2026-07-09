#!/usr/bin/env bash
# Drive the One Good Tutorial path against a real canonic binary.
# Usage (repo root):
#   cargo build --release --locked
#   ./scripts/tutorial-run.sh ./target/release/canonic
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

BIN="${1:-}"
if [[ -z "$BIN" ]]; then
  if [[ -x "$ROOT/target/release/canonic" ]]; then
    BIN="$ROOT/target/release/canonic"
  elif command -v canonic >/dev/null 2>&1; then
    BIN="$(command -v canonic)"
  else
    echo "usage: $0 /path/to/canonic" >&2
    exit 2
  fi
fi
if [[ ! -x "$BIN" ]]; then
  echo "error: not executable: $BIN" >&2
  exit 2
fi

DEMO="$ROOT/corpus/responses/resp-demo-shared-quota.md"
test -f "$DEMO" || { echo "error: missing $DEMO" >&2; exit 1; }
test -f "$ROOT/canonic.toml" || { echo "error: missing canonic.toml" >&2; exit 1; }

echo "==> doctor"
"$BIN" doctor

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
"$BIN" reindex
SEARCH_OUT="$("$BIN" search "shared quota" -n 5)"
echo "$SEARCH_OUT"
echo "$SEARCH_OUT" | grep -q 'resp-demo-shared-quota' || {
  echo "error: search missed resp-demo-shared-quota" >&2
  exit 1
}

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

echo "OK: tutorial path passed with $BIN"
