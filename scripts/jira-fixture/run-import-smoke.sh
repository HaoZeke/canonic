#!/usr/bin/env bash
# Spin a disposable Jira REST v2 fixture (podman/docker), run canonic import-jira,
# assert drafts, then tear everything down.
#
# Usage (repo root, preferably on a remote builder):
#   ./scripts/jira-fixture/run-import-smoke.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

RUNTIME=""
if command -v podman >/dev/null 2>&1; then
  RUNTIME=podman
elif command -v docker >/dev/null 2>&1; then
  RUNTIME=docker
else
  echo "need podman or docker" >&2
  exit 1
fi

NAME="canonic-jira-fixture-$$"
IMG="localhost/canonic-jira-fixture:smoke"
PORT="${CANONIC_JIRA_FIXTURE_PORT:-18080}"
WORKDIR="${CANONIC_JIRA_SMOKE_DIR:-$(mktemp -d /tmp/canonic-jira-smoke.XXXXXX)}"
LOG="$WORKDIR/fixture.log"
OUT="$WORKDIR/corpus/imports"
BIN="${CANONIC_BIN:-}"

cleanup() {
  set +e
  "$RUNTIME" rm -f "$NAME" >/dev/null 2>&1
  # leave WORKDIR for the caller to inspect unless CANONIC_JIRA_SMOKE_KEEP=0 and trash requested
  if [[ "${CANONIC_JIRA_SMOKE_TRASH:-1}" == "1" ]]; then
    if command -v rtrash >/dev/null 2>&1; then
      rtrash -rf "$WORKDIR" 2>/dev/null || rm -rf "$WORKDIR"
    else
      rm -rf "$WORKDIR"
    fi
  fi
}
trap cleanup EXIT

echo "==> workdir $WORKDIR"
mkdir -p "$OUT" "$WORKDIR"

echo "==> build fixture image ($RUNTIME)"
"$RUNTIME" build -t "$IMG" -f "$ROOT/scripts/jira-fixture/Dockerfile" "$ROOT/scripts/jira-fixture"

echo "==> run fixture on 127.0.0.1:$PORT"
"$RUNTIME" run -d --name "$NAME" -p "127.0.0.1:${PORT}:8080" "$IMG" >/dev/null

echo "==> wait for health"
for i in $(seq 1 40); do
  if curl -sf "http://127.0.0.1:${PORT}/health" >/dev/null; then
    echo "    ready ($i)"
    break
  fi
  if [[ "$i" -eq 40 ]]; then
    echo "fixture never became healthy" >&2
    "$RUNTIME" logs "$NAME" || true
    exit 1
  fi
  sleep 0.25
done

if [[ -z "$BIN" ]]; then
  echo "==> cargo build --release canonic"
  cargo build --release -q
  BIN="$ROOT/target/release/canonic"
fi
test -x "$BIN"

export JIRA_BASE_URL="http://127.0.0.1:${PORT}"
export JIRA_EMAIL="advisor"
export JIRA_API_TOKEN="canonic-test"
JQL='project = HSP AND labels = canned-response'

echo "==> dry-run import (no files, no comments fetch for bodies)"
DRY_OUT="$("$BIN" import-jira "$JQL" --out "$OUT" --dry-run 2>&1)"
echo "$DRY_OUT"
# draft ids embed lower-case issue keys: …-hsp-101.md
echo "$DRY_OUT" | grep -q 'hsp-101' || { echo "dry-run missing hsp-101" >&2; exit 1; }
echo "$DRY_OUT" | grep -q 'hsp-102' || { echo "dry-run missing hsp-102" >&2; exit 1; }
echo "$DRY_OUT" | grep -q 'hsp-104' || { echo "dry-run missing hsp-104" >&2; exit 1; }
# unrelated networking issue must not appear
if echo "$DRY_OUT" | grep -qi 'hsp-103'; then
  echo "dry-run incorrectly included hsp-103" >&2
  exit 1
fi
# dry-run should not write files
if compgen -G "$OUT"/*.md >/dev/null; then
  echo "dry-run wrote files unexpectedly" >&2
  exit 1
fi

echo "==> real import (Basic auth)"
"$BIN" import-jira "$JQL" --out "$OUT" --max-results 50 2>&1 | tee "$WORKDIR/import-basic.log"
mapfile -t MDS < <(find "$OUT" -maxdepth 1 -name 'resp-*.md' | sort)
echo "wrote ${#MDS[@]} drafts"
[[ "${#MDS[@]}" -eq 3 ]] || { echo "expected 3 drafts, got ${#MDS[@]}" >&2; ls -la "$OUT"; exit 1; }

for f in "${MDS[@]}"; do
  echo "--- $(basename "$f") ---"
  # must be under imports, never responses
  [[ "$f" == *"/corpus/imports/"* ]] || [[ "$f" == "$OUT"* ]]
  grep -q '^prefix: resp$' "$f"
  grep -q 'imported from HSP-' "$f"
  # wiki heading should have been converted toward markdown by pandoc
  # (h1. -> # or similar; at least not leave only raw empty file)
  test "$(wc -c < "$f")" -gt 80
done

# personal sign-off still present in draft (review-before-migrate: human must fix)
grep -q 'Alice Advisor' "$OUT"/resp-project-space-is-not-a-backup-hsp-101.md \
  || grep -qi 'alice' "$OUT"/resp-*-hsp-101.md

echo "==> Bearer PAT auth path"
rm -f "$OUT"/*.md
unset JIRA_EMAIL JIRA_API_TOKEN
export JIRA_AUTH_HEADER="Bearer pat-canonic-fixture-token"
"$BIN" import-jira "$JQL" --out "$OUT" 2>&1 | tee "$WORKDIR/import-bearer.log"
[[ "$(find "$OUT" -maxdepth 1 -name 'resp-*.md' | wc -l)" -eq 3 ]]

echo "==> convert one draft body path sanity (pandoc jira writer available)"
# create a tiny response-like md and convert to jira (publication surface direction)
SAMPLE="$WORKDIR/resp-smoke-topic.md"
cat > "$SAMPLE" <<'MD'
---
id: resp-smoke-topic
title: Smoke topic
prefix: resp
tags: [smoke]
sop: none
---

# Smoke topic

Use *self-service* for backups.

Regards,
Support Team
MD
"$BIN" convert "$SAMPLE" > "$WORKDIR/smoke.jira"
grep -qi 'self-service\|self service\|[*]self-service[*]' "$WORKDIR/smoke.jira" \
  || grep -q 'backup' "$WORKDIR/smoke.jira"

echo ""
echo "OK: import-jira exercised against disposable Jira REST fixture"
echo "    dry-run filtered labels; Basic + Bearer wrote 3 review drafts under imports/"
echo "    workdir (trashed on exit unless CANONIC_JIRA_SMOKE_TRASH=0): $WORKDIR"

# keep a copy of proof for the agent scratch if requested
if [[ -n "${CANONIC_JIRA_SMOKE_PROOF:-}" ]]; then
  mkdir -p "$(dirname "$CANONIC_JIRA_SMOKE_PROOF")"
  {
    echo "BIN=$BIN"
    echo "RUNTIME=$RUNTIME"
    echo "PORT=$PORT"
    echo "--- dry-run ---"
    echo "$DRY_OUT"
    echo "--- drafts (basic) ---"
    # re-import for proof if we wiped for bearer — list bearer drafts
    for f in "$OUT"/resp-*.md; do
      echo "FILE $f"
      head -20 "$f"
      echo
    done
    echo "--- convert ---"
    head -20 "$WORKDIR/smoke.jira"
  } > "$CANONIC_JIRA_SMOKE_PROOF"
fi

# remove image too when trashing
if [[ "${CANONIC_JIRA_SMOKE_TRASH:-1}" == "1" ]]; then
  "$RUNTIME" rmi "$IMG" >/dev/null 2>&1 || true
fi
