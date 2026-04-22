# Telegram クイックスタート

この手順では、Telegram から Windows PC 上の Codex スレッドへ依頼できるようにします。

## 仕組み

1. Windows PC で `remotty` を起動します。
2. Telegram bot へメッセージを送ります。
3. `remotty` が、選択済みの Codex スレッドへ文を渡します。
4. Codex が返答し、`remotty` が Telegram へ戻します。

## 必要なもの

- Windows 10 または Windows 11
- Codex App と Codex CLI
- Node.js と `npm`
- Telegram
- `@BotFather` で作った専用 bot

## 1. `remotty` を入れる

PowerShell で実行します。

```powershell
npm install -g remotty
```

インストール先のフォルダを開きます。

```powershell
$remottyRoot = Join-Path (npm root -g) "remotty"
Set-Location $remottyRoot
```

設定ファイルをコピーします。

```powershell
$configDir = Join-Path $env:APPDATA "remotty"
New-Item -ItemType Directory -Force -Path $configDir | Out-Null
Copy-Item -Force .\bridge.toml (Join-Path $configDir "bridge.toml")
$configPath = Join-Path $configDir "bridge.toml"
```

## 2. プロジェクトフォルダを指定する

`%APPDATA%\remotty\bridge.toml` を開きます。

次の2行を、Codex に作業させたいプロジェクトへ変えます。

```toml
path = "C:/Users/you/Documents/project"
writable_roots = ["C:/Users/you/Documents/project"]
```

Windows のパスは `/` で書くと安全です。

通常の設定では、ほかの項目を変更する必要はありません。
同梱の設定は、Codex スレッドへ渡す形になっています。

## 3. Telegram bot を作る

1. Telegram で `@BotFather` を開きます。
2. `/newbot` を送ります。
3. 表示名を決めます。
4. `bot` で終わる username を決めます。
5. BotFather が返した token を控えます。

token をチャット、スクリーンショット、issue、PR に貼らないでください。

## 4. ローカルプラグインを入れる

Codex App で `remotty` のパッケージフォルダを開きます。

Plugins 画面で次を行います。

1. `.agents/plugins/marketplace.json` を追加します。
2. `remotty` というプラグインを入れます。
3. Plugins 画面に `remotty` が表示されることを確認します。

## 5. bot token を保存する

Codex App で実行します。

```text
/remotty-configure
```

表示に従って token を貼ります。
このコマンドは token を再表示せず、Windows の保護領域へ保存します。

## 6. ブリッジを起動する

Codex App で実行します。

```text
/remotty-start
```

Telegram から使う間は、ブリッジを起動したままにします。
止まっていると bot は返信できません。

## 7. Telegram をペアリングする

Telegram の private chat で、bot へ任意のメッセージを送ります。

bot は `remotty pairing code` を返します。
Codex App で実行します。

```text
/remotty-access-pair <code>
```

次に、送信者を許可します。

```text
/remotty-policy-allowlist
```

これで、他の Telegram ユーザーが手元の Codex を操作できなくなります。

## 8. Codex スレッドを選ぶ

Codex App で実行します。

```text
/remotty-sessions
```

Telegram から続けたいスレッドを選びます。
この Telegram チャットへ対応付けます。

```text
/remotty-sessions <thread_id>
```

対応付けは `%APPDATA%\remotty` へ保存します。
プロジェクトのリポジトリには書き込みません。

## 9. テストメッセージを送る

Telegram で次を送ります。

```text
Summarize the current thread and suggest the next step.
```

`remotty` は選択済みスレッドへ文を渡します。
返答は Telegram に表示されます。

## 承認待ち

Codex が承認を求めると、`remotty` は Telegram へ中継します。

`Approve` または `Deny` を押せます。
文字コマンドも使えます。

```text
/approve <request_id>
/deny <request_id>
```

承認結果は同じ Codex の処理へ返ります。

## 困った時

### bot が返信しない

- `/remotty-start` が動いているか確認します。
- Codex App で `/remotty-status` を実行します。
- Codex App で `/remotty-live-env-check` を実行します。
- webhook 状態が `webhook-configured` なら polling へ戻します。

### Codex スレッドが出ない

- Codex CLI を更新してから、もう一度試します。
- Codex App か Codex CLI でスレッドを作ります。
- もう一度 `/remotty-sessions` を実行します。

### pairing code が通らない

- bot との private chat で送ってください。
- 最新の code を使ってください。
- 期限切れ前に `/remotty-access-pair <code>` を実行してください。

### polling 競合が出る

同じ Telegram bot を polling できるプロセスは1つだけです。

Windows では候補を確認できます。

```powershell
Get-Process remotty, codex -ErrorAction SilentlyContinue | Select-Object Id,ProcessName,Path
```

同じ bot を読んでいるプロセスを止めます。

```powershell
Stop-Process -Id <PID>
```

## 関連

- [Fakechat デモ](fakechat-demo.ja.md)
- [高度な CLI モード](exec-transport.ja.md)
- [更新時の注意](upgrading.ja.md)

補足: コードとシェルが SSH 先にある場合は、
Codex Remote connections も選択肢です。
`remotty` は、Telegram から手元の Windows PC 上の Codex 作業へ戻るためのツールです。
