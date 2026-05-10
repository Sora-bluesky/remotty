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

ここでいう「Codex の主な作業画面」は、特定のスマホアプリ名を指しているわけではありません。
作業内容を読み、差分を確認し、まとまった指示を出すための画面全般を指しています。
`remotty` はその画面を置き換えません。

現在の公開手順では、`remotty` がつなぐ相手はローカルの `Codex CLI` セッションです。
Telegram bot へ送った文を `remotty` が Windows PC で受け取り、連携した `Codex CLI` セッションへ渡します。
返答は同じ Telegram チャットへ戻ります。

`remotty` は公式のリモートコントロール機能ではありません。
公開 webhook サーバも使いません。
開いている Codex App 画面へキー入力もしません。
ローカルの `codex` コマンドと `app_server` 接続を通じて Codex とやり取りします。

## remotty がやらないこと

`remotty` は「スマホ版 Codex App」や「Web IDE」ではありません。
強みは、外出中に必要な通知、承認、短い返答、追加入力だけを Telegram で返せることです。
つまり、フルなリモート作業環境を提供するのではなく、Codex が人間に確認したい場面だけを受け止める補助的な接点です。

位置づけとしては、次のように整理できます。

- [Codex Remote connections](https://developers.openai.com/codex/remote-connections): OpenAI 側の機能として、Codex が SSH 接続先やクラウド環境のプロジェクトで作業できるようにする仕組み
- スマホ向け Web アプリ: 開発者が別途用意する場合の、ブラウザから Codex を操作するフル UI
- `remotty`: Codex からの確認、承認、追加入力を Telegram で受ける軽量な連絡口

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

インストール、bot 作成、`remotty remote-control` の起動、ペアリング、最初のテストまで順に進められます。

Telegram bot を作る前に試す場合は、
[Fakechat デモ](docs/fakechat-demo.ja.md) を使えます。

## 更新方法

公開済みの最新版へ更新する時は、通常の PowerShell で実行します。

```powershell
npm install -g remotty
```

その後、利用したいプロジェクトで [Telegram クイックスタート](docs/telegram-quickstart.ja.md) に沿って進めます。

## 最初に知っておくこと

最短の手順では、プロジェクトで `Codex CLI` を起動し、別の PowerShell で
`remotty remote-control` を実行します。
初回だけ、`remotty` が `%APPDATA%\remotty\bridge.toml` を作り、現在のプロジェクトを登録し、Telegram bot token の入力を求めます。

| 画面 | 開いたままにするか | 使う場面 |
| --- | --- | --- |
| 通常の PowerShell | いいえ | `remotty` のインストールと、必要な時の Telegram ペアリング |
| Codex 用 PowerShell | はい | Telegram から続けたいプロジェクトで `codex` を起動 |
| Remote Control 用 PowerShell | はい | 同じプロジェクトで `remotty remote-control` を起動 |

具体的なコマンドは、[Telegram クイックスタート](docs/telegram-quickstart.ja.md) に沿って実行してください。

起動に成功すると、ターミナルに `Remote Control active` と
`Listening for Telegram channel messages from: remotty:telegram` と表示されます。
Telegram から使う間は、Remote Control 用 PowerShell を開いたままにしてください。

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
/answer <request_id> <value>
/workspace
/workspace <id>
```

`/answer` の詳しい使い方は、[Codex の追加入力](docs/telegram-quickstart.ja.md#codex-の追加入力) を参照してください。

## 安全な情報の扱い

- `remotty telegram configure` で bot token を保護領域へ保存する
- `remotty` 専用の Telegram bot を使う
- token をチャット、issue、PR へ貼らない
- token や `api.telegram.org/bot...` の URL を issue へ貼らない
- `remotty` は、Codex が秘密入力として示した要求を Telegram からは受け付けない。Telegram のメッセージはチャット履歴として扱い、パスワード、API キー、リカバリーコードなどの秘密情報を `/answer` で送らない
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
