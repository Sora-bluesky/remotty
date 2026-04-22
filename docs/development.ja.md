# 開発者向け情報

このページは、コントリビューターと保守者向けです。
利用者向けの導入手順は [README](../README.ja.md) に残しています。

## 基本確認

```powershell
cargo fmt --check
cargo test
cargo check
node --check npm/install.js
node --check bin/remotty.js
pwsh -NoProfile -File scripts/audit-public-surface.ps1
pwsh -NoProfile -File scripts/audit-secret-surface.ps1
pwsh -NoProfile -File scripts/audit-doc-terminology.ps1
```

リリース作業の前に、全履歴の秘密情報スキャンも実行します。

```powershell
gitleaks git --log-opts=--all --redact --verbose .
```

秘密情報の検査は、次の層で行います。

| 層 | 実装 | 範囲 |
| --- | --- | --- |
| 事前確認 | `git-guard` 相当の正規表現を使うグローバル `~/.git-hooks/pre-commit` | ステージ済み差分 |
| CI | `.github/workflows/gitleaks.yml` の Gitleaks GitHub Action | push と pull request の変更 |
| 手動の全履歴確認 | `gitleaks git --log-opts=--all --redact --verbose .` | git の全履歴 |

## `npm registry` への公開

GitHub Release には、`remotty.tgz` と `remotty-0.1.x.tgz` のような版数付きパッケージを添付します。
リリース用ワークフローは、GitHub Actions の secret に `NPM_TOKEN` がある時だけ、版数付きパッケージを `npm registry` へ公開します。

`remotty` パッケージを管理できる `npm` アカウントで token を作ります。
GitHub の **Settings > Secrets and variables > Actions > New repository secret** に保存してください。

```text
Name: NPM_TOKEN
Secret: npm token value
```

この secret がない場合でも、GitHub Release は作成されます。
その場合、`npm registry` への公開だけをスキップします。

保守者の手元から手動で公開する場合は、次を使います。

```powershell
npm publish .\release\remotty.tgz
```

どちらの公開方法も、`remotty` パッケージを管理できる `npm` アカウントだけで実行してください。

## 任意の手動スモーク

手動スモークは任意です。CI では動きません。
実行前に、`remotty` の設定を済ませてください。

1. `remotty telegram configure --config C:/path/to/custom-bridge.toml` で Telegram bot token を保存します。
2. `remotty telegram access-pair <code> --config C:/path/to/custom-bridge.toml` で自分の Telegram sender を allowlist に追加します。
3. `remotty telegram live-env-check --config C:/path/to/custom-bridge.toml` で実機スモークの入力を確認します。

手動スモークは、保存済みの token と pairing 済み sender を使います。
pairing 済み sender が1件なら、`chat_id` と `sender_id` は自動で決まります。
`LIVE_WORKSPACE` が未設定なら、`target/live-smoke-workspace` を使います。
その場合、`.remotty-live-smoke-ok` も自動で作ります。
実機環境の確認では、bot が polling で使える状態かも確認します。
webhook が残っていない場合は `polling-ready` と表示します。
webhook が残っている場合は `webhook-configured` と表示します。

`LIVE_*` は、既定値を上書きしたい時だけ設定してください。
秘密値をチャットへ貼らないでください。
端末全体のスクリーンショットも避けてください。

上書き用の環境変数:

- `LIVE_TELEGRAM_BOT_TOKEN`
- `LIVE_TELEGRAM_CHAT_ID`
- `LIVE_TELEGRAM_SENDER_ID`
- `LIVE_WORKSPACE`

任意の環境変数:

- `LIVE_CODEX_BIN`
- `LIVE_CODEX_PROFILE`
- `LIVE_TIMEOUT_SEC`
- `LIVE_APPROVAL_MODE`

まず環境の有無だけを確認します。

```powershell
remotty telegram live-env-check
```

既定ではない設定ファイルを使う場合:

```powershell
remotty telegram live-env-check --config C:/path/to/custom-bridge.toml
```

次に、承認して続ける経路を確認します。

```powershell
remotty telegram smoke approval accept --config C:/path/to/custom-bridge.toml
```

非承認で安全側へ止まる経路は、次です。

```powershell
remotty telegram smoke approval decline --config C:/path/to/custom-bridge.toml
```

できれば手動スモーク専用の bot、チャット、作業フォルダを使ってください。
polling 競合が出た場合は、同じ bot を読んでいる別の `remotty` を止めてから再実行してください。

## リポジトリレイアウト

```text
remotty/
├── src/                    # ブリッジ本体、Telegram 連携、Codex 実行
├── tests/                  # 設定、模擬 Telegram、公開面のテスト
├── scripts/                # 保守と検証の補助スクリプト
├── bridge.toml             # ローカル設定の土台
├── README.md               # 英語版 README
└── README.ja.md            # 日本語版 README
```
