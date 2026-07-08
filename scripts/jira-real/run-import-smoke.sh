#!/usr/bin/env bash
# Spin official Atlassian Jira Software (podman/docker), classic setup with the
# public 3-hour Jira Software Data Center timebomb license from Atlassian's
# Marketplace developer docs, seed meeting-shaped issues, run canonic
# import-jira, then tear everything down.
#
# Usage (preferably on a remote builder with podman):
#   ./scripts/jira-real/run-import-smoke.sh
#
# Notes:
# - Self-hosted Jira Software is not a free product; this uses Atlassian's
#   published short-lived *developer timebomb* key for local testing only.
# - Needs several GB RAM and ~5–10 minutes for first boot + setup.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

RUNTIME=""
if command -v podman >/dev/null 2>&1; then RUNTIME=podman
elif command -v docker >/dev/null 2>&1; then RUNTIME=docker
else echo "need podman or docker" >&2; exit 1; fi

NAME=canonic-jira-real
VOL=canonic-jira-home
IMG=docker.io/atlassian/jira-software:9.12.15
PORT="${CANONIC_JIRA_PORT:-18080}"
BIN="${CANONIC_BIN:-$ROOT/target/release/canonic}"
OUT="${CANONIC_JIRA_IMPORT_DIR:-$(mktemp -d /tmp/canonic-real-jira-import.XXXXXX)}"

cleanup() {
  set +e
  "$RUNTIME" rm -f "$NAME" >/dev/null 2>&1
  "$RUNTIME" volume rm -f "$VOL" >/dev/null 2>&1
  if [[ "${CANONIC_JIRA_SMOKE_TRASH:-1}" == "1" ]]; then
    if command -v rtrash >/dev/null 2>&1; then rtrash -rf "$OUT" 2>/dev/null || rm -rf "$OUT"
    else rm -rf "$OUT"; fi
    if [[ "${CANONIC_JIRA_RMI_IMAGE:-0}" == "1" ]]; then
      "$RUNTIME" rmi "$IMG" >/dev/null 2>&1 || true
    fi
  fi
}
trap cleanup EXIT

echo "==> pull/start $IMG"
"$RUNTIME" pull "$IMG"
"$RUNTIME" rm -f "$NAME" >/dev/null 2>&1 || true
"$RUNTIME" volume rm -f "$VOL" >/dev/null 2>&1 || true
"$RUNTIME" volume create "$VOL" >/dev/null
"$RUNTIME" run -d --name "$NAME" \
  -p "127.0.0.1:${PORT}:8080" \
  -v "${VOL}:/var/atlassian/application-data/jira" \
  -e JVM_MINIMUM_MEMORY=1024m \
  -e JVM_MAXIMUM_MEMORY=4096m \
  "$IMG" >/dev/null

export JIRA_BASE_URL="http://127.0.0.1:${PORT}"
export JIRA_EMAIL=admin
export JIRA_API_TOKEN='CanonicAdmin!2026'
# bootstrap sets admin CanonicAdmin!2026 + completes classic setup
python3 "$ROOT/scripts/jira-real/bootstrap.py" "$JIRA_BASE_URL"
# seed meeting-shaped HSP issues (idempotent if already present)
python3 "$ROOT/scripts/jira-real/seed_issues.py"

if [[ ! -x "$BIN" ]]; then
  echo "==> cargo build --release"
  cargo build --release -q
  BIN="$ROOT/target/release/canonic"
fi

JQL='project = HSP AND labels = canned-response'
mkdir -p "$OUT"

echo "==> canonic dry-run"
"$BIN" import-jira "$JQL" --out "$OUT" --dry-run
echo "==> canonic import"
"$BIN" import-jira "$JQL" --out "$OUT"
count=$(find "$OUT" -maxdepth 1 -name 'resp-*.md' | wc -l)
[[ "$count" -eq 3 ]] || { echo "expected 3 drafts, got $count" >&2; exit 1; }
! find "$OUT" -name '*hsp-3*' | grep -q . || { echo "hsp-3 should be excluded" >&2; exit 1; }
grep -q 'prefix: resp' "$OUT"/resp-*-hsp-1.md
grep -qi 'Alice' "$OUT"/resp-*-hsp-1.md

echo "OK: real Atlassian Jira Software import smoke passed ($count drafts)"
echo "    OUT=$OUT (trashed on exit unless CANONIC_JIRA_SMOKE_TRASH=0)"
