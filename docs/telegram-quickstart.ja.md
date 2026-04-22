# Telegram クイックスタート

このページは、`remotty` を Telegram bot につなぎ、スマホから手元の Codex へ依頼するための手順です。

`remotty` は Codex Remote connections の代替ではありません。Remote connections は、Codex アプリから SSH 先の開発環境を扱う機能です。`remotty` は、Windows PC 上で使える Codex の作業環境へ Telegram から依頼を送るブリッジです。

`remotty` は Claude Code Channels とも仕組みが違います。Channels は channel plugin と `--channels` 付き起動が必要です。`remotty` はローカルのブリッジプロセスとして動くため、Codex を channel 用のフラグ付きで起動する必要はありません。ローカル `plugin` は、`/remotty-*` の設定と操作コマンドを提供するために使います。

## 必要なもの

- Windows 10 または Windows 11
- Codex アプリと `codex` CLI
- Node.js と `npm`
- Telegram
- `@BotFather` で作った Telegram bot token

できれば `remotty` 専用の bot を使ってください。

## 1. `remotty` を入れる

`npm` から入れます。

```powershell
npm install -g remotty
$remottyRoot = Join-Path (npm root -g) "remotty"
Set-Location $remottyRoot
```

この手順で `remotty` コマンドが入り、同じ版の Windows 用バイナリも取得されます。

GitHub Release の tarball から直接入れたい場合は、次を使います。

```powershell
npm install -g https://github.com/Sora-bluesky/remotty/releases/latest/download/remotty.tgz
```

## 2. Telegram bot を作る

1. Telegram で `@BotFather` を開きます。
2. `/newbot` を送ります。
3. 表示名を決めます。
4. `bot` で終わる一意の username を決めます。
5. BotFather が返した token を控えます。

token をチャット、スクリーンショット、issue、PR に貼らないでください。

## 3. token を設定する

Codex で `remotty` のパッケージフォルダを開き、ローカル `plugin` を有効化します。次に、これを実行します。

```text
/remotty-configure
```

表示に従って token を貼ります。このコマンドは token を再表示せず、Windows の保護領域へ保存します。

## 4. ブリッジを起動する

次を実行します。

```text
/remotty-start
```

Telegram から使う間は、ブリッジを起動したままにしてください。止まっていると bot は返信できません。

状態確認:

```text
/remotty-status
```

停止:

```text
/remotty-stop
```

## 5. Telegram アカウントを pairing する

許可したい Telegram アカウントから、bot へ任意のメッセージを送ります。

bot は `remotty pairing code` を返します。Codex で次を実行します。

```text
/remotty-access-pair <code>
```

次に allowlist を確認します。

```text
/remotty-policy-allowlist
```

allowlist に入った送信者だけが、通常メッセージと承認操作を送れます。

## 6. テストメッセージを送る

Telegram で、次のような小さな依頼を送ります。

```text
What files are in the current workspace?
```

`remotty` がメッセージを受け取り、手元の `codex` を動かし、同じ Telegram チャットへ返信します。

## 7. 手動スモークを実行する

手動スモークは任意です。実 Telegram bot とローカルの一時 workspace を使います。

まず入力を確認します。

```text
/remotty-live-env-check
```

別の設定ファイルを使う場合:

```powershell
remotty telegram live-env-check --config bridge.local.toml
```

webhook 行が `polling-ready` なら実行できます。`webhook-configured` の場合は、先に bot を polling へ戻してください。

承認する経路:

```text
/remotty-smoke-approval-accept
```

非承認にする経路:

```text
/remotty-smoke-approval-decline
```

端末の案内に従い、Telegram に承認ボタンが出たら押してください。

## 困った時

### bot が返信しない

- `/remotty-start` が動いているか確認します。
- `/remotty-status` を実行します。
- `/remotty-live-env-check` を実行します。
- webhook 状態が `webhook-configured` なら、bot を polling へ戻します。

### pairing code が通らない

- bot との private chat で送ってください。
- 最新の code を使ってください。
- code が期限切れになる前に `/remotty-access-pair <code>` を実行してください。

### polling 競合が出る

同じ Telegram bot を polling できるプロセスは1つだけです。

Windows では候補を確認できます。

```powershell
Get-Process remotty, codex -ErrorAction SilentlyContinue | Select-Object Id,ProcessName,Path
```

同じ bot を読んでいるプロセスを止めてから再実行してください。

```powershell
Stop-Process -Id <PID>
```

### Codex Remote connections との違い

Codex Remote connections は、Codex アプリから SSH 先の開発マシンへ接続する機能です。コードとシェルがリモートホスト上にある時に使います。

`remotty` は、Windows PC 上の Codex 作業環境へ Telegram から依頼を送り、同じチャットで返信を受け取るために使います。
