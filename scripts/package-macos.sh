#!/usr/bin/env bash
set -euo pipefail

PETSONA_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${1:-$PETSONA_ROOT/dist}"
ARCH="${PETSONA_ARCH:-$(uname -m)}"
VERSION="${PETSONA_VERSION:-$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$PETSONA_ROOT/Cargo.toml" | head -n 1)}"
INCLUDE_ACCEPTANCE_PACKAGE="${PETSONA_INCLUDE_ACCEPTANCE_PACKAGE:-1}"

case "$INCLUDE_ACCEPTANCE_PACKAGE" in
  0|1) ;;
  *) printf 'PETSONA_INCLUDE_ACCEPTANCE_PACKAGE must be 0 or 1\n' >&2; exit 2 ;;
esac

case "$OUTPUT_DIR" in
  /*) ;;
  *) OUTPUT_DIR="$PETSONA_ROOT/$OUTPUT_DIR" ;;
esac

APP_DIR="$OUTPUT_DIR/Petsona.app"
ZIP_PATH="$OUTPUT_DIR/Petsona-macos-$ARCH.zip"
ACCEPTANCE_NAME="Petsona-macos-$ARCH-acceptance"
ACCEPTANCE_DIR="$OUTPUT_DIR/$ACCEPTANCE_NAME"
ACCEPTANCE_APP="$ACCEPTANCE_DIR/Petsona.app"
ACCEPTANCE_DATA="$ACCEPTANCE_DIR/acceptance-data"
ACCEPTANCE_README="$ACCEPTANCE_DIR/README.txt"
ACCEPTANCE_ZIP="$OUTPUT_DIR/$ACCEPTANCE_NAME.zip"
DERIVED_DATA="${PETSONA_NATIVE_DERIVED_DATA:-$PETSONA_ROOT/.scratch/macos-package-build}"

for command_name in cargo xcodebuild ditto plutil codesign; do
  command -v "$command_name" >/dev/null 2>&1 || {
    printf 'Missing required command: %s\n' "$command_name" >&2
    exit 1
  }
done

cd "$PETSONA_ROOT"
if [[ "${PETSONA_SKIP_BUILD:-0}" != "1" ]]; then
  cargo build -p petsona-ffi --release --locked
  xcodebuild \
    -project "$PETSONA_ROOT/apps/macos/Petsona.xcodeproj" \
    -scheme Petsona \
    -configuration Release \
    -arch "$ARCH" \
    -derivedDataPath "$DERIVED_DATA" \
    CODE_SIGNING_ALLOWED=NO \
    build
fi

BUILT_APP="$DERIVED_DATA/Build/Products/Release/Petsona.app"
[[ -d "$BUILT_APP" ]] || {
  printf 'Native app bundle was not found: %s\n' "$BUILT_APP" >&2
  exit 1
}

rm -rf "$APP_DIR"
rm -f "$ZIP_PATH"
if [[ "$INCLUDE_ACCEPTANCE_PACKAGE" == "1" ]]; then
  rm -f "$ACCEPTANCE_ZIP"
fi
mkdir -p "$OUTPUT_DIR"
ditto "$BUILT_APP" "$APP_DIR"
mkdir -p "$APP_DIR/Contents/Resources"
cp "$PETSONA_ROOT/packaging/macos/Petsona.icns" "$APP_DIR/Contents/Resources/Petsona.icns"

INFO="$APP_DIR/Contents/Info.plist"
EXECUTABLE="$APP_DIR/Contents/MacOS/Petsona"
plutil -replace CFBundleShortVersionString -string "$VERSION" "$INFO"
plutil -replace CFBundleVersion -string "$VERSION" "$INFO"
plutil -replace CFBundleIconFile -string "Petsona" "$INFO" 2>/dev/null || \
  plutil -insert CFBundleIconFile -string "Petsona" "$INFO"
MINIMUM_SYSTEM_VERSION="$(/usr/libexec/PlistBuddy -c 'Print :LSMinimumSystemVersion' "$INFO")"
[[ "$MINIMUM_SYSTEM_VERSION" == "26.0" ]] || {
  printf 'Expected macOS 26.0 minimum, found %s\n' "$MINIMUM_SYSTEM_VERSION" >&2
  exit 1
}
[[ -x "$EXECUTABLE" ]] || { printf 'Native app executable is missing\n' >&2; exit 1; }
plutil -lint "$INFO" >/dev/null
codesign --force --deep --sign - --timestamp=none "$APP_DIR"
codesign --verify --deep --strict "$APP_DIR"
ditto -c -k --norsrc --keepParent "$APP_DIR" "$ZIP_PATH"

write_acceptance_config() {
  printf '%s\n' '{"activePet":null,"firstRun":true,"greeting":{"enabled":false},"window":{"autoWalk":{"enabled":false}},"stateServer":{"enabled":false,"port":17873}}' > "$1"
}

# Keep the extracted acceptance home stable across package refreshes so that
# repeated runs do not scatter throwaway directories or erase the user's test
# data. The downloadable archive below is always staged with a clean home.
if [[ "$INCLUDE_ACCEPTANCE_PACKAGE" == "1" ]]; then
  mkdir -p "$ACCEPTANCE_DATA/pets"
  if [[ ! -f "$ACCEPTANCE_DATA/config.json" ]]; then
    write_acceptance_config "$ACCEPTANCE_DATA/config.json"
  fi
  rm -rf "$ACCEPTANCE_APP"
  ditto "$APP_DIR" "$ACCEPTANCE_APP"

  cat > "$ACCEPTANCE_README" <<EOF
Petsona macOS 隔离验收包

目录内容：
- Petsona.app：当前 macOS 原生应用
- acceptance-data：独立配置、宠物库、日志和实例锁；重复打包会保留此目录

首次启动前请从菜单栏退出日常运行的 Petsona，避免 LaunchServices 复用日常实例。
从 Terminal 启动（将路径替换为本目录的实际位置）：
cd "/path/to/$ACCEPTANCE_NAME" && open --env "PETSONA_HOME=\$PWD/acceptance-data" --env "PETSONA_AUTOSTART_PLIST_DIR=\$PWD/acceptance-data/LaunchAgents" --env "PETSONA_OPEN_SETTINGS_ON_LAUNCH=1" "\$PWD/Petsona.app"
重复运行此命令会复用已运行的验收实例，并重新打开、聚焦设置窗口；不要添加 -n。

不要直接双击 Petsona.app：Finder 启动不会自动继承上面的 PETSONA_HOME，可能使用日常数据目录。

隔离边界：
- 初始配置关闭状态服务，预留端口 17873；宠物库为空，不会自动导入 Codex 宠物。
- PETSONA_HOME 存在时，设置页上次浏览位置保存在独立验收偏好域，不会写入日常 Petsona 偏好。
- LaunchAgent 测试文件重定向到 acceptance-data/LaunchAgents，不会注册真实登录自启；本包不验收登录后自启。
- macOS 钥匙串仍使用正式服务标识 com.petsona.desktop。不要在本包中保存或清除 API Key，以免影响日常应用凭据。
- 验收完成后可删除整个 $ACCEPTANCE_NAME 目录；其中的验收配置和导入宠物会一并移除。

本地签名为 ad-hoc，不含 Developer ID，也未公证。首次打开如被 macOS 拦截，请按系统安全提示处理。
最低系统版本：macOS 26。
EOF

  ACCEPTANCE_STAGE="$(mktemp -d "${TMPDIR:-/tmp}/petsona-acceptance-package.XXXXXX")"
  cleanup_acceptance_stage() {
    local exit_code=$?
    trap - EXIT INT TERM
    rm -rf "$ACCEPTANCE_STAGE"
    exit "$exit_code"
  }
  trap cleanup_acceptance_stage EXIT INT TERM
  STAGED_ACCEPTANCE="$ACCEPTANCE_STAGE/$ACCEPTANCE_NAME"
  mkdir -p "$STAGED_ACCEPTANCE/acceptance-data/pets"
  ditto "$APP_DIR" "$STAGED_ACCEPTANCE/Petsona.app"
  cp "$ACCEPTANCE_README" "$STAGED_ACCEPTANCE/README.txt"
  write_acceptance_config "$STAGED_ACCEPTANCE/acceptance-data/config.json"
  ditto -c -k --norsrc --keepParent "$STAGED_ACCEPTANCE" "$ACCEPTANCE_ZIP"
  rm -rf "$ACCEPTANCE_STAGE"
  trap - EXIT INT TERM
fi

printf 'Created %s\n' "$APP_DIR"
printf 'Created %s\n' "$ZIP_PATH"
if [[ "$INCLUDE_ACCEPTANCE_PACKAGE" == "1" ]]; then
  printf 'Created isolated acceptance folder %s\n' "$ACCEPTANCE_DIR"
  printf 'Created isolated acceptance archive %s\n' "$ACCEPTANCE_ZIP"
fi
printf 'Native SwiftUI/AppKit app; ad-hoc signed, without Developer ID or notarization.\n'
