# Telegram クイックスタート

このページは、`remotty` を Telegram bot につなぎ、スマホから手元の Codex へ依頼するための手順です。

`remotty` は Codex Remote connections の代替ではありません。Remote connections は、Codex App から SSH 先の開発環境を扱う機能です。`remotty` は、Windows PC 上で使える Codex CLI の作業環境へ Telegram から依頼を送るブリッジです。

`remotty` はローカルのブリッジプロセスとして動くため、Codex に特別な起動フラグを付ける必要はありません。ローカルプラグインは、`/remotty-*` の設定と操作コマンドを提供するために使います。

## 必要なもの

- Windows 10 または Windows 11
- Codex App と Codex CLI
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
$configDir = Join-Path $env:APPDATA "remotty"
New-Item -ItemType Directory -Force -Path $configDir | Out-Null
Copy-Item -Force .\bridge.toml (Join-Path $configDir "bridge.toml")
$configPath = Join-Path $configDir "bridge.toml"
```

この手順で `remotty` コマンドが入り、同じ版の Windows 用バイナリも取得されます。
`npm root -g` は、グローバル npm パッケージの保存先を返します。
次の2行で、同梱の `bridge.toml` を `Copy-Item` で読める場所へ移動します。
手順3では、同じフォルダを Codex App から開きます。
残りの行は、設定の土台を `%APPDATA%\remotty\bridge.toml` へコピーします。
設定と実行時の状態を、グローバル npm パッケージフォルダに置かないためです。

GitHub Release の tarball から直接入れたい場合は、次を使います。

```powershell
npm install -g https://github.com/Sora-bluesky/remotty/releases/latest/download/remotty.tgz
$remottyRoot = Join-Path (npm root -g) "remotty"
Set-Location $remottyRoot
$configDir = Join-Path $env:APPDATA "remotty"
New-Item -ItemType Directory -Force -Path $configDir | Out-Null
Copy-Item -Force .\bridge.toml (Join-Path $configDir "bridge.toml")
$configPath = Join-Path $configDir "bridge.toml"
```

## 2. Telegram bot を作る

1. Telegram で `@BotFather` を開きます。
2. `/newbot` を送ります。
3. 表示名を決めます。
4. `bot` で終わる一意の username を決めます。
5. BotFather が返した token を控えます。

token をチャット、スクリーンショット、issue、PR に貼らないでください。

## 3. token を設定する

Codex App で `remotty` のパッケージフォルダを開きます。
Plugins 画面で、ローカル marketplace の `.agents/plugins/marketplace.json` を追加します。
次に、`remotty` というプラグインを入れます。
Plugins 画面に `remotty` が表示されることを確認してから進んでください。
その後、これを実行します。

```text
/remotty-configure
```

表示に従って token を貼ります。このコマンドは token を再表示せず、Windows の保護領域へ保存します。

## 4. `bridge.toml` を編集する

最初の起動前に、`%APPDATA%\remotty\bridge.toml` の次の値を確認してください。

- `workspaces[0].path`: Codex を動かす作業フォルダ
- `workspaces[0].writable_roots`: Codex に編集を許可するフォルダ
- `codex.model`: `gpt-5.4` のまま使うか、手元の Codex CLI で使う model 名へ変える
- `codex.transport`: まずは `exec` のままで構いません。Telegram の承認ボタンを使う時は `app_server` にします

Windows のパスは、`C:/Users/you/Documents/project` のように `/` で書くと安全です。

プラグインのペアリングを使うなら、`telegram.admin_sender_ids` は空のままで構いません。

この設定内の相対 `state/` パスは、コピー先の `%APPDATA%\remotty` から解決されます。

## 5. ブリッジを起動する

次を実行します。

```text
/remotty-start
```

Telegram から使う間は、ブリッジを起動したままにしてください。止まっていると bot は返信できません。
前面起動のブリッジは、止めるまでその PowerShell を使い続けます。開いたままにし、ペアリングコマンドは Codex App か別の端末から実行してください。

状態確認:

```text
/remotty-status
```

停止:

```text
/remotty-stop
```

## 6. Telegram アカウントをペアリングする

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

bot が pairing code を返せない場合は、ブリッジを止めてから `/remotty-pair` を使ってください。

## 7. テストメッセージを送る

Telegram で、次のような小さな依頼を送ります。

```text
What files are in the current workspace?
```

`remotty` がメッセージを受け取り、手元の Codex CLI を動かし、同じ Telegram チャットへ返信します。

## 8. 手動スモークを実行する

手動スモークは任意です。実 Telegram bot とローカルの一時 workspace を使います。
スモークコマンドは一時的な `app_server` 実行を作ります。普段使う `codex.transport = "exec"` は変えなくて構いません。

まず入力を確認します。

```text
/remotty-live-env-check
```

別の設定ファイルを使う場合:

```powershell
remotty telegram live-env-check --config C:/path/to/custom-bridge.toml
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

Codex Remote connections は、Codex App から SSH 先の開発マシンへ接続する機能です。コードとシェルがリモートホスト上にある時に使います。

`remotty` は、Windows PC 上の Codex 作業環境へ Telegram から依頼を送り、同じチャットで返信を受け取るために使います。
