#!/usr/bin/env bash
# Validate the macOS bundle and launch-agent artifacts without installing them.
set -euo pipefail

PETSONA_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PETSONA_OUTPUT="$(mktemp -d "${TMPDIR:-/tmp}/petsona-package-smoke.XXXXXX")"
PETSONA_UNPACKED="$PETSONA_OUTPUT/unpacked"

cleanup() {
  rm -rf "$PETSONA_OUTPUT"
}
trap cleanup EXIT INT TERM

fail() {
  printf '[FAIL] %s\n' "$1" >&2
  exit 1
}

pass() {
  printf '[PASS] %s\n' "$1"
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || fail "缺少命令：$1"
}

plist_value() {
  /usr/bin/plutil -extract "$2" raw -o - "$1" 2>/dev/null || true
}

require_command cargo
require_command ditto
require_command plutil
[[ "$(uname -s)" == "Darwin" ]] || fail 'macOS package smoke 只能在 macOS 上运行'

"$PETSONA_ROOT/scripts/package-macos.sh" "$PETSONA_OUTPUT" >/dev/null

PETSONA_APP="$PETSONA_OUTPUT/Petsona.app"
PETSONA_ZIP="$PETSONA_OUTPUT/Petsona-macos-$(uname -m).zip"
PETSONA_INFO="$PETSONA_APP/Contents/Info.plist"
PETSONA_EXECUTABLE="$PETSONA_APP/Contents/MacOS/Petsona"
PETSONA_ICON="$PETSONA_APP/Contents/Resources/Petsona.icns"

[[ -d "$PETSONA_APP" ]] || fail 'Petsona.app 未生成'
[[ -x "$PETSONA_EXECUTABLE" ]] || fail 'app 可执行文件缺失或不可执行'
[[ -f "$PETSONA_ICON" ]] || fail 'Petsona.icns 缺失'
[[ -f "$PETSONA_ZIP" ]] || fail 'macOS zip 未生成'
pass 'app bundle、可执行文件和图标存在'

plutil -lint "$PETSONA_INFO" >/dev/null || fail 'Info.plist 无法解析'
[[ "$(plist_value "$PETSONA_INFO" CFBundleIdentifier)" == 'com.petsona.desktop' ]] || fail 'Bundle identifier 不正确'
[[ "$(plist_value "$PETSONA_INFO" CFBundleExecutable)" == 'Petsona' ]] || fail 'Bundle executable 不正确'
[[ "$(plist_value "$PETSONA_INFO" LSUIElement)" == 'true' ]] || fail 'LSUIElement 未启用'
[[ "$(plist_value "$PETSONA_INFO" NSHighResolutionCapable)" == 'true' ]] || fail 'Retina 能力未启用'
pass 'Info.plist 关键发布字段正确'

ditto -x -k --norsrc "$PETSONA_ZIP" "$PETSONA_UNPACKED"
[[ -x "$PETSONA_UNPACKED/Petsona.app/Contents/MacOS/Petsona" ]] || fail 'zip 中缺少可运行 app'
pass 'zip 可解压且包含可运行 app'

PETSONA_LAUNCH_AGENT="$PETSONA_ROOT/packaging/macos/com.petsona.desktop.plist"
plutil -lint "$PETSONA_LAUNCH_AGENT" >/dev/null || fail 'LaunchAgent plist 模板无效'
[[ "$(plist_value "$PETSONA_LAUNCH_AGENT" Label)" == 'com.petsona.desktop' ]] || fail 'LaunchAgent label 不正确'
[[ "$(plist_value "$PETSONA_LAUNCH_AGENT" RunAtLoad)" == 'true' ]] || fail 'LaunchAgent 未配置 RunAtLoad'
[[ "$(plist_value "$PETSONA_LAUNCH_AGENT" LimitLoadToSessionType)" == 'Aqua' ]] || fail 'LaunchAgent 未限制到 Aqua 会话'
pass 'LaunchAgent 模板关键字段正确（未安装）'

printf '\nmacOS package smoke passed\n'
