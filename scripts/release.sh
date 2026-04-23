#!/usr/bin/env bash
set -euo pipefail

# Typercise release script: build → sign → notarize → staple
#
# 前提:
#   - Keychain に "Developer ID Application: Garage Standard Inc. (RLP4F244RK)" が入っている
#   - 下記いずれかで notarize クレデンシャルを準備済み
#       (A) Keychain profile: `xcrun notarytool store-credentials <profile>`
#       (B) 環境変数: APPLE_ID / APPLE_PASSWORD / APPLE_TEAM_ID
#
# Usage:
#   ./scripts/release.sh             # build + sign + notarize + staple
#   ./scripts/release.sh build       # build のみ
#   ./scripts/release.sh sign        # 既存バンドルに署名のみ
#   ./scripts/release.sh notarize    # notarize + staple のみ
#   ./scripts/release.sh dev-install # adhoc 署名して /Applications/ にインストール（dev用）

cd "$(dirname "$0")/.."

PRODUCT_NAME="Typercise"
BUNDLE_ID="jp.garage-standard.keycount"
APP_PATH="target/release/bundle/macos/${PRODUCT_NAME}.app"
DMG_PATH="target/release/bundle/dmg"

# デフォルト署名 ID（環境変数で上書き可）
: "${APPLE_SIGNING_IDENTITY:=Developer ID Application: Garage Standard Inc. (RLP4F244RK)}"
# notarytool 用 Keychain Profile（環境変数で指定、未設定なら APPLE_ID/APPLE_PASSWORD を使用）
: "${NOTARY_PROFILE:=typercise-notary}"

step() { printf "\n\033[1;34m▶ %s\033[0m\n" "$*"; }
require_file() { [ -e "$1" ] || { echo "missing: $1" >&2; exit 1; }; }

do_build() {
  step "Tauri production build"
  cargo tauri build
  require_file "$APP_PATH"
}

do_sign() {
  require_file "$APP_PATH"
  step "codesign (Hardened Runtime + entitlements + timestamp)"
  codesign --force --options runtime \
    --entitlements src-tauri/entitlements.plist \
    --sign "$APPLE_SIGNING_IDENTITY" \
    --timestamp \
    --deep "$APP_PATH"
  step "署名確認"
  codesign -dv --verbose=4 "$APP_PATH" 2>&1 || true
  step "Gatekeeper 事前チェック"
  spctl -a -vv --type execute "$APP_PATH" || echo "(公証前なので rejected で正常)"
}

do_notarize() {
  require_file "$APP_PATH"
  local zip_path="${APP_PATH%.app}.zip"
  step "notarize 用に zip 化"
  rm -f "$zip_path"
  ditto -c -k --keepParent "$APP_PATH" "$zip_path"

  step "notarytool で Apple に submit"
  if xcrun notarytool history --keychain-profile "$NOTARY_PROFILE" >/dev/null 2>&1; then
    echo "using keychain profile: $NOTARY_PROFILE"
    xcrun notarytool submit "$zip_path" \
      --keychain-profile "$NOTARY_PROFILE" \
      --wait
  else
    : "${APPLE_ID:?APPLE_ID が必要（または NOTARY_PROFILE を store-credentials で準備）}"
    : "${APPLE_PASSWORD:?APPLE_PASSWORD が必要}"
    : "${APPLE_TEAM_ID:=RLP4F244RK}"
    xcrun notarytool submit "$zip_path" \
      --apple-id "$APPLE_ID" \
      --password "$APPLE_PASSWORD" \
      --team-id "$APPLE_TEAM_ID" \
      --wait
  fi

  step "staple でチケットを埋め込み"
  xcrun stapler staple "$APP_PATH"
  xcrun stapler validate "$APP_PATH"

  step "最終 Gatekeeper チェック"
  spctl -a -vv --type execute "$APP_PATH"
}

do_dmg() {
  require_file "$APP_PATH"
  local version
  version=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP_PATH/Contents/Info.plist")
  local arch
  arch=$(uname -m)
  local out_dmg="${DMG_PATH}/${PRODUCT_NAME}_${version}_${arch}.dmg"
  local staging
  staging=$(mktemp -d)

  step "staging 作成（Typercise.app + /Applications シンボリックリンク）"
  cp -R "$APP_PATH" "$staging/"
  ln -s /Applications "$staging/Applications"

  step "hdiutil で dmg を作成"
  mkdir -p "$DMG_PATH"
  rm -f "$out_dmg"
  hdiutil create -volname "$PRODUCT_NAME" -srcfolder "$staging" -ov -format UDZO "$out_dmg"
  rm -rf "$staging"

  step "dmg に Developer ID 署名"
  codesign --force --sign "$APPLE_SIGNING_IDENTITY" --timestamp "$out_dmg"

  step "dmg を notarize"
  if xcrun notarytool history --keychain-profile "$NOTARY_PROFILE" >/dev/null 2>&1; then
    xcrun notarytool submit "$out_dmg" --keychain-profile "$NOTARY_PROFILE" --wait
  else
    xcrun notarytool submit "$out_dmg" \
      --apple-id "${APPLE_ID:?}" --password "${APPLE_PASSWORD:?}" --team-id "${APPLE_TEAM_ID:-RLP4F244RK}" --wait
  fi

  step "dmg を staple"
  xcrun stapler staple "$out_dmg"
  xcrun stapler validate "$out_dmg"

  step "完成"
  echo "$out_dmg"
}

do_dev_install() {
  step "Tauri production build"
  cargo tauri build
  require_file "$APP_PATH"

  step "旧 /Applications/${PRODUCT_NAME}.app を差し替え"
  rm -rf "/Applications/${PRODUCT_NAME}.app"
  cp -R "$APP_PATH" "/Applications/"

  step "拡張属性クリア + バンドル adhoc 署名"
  xattr -cr "/Applications/${PRODUCT_NAME}.app"
  codesign --force --deep --sign - "/Applications/${PRODUCT_NAME}.app"

  step "署名確認"
  codesign -dv --verbose=4 "/Applications/${PRODUCT_NAME}.app" 2>&1 | grep -E "Identifier|adhoc" || true

  step "既存プロセスを止めて再起動"
  pkill -f "${PRODUCT_NAME}.app/Contents/MacOS/keycount" || true
  sleep 1
  open "/Applications/${PRODUCT_NAME}.app"
  echo "→ 初回起動でアクセシビリティ権限プロンプトが出ます"
}

case "${1:-all}" in
  build)       do_build ;;
  sign)        do_sign ;;
  notarize)    do_notarize ;;
  dmg)         do_dmg ;;
  dev-install) do_dev_install ;;
  all|"")      do_build; do_sign; do_notarize; do_dmg; step "完了: $APP_PATH" ;;
  *) echo "usage: $0 [build|sign|notarize|dmg|dev-install|all]"; exit 2 ;;
esac
