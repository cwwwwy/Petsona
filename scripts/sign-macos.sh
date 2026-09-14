#!/usr/bin/env bash
set -euo pipefail

PETSONA_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_PATH="${1:-$PETSONA_ROOT/dist/Petsona.app}"
IDENTITY="${CODESIGN_IDENTITY:-}"
ARCH="${PETSONA_ARCH:-$(uname -m)}"
ARCHIVE="${2:-${PETSONA_ARCHIVE:-$PETSONA_ROOT/dist/Petsona-macos-$ARCH.zip}}"

case "$APP_PATH" in
  /*) ;;
  *) APP_PATH="$PETSONA_ROOT/$APP_PATH" ;;
esac
case "$ARCHIVE" in
  /*) ;;
  *) ARCHIVE="$PETSONA_ROOT/$ARCHIVE" ;;
esac

if [[ -z "$IDENTITY" ]]; then
  printf 'Set CODESIGN_IDENTITY to a Developer ID Application identity.\n' >&2
  printf 'Example: CODESIGN_IDENTITY="Developer ID Application: Example" %s\n' "$0" >&2
  exit 2
fi
if [[ ! -d "$APP_PATH" ]]; then
  printf 'App bundle was not found: %s\n' "$APP_PATH" >&2
  exit 1
fi

codesign_args=(--force --options runtime)
if [[ "$IDENTITY" != "-" ]]; then
  codesign_args+=(--timestamp)
fi
codesign "${codesign_args[@]}" --sign "$IDENTITY" "$APP_PATH"
codesign --verify --deep --strict --verbose=2 "$APP_PATH"
printf 'Signed and verified %s\n' "$APP_PATH"

if [[ -f "$ARCHIVE" ]]; then
  rm -f "$ARCHIVE"
  ditto -c -k --norsrc --keepParent "$APP_PATH" "$ARCHIVE"
  printf 'Repacked signed archive %s\n' "$ARCHIVE"
fi
