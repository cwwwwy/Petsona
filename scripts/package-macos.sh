#!/usr/bin/env bash
set -euo pipefail

BYTEPET_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${1:-$BYTEPET_ROOT/dist}"
PACKAGE_VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$BYTEPET_ROOT/Cargo.toml" | head -n 1)"
VERSION="${BYTEPET_VERSION:-${PACKAGE_VERSION:-0.1.0}}"
TARGET="${BYTEPET_TARGET:-}"
ARCH="${BYTEPET_ARCH:-$(uname -m)}"

case "$OUTPUT_DIR" in
  /*) ;;
  *) OUTPUT_DIR="$BYTEPET_ROOT/$OUTPUT_DIR" ;;
esac

APP_DIR="$OUTPUT_DIR/BytePet.app"
ZIP_PATH="$OUTPUT_DIR/BytePet-macos-$ARCH.zip"

if ! command -v cargo >/dev/null 2>&1; then
  printf 'Missing required command: cargo\n' >&2
  exit 1
fi
for command_name in ditto plutil; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    printf 'Missing required command: %s\n' "$command_name" >&2
    exit 1
  fi
done

cd "$BYTEPET_ROOT"

cargo_args=(build --release --locked -p bytepet-app)
binary_path="$BYTEPET_ROOT/target/release/bytepet"
if [[ -n "$TARGET" ]]; then
  cargo_args+=(--target "$TARGET")
  binary_path="$BYTEPET_ROOT/target/$TARGET/release/bytepet"
fi
cargo "${cargo_args[@]}"

if [[ ! -x "$binary_path" ]]; then
  printf 'Release binary was not found: %s\n' "$binary_path" >&2
  exit 1
fi

rm -rf "$APP_DIR"
rm -f "$ZIP_PATH"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"

cp "$binary_path" "$APP_DIR/Contents/MacOS/BytePet"
cp "$BYTEPET_ROOT/packaging/macos/BytePet.icns" "$APP_DIR/Contents/Resources/BytePet.icns"
sed "s/@VERSION@/$VERSION/g" \
  "$BYTEPET_ROOT/packaging/macos/Info.plist" \
  > "$APP_DIR/Contents/Info.plist"
chmod 755 "$APP_DIR/Contents/MacOS/BytePet"

plutil -lint "$APP_DIR/Contents/Info.plist" >/dev/null
ditto -c -k --norsrc --keepParent "$APP_DIR" "$ZIP_PATH"

printf 'Created %s\n' "$APP_DIR"
printf 'Created %s\n' "$ZIP_PATH"
printf 'Bundle identifier: com.bytepet.BytePet\n'
printf 'The bundle is unsigned unless sign-macos.sh is run.\n'
