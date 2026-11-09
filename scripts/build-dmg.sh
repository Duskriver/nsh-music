#!/usr/bin/env bash
# Builds nsh-music for macOS and creates a .dmg installer and a .zip package.
# Output is placed in dist/
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> Checking prerequisites..."
command -v nsh > /dev/null || { echo "Error: 'nsh' CLI not found. Please install via: curl -fsSL https://nsh.dev/install.sh | sh -s -- --nightly" >&2; exit 1; }

VERSION=$(jq -r '.version // "0.1.0"' nsh.json 2>/dev/null || echo "0.1.0")
APP_NAME=$(jq -r '.name // "nsh-music"' nsh.json 2>/dev/null || echo "nsh-music")
BUILD_NUM=$(jq -r '.build // 1' nsh.json 2>/dev/null || echo "1")

DIST_DIR="${1:-dist}"
TEMP_BUILD="target/agents/out"
mkdir -p "$DIST_DIR" target/agents/tmp
export TMPDIR="$PWD/target/agents/tmp"

echo "==> Packaging macOS App with nsh (version $VERSION, build $BUILD_NUM)..."
nsh package --target macos -o "$TEMP_BUILD" --app-version "$VERSION" --build "$BUILD_NUM"

APP_PATH="$TEMP_BUILD/$APP_NAME.app"
if [ ! -d "$APP_PATH" ]; then
  echo "Error: Application bundle not found at $APP_PATH" >&2
  exit 1
fi

echo "==> Creating macOS DMG installer..."
DMG_STAGING="target/agents/dmg_staging"
rm -rf "$DMG_STAGING"
mkdir -p "$DMG_STAGING"
cp -R "$APP_PATH" "$DMG_STAGING/"
ln -s /Applications "$DMG_STAGING/Applications"

DMG_OUT="$DIST_DIR/nsh-music-macos-arm64.dmg"
rm -f "$DMG_OUT"
hdiutil create -volname "NSH Music" -srcfolder "$DMG_STAGING" -ov -format UDZO "$DMG_OUT"
rm -rf "$DMG_STAGING"

echo "==> Creating macOS ZIP package..."
ZIP_OUT="$DIST_DIR/nsh-music-macos-arm64.zip"
rm -f "$ZIP_OUT"
ditto -c -k --keepParent "$APP_PATH" "$ZIP_OUT"

echo "==> Copying updater tarball if present..."
cp "$TEMP_BUILD"/nsh-music-*.tar.gz "$DIST_DIR/" 2>/dev/null || true

echo "==> Generating SHA-256 checksums..."
(
  cd "$DIST_DIR"
  if command -v sha256sum > /dev/null; then
    sha256sum nsh-music-* > sha256sums 2>/dev/null || true
  else
    shasum -a 256 nsh-music-* > sha256sums 2>/dev/null || true
  fi
)

echo ""
echo "=================================================="
echo "🎉 macOS 安装包制作完成！"
echo "DMG 安装包: $DMG_OUT"
echo "ZIP 压缩包: $ZIP_OUT"
echo "发布目录:   $DIST_DIR"
echo "=================================================="
