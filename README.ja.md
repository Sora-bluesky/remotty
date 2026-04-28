[English](README.md) | [日本語](README.ja.md)

# `remotty`

![remotty: Codex と Telegram をつなぐ Windows ブリッジ](docs/assets/hero.png)

`remotty` は、汎用の遠隔操作ツールではありません。
Windows 上の Codex 作業を、普段使う Telegram から続けるためのブリッジです。

`remotty` は、Telegram から Codex 作業の続きを進められるようにします。
新しいモバイルアプリを入れる必要はありません。

Telegram bot へメッセージを送ります。
`remotty` が Windows PC で受け取り、選択した Codex スレッドへ渡します。
返答は同じ Telegram チャットへ戻ります。

`remotty` は公開 webhook サーバを使いません。
開いている Codex App 画面へキー入力もしません。
ローカルの `codex` コマンドを通じて Codex とやり取りします。

## できること

- Windows PC 上の Codex と Telegram bot をつなぐ
- Telegram チャットから続けたい Codex スレッドを選ぶ
- Telegram のメッセージをそのスレッドへ渡す
- Codex が処理中の間に送ったテキストを、次の入力としてキューに溜める
- Codex の返答を同じ Telegram チャットへ返す
- 承認待ちを Telegram へ中継する
- bot token を Windows の保護領域へ保存する
- `remotty` の状態を `%APPDATA%\remotty` に置く

## 使う場面

席を離れている時に、Windows PC 上の Codex 作業を Telegram から続けたい場合に使います。

## 必要なもの

- Windows 10 または Windows 11
- `Codex CLI`
- Node.js と `npm`
- `@BotFather` で作った Telegram bot token

ソースからビルドする場合だけ、Rust が必要です。

## はじめ方

[Telegram クイックスタート](docs/telegram-quickstart.ja.md) を使ってください。

インストール、bot 作成、token 保存、ペアリング、スレッド選択、最初のテストまで順に進められます。

Telegram bot を作る前に試す場合は、
[Fakechat デモ](docs/fakechat-demo.ja.md) を使えます。

## 更新方法

公開済みの最新版へ更新する時は、通常の PowerShell で実行します。

```powershell
npm install -g remotty
```

その後、利用したいプロジェクトで [Telegram クイックスタート](docs/telegram-quickstart.ja.md) に沿って進めます。

## 主なコマンド

PowerShell で実行します。

```powershell
$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"
remotty config workspace upsert --config $configPath --path (Get-Location).Path
remotty telegram configure --config $configPath
remotty --config $configPath
remotty telegram access-pair <code> --config $configPath
remotty telegram policy allowlist --config $configPath
remotty telegram sessions --config $configPath
```

`remotty --config $configPath` が成功すると、ターミナルに
`Listening for Telegram channel messages from: remotty:telegram` と表示されます。
Telegram から使う間は、そのターミナルを開いたままにしてください。

Codex App も使う場合は、同梱のプラグインで設定作業を補助できます。
プラグインは任意です。
サポート対象の Telegram 連携は、上記の `Codex CLI` と `remotty` の PowerShell コマンドを使うフローです。

Telegram から送るコマンドは次のとおりです。

```text
/help
/status
/stop
/approve <request_id>
/deny <request_id>
/remotty-sessions <スレッド名または ID>
/workspace
/workspace <id>
```

スレッド名に空白があっても、そのまま送れます。
引用符は不要です。
同じ名前が複数ある場合は、表示された `ID` を使います。
名前が別スレッドの `ID` に見える場合も、表示された `ID` を使います。

## 安全な情報の扱い

- `remotty telegram configure` で bot token を保護領域へ保存する
- `remotty` 専用の Telegram bot を使う
- token をチャット、issue、PR へ貼らない
- token や `api.telegram.org/bot...` の URL を issue へ貼らない
- プロジェクトファイルと `%APPDATA%\remotty` の状態を分ける

## 関連ドキュメント

- [Telegram クイックスタート](docs/telegram-quickstart.ja.md)
- [Fakechat デモ](docs/fakechat-demo.ja.md)
- [高度な CLI モード](docs/exec-transport.ja.md)
- [更新時の注意](docs/upgrading.ja.md)

補足: SSH 先のプロジェクトで作業する場合は、Codex Remote connections も選択肢です。
`remotty` は、Telegram から手元の Windows PC 上の Codex 作業へ戻るためのツールです。

## ライセンス

[MIT](LICENSE)

## 免責

本プロジェクトは、非公式のコミュニティプロジェクトです。
本プロジェクトは、OpenAI の支援、承認、提携を受けていません。

`Codex`、`ChatGPT`、関連する名称は OpenAI の商標です。
ここでは、連携先のローカルツールを説明する目的でのみ使っています。
その他の商標は、それぞれの権利者に帰属します。
