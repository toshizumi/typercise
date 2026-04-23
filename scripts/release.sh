#!/usr/bin/env bash
set -euo pipefail

# Typercise release script: build → sign → notarize → staple
#
# Required env vars:
#   APPLE_SIGNING_IDENTITY  e.g. "Developer ID Application: Taro Yamada (TEAMID)"
#   APPLE_ID                Apple ID email used for notarization
#   APPLE_PASSWORD          App-specific password (create at appleid.apple.com)
#   APPLE_TEAM_ID           10-char Team ID
#
# Usage:
#   ./scripts/release.sh           # build + sign + notarize + staple
#   ./scripts/release.sh build     # build only
#   ./scripts/release.sh notarize  # notarize an already-signed bundle

cd "$(dirname "$0")/.."

PRODUCT_NAME="Typercise"
BUNDLE_ID="jp.garage-standard.keycount"
APP_PATH="src-tauri/target/release/bundle/macos/${PRODUCT_NAME}.app"
DMG_PATH="src-tauri/target/release/bundle/dmg"

step() { printf "\n\033[1;34m▶ %s\033[0m\n" "$*"; }
require_env() {
  local name=$1
  if [ -z "${!name:-}" ]; then
    echo "env $name is required" >&2
    exit 1
  fi
}

do_build() {
  step "Tauri production build"
  cargo tauri build
  [ -d "$APP_PATH" ] || { echo "app not found at $APP_PATH"; exit 1; }
}

do_sign() {
  require_env APPLE_SIGNING_IDENTITY
  step "codesign (Hardened Runtime + entitlements)"
  codesign --force --options runtime \
    --entitlements src-tauri/entitlements.plist \
    --sign "$APPLE_SIGNING_IDENTITY" \
    --timestamp \
    --deep "$APP_PATH"
  step "codesign verification"
  codesign -dv --verbose=4 "$APP_PATH"
  spctl -a -vv --type execute "$APP_PATH" || true
}

do_notarize() {
  require_env APPLE_ID
  require_env APPLE_PASSWORD
  require_env APPLE_TEAM_ID

  local zip_path="${APP_PATH%.app}.zip"
  step "Zipping for notarization"
  ditto -c -k --keepParent "$APP_PATH" "$zip_path"

  step "Submitting to notarytool (waits)"
  xcrun notarytool submit "$zip_path" \
    --apple-id "$APPLE_ID" \
    --password "$APPLE_PASSWORD" \
    --team-id "$APPLE_TEAM_ID" \
    --wait

  step "Stapling ticket"
  xcrun stapler staple "$APP_PATH"
  xcrun stapler validate "$APP_PATH"
}

case "${1:-all}" in
  build)    do_build ;;
  sign)     do_sign ;;
  notarize) do_notarize ;;
  all|"")   do_build; do_sign; do_notarize; step "Done: $APP_PATH" ;;
  *) echo "usage: $0 [build|sign|notarize|all]"; exit 2 ;;
esac
