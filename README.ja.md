[English](README.md) | [日本語](README.ja.md)

# `remotty`

![remotty: Codex と Telegram をつなぐ Windows ブリッジ](docs/assets/hero.png)

`remotty` は、Telegram から手元の Codex を呼び出すための Windows 向けツールです。

スマホの Telegram で bot に指示を送ると、手元の Windows PC で Codex CLI が動き、結果が同じチャットへ返ります。公開サーバや webhook は不要で、bot token と会話履歴は PC 側で扱います。

> [!WARNING]
> **免責**
>
> 本プロジェクトは、OpenAI による支援、承認、提携を受けていない非公式のコミュニティツールです。
> `Codex`、`ChatGPT`、および関連する名称は OpenAI の商標です。
> ここでは、このツールが連携する CLI やアプリを説明する目的でのみ言及しています。
> その他の商標は、それぞれの権利者に帰属します。

## できること

- Telegram bot とローカルの Codex をつなぐ
- 会話の状態を SQLite に保存し、再開しやすくする
- 返信待ちモードと自動継続モードを切り替える
- Codex が確認を求めた時は、Telegram へ承認待ちを返す
- bot token を `DPAPI` でローカル保護する
- 通常起動と Windows サービス起動の両方に対応する

## 向いている人

このプロジェクトは、次のような人に向いています。

- Windows で Telegram から Codex を動かしたい人
- 手元の PC で安全に運用したい人
- 外出先から手元の PC を少し操作したい人

`PowerShell` を開けて、Telegram bot の作成ができる人なら進めやすい構成です。

## プラグインで設定する

プラグインから、次を 1 か所で進められます。

- bot token を画面へ出さずに保存する
- bot が返した code で Telegram アカウントをペアリングする
- ブリッジの起動、停止、状態確認をまとめる

Codex App でインストール済みの `remotty` パッケージフォルダを開き、ローカルプラグインを有効化してください。これで `/remotty-*` コマンドが使えます。Codex に特別な起動フラグを付ける必要はありません。Telegram ブリッジは別のローカルプロセスとして動き、手元の Codex CLI を呼び出します。

## 必要なもの

- Windows 10 または Windows 11
- ローカルプラグインを使うための Codex App
- パッケージ版を入れるための Node.js と `npm`
- `PATH` に通った Codex CLI
- `@BotFather` で作った Telegram bot

ソースからビルドする場合だけ、Rust の実行環境と `cargo` が必要です。

## はじめ方

Telegram bot を作る前にローカルの会話ループを試したい場合は、[Fakechat デモ](docs/fakechat-demo.ja.md) を使ってください。`localhost` だけで動き、token は不要です。

Telegram の設定を一本道で進めたい場合は、専用の [Telegram クイックスタート](docs/telegram-quickstart.ja.md) を使ってください。
`remotty` と Codex Remote connections の違いも説明しています。

### 1. `remotty` を入れる

- **1. `npm` からインストールします。**

```powershell
npm install -g remotty
```

このコマンドで `remotty` が入り、同じ版の Windows 用バイナリも取得されます。

- **2. インストール先のフォルダへ移動します。**

```powershell
$remottyRoot = Join-Path (npm root -g) "remotty"
Set-Location $remottyRoot
```

あとでローカルプラグインを入れる時に、この `$remottyRoot` フォルダを Codex App から開きます。

- **3. 設定ファイルをユーザー用の設定フォルダへコピーします。**

```powershell
$configDir = Join-Path $env:APPDATA "remotty"
New-Item -ItemType Directory -Force -Path $configDir | Out-Null
Copy-Item -Force .\bridge.toml (Join-Path $configDir "bridge.toml")
$configPath = Join-Path $configDir "bridge.toml"
```

設定と実行時の状態は、グローバル npm パッケージフォルダではなく `%APPDATA%\remotty` に置かれます。

ソースから作業したい場合は、[開発者向け情報](docs/development.ja.md) を参照してください。

### 2. Telegram bot を作る

1. Telegram で `@BotFather` を開きます。
2. `/newbot` を送ります。
3. bot の名前と username を決めます。
4. 表示された bot token を控えます。

### 3. ローカルのプラグインを入れる

Codex App で `remotty` のパッケージフォルダを開きます。
Plugins 画面で、ローカル marketplace の `.agents/plugins/marketplace.json` を追加します。
次に、`remotty` というプラグインを入れてください。
Plugins 画面に `remotty` が表示されることを確認してから進んでください。

インストール済みパッケージには、次が同梱されています。

- `.agents/plugins/marketplace.json`
- `plugins/remotty/.codex-plugin/plugin.json`

### 4. bot token を設定する

次のプラグインコマンドを使います。

```text
/remotty-configure
```

このコマンドは bot token を端末へ再表示せず、Windows の保護領域へ保存します。

### 5. `bridge.toml` を編集する

コピーした `%APPDATA%\remotty\bridge.toml` を編集します。

最初の起動前に、次の値を確認してください。

- `workspaces[0].path`: Codex を動かす作業フォルダ
- `workspaces[0].writable_roots`: Codex に編集を許可するフォルダ
- `codex.model`: `gpt-5.4` のまま使うか、手元の Codex CLI で使う model 名へ変える
- `codex.transport`: まずは `exec` のままで構いません。Telegram の承認ボタンを使う時は `app_server` にします

Windows のパスは、`C:/Users/you/Documents/project` のように `/` で書くと安全です。

プラグインのペアリングを使うなら、`telegram.admin_sender_ids` は空のままで構いません。許可済み sender は SQLite 側へ保存します。

名前付きの Codex profile をすでに使っている場合だけ、`codex.profile` を追加します。不要なら書かなくて構いません。追加しない場合は、ローカルの `codex` CLI の既定設定に従います。

この設定内の相対 `state/` パスは、コピー先の `%APPDATA%\remotty` から解決されます。

### 6. ブリッジを起動する

```text
/remotty-start
```

CLI で直接起動する場合は、`remotty --config $configPath` を使います。
前面起動のブリッジは、止めるまでその PowerShell を使い続けます。開いたままにし、ペアリングコマンドは Codex App か別の端末から実行してください。
あとで新しい PowerShell を開いた場合は、先に変数を定義し直してください。

```powershell
$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"
remotty --config $configPath
```

### 7. 自分の Telegram をペアリングする

ブリッジを起動したままにします。
次に、許可したい Telegram アカウントから bot へ任意のメッセージを送ります。
bot は `remotty pairing code` を返信します。

その code を使って、Codex 側で次を実行します。

```text
/remotty-access-pair <code>
```

コマンドは対象の `sender_id` と `chat_id` を表示します。
その sender をローカル allowlist へ追加します。

ブリッジから code が返らない場合は、起動中のブリッジを止めてから `/remotty-pair` を使います。
この古い pairing 経路では、ローカル端末に表示された code を Telegram で `/pair <code>` として送ります。

### 8. allowlist を確認する

次を実行します。

```text
/remotty-policy-allowlist
```

これで、通常メッセージと承認操作を送れる sender を確認できます。

### 9. Telegram で bot を開く

bot に `/help` を送ってください。ブリッジが起動していて、送信者が許可済みなら、使えるコマンドが返ります。

## よく使うコマンド

Telegram 側では次を使えます。

```text
/help
/status                  # 今の状態を見る
/stop                    # 動いている Codex を止める
/approve <request_id>    # 承認待ちを文字コマンドで承認する
/deny <request_id>       # 承認待ちを文字コマンドで非承認にする
/workspace               # 現在の workspace と一覧を見る
/workspace docs          # この会話の workspace を切り替える
/mode completion_checks  # 確認に失敗した時だけ続ける
/mode infinite           # Codex が自然に止まるまで続ける
/mode max_turns 3        # 最大 3 回まで自動で続ける
```

`codex.transport = "app_server"` にすると、承認待ちは Telegram のボタンと文字コマンドの両方で処理できます。

## 承認フロー

Telegram だけで承認を返したい場合は、`codex.transport = "app_server"` を使います。

流れは次です。

1. 承認が必要な依頼を送る
2. Telegram に承認待ちメッセージが届く
3. `Approve` か `Deny` を押す
4. 同じ Codex の処理が Windows 側で続く

従来どおり `exec` 経路だけで動かしたい場合は、`codex.transport = "exec"` のままで構いません。

## 基本設定

主な設定ファイルは `bridge.toml` です。

### 重要な項目

- `service`: 起動モードと終了待ち時間
- `telegram`: 許可するチャット種別と送信者
- `codex`: 実行バイナリ、model、sandbox、承認方式、transport、任意の profile
- `storage`: SQLite、状態、作業用一時領域、ログの保存先
- `policy`: 既定モードと出力上限
- `checks`: 実行後の任意確認コマンド
- `workspaces`: Codex を動かす場所と編集許可範囲

### 最小構成の例

この例は、最初に触る項目だけを抜き出したものです。残りの既定値は同梱の `bridge.toml` に入っています。

```toml
[service]
run_mode = "console"
poll_timeout_sec = 30
shutdown_grace_sec = 15

[telegram]
token_secret_ref = "remotty-telegram-bot"
allowed_chat_types = ["private"]
admin_sender_ids = []

[codex]
binary = "codex"
model = "gpt-5.4"
sandbox = "workspace-write"
approval = "on-request"
transport = "exec"

[storage]
db_path = "state/bridge.db"
state_dir = "state"
temp_dir = "state/tmp"
log_dir = "state/logs"

[policy]
default_mode = "await_reply"
progress_edit_interval_ms = 5000
max_output_chars = 12000
max_turns_limit = 3

[[workspaces]]
id = "main"
path = "C:/path/to/workspace"
writable_roots = ["C:/path/to/workspace"]
default_mode = "await_reply"
continue_prompt = "Continue with the needed checks. If you must stop, reply with the short reason."
checks_profile = "default"
```

## 安全な情報の扱い

- 普段の利用では `/remotty-configure` を使い、bot token を Windows の保護領域に保存してください
- `TELEGRAM_BOT_TOKEN` は、CI や短時間の確認で DPAPI を使いにくい場合だけ使ってください
- bot token を含む URL、`api.telegram.org/bot...` の文字列、端末全体のスクリーンショットは貼らないでください

## CLI コマンド

`npm` で入る `remotty` コマンドが、パッケージ版の CLI です。
プラグインの裏では、次の CLI を呼びます。
新しい PowerShell に貼る場合は、先に設定ファイルの場所を定義してください。

```powershell
$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"
```

- `/remotty-configure` -> `remotty telegram configure --config $configPath`
- `/remotty-access-pair <code>` -> `remotty telegram access-pair <code> --config $configPath`
- `/remotty-pair` -> `remotty telegram pair --config $configPath`
- `/remotty-policy-allowlist` -> `remotty telegram policy allowlist --config $configPath`
- `/remotty-start` -> `remotty --config $configPath`
- `/remotty-stop` -> サービスとして動いている場合は `remotty service stop`。前面起動の場合は、その端末で停止する
- `/remotty-status` -> `remotty service status`。別端末の前面起動ブリッジではなく、Windows サービスの状態を見る
- `/remotty-fakechat-demo` -> `remotty demo fakechat`
- `/remotty-live-env-check` -> `remotty telegram live-env-check`
- `/remotty-smoke-approval-accept` -> `remotty telegram smoke approval accept --config $configPath`
- `/remotty-smoke-approval-decline` -> `remotty telegram smoke approval decline --config $configPath`

設定ファイルを既定以外の場所へ置く場合は、CLI にも同じ `--config <path>` を付けてください。

## Windows サービスとして動かす

バックグラウンドで常駐させたい場合は、次を使います。

インストール時は、管理者権限付きの `PowerShell` を開いてください。

```powershell
$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"
remotty service install --config $configPath
remotty service start
remotty service status
```

停止や削除は次です。

```powershell
remotty service stop
remotty service uninstall
```

## 関連ドキュメント

- [Telegram クイックスタート](docs/telegram-quickstart.ja.md)
- [Fakechat デモ](docs/fakechat-demo.ja.md)
- [開発者向け情報](docs/development.ja.md)

## ライセンス

[MIT](LICENSE)
