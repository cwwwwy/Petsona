#!/usr/bin/env bash
set -euo pipefail

PETSONA_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LABEL="com.petsona.desktop"
ACTION="${1:-install}"
APP_PATH="${2:-$PETSONA_ROOT/dist/Petsona.app}"
USER_ID="$(id -u)"
PLIST_DIR="$HOME/Library/LaunchAgents"
PLIST_PATH="$PLIST_DIR/$LABEL.plist"

case "$ACTION" in
  install|uninstall) ;;
  *)
    printf 'Usage: %s [install|uninstall] [path/to/Petsona.app]\n' "$0" >&2
    exit 2
    ;;
esac

if [[ "$ACTION" == "uninstall" ]]; then
  launchctl bootout "gui/$USER_ID/$LABEL" >/dev/null 2>&1 || true
  rm -f "$PLIST_PATH"
  printf 'Removed %s\n' "$PLIST_PATH"
  exit 0
fi

case "$APP_PATH" in
  /*) ;;
  *) APP_PATH="$PETSONA_ROOT/$APP_PATH" ;;
esac
if [[ ! -d "$APP_PATH" ]]; then
  printf 'App bundle was not found: %s\n' "$APP_PATH" >&2
  exit 1
fi
APP_PATH="$(cd "$APP_PATH" && pwd)"
EXECUTABLE="$APP_PATH/Contents/MacOS/Petsona"
if [[ ! -x "$EXECUTABLE" ]]; then
  printf 'Bundle executable was not found: %s\n' "$EXECUTABLE" >&2
  exit 1
fi

mkdir -p "$PLIST_DIR"
ESCAPED_EXECUTABLE="$(printf '%s' "$EXECUTABLE" | sed 's/[&|]/\\&/g')"
ESCAPED_WORKING_DIRECTORY="$(printf '%s' "$APP_PATH/Contents/MacOS" | sed 's/[&|]/\\&/g')"
sed \
  -e "s|@EXECUTABLE@|$ESCAPED_EXECUTABLE|g" \
  -e "s|@WORKING_DIRECTORY@|$ESCAPED_WORKING_DIRECTORY|g" \
  "$PETSONA_ROOT/packaging/macos/com.petsona.desktop.plist" \
  > "$PLIST_PATH"
chmod 644 "$PLIST_PATH"
plutil -lint "$PLIST_PATH" >/dev/null
launchctl bootout "gui/$USER_ID/$LABEL" >/dev/null 2>&1 || true
launchctl bootstrap "gui/$USER_ID" "$PLIST_PATH"

printf 'Installed %s\n' "$PLIST_PATH"
printf 'Petsona will start when the Aqua user session logs in.\n'
