#!/usr/bin/env bash
# The single macOS verification entry point.
#
#   bash scripts/verify-macos-all.sh               # Rust gates + runtime smoke + package smoke
#   bash scripts/verify-macos-all.sh --gates-only  # fmt / clippy / test / release build only
#
# The runtime smoke stays a separate file because it is long and also useful on
# its own (scripts/macos-smoke.sh); everything else runs from here.
set -euo pipefail

PETSONA_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PETSONA_GATES_ONLY=0

case "${1:-}" in
  "") ;;
  --gates-only | --fast) PETSONA_GATES_ONLY=1 ;;
  -h | --help)
    sed -n '2,8p' "$0"
    exit 0
    ;;
  *)
    printf 'Unknown option: %s\n' "$1" >&2
    printf 'Usage: %s [--gates-only]\n' "$0" >&2
    exit 2
    ;;
esac

step() {
  printf '\n=== %s ===\n' "$1"
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf 'Missing required command: %s\n' "$1" >&2
    exit 1
  fi
}

pass() {
  printf '[PASS] %s\n' "$1"
}

fail() {
  printf '[FAIL] %s\n' "$1" >&2
  exit 1
}

plist_value() {
  /usr/bin/plutil -extract "$2" raw -o - "$1" 2>/dev/null || true
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

step 'macOS toolchain'
require_command rustc
require_command cargo
require_command xcode-select
require_command xcrun

if ! xcode-select -p >/dev/null 2>&1; then
  printf 'Xcode Command Line Tools are not configured. Run: xcode-select --install\n' >&2
  exit 1
fi

sw_vers
printf 'architecture: %s\n' "$(uname -m)"
rustc --version
cargo --version
xcrun --find clang

step 'cargo fmt'
cargo fmt --all -- --check

step 'cargo clippy'
cargo clippy --workspace --all-targets --locked -- -D warnings

step 'cargo test'
cargo test --workspace --locked

step 'cargo build (release linker check)'
cargo build --workspace --release --locked

if [[ "$PETSONA_GATES_ONLY" == "1" ]]; then
  printf '\nAll macOS Rust gates passed.\n'
  printf 'Manual window checks remain: docs/MACOS_VERIFICATION.md\n'
  exit 0
fi

choose_smoke_ports
step "macOS runtime smoke (ports $PETSONA_VERIFY_STATE_PORT/$PETSONA_VERIFY_HOOK_PORT)"
PETSONA_SMOKE_STATE_PORT="$PETSONA_VERIFY_STATE_PORT" \
PETSONA_SMOKE_HOOK_PORT="$PETSONA_VERIFY_HOOK_PORT" \
bash "$PETSONA_ROOT/scripts/macos-smoke.sh"

# Bundle / zip / LaunchAgent structure. Nothing is installed here: the app
# bundle is built into a temporary directory with PETSONA_SKIP_BUILD=1 reuse of
# the release binary that was just linked above.
step 'macOS package structure smoke'
require_command ditto
require_command plutil

PETSONA_PACKAGE_TMP="$(mktemp -d "${TMPDIR:-/tmp}/petsona-package-smoke.XXXXXX")"
cleanup_package_tmp() {
  rm -rf "$PETSONA_PACKAGE_TMP"
}
trap cleanup_package_tmp EXIT INT TERM

PETSONA_SKIP_BUILD=1 "$PETSONA_ROOT/scripts/package-macos.sh" "$PETSONA_PACKAGE_TMP" >/dev/null

PETSONA_APP="$PETSONA_PACKAGE_TMP/Petsona.app"
PETSONA_ZIP="$PETSONA_PACKAGE_TMP/Petsona-macos-$(uname -m).zip"
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

PETSONA_UNPACKED="$PETSONA_PACKAGE_TMP/unpacked"
ditto -x -k --norsrc "$PETSONA_ZIP" "$PETSONA_UNPACKED"
[[ -x "$PETSONA_UNPACKED/Petsona.app/Contents/MacOS/Petsona" ]] || fail 'zip 中缺少可运行 app'
pass 'zip 可解压且包含可运行 app'

PETSONA_LAUNCH_AGENT="$PETSONA_ROOT/packaging/macos/com.petsona.desktop.plist"
plutil -lint "$PETSONA_LAUNCH_AGENT" >/dev/null || fail 'LaunchAgent plist 模板无效'
[[ "$(plist_value "$PETSONA_LAUNCH_AGENT" Label)" == 'com.petsona.desktop' ]] || fail 'LaunchAgent label 不正确'
[[ "$(plist_value "$PETSONA_LAUNCH_AGENT" RunAtLoad)" == 'true' ]] || fail 'LaunchAgent 未配置 RunAtLoad'
[[ "$(plist_value "$PETSONA_LAUNCH_AGENT" LimitLoadToSessionType)" == 'Aqua' ]] || fail 'LaunchAgent 未限制到 Aqua 会话'
pass 'LaunchAgent 模板关键字段正确（未安装）'

printf '\nAll macOS automated checks passed.\n'
printf 'Manual checks remain: docs/MACOS_VERIFICATION.md\n'