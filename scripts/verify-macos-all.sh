#!/usr/bin/env bash
# One macOS verification entry point.
#
# The underlying checks remain separate so a failure is easy to rerun in
# isolation: Rust gates, runtime smoke, and package-structure smoke.
set -euo pipefail

PETSONA_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

step() {
  printf '\n=== %s ===\n' "$1"
}

choose_smoke_ports() {
  if [[ -n "${PETSONA_SMOKE_STATE_PORT:-}" || -n "${PETSONA_SMOKE_HOOK_PORT:-}" ]]; then
    PETSONA_VERIFY_STATE_PORT="${PETSONA_SMOKE_STATE_PORT:-17872}"
    PETSONA_VERIFY_HOOK_PORT="${PETSONA_SMOKE_HOOK_PORT:-17873}"
    return
  fi

  PETSONA_VERIFY_STATE_PORT=17872
  PETSONA_VERIFY_HOOK_PORT=17873
  if ! command -v lsof >/dev/null 2>&1; then
    return
  fi

  for _ in {1..40}; do
    if ! lsof -nP -iTCP:"$PETSONA_VERIFY_STATE_PORT" -sTCP:LISTEN -t 2>/dev/null | grep -q . \
      && ! lsof -nP -iTCP:"$PETSONA_VERIFY_HOOK_PORT" -sTCP:LISTEN -t 2>/dev/null | grep -q .; then
      return
    fi
    PETSONA_VERIFY_STATE_PORT=$((PETSONA_VERIFY_STATE_PORT + 2))
    PETSONA_VERIFY_HOOK_PORT=$((PETSONA_VERIFY_HOOK_PORT + 2))
  done
  printf 'Could not find two free smoke ports. Set PETSONA_SMOKE_STATE_PORT and PETSONA_SMOKE_HOOK_PORT.\n' >&2
  exit 1
}

cd "$PETSONA_ROOT"

step 'macOS Rust gates'
bash "$PETSONA_ROOT/scripts/verify-macos.sh"

choose_smoke_ports
step "macOS runtime smoke (ports $PETSONA_VERIFY_STATE_PORT/$PETSONA_VERIFY_HOOK_PORT)"
PETSONA_SMOKE_STATE_PORT="$PETSONA_VERIFY_STATE_PORT" \
PETSONA_SMOKE_HOOK_PORT="$PETSONA_VERIFY_HOOK_PORT" \
bash "$PETSONA_ROOT/scripts/macos-smoke.sh"

step 'macOS package structure smoke'
PETSONA_SKIP_BUILD=1 bash "$PETSONA_ROOT/scripts/macos-package-smoke.sh"

printf '\nAll macOS automated checks passed.\n'
printf 'Manual checks remain: docs/MACOS_VERIFICATION.md\n'
