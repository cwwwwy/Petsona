#!/usr/bin/env bash
set -euo pipefail

PETSONA_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${1:-$PETSONA_ROOT/dist}"
ARCH="${PETSONA_ARCH:-$(uname -m)}"
VERSION="${PETSONA_VERSION:-$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$PETSONA_ROOT/Cargo.toml" | head -n 1)}"

case "$OUTPUT_DIR" in
  /*) ;;
  *) OUTPUT_DIR="$PETSONA_ROOT/$OUTPUT_DIR" ;;
esac

APP_DIR="$OUTPUT_DIR/Petsona.app"
ZIP_PATH="$OUTPUT_DIR/Petsona-macos-$ARCH.zip"
DERIVED_DATA="${PETSONA_NATIVE_DERIVED_DATA:-$PETSONA_ROOT/.scratch/macos-package-build}"

for command_name in cargo xcodebuild ditto plutil; do
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
[[ -x "$EXECUTABLE" ]] || { printf 'Native app executable is missing\n' >&2; exit 1; }
plutil -lint "$INFO" >/dev/null
ditto -c -k --norsrc --keepParent "$APP_DIR" "$ZIP_PATH"

printf 'Created %s\n' "$APP_DIR"
printf 'Created %s\n' "$ZIP_PATH"
printf 'Native SwiftUI/AppKit app; unsigned unless scripts/sign-macos.sh is run.\n'
