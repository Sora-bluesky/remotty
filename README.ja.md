[English](README.md) | [日本語](README.ja.md)

# `remotty`

![remotty: Codex と Telegram をつなぐ Windows ブリッジ](docs/assets/hero.png)

`remotty` は、Telegram から手元の Codex 作業を続けるためのツールです。

Telegram bot へメッセージを送ります。
`remotty` が Windows PC で受け取り、選択した Codex スレッドへ渡します。
返答は同じ Telegram チャットへ戻ります。

`remotty` は公開 webhook サーバを使いません。
開いている Codex App 画面へキー入力もしません。
ローカルの `codex` コマンドを通じて Codex とやり取りします。

> [!WARNING]
> **免責**
>
> 本プロジェクトは、OpenAI の支援、承認、提携を受けていません。
> `Codex`、`ChatGPT`、関連する名称は OpenAI の商標です。
> ここでは、連携先のローカルツールを説明する目的でのみ使っています。
> その他の商標は、それぞれの権利者に帰属します。

## できること

- Windows PC 上の Codex と Telegram bot をつなぐ
- Telegram チャットから続けたい Codex スレッドを選ぶ
- Telegram のメッセージをそのスレッドへ渡す
- Codex の返答を同じ Telegram チャットへ返す
- 承認待ちを Telegram へ中継する
- bot token を Windows の保護領域へ保存する
- `remotty` の状態を `%APPDATA%\remotty` に置く

## 使う場面

席を離れている時に、Windows PC 上の Codex 作業を Telegram から続けたい場合に使います。

## 必要なもの

- Windows 10 または Windows 11
- Codex App と Codex CLI
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

その後、Codex App の Plugins 画面を開きます。
`remotty` に更新ボタンがあれば押します。
出ない場合は、ローカルプラグインを入れ直してください。
画面内のプラグイン元は `remotty local plugins` です。

## 主なコマンド

Codex App では、チャット欄で `@` を入力します。
候補から `remotty` を選び、次のように依頼します。

```text
Telegram bot token を保存して
このプロジェクトを remotty に登録して
ブリッジを起動して
Telegram に表示された pairing code でペアリングして
Telegram の allowlist を有効化して
状態を確認して
Codex スレッドを一覧して
```

bot token は、`remotty` が開く PowerShell にだけ入力します。
Codex App のチャット欄には貼らないでください。

Codex CLI を使う場合は、PowerShell から同じ設定を行えます。
どちらの場合も、ブリッジはローカルの `codex` 実行ファイルを呼びます。
PowerShell のコマンドは、クイックスタートに載せています。

Telegram で使います。

```text
/help
/status
/stop
/approve <request_id>
/deny <request_id>
/remotty-sessions <thread_id>
/workspace
/workspace <id>
```

## 安全な情報の扱い

- `@remotty` で bot token を保護領域へ保存する
- `remotty` 専用の Telegram bot を使う
- token を Codex App のチャット欄へ貼らない
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
