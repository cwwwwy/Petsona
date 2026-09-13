#!/usr/bin/env bash
set -euo pipefail

BYTEPET_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARCH="${BYTEPET_ARCH:-$(uname -m)}"
ARCHIVE="${1:-$BYTEPET_ROOT/dist/BytePet-macos-$ARCH.zip}"
APP_PATH="${2:-$BYTEPET_ROOT/dist/BytePet.app}"
PROFILE="${NOTARYTOOL_PROFILE:-}"

case "$ARCHIVE" in
  /*) ;;
  *) ARCHIVE="$BYTEPET_ROOT/$ARCHIVE" ;;
esac
case "$APP_PATH" in
  /*) ;;
  *) APP_PATH="$BYTEPET_ROOT/$APP_PATH" ;;
esac

if [[ -z "$PROFILE" ]]; then
  printf 'Set NOTARYTOOL_PROFILE to an xcrun notarytool keychain profile.\n' >&2
  exit 2
fi
if [[ ! -f "$ARCHIVE" || ! -d "$APP_PATH" ]]; then
  printf 'Expected both archive and app bundle:\n  %s\n  %s\n' "$ARCHIVE" "$APP_PATH" >&2
  exit 1
fi

xcrun notarytool submit "$ARCHIVE" --keychain-profile "$PROFILE" --wait
xcrun stapler staple "$APP_PATH"
xcrun stapler validate "$APP_PATH"
spctl --assess --type execute --verbose=2 "$APP_PATH"

stapled_archive="${ARCHIVE%.zip}-stapled.zip"
rm -f "$stapled_archive"
ditto -c -k --norsrc --keepParent "$APP_PATH" "$stapled_archive"
printf 'Created stapled archive %s\n' "$stapled_archive"
