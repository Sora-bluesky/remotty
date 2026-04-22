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

設定ファイルの場所を変数に入れます。

```powershell
$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"
```

以降の PowerShell 例では、`$configPath` を使います。

## 入力場所と保存先

Codex App 用の `/remotty-...` コマンドは、Codex App のチャット欄へ入力します。
PowerShell へ入力するコマンドではありません。
Telegram へ入力する場合は、この手順内で明示します。

bot token は、プロジェクトのリポジトリへ保存しません。
Windows の保護領域へ保存します。
`remotty` の設定と状態は `%APPDATA%\remotty` 配下へ保存します。

`/remotty-use-this-project` は、対象プロジェクトを開いた状態で実行します。
`/remotty-configure` と `/remotty-start` は、リポジトリへ書き込みません。
ただし、迷わないために同じプロジェクトで続けて実行してください。
プロジェクトのルートに `remotty` 用のファイルは作りません。
そのため、通常はコミット対象物も増えません。

## 手順の分け方

同じ Windows ユーザーで初回だけ行う手順:

- `remotty` を入れる
- Codex App へローカルプラグインを入れる
- Telegram bot を用意する。すでにある場合は不要
- bot token を保存する
- Telegram アカウントをペアリングする

新しいプロジェクトを使う時だけ行う手順:

- 作業したいプロジェクトを開く
- そのプロジェクトを `remotty` へ登録する

Telegram チャットごとに行う手順:

- Telegram チャットへ Codex スレッドを対応付ける

使うたびに確認する手順:

- ブリッジが起動しているか確認する
- Telegram チャットが意図した Codex スレッドへ向いているか確認する
- Telegram からメッセージを送る

## 2. ローカルプラグインを入れる（初回だけ）

Codex App では、ローカルプラグインを使えます。

Plugins 画面で次を行います。

1. プラグイン元の選択欄で `remotty local plugins` を選びます。
2. 一覧の `remotty` で追加ボタンを押します。
3. インストール確認画面で確定します。

`remotty local plugins` を選ぶと、一覧に `remotty` が出ます。

![Codex のプラグイン一覧で remotty local plugins を選ぶ](assets/quickstart/codex-plugin-marketplace-select.png)

追加ボタンを押し、インストール確認を進めます。

![Codex の remotty プラグインインストール確認画面](assets/quickstart/codex-plugin-install-remotty.png)

Codex CLI だけで使う場合は、この手順を飛ばせます。
以降にある PowerShell のコマンドを使ってください。

## 3. 作業したいプロジェクトへ入る（新しいプロジェクトを使う時）

Telegram から続けたいプロジェクトを使います。
毎回同じプロジェクトを使う必要はありません。

Codex App を使う場合は、そのプロジェクトを App で開きます。

Codex CLI を使う場合は、PowerShell でフォルダへ入ります。

```powershell
Set-Location C:\path\to\your\project
```

## 4. このプロジェクトを登録する（同じプロジェクトでは初回だけ）

Codex App では、チャット欄へ次を入力します。
このコマンドだけは、対象プロジェクトを開いた状態で実行します。

```text
/remotty-use-this-project
```

Codex CLI では、プロジェクトフォルダで次を実行します。

```powershell
remotty config workspace upsert --config $configPath --path (Get-Location).Path
```

この操作は、プロジェクトを `%APPDATA%\remotty` の設定へ保存します。
プロジェクトのリポジトリには書き込みません。
プロジェクトのルートにも、`.remotty` などのファイルは作りません。
コミット対象物が増えないことを確認したい場合は、`git status` を見てください。

## 5. Telegram bot を用意する（初回だけ）

この手順は初回だけです。
すでに `remotty` 用の bot がある場合は、その token を使います。

新しく作る場合だけ、次を行います。

1. Telegram で `@BotFather` を開きます。
2. `/newbot` を送ります。
3. 表示名を決めます。
4. `bot` で終わる username を決めます。
5. BotFather が返した token を控えます。

token をチャット、スクリーンショット、issue、PR に貼らないでください。

## 6. bot token を保存する（初回だけ／token 変更時）

Codex App では、チャット欄へ次を入力します。
この操作は、今開いているリポジトリへ token を保存しません。
一度保存すれば、同じ Windows ユーザーでは再利用できます。
token を変える時だけ、もう一度実行します。

```text
/remotty-configure
```

Codex CLI では、次を実行します。

```powershell
remotty telegram configure --config $configPath
```

表示に従って token を貼ります。
このコマンドは token を再表示せず、Windows の保護領域へ保存します。
保存先は Windows ユーザーごとの保護領域です。
プロジェクトを変えても、同じ Windows ユーザーなら同じ保存先を使います。

## 7. ブリッジを起動する（使うたび）

Codex App では、チャット欄へ次を入力します。
起動時は `%APPDATA%\remotty\bridge.toml` の設定を使います。
今開いているリポジトリへ状態ファイルは置きません。

```text
/remotty-start
```

Codex CLI では、次を実行します。

```powershell
# フォアグラウンドでブリッジを起動します。
remotty --config $configPath
```

Telegram から使う間は、ブリッジを起動したままにします。
止まっていると bot は返信できません。

## 8. Telegram をペアリングする（初回だけ）

Telegram の private chat で、bot へ任意のメッセージを送ります。

bot は `remotty pairing code` を返します。
Codex App では、次を実行します。

```text
/remotty-access-pair <code>
```

Codex CLI では、次を実行します。

```powershell
remotty telegram access-pair <code> --config $configPath
```

次に、送信者を許可します。

```text
/remotty-policy-allowlist
```

Codex CLI では、次を実行します。

```powershell
remotty telegram policy allowlist --config $configPath
```

これで、他の Telegram ユーザーが手元の Codex を操作できなくなります。

## 9. Codex スレッドを選ぶ（Telegram チャットごと）

Codex App では、次を実行します。

```text
/remotty-sessions
```

Codex CLI では、次を実行します。

```powershell
remotty telegram sessions --config $configPath
```

Telegram から続けたいスレッドを選びます。
対象の Telegram チャットで次を送ります。

```text
/remotty-sessions <thread_id>
```

対応付けは `%APPDATA%\remotty` へ保存します。
プロジェクトのリポジトリには書き込みません。

## 10. テストメッセージを送る

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

## Q&A

### 安全性の Q&A

> Q. `/remotty-use-this-project` は、プロジェクトにファイルを作りますか?
>
> A. 作りません。設定は `%APPDATA%\remotty\bridge.toml` へ保存します。プロジェクトのルートに `.remotty` などは作りません。気になる場合は、実行後に `git status` を確認してください。

> Q. bot token はどこへ保存されますか?
>
> A. Windows の保護領域へ保存します。プロジェクトのリポジトリ、GitHub、Telegram のチャットへは保存しません。

> Q. bot token は OpenAI や外部サーバへ送られますか?
>
> A. `remotty` は、Telegram API へ接続するために token を使います。OpenAI へ token を送る必要はありません。issue、PR、スクリーンショットには token を貼らないでください。

> Q. 公開 webhook サーバを立てますか?
>
> A. 立てません。通常は Windows PC から Telegram を polling します。ルーターのポート開放も不要です。

> Q. 誰でも私の Codex を操作できますか?
>
> A. できません。ペアリングした送信者だけを許可します。ペアリング後は `/remotty-policy-allowlist` を実行してください。
> これにより、設定済みの送信者だけが操作できます。

> Q. Telegram から承認操作を押しても安全ですか?
>
> A. 承認できる人は、許可済み送信者だけです。ただし、承認は手元の Codex 作業を進めます。信頼できる自分のアカウントだけを許可してください。

> Q. 複数プロジェクトで同じ bot を使えますか?
>
> A. 使えます。bot token は Windows ユーザーごとに保存します。プロジェクトごとの登録と、Telegram チャットごとのスレッド対応付けは別です。

> Q. token が漏れたかもしれない時は?
>
> A. Telegram の `@BotFather` で token を再発行してください。その後、`/remotty-configure` で新しい token を保存します。

### 接続の Q&A

> Q. bot が返信しません。
>
> A. まず `/remotty-start` が動いているか確認します。Codex App では `/remotty-status` と `/remotty-live-env-check` を実行します。PowerShell では `remotty service status` と `remotty telegram live-env-check --config $configPath` を実行します。webhook 状態が `webhook-configured` なら polling へ戻します。

> Q. polling 競合が出ます。
>
> A. 同じ Telegram bot を polling できるプロセスは1つだけです。Windows では次のコマンドで候補を確認できます。

```powershell
Get-Process remotty, codex -ErrorAction SilentlyContinue | Select-Object Id,ProcessName,Path
```

> 同じ bot を読んでいるプロセスを止めます。

```powershell
Stop-Process -Id <PID>
```

### ペアリングの Q&A

> Q. pairing code が通りません。
>
> A. bot との private chat で送ってください。最新の code を使います。期限切れ前に `/remotty-access-pair <code>` を実行してください。

### スレッド選択の Q&A

> Q. Codex スレッドが出ません。
>
> A. Codex CLI を更新してから、もう一度試します。Codex App か Codex CLI でスレッドを作ります。その後、もう一度 `/remotty-sessions` を実行します。

## 関連

- [Fakechat デモ](fakechat-demo.ja.md)
- [高度な CLI モード](exec-transport.ja.md)
- [更新時の注意](upgrading.ja.md)

補足: コードとシェルが SSH 先にある場合は、
Codex Remote connections も選択肢です。
`remotty` は、Telegram から手元の Windows PC 上の Codex 作業へ戻るためのツールです。
