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

default_petsona_data_fingerprint() {
  local data_dir="$HOME/Library/Application Support/Petsona"
  local relative path
  for relative in config.json logs/petsona.log petsona.lock; do
    path="$data_dir/$relative"
    if [[ -e "$path" ]]; then
      printf '%s:%s\n' "$relative" "$(/usr/bin/stat -f '%d:%i:%m:%z' "$path")"
    else
      printf '%s:missing\n' "$relative"
    fi
  done
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
require_command xcodebuild

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

step 'macOS native frontend build'
PETSONA_NATIVE_DERIVED_DATA="$PETSONA_ROOT/.scratch/macos-native-gates"
mkdir -p "$PETSONA_NATIVE_DERIVED_DATA"
PETSONA_NATIVE_ARCH="${PETSONA_NATIVE_ARCH:-$(uname -m)}"
xcodebuild \
  -project "$PETSONA_ROOT/apps/macos/Petsona.xcodeproj" \
  -scheme Petsona \
  -configuration Release \
  -arch "$PETSONA_NATIVE_ARCH" \
  -derivedDataPath "$PETSONA_NATIVE_DERIVED_DATA" \
  CODE_SIGNING_ALLOWED=NO \
  build

step 'macOS native XCTest'
PETSONA_NATIVE_TEST_DATA="$PETSONA_ROOT/.scratch/macos-native-tests"
rm -rf "$PETSONA_NATIVE_TEST_DATA"
PETSONA_DEFAULT_DATA_BEFORE="$(default_petsona_data_fingerprint)"
set +e
xcodebuild \
  -quiet \
  -project "$PETSONA_ROOT/apps/macos/Petsona.xcodeproj" \
  -scheme Petsona \
  -configuration Debug \
  -destination "platform=macOS,arch=$PETSONA_NATIVE_ARCH" \
  -derivedDataPath "$PETSONA_NATIVE_TEST_DATA" \
  -only-testing:PetsonaTests/EngineClientTests \
  -only-testing:PetsonaTests/AbiTests \
  -only-testing:PetsonaTests/GazeStabilizerTests \
  -only-testing:PetsonaTests/PetResourceTests \
  -only-testing:PetsonaTests/NativeLifecycleTests \
  -only-testing:PetsonaTests/SystemServiceTests \
  CODE_SIGNING_ALLOWED=NO \
  test
PETSONA_XCTEST_EXIT=$?
set -e
PETSONA_DEFAULT_DATA_AFTER="$(default_petsona_data_fingerprint)"
[[ "$PETSONA_DEFAULT_DATA_BEFORE" == "$PETSONA_DEFAULT_DATA_AFTER" ]] || \
  fail 'XCTest changed the real user Petsona config/log/lock; host isolation regression'
pass 'XCTest kept real user Petsona config/log/lock unchanged'
PETSONA_TEST_HOST_HOME="$PETSONA_NATIVE_TEST_DATA/host-home"
PETSONA_TEST_HOST_MARKER="$PETSONA_NATIVE_TEST_DATA/host-started"
[[ -f "$PETSONA_TEST_HOST_MARKER" ]] || fail 'XCTest app host did not report isolated bootstrap'
grep -Fqx "$PETSONA_TEST_HOST_HOME" "$PETSONA_TEST_HOST_MARKER" || \
  fail 'XCTest app host bootstrap used an unexpected data directory'
pass 'XCTest app host booted from its isolated home with state protocol disabled'
[[ "$PETSONA_XCTEST_EXIT" == '0' ]] || exit "$PETSONA_XCTEST_EXIT"
PETSONA_TEST_RESULT="$(find "$PETSONA_NATIVE_TEST_DATA/Logs/Test" -maxdepth 1 -type d -name '*.xcresult' -print | sort | tail -1)"
[[ -n "$PETSONA_TEST_RESULT" ]] || fail '未找到原生 XCTest xcresult'
if command -v xcrun >/dev/null 2>&1; then
  xcrun xcresulttool get test-results summary --path "$PETSONA_TEST_RESULT" > "$PETSONA_NATIVE_TEST_DATA/test-summary.json"
  grep -q '"result" : "Passed"' "$PETSONA_NATIVE_TEST_DATA/test-summary.json" || fail '原生 XCTest 结果不是 Passed'
  pass '原生 XCTest 14 项通过（摘要已保存）'
fi

if [[ "$PETSONA_GATES_ONLY" == "1" ]]; then
  printf '\nAll macOS Rust and native frontend gates passed.\n'
  printf 'Manual window checks remain: docs/MACOS_VERIFICATION.md\n'
  exit 0
fi

choose_smoke_ports
step "macOS runtime smoke (ports $PETSONA_VERIFY_STATE_PORT/$PETSONA_VERIFY_HOOK_PORT)"
PETSONA_SMOKE_STATE_PORT="$PETSONA_VERIFY_STATE_PORT" \
PETSONA_NATIVE_APP="$PETSONA_NATIVE_DERIVED_DATA/Build/Products/Release/Petsona.app" \
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

PETSONA_SKIP_BUILD=1 \
PETSONA_NATIVE_DERIVED_DATA="$PETSONA_NATIVE_DERIVED_DATA" \
PETSONA_ARCH="$PETSONA_NATIVE_ARCH" \
"$PETSONA_ROOT/scripts/package-macos.sh" "$PETSONA_PACKAGE_TMP" >/dev/null

PETSONA_APP="$PETSONA_PACKAGE_TMP/Petsona.app"
PETSONA_ZIP="$PETSONA_PACKAGE_TMP/Petsona-macos-$(uname -m).zip"
PETSONA_ACCEPTANCE_NAME="Petsona-macos-$PETSONA_NATIVE_ARCH-acceptance"
PETSONA_ACCEPTANCE_DIR="$PETSONA_PACKAGE_TMP/$PETSONA_ACCEPTANCE_NAME"
PETSONA_ACCEPTANCE_ZIP="$PETSONA_PACKAGE_TMP/$PETSONA_ACCEPTANCE_NAME.zip"
PETSONA_INFO="$PETSONA_APP/Contents/Info.plist"
PETSONA_EXECUTABLE="$PETSONA_APP/Contents/MacOS/Petsona"
PETSONA_ICON="$PETSONA_APP/Contents/Resources/Petsona.icns"

[[ -d "$PETSONA_APP" ]] || fail 'Petsona.app 未生成'
[[ -x "$PETSONA_EXECUTABLE" ]] || fail 'app 可执行文件缺失或不可执行'
[[ -f "$PETSONA_ICON" ]] || fail 'Petsona.icns 缺失'
[[ -f "$PETSONA_ZIP" ]] || fail 'macOS zip 未生成'
pass 'app bundle、可执行文件和图标存在'

[[ -x "$PETSONA_ACCEPTANCE_DIR/Petsona.app/Contents/MacOS/Petsona" ]] || fail '整体文件夹验收包缺少可运行 app'
[[ -d "$PETSONA_ACCEPTANCE_DIR/acceptance-data/pets" ]] || fail '整体文件夹验收包缺少隔离宠物库目录'
[[ -f "$PETSONA_ACCEPTANCE_DIR/acceptance-data/config.json" ]] || fail '整体文件夹验收包缺少隔离配置'
[[ -f "$PETSONA_ACCEPTANCE_DIR/README.txt" ]] || fail '整体文件夹验收包缺少启动说明'
grep -F '"stateServer":{"enabled":false,"port":17873}' "$PETSONA_ACCEPTANCE_DIR/acceptance-data/config.json" >/dev/null || fail '验收包初始状态服务未隔离'
grep -F 'PETSONA_AUTOSTART_PLIST_DIR' "$PETSONA_ACCEPTANCE_DIR/README.txt" >/dev/null || fail '验收包未说明自启项隔离'
grep -F '不要在本包中保存或清除 API Key' "$PETSONA_ACCEPTANCE_DIR/README.txt" >/dev/null || fail '验收包未说明钥匙串边界'
[[ -f "$PETSONA_ACCEPTANCE_ZIP" ]] || fail '整体文件夹验收包 zip 未生成'
codesign --verify --deep --strict "$PETSONA_ACCEPTANCE_DIR/Petsona.app" >/dev/null || fail '整体文件夹验收包的 app 签名结构无效'

PETSONA_ACCEPTANCE_MARKER="$PETSONA_ACCEPTANCE_DIR/acceptance-data/.repackage-preserves-data"
: > "$PETSONA_ACCEPTANCE_MARKER"
PETSONA_SKIP_BUILD=1 \
PETSONA_NATIVE_DERIVED_DATA="$PETSONA_NATIVE_DERIVED_DATA" \
PETSONA_ARCH="$PETSONA_NATIVE_ARCH" \
"$PETSONA_ROOT/scripts/package-macos.sh" "$PETSONA_PACKAGE_TMP" >/dev/null
[[ -f "$PETSONA_ACCEPTANCE_MARKER" ]] || fail '重新打包意外清除了本机验收数据'
rm -f "$PETSONA_ACCEPTANCE_MARKER"
pass '整体文件夹验收包有独立 home、有效 app 签名且重打包保留数据'

if command -v otool >/dev/null 2>&1; then
  otool -L "$PETSONA_EXECUTABLE" > "$PETSONA_PACKAGE_TMP/otool.txt"
  ! grep -F "$PETSONA_ROOT" "$PETSONA_PACKAGE_TMP/otool.txt" || fail '原生 app 仍依赖仓库绝对路径'
  ! grep -F 'libpetsona_ffi' "$PETSONA_PACKAGE_TMP/otool.txt" || fail '原生 app 仍动态依赖 petsona_ffi'
  [[ "$(lipo -archs "$PETSONA_EXECUTABLE")" == "$PETSONA_NATIVE_ARCH" ]] || fail '原生 app 架构与构建架构不一致'
  pass '原生 app 无仓库动态依赖且架构正确'
fi

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

PETSONA_ACCEPTANCE_UNPACKED="$PETSONA_PACKAGE_TMP/acceptance-unpacked"
ditto -x -k --norsrc "$PETSONA_ACCEPTANCE_ZIP" "$PETSONA_ACCEPTANCE_UNPACKED"
[[ -x "$PETSONA_ACCEPTANCE_UNPACKED/$PETSONA_ACCEPTANCE_NAME/Petsona.app/Contents/MacOS/Petsona" ]] || fail '验收 zip 解压后缺少可运行 app'
[[ -f "$PETSONA_ACCEPTANCE_UNPACKED/$PETSONA_ACCEPTANCE_NAME/acceptance-data/config.json" ]] || fail '验收 zip 解压后缺少隔离 home'
[[ ! -e "$PETSONA_ACCEPTANCE_UNPACKED/$PETSONA_ACCEPTANCE_NAME/acceptance-data/.repackage-preserves-data" ]] || fail '验收 zip 意外包含本机验收数据'
pass '整体文件夹验收 zip 可解压，且使用干净隔离数据而非本机验收数据'

PETSONA_RELEASE_PACKAGE_TMP="$PETSONA_PACKAGE_TMP/release-only"
mkdir -p "$PETSONA_RELEASE_PACKAGE_TMP"
PETSONA_SKIP_BUILD=1 \
PETSONA_NATIVE_DERIVED_DATA="$PETSONA_NATIVE_DERIVED_DATA" \
PETSONA_ARCH="$PETSONA_NATIVE_ARCH" \
PETSONA_INCLUDE_ACCEPTANCE_PACKAGE=0 \
"$PETSONA_ROOT/scripts/package-macos.sh" "$PETSONA_RELEASE_PACKAGE_TMP" >/dev/null
[[ -f "$PETSONA_RELEASE_PACKAGE_TMP/Petsona-macos-$PETSONA_NATIVE_ARCH.zip" ]] || fail 'release-only 打包缺少常规 macOS zip'
[[ ! -e "$PETSONA_RELEASE_PACKAGE_TMP/Petsona-macos-$PETSONA_NATIVE_ARCH-acceptance" ]] || fail 'release-only 打包意外生成验收目录'
[[ ! -e "$PETSONA_RELEASE_PACKAGE_TMP/Petsona-macos-$PETSONA_NATIVE_ARCH-acceptance.zip" ]] || fail 'release-only 打包意外生成验收 zip'
pass 'release-only 打包不会生成或上传验收数据包'

PETSONA_LAUNCH_AGENT="$PETSONA_ROOT/packaging/macos/com.petsona.desktop.plist"
plutil -lint "$PETSONA_LAUNCH_AGENT" >/dev/null || fail 'LaunchAgent plist 模板无效'
[[ "$(plist_value "$PETSONA_LAUNCH_AGENT" Label)" == 'com.petsona.desktop' ]] || fail 'LaunchAgent label 不正确'
[[ "$(plist_value "$PETSONA_LAUNCH_AGENT" RunAtLoad)" == 'true' ]] || fail 'LaunchAgent 未配置 RunAtLoad'
[[ "$(plist_value "$PETSONA_LAUNCH_AGENT" LimitLoadToSessionType)" == 'Aqua' ]] || fail 'LaunchAgent 未限制到 Aqua 会话'
pass 'LaunchAgent 模板关键字段正确（未安装）'

printf '\nAll macOS automated checks passed.\n'
printf 'Manual checks remain: docs/MACOS_VERIFICATION.md\n'
