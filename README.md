# KeyCount

macOS メニューバー常駐の打鍵カウンタ（Rust + Tauri v2 + Leptos）。

- 文字/記号キーのみをカウント（修飾キー・マウスは除外）
- Today / 曜日別平均 / 週 / 月 / 累計 の統計
- 消費カロリー参考値（1打鍵 ≒ 0.0014 kcal）
- キーの内容は一切保存しない。カウントのみ
- Dock 非表示（LSUIElement）

## 開発

依存: Rust stable, wasm32-unknown-unknown, trunk, cargo-tauri v2。

```bash
# 初回セットアップ
rustup target add wasm32-unknown-unknown
cargo install tauri-cli --version '^2.0' --locked
cargo install --locked trunk

# 開発モード
cargo tauri dev
```

初回起動時に macOS から「アクセシビリティ権限」を求められる。許可しないとキーを検知できない。

## ビルド

```bash
# ローカルビルドのみ
cargo tauri build --no-bundle

# 配布用 .app / .dmg + 署名 + notarize
export APPLE_SIGNING_IDENTITY="Developer ID Application: ... (TEAMID)"
export APPLE_ID="you@example.com"
export APPLE_PASSWORD="app-specific-password"
export APPLE_TEAM_ID="ABCDE12345"
./scripts/release.sh
```

## データの保存場所

```
~/Library/Application Support/jp.garage-standard.keycount/db.sqlite
```

スキーマは分単位の累計のみ。

```sql
CREATE TABLE keystrokes (
  minute_ts   INTEGER PRIMARY KEY,  -- UNIX秒 / 60
  count       INTEGER NOT NULL,
  corrections INTEGER NOT NULL DEFAULT 0
);
```

## License

MIT。詳細は [LICENSE](./LICENSE) を参照。

## 構成

```
keycount/
├── Cargo.toml         # workspace (UIクレート)
├── index.html         # Trunkエントリ
├── Trunk.toml
├── styles.css
├── src/               # Leptos (WASM)
│   ├── main.rs
│   └── app.rs
└── src-tauri/         # Tauri backend
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── Info.plist     # LSUIElement, Accessibility用途説明
    ├── entitlements.plist
    └── src/
        ├── lib.rs       # Tauri setup + tray
        ├── store.rs     # SQLite
        ├── buffer.rs    # AtomicU64 + 10秒flush
        ├── keystroke.rs # CGEventTap (macOS)
        ├── perms.rs     # AXIsProcessTrustedWithOptions
        ├── stats.rs     # 集計
        └── commands.rs  # invoke handlers
```
