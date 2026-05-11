# 更新時の注意

すでに `remotty` の設定がある場合に見てください。

新規に入れる場合は、[Telegram クイックスタート](telegram-quickstart.ja.md) を使ってください。

## 推奨設定

`%APPDATA%\remotty\bridge.toml` を開きます。

通常の Telegram 連携では、次を使います。

```toml
[codex]
transport = "app_server"
```

これで、そのプロジェクトで起動した `remotty remote-control` のプロセスを通じて、
Telegram から `Codex CLI` セッションを続けられます。
`remotty` は、Telegram bot token を保存した Windows ユーザーで起動してください。

## 設定が `exec` の場合

`exec` も使えます。
これは Telegram の依頼ごとに別の Codex CLI 実行を始めます。

その動きが必要なら、次のままにします。

```toml
[codex]
transport = "exec"
```

詳しくは [高度な CLI モード](exec-transport.ja.md) を見てください。

## 実行時ファイル

`remotty` は自分の状態を `%APPDATA%\remotty` に保存します。

プロジェクトのリポジトリへ、`remotty` の実行時ファイルは置きません。
ただし、依頼内容によっては、Codex 自体がプロジェクトを編集することがあります。
