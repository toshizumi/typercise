# typercise-api

Typercise クライアントから 1 日 1 回送られる集計値を受け取り、世界の Typercizer ダッシュボード用に集計するワーカー。Cloudflare Workers + D1。

## 受け取る情報

- `client_id` (UUID v4, クライアントローカル生成)
- `date` (YYYY-MM-DD, クライアントローカル日)
- `keys`, `corrections`, `kcal`, `peak_kpm`, `avg_kpm`, `active_minutes`
- `nickname` (任意。設定したクライアントのみランキング表示対象)
- `app_version`, `os_version`

**送りません**: キーの内容、入力先アプリ、文書、Apple ID、IP（Worker は受信時点でログに残しません）。

## ローカル開発

```bash
cd server
npm install
npx wrangler d1 create typercise            # 初回のみ。出力された database_id を wrangler.toml に貼る
npm run migrate:local                        # ローカル D1 にスキーマ適用
npm run dev                                  # http://localhost:8787 で起動
```

スモーク:

```bash
curl -X POST http://localhost:8787/api/v1/report \
  -H 'content-type: application/json' \
  -d '{"client_id":"00000000-0000-4000-8000-000000000001","date":"2026-04-25","keys":100,"corrections":2,"kcal":0.14,"peak_kpm":80,"avg_kpm":50,"active_minutes":2,"nickname":"alice","app_version":"0.2.0","os_version":"15.6.1"}'

curl http://localhost:8787/api/v1/world
```

## デプロイ

```bash
npx wrangler login           # 初回のみ
npm run migrate:prod          # 本番 D1 にスキーマ適用
npm run deploy
```

公開 URL は `https://typercise-api.<account>.workers.dev`。

## ライセンス

MIT (リポジトリルートと同一)。
