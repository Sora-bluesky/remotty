[English](README.md) | [日本語](README.ja.md)

# `remotty`

![remotty: Windows 上の Codex 向け Telegram ブリッジ](docs/assets/hero.png)

`remotty` は、Windows PC で動いている Codex に、Telegram から声をかけるための小さなブリッジです。

AI に作業を任せている時、いちばん困るのは「席を離れた瞬間に止まる」ことです。
承認待ちかもしれません。
エラーで止まっているかもしれません。
あと一言だけ伝えれば、続きが進む場面もあります。

`remotty` は、そうした小さな確認と追加入力を Telegram から行うためのツールです。
本格的な作業、差分の確認、長い指示は、Codex の主な作業画面、または `Codex CLI` の画面で行います。

この文書では、Codex の主な作業画面を Codex App と呼びます。
特定のスマホアプリ名を指しているわけではありません。
作業内容を読み、差分を確認し、まとまった指示を出すための Codex の主な画面を指しています。
`remotty` はその画面を置き換えません。
席を離れている間の確認と、短い追加入力を助けます。

現在の公開手順では、`remotty` がつなぐ相手はローカルの `Codex CLI` セッションです。
Telegram bot へ送った文を `remotty` が Windows PC で受け取り、連携した `Codex CLI` セッションへ渡します。
返答は同じ Telegram チャットへ戻ります。

`remotty` は公式のリモートコントロール機能ではありません。
公開 webhook サーバも使いません。
開いている Codex App 画面へキー入力もしません。
ローカルの `codex` コマンドと `app_server` 接続を通じて Codex とやり取りします。

## できること

- Windows PC 上の Codex と Telegram bot をつなぐ
- プロジェクトで起動した `Codex CLI` セッションを Telegram とつなぐ
- Telegram のメッセージをそのセッションへ渡す
- Codex が処理中の間に送ったテキストを、次の入力としてキューに溜める
- Codex の返答を同じ Telegram チャットへ返す
- 承認待ちを Telegram へ中継する
- bot token を Windows の保護領域へ保存する
- `remotty` の状態を `%APPDATA%\remotty` に置く

## 使う場面

席を離れている時に、Windows PC 上の Codex 作業を Telegram から見守りたい場合に使います。
承認待ちなら内容を確認し、必要なら短い追加入力を送ります。

## 必要なもの

- Windows 10 または Windows 11
- `Codex CLI`
- Node.js と `npm`
- `@BotFather` で作った Telegram bot token

ソースからビルドする場合だけ、Rust が必要です。

## はじめ方

[Telegram クイックスタート](docs/telegram-quickstart.ja.md) を使ってください。

インストール、bot 作成、token 保存、ペアリング、チャンネル起動、最初のテストまで順に進められます。

Telegram bot を作る前に試す場合は、
[Fakechat デモ](docs/fakechat-demo.ja.md) を使えます。

## 更新方法

公開済みの最新版へ更新する時は、通常の PowerShell で実行します。

```powershell
npm install -g remotty
```

その後、利用したいプロジェクトで [Telegram クイックスタート](docs/telegram-quickstart.ja.md) に沿って進めます。

## 最初に知っておくこと

設定コマンドを1つのスクリプトとしてまとめて貼らないでください。
Telegram 連携では、`codex` と `remotty` の両方を起動したまま使うため、
PowerShell の画面を分けます。

| 画面 | 開いたままにするか | 使う場面 |
| --- | --- | --- |
| 設定用 PowerShell | いいえ | `remotty` のインストール、プロジェクト登録、bot token の保存、Telegram のペアリング |
| Codex 用 PowerShell | はい | Telegram から続けたいプロジェクトで `codex` を起動 |
| ブリッジ用 PowerShell | はい | 同じプロジェクトで `remotty --config "$env:APPDATA\remotty\bridge.toml"` を起動 |

具体的なコマンドは、[Telegram クイックスタート](docs/telegram-quickstart.ja.md) に沿って実行してください。

ブリッジの起動に成功すると、ターミナルに
`Listening for Telegram channel messages from: remotty:telegram` と表示されます。
Telegram から使う間は、ブリッジ用 PowerShell を開いたままにしてください。

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
/workspace
/workspace <id>
```

## 安全な情報の扱い

- `remotty telegram configure` で bot token を保護領域へ保存する
- `remotty` 専用の Telegram bot を使う
- token をチャット、issue、PR へ貼らない
- token や `api.telegram.org/bot...` の URL を issue へ貼らない
- プロジェクトファイルと `%APPDATA%\remotty` の状態を分ける

## 関連ドキュメント

- [Telegram クイックスタート](docs/telegram-quickstart.ja.md)
- [Telegram ブリッジとしての方針](docs/remote-companion.ja.md)
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
