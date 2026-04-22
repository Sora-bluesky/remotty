# Fakechat デモ

Telegram bot を作る前にローカルの会話ループを試したい時は、`remotty demo fakechat` を使います。

このデモは `localhost` にブラウザ用のチャット画面を起動します。送ったメッセージをローカルの Codex CLI へ渡し、返答を同じ画面へ表示します。Telegram、bot token、webhook、DPAPI、公開サーバは使いません。

`--thread-id` を渡すと、`codex exec` ではなく `codex app-server` を使います。
Telegram の認証なしで、保存済みスレッドの再開を確認できます。

## 必要なもの

- Windows 10 または Windows 11
- `remotty`
- `PATH` に通った Codex CLI

## デモを起動する

Codex に見せたいフォルダで次を実行します。

```powershell
remotty demo fakechat
```

表示された URL を開きます。

```text
http://127.0.0.1:8787
```

小さな依頼を送ります。

```text
What files are in this workspace?
```

`remotty` は読み取り専用で `codex exec` を実行し、返答を画面へ戻します。

## オプション

別のポートを使う場合:

```powershell
remotty demo fakechat --port 8790
```

作業フォルダを指定する場合:

```powershell
remotty demo fakechat --workspace C:/Users/you/Documents/project
```

Codex CLI の実行ファイルや model を指定する場合:

```powershell
remotty demo fakechat --codex-binary codex --model <your-codex-model>
```

保存済み Codex スレッドを再開する場合:

```powershell
remotty demo fakechat --thread-id <codex-thread-id>
```

## Telegram との違い

Fakechat はローカル専用のデモです。`remotty` がローカルの Codex を呼び、チャット形式で返答できることを確認するために使います。

Telegram mode では、次の実運用向け機能が加わります。

- `@BotFather` を使った bot 作成
- アカウントの pairing と allowlist
- Telegram への承認メッセージ
- ブリッジ起動中にスマホから使える導線

デモが動いたら、次は [Telegram クイックスタート](telegram-quickstart.ja.md) に進んでください。
