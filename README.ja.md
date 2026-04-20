[English](README.md) | [日本語](README.ja.md)

# `codex-channels`

`codex-channels` は、Telegram から手元の `codex` CLI を呼び出すための Windows 向けツールです。

スマホの Telegram で bot に指示を送ると、手元の Windows PC で `codex` が動き、結果が同じチャットへ返ります。公開サーバや webhook は不要で、bot token と会話履歴は PC 側で扱います。

## できること

- Telegram bot とローカルの Codex をつなぐ
- 会話の状態を `SQLite` に保存し、再開しやすくする
- 返信待ちモードと自動継続モードを切り替える
- bot token を `DPAPI` でローカル保護する
- 通常起動と Windows サービス起動の両方に対応する

## 向いている人

このプロジェクトは、次のような人に向いています。

- Windows で Telegram から Codex を動かしたい人
- 手元の PC で安全に運用したい人
- 外出先から手元の PC を少し操作したい人

現時点の初期設定はコマンドライン中心です。`PowerShell` を開けて、Telegram bot の作成ができる人なら進めやすい構成です。

## 必要なもの

- Windows 10 または Windows 11
- Rust の実行環境。`cargo` が使える状態にしてください
- `PATH` に通った `codex` CLI
- `@BotFather` で作った Telegram bot
- 利用を許可する自分の Telegram user ID

## はじめ方

### 1. リポジトリを取得する

```powershell
git clone https://github.com/Sora-bluesky/codex-channels.git
cd codex-channels
```

### 2. Telegram bot を作る

1. Telegram で `@BotFather` を開きます。
2. `/newbot` を送ります。
3. bot の名前と username を決めます。
4. 表示された bot token を控えます。

### 3. bot token をローカルに保存する

token は git 管理下のファイルではなく、Windows の保護領域へ保存します。最初の `cargo run` ではビルドに少し時間がかかることがあります。

```powershell
cargo run -- secret set codex-telegram-bot <YOUR_TELEGRAM_BOT_TOKEN>
```

### 4. `bridge.toml` を編集する

リポジトリには、最初の土台になる `bridge.toml` が入っています。

最初の起動前に、次の値を確認してください。

- `telegram.admin_sender_ids`: 自分の Telegram user ID
- `workspaces[0].path`: Codex を動かす作業フォルダ
- `workspaces[0].writable_roots`: Codex に編集を許可するフォルダ

Telegram user ID がまだ分からない場合は、先に bot へ 1 通送り、次で最新の `message.from.id` を確認してください。

```powershell
Invoke-RestMethod "https://api.telegram.org/bot<YOUR_TELEGRAM_BOT_TOKEN>/getUpdates" | ConvertTo-Json -Depth 8
```

名前付きの Codex profile をすでに使っている場合だけ、`codex.profile` を追加します。不要なら書かなくて構いません。追加しない場合は、ローカルの `codex` CLI の既定設定に従います。

### 5. ブリッジを起動する

```powershell
cargo run
```

起動に成功したら、そのままターミナルを開いたままにします。

### 6. Telegram で bot を開く

bot に `/help` を送ってください。ブリッジが起動していて、送信者が許可済みなら、使えるコマンドが返ります。

## よく使うコマンド

Telegram 側では次を使えます。

```text
/help
/status                  # 今の状態を見る
/stop                    # 動いている Codex を止める
/mode completion_checks  # 確認に失敗した時だけ続ける
/mode infinite           # Codex が自然に止まるまで続ける
/mode max_turns 3        # 最大 3 回まで自動で続ける
```

## 基本設定

主な設定ファイルは `bridge.toml` です。

### 重要な項目

- `service`: 起動モードと終了待ち時間
- `telegram`: 許可するチャット種別と送信者
- `codex`: 実行バイナリ、model、sandbox、承認方式、任意の profile
- `storage`: `SQLite`、状態、作業用一時領域、ログの保存先
- `policy`: 既定モードと出力上限
- `checks`: 実行後の任意確認コマンド
- `workspaces`: Codex を動かす場所と編集許可範囲

### 最小構成の例

この例は、最初に触る項目だけを抜き出したものです。残りの既定値は同梱の `bridge.toml` に入っています。

```toml
[service]
run_mode = "console"

[telegram]
token_secret_ref = "codex-telegram-bot"
allowed_chat_types = ["private"]
admin_sender_ids = [123456789]

[codex]
binary = "codex"
model = "<your-codex-model>"
sandbox = "workspace-write"
approval = "on-request"

[[workspaces]]
id = "main"
path = "C:/path/to/workspace"
writable_roots = ["C:/path/to/workspace"]
default_mode = "await_reply"
continue_prompt = "必要なら続けてください。"
checks_profile = "default"
```

## 安全な情報の扱い

- bot token はローカル保護領域か環境変数で扱ってください
- `cargo run -- secret set` を使わない場合は、`TELEGRAM_BOT_TOKEN` でも渡せます
- `LIVE_TELEGRAM_BOT_TOKEN` や `LIVE_WORKSPACE` の実値はコミットしないでください
- 実行時の状態ファイルは `.gitignore` で除外しています
- コミットと push の前に、秘密情報を点検するローカルフックの利用を勧めます

## Windows サービスとして動かす

バックグラウンドで常駐させたい場合は、次を使います。

インストール時は、管理者権限付きの `PowerShell` を開いてください。

```powershell
cargo run -- service install --config bridge.toml
cargo run -- service start
cargo run -- service status
```

停止や削除は次です。

```powershell
cargo run -- service stop
cargo run -- service uninstall
```

## 開発者向け情報

### 基本確認

```powershell
cargo fmt --check
cargo test
cargo check
```

### 任意の実機スモーク

実機スモークは任意です。CI では動きません。

必須の環境変数:

- `LIVE_TELEGRAM_BOT_TOKEN`
- `LIVE_TELEGRAM_CHAT_ID`
- `LIVE_TELEGRAM_SENDER_ID`
- `LIVE_WORKSPACE`

任意の環境変数:

- `LIVE_CODEX_BIN`
- `LIVE_CODEX_PROFILE`
- `LIVE_TIMEOUT_SEC`

実行コマンド:

```powershell
cargo test --features live-e2e --test live_end_to_end -- --ignored --nocapture
```

できれば実機スモーク専用の bot とチャットを使ってください。

## リポジトリレイアウト

```text
codex-channels/
├── src/                    # ブリッジ本体、Telegram 連携、Codex 実行
├── tests/                  # 設定、実機、公開面のテスト
├── scripts/                # 保守と検証の補助スクリプト
├── bridge.toml             # ローカル設定の土台
├── README.md               # 英語版 README
└── README.ja.md            # 日本語版 README
```

## ライセンス

[MIT](LICENSE)
