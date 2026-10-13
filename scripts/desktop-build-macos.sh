#!/bin/bash
# Builds the current Rust+TS shell and stages an unsigned local acceptance app.
# Signing/notarization and distributable packaging remain M6.
set -euo pipefail
petsona_profile="${1:-debug}"
petsona_mode="${2:-acceptance}"
case "$petsona_profile" in debug|release) ;; *) echo 'Usage: desktop-build-macos.sh [debug|release] [acceptance|formal]' >&2; exit 2 ;; esac
case "$petsona_mode" in acceptance|formal) ;; *) echo "Invalid mode: $petsona_mode" >&2; exit 2 ;; esac
if [ "$(uname -s)" != Darwin ]; then echo 'This build requires macOS.' >&2; exit 2; fi
petsona_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$petsona_root"
pnpm --dir apps/desktop build
petsona_manifest="$petsona_root/apps/desktop/src-tauri/Cargo.toml"
petsona_build_args=(build --manifest-path "$petsona_manifest" --locked --no-default-features)
if [ "$petsona_profile" = release ]; then petsona_build_args+=(--release); fi
if [ "$petsona_mode" = acceptance ]; then petsona_build_args+=(--features acceptance-hooks); fi
MACOSX_DEPLOYMENT_TARGET=26.0 cargo "${petsona_build_args[@]}"
petsona_target="$(cargo metadata --manifest-path "$petsona_manifest" --no-deps --format-version 1 | python3 -c 'import sys,json; print(json.load(sys.stdin)["target_directory"])')"
petsona_app="$petsona_root/.scratch/macos/$petsona_profile/Petsona.app"
petsona_identifier="com.petsona.desktop.dev"
if [ "$petsona_mode" = formal ]; then
  petsona_app="$petsona_root/.scratch/macos/formal/$petsona_profile/Petsona.app"
  petsona_identifier="com.petsona.desktop"
fi
mkdir -p "$petsona_app/Contents/MacOS" "$petsona_app/Contents/Resources"
# Do not overwrite executed signed code in place: macOS caches its signature
# by inode. Stage a fresh file, then atomically replace this local build output.
petsona_binary_temp="$(mktemp "$petsona_app/Contents/MacOS/.petsona-desktop.XXXXXX")"
trap 'if [ -n "${petsona_binary_temp:-}" ]; then rm -f "$petsona_binary_temp"; fi' EXIT
cp -p "$petsona_target/$petsona_profile/petsona-desktop" "$petsona_binary_temp"
mv -f "$petsona_binary_temp" "$petsona_app/Contents/MacOS/petsona-desktop"
petsona_binary_temp=""
cp apps/desktop/src-tauri/icons/icon.icns "$petsona_app/Contents/Resources/icon.icns"
petsona_version="$(python3 -c 'import json; print(json.load(open("apps/desktop/src-tauri/tauri.conf.json"))["version"])')"
cat > "$petsona_app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Petsona</string>
<key>CFBundleDisplayName</key><string>Petsona</string>
<key>CFBundleIdentifier</key><string>$petsona_identifier</string>
<key>CFBundleExecutable</key><string>petsona-desktop</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$petsona_version</string>
<key>CFBundleVersion</key><string>1</string>
<key>CFBundleIconFile</key><string>icon.icns</string>
<key>LSMinimumSystemVersion</key><string>26.0</string>
<key>LSUIElement</key><true/>
<key>NSHighResolutionCapable</key><true/>
<key>NSPrincipalClass</key><string>NSApplication</string>
</dict></plist>
PLIST
plutil -lint "$petsona_app/Contents/Info.plist"
echo "Unsigned $petsona_mode app: $petsona_app"
