#!/usr/bin/env bash
set -euo pipefail

PETSONA_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${1:-$PETSONA_ROOT/dist}"
PACKAGE_VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$PETSONA_ROOT/Cargo.toml" | head -n 1)"
VERSION="${PETSONA_VERSION:-${PACKAGE_VERSION:-0.1.0}}"
TARGET="${PETSONA_TARGET:-}"
ARCH="${PETSONA_ARCH:-$(uname -m)}"

case "$OUTPUT_DIR" in
  /*) ;;
  *) OUTPUT_DIR="$PETSONA_ROOT/$OUTPUT_DIR" ;;
esac

APP_DIR="$OUTPUT_DIR/Petsona.app"
ZIP_PATH="$OUTPUT_DIR/Petsona-macos-$ARCH.zip"

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

cd "$PETSONA_ROOT"

cargo_args=(build --release --locked -p petsona-app)
binary_path="$PETSONA_ROOT/target/release/petsona"
if [[ -n "$TARGET" ]]; then
  cargo_args+=(--target "$TARGET")
  binary_path="$PETSONA_ROOT/target/$TARGET/release/petsona"
fi
if [[ "${PETSONA_SKIP_BUILD:-0}" == "1" ]]; then
  printf 'Skipping release build because PETSONA_SKIP_BUILD=1\n'
else
  cargo "${cargo_args[@]}"
fi

if [[ ! -x "$binary_path" ]]; then
  printf 'Release binary was not found: %s\n' "$binary_path" >&2
  exit 1
fi

rm -rf "$APP_DIR"
rm -f "$ZIP_PATH"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"

cp "$binary_path" "$APP_DIR/Contents/MacOS/Petsona"
cp "$PETSONA_ROOT/packaging/macos/Petsona.icns" "$APP_DIR/Contents/Resources/Petsona.icns"
sed "s/@VERSION@/$VERSION/g" \
  "$PETSONA_ROOT/packaging/macos/Info.plist" \
  > "$APP_DIR/Contents/Info.plist"
chmod 755 "$APP_DIR/Contents/MacOS/Petsona"

plutil -lint "$APP_DIR/Contents/Info.plist" >/dev/null
ditto -c -k --norsrc --keepParent "$APP_DIR" "$ZIP_PATH"

printf 'Created %s\n' "$APP_DIR"
printf 'Created %s\n' "$ZIP_PATH"
printf 'Bundle identifier: com.petsona.desktop\n'
printf 'The bundle is unsigned unless scripts/sign-macos.sh is run.\n'
