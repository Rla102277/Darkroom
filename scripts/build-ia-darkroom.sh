#!/usr/bin/env bash
# Build IA Darkroom.app on this Mac and put it in /Applications.
#
#   scripts/build-ia-darkroom.sh            build everything, assemble, install
#   scripts/build-ia-darkroom.sh --no-install   leave the app in dist/
#
# What goes in the bundle: the app binary, lightcraft-cli, the Swift helper (Contents/Helpers),
# the bundled Looks (Contents/Resources/looks), the icon and Info.plist. Ad hoc signed: fine for
# this Mac and for a Mac you copy it to yourself (right-click > Open the first time).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
INSTALL=1
[ "${1:-}" = "--no-install" ] && INSTALL=0

# Rust from rustup, whether or not the calling shell has it on PATH.
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
command -v cargo >/dev/null || { echo "cargo not found: install Rust with  curl https://sh.rustup.rs -sSf | sh"; exit 1; }

VERSION="$(awk '/^\[/ { in_pkg = ($0 == "[workspace.package]") ; next } in_pkg && $1 == "version" { gsub(/[" ]/, "", $3); print $3; exit }' Cargo.toml)"
SHA="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
APP_NAME="IA Darkroom"
DIST="$ROOT/dist"
APP="$DIST/$APP_NAME.app"

echo "==> helper (Swift)"
(cd helper/darkroom-helper && swift build -c release 2>&1 | tail -3)
HELPER="helper/darkroom-helper/.build/release/darkroom-helper"
[ -x "$HELPER" ] || { echo "helper did not build"; exit 1; }

echo "==> app and CLI (Rust, release)"
cargo build --release -p lightcraft -p lightcraft-cli 2>&1 | tail -3

echo "==> assembling $APP"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$APP/Contents/Helpers"
cp target/release/lightcraft "$APP/Contents/MacOS/IA Darkroom"
cp target/release/lightcraft-cli "$APP/Contents/MacOS/lightcraft-cli"
cp "$HELPER" "$APP/Contents/Helpers/darkroom-helper"
cp assets/app-icon/ia-darkroom.icns "$APP/Contents/Resources/IA Darkroom.icns"
cp -R looks "$APP/Contents/Resources/looks"
sed -e "s/@VERSION@/$VERSION/g" -e "s/@SHORT_VERSION@/${VERSION%%-*}/g" -e "s/@BUILD_SHA@/$SHA/g" \
  packaging/macos/Info.plist.in >"$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist" >/dev/null
printf 'APPL????' >"$APP/Contents/PkgInfo"
# clear extended attributes (newer macOS xattr has no -r)
find "$APP" -exec xattr -c {} + 2>/dev/null || true

echo "==> signing (ad hoc)"
ENT="packaging/macos/entitlements.plist"
codesign --force --sign - --timestamp=none --options runtime --entitlements "$ENT" "$APP/Contents/Helpers/darkroom-helper"
codesign --force --sign - --timestamp=none --options runtime --entitlements "$ENT" "$APP/Contents/MacOS/lightcraft-cli"
codesign --force --sign - --timestamp=none --options runtime --entitlements "$ENT" "$APP/Contents/MacOS/IA Darkroom"
codesign --force --sign - --timestamp=none --options runtime --entitlements "$ENT" "$APP"
codesign --verify --strict --deep "$APP"

if [ "$INSTALL" = 1 ]; then
  echo "==> installing to /Applications"
  rm -rf "/Applications/$APP_NAME.app"
  cp -R "$APP" "/Applications/$APP_NAME.app"
  echo "Done: /Applications/$APP_NAME.app  (version $VERSION, $SHA)"
else
  echo "Done: $APP  (version $VERSION, $SHA)"
fi
