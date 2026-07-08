#!/usr/bin/env bash
# Spin the free-tier Jira REST fixture in Docker/Podman and drive canonic against it.
# Prefer a nix-built image (canonic-jira-fixture:latest) when present; else Dockerfile.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

RUNTIME=""
if command -v podman >/dev/null 2>&1; then RUNTIME=podman
elif command -v docker >/dev/null 2>&1; then RUNTIME=docker
else echo "need podman or docker" >&2; exit 1; fi

NAME=canonic-jira-fixture-ci
PORT="${CANONIC_JIRA_FIXTURE_PORT:-18081}"
BIN="${CANONIC_BIN:-$ROOT/target/release/canonic}"
IMG_LOCAL=canonic-jira-fixture:latest

cleanup() {
  set +e
  "$RUNTIME" rm -f "$NAME" >/dev/null 2>&1
}
trap cleanup EXIT

if [[ ! -x "$BIN" ]]; then
  echo "==> cargo build --release --bin canonic"
  cargo build --locked --release --bin canonic
  BIN="$ROOT/target/release/canonic"
fi

if ! "$RUNTIME" image inspect "$IMG_LOCAL" >/dev/null 2>&1; then
  if [[ -n "${CANONIC_NIX_FIXTURE_ARCHIVE:-}" && -f "${CANONIC_NIX_FIXTURE_ARCHIVE}" ]]; then
    echo "==> docker load nix fixture image"
    "$RUNTIME" load -i "$CANONIC_NIX_FIXTURE_ARCHIVE"
  else
    echo "==> build fixture image from scripts/jira-fixture/Dockerfile"
    "$RUNTIME" build -t "$IMG_LOCAL" -f "$ROOT/scripts/jira-fixture/Dockerfile" \
      "$ROOT/scripts/jira-fixture"
  fi
fi

# Resolve image name after nix load (may be canonic-jira-fixture:latest)
IMG="$IMG_LOCAL"
if ! "$RUNTIME" image inspect "$IMG" >/dev/null 2>&1; then
  # nix dockerTools often tags as canonic-jira-fixture:latest already
  IMG=$("$RUNTIME" images --format '{{.Repository}}:{{.Tag}}' | grep -E 'canonic-jira-fixture' | head -1 || true)
  [[ -n "$IMG" ]] || { echo "no fixture image" >&2; exit 1; }
fi

echo "==> run $IMG on 127.0.0.1:${PORT}"
"$RUNTIME" rm -f "$NAME" >/dev/null 2>&1 || true
"$RUNTIME" run -d --name "$NAME" \
  -p "127.0.0.1:${PORT}:8080" \
  -e CANONIC_JIRA_FIXTURE_PORT=8080 \
  "$IMG" >/dev/null

export JIRA_BASE_URL="http://127.0.0.1:${PORT}"
export JIRA_EMAIL=advisor
export JIRA_API_TOKEN=canonic-test
unset JIRA_AUTH_HEADER || true

echo "==> wait for /health"
for i in $(seq 1 60); do
  if curl -sf "$JIRA_BASE_URL/health" >/dev/null; then
    break
  fi
  sleep 0.25
  if [[ "$i" -eq 60 ]]; then
    echo "fixture never became healthy" >&2
    "$RUNTIME" logs "$NAME" || true
    exit 1
  fi
done

echo "==> jira-probe"
"$BIN" jira-probe | tee /tmp/canonic-ci-jira-probe.log
grep -qi 'free REST\|jira: ok\|Fixture Advisor\|advisor' /tmp/canonic-ci-jira-probe.log

OUT=$(mktemp -d)
echo "==> import-jira"
"$BIN" import-jira 'project = HSP AND labels = canned-response' --out "$OUT"
count=$(find "$OUT" -maxdepth 1 -name 'resp-*.md' | wc -l)
[[ "$count" -ge 1 ]] || { echo "expected imports, got $count" >&2; exit 1; }

MD="$OUT/ci-comment.md"
cat > "$MD" <<'MD'
---
id: resp-ci-comment
title: CI comment
prefix: resp
sop: none
---

# CI comment

Hello,

Docker fixture smoke body.

Regards,
Support Team
MD

echo "==> jira-comment (wiki)"
"$BIN" jira-comment --issue HSP-101 --body-format wiki "$MD" | tee /tmp/canonic-ci-jira-comment.log
grep -qi 'posted comment' /tmp/canonic-ci-jira-comment.log

echo "OK: jira docker fixture smoke passed (imports=$count)"
