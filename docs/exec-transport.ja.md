# 高度な CLI モード

通常は [Telegram クイックスタート](telegram-quickstart.ja.md) を使ってください。

このページは、Telegram の依頼ごとに別の Codex CLI 実行を始めたい人向けです。

## 使う場面

次の場合だけ使います。

- 選択済みの Codex スレッドを続ける必要がない
- 1つの依頼ごとに1つの実行でよい
- 手元の `codex` コマンドが `app-server` に対応していない

## 設定

`%APPDATA%\remotty\bridge.toml` を開きます。

次にします。

```toml
[codex]
transport = "exec"
```

`workspaces` の設定は、クイックスタートと同じです。

## 動き

このモードでは、`remotty` は Telegram の依頼に対して `codex exec` を呼びます。

結果は Telegram へ戻ります。
ただし、選択済みの Codex スレッドにはつながりません。

Codex スレッドを続けたい場合は、通常のクイックスタートを使ってください。
