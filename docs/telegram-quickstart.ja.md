# Telegram クイックスタート

この手順では、Windows の `Codex CLI` と Telegram をつなぎます。
`remotty` は Codex App の画面へ入力しません。
ローカルの `codex` コマンドを通じて Codex とやり取りします。

## 仕組み

1. 作業したいプロジェクトで `Codex CLI` を起動します。
2. 同じ Windows ユーザーで `remotty` を起動します。
3. `remotty` がチャンネル型の起動表示を出します。
4. Telegram bot へメッセージを送ります。
5. `remotty` が選択済みの Codex スレッドへ文を渡します。
6. Codex が返答し、`remotty` が Telegram へ戻します。

起動に成功すると、`remotty` のターミナルに次の表示が出ます。

```text
Listening for Telegram channel messages from: remotty:telegram
```

Telegram から使う間は、このターミナルを開いたままにします。

## 必要なもの

- Windows 10 または Windows 11
- `Codex CLI`
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

以降の例では、`$configPath` を使います。

## 2. プロジェクトへ入る

Telegram から続けたいプロジェクトへ移動します。

```powershell
Set-Location C:\path\to\your\project
```

手元の画面でセッションを確認したい場合は、同じプロジェクトで `Codex CLI` を起動します。

```powershell
codex
```

## 3. プロジェクトを登録する

プロジェクトごとに1回実行します。

```powershell
remotty config workspace upsert --config $configPath --path (Get-Location).Path
```

この操作は、プロジェクトを `%APPDATA%\remotty\bridge.toml` へ保存します。
プロジェクトのルートにはファイルを作りません。
確認したい場合は、`git status` を実行してください。

## 4. Telegram bot を用意する

すでに `remotty` 用の bot がある場合は、その token を使います。
新しく作る場合だけ、次を行います。

1. Telegram で `@BotFather` を開きます。
2. `/newbot` を送ります。
3. 表示名を決めます。
4. `bot` で終わる username を決めます。
5. BotFather が返した token を控えます。

token をチャット、スクリーンショット、issue、PR に貼らないでください。

## 5. bot token を保存する

次を実行します。

```powershell
remotty telegram configure --config $configPath
```

表示に従って token を貼ります。
このコマンドは token を再表示せず、Windows の保護領域へ保存します。
暗号化されたファイルは `%LOCALAPPDATA%\remotty\secrets` 配下です。
既定のファイル名は `remotty-telegram-bot.bin` です。

## 6. Telegram チャンネルを起動する

Telegram とつなぐ時に実行します。

```powershell
remotty --config $configPath
```

起動時は `%APPDATA%\remotty\bridge.toml` の設定を使います。
起動に成功したら、ターミナルに次の表示があることを確認します。

```text
Listening for Telegram channel messages from: remotty:telegram
```

同じ起動ログには、Telegram bot、Codex の接続方式、登録済みワークスペースも表示されます。
Telegram から使う間は、このプロセスを起動したままにしてください。

## 7. Telegram をペアリングする

Telegram の 1 対 1 のチャットで、bot へ任意のメッセージを送ります。
bot は `remotty pairing code` を返します。

次を実行します。

```powershell
remotty telegram access-pair <code> --config $configPath
```

次に、許可済み送信者を確認します。

```powershell
remotty telegram policy allowlist --config $configPath
```

これで、他の Telegram ユーザーが手元の Codex を操作できなくなります。

## 8. Codex スレッドを選ぶ

利用できる Codex スレッドを表示します。

```powershell
remotty telegram sessions --config $configPath
```

Telegram から続けたいスレッドを選びます。
対象の Telegram チャットで次を送ります。

```text
/remotty-sessions <スレッド名または ID>
```

例:

```text
/remotty-sessions Start workspace session
```

`/remotty-sessions` の後ろは、まとめて1つの名前として扱います。
引用符は不要です。
大文字と小文字は区別しません。
`remotty` は `ID`、完全な名前、`ID` の先頭、名前の一部の順に探します。
複数のスレッドが一致した場合は、表示された `ID` を使います。

## 9. テストメッセージを送る

Telegram で次を送ります。

```text
このスレッドの内容を要約して、次にやることを提案してください。
```

`remotty` は選択済みスレッドへ文を渡します。
返答は Telegram に表示されます。

## 承認待ち

Codex が承認を求めると、`remotty` は Telegram へ中継します。
許可済み送信者だけが承認できます。

## 接続の Q&A

> Q. Telegram とつながっているか、どこで分かりますか?
>
> A. `remotty` のターミナルに `Listening for Telegram channel messages from: remotty:telegram` が出ていることを確認してください。
> 出ていない場合は、`remotty --config $configPath` を起動し直します。

> Q. `remotty` に Codex App は必要ですか?
>
> A. いいえ。この手順は `Codex CLI` 向けです。`remotty` はローカルの `codex` コマンドを使います。

> Q. プロジェクトにファイルを書きますか?
>
> A. 書きません。設定と状態は `%APPDATA%\remotty` 配下へ保存します。

> Q. bot が返答しません。
>
> A. まず `remotty` のターミナルが動いているか確認します。
> 次に `remotty telegram live-env-check --config $configPath` を実行します。
> webhook 状態が `webhook-configured` の場合は polling に戻してください。

> Q. Telegram の polling 競合が出ます。
>
> A. 同じ Telegram bot を polling できるプロセスは1つだけです。
> 別の `remotty`、live smoke、bot worker を止めてください。

## 安全性の Q&A

> Q. bot token はどこに保存しますか?
>
> A. `%LOCALAPPDATA%\remotty\secrets` 配下の Windows 保護領域へ保存します。
> プロジェクトのリポジトリ、GitHub、Telegram チャットには保存しません。

> Q. token を `Codex CLI` に貼ってよいですか?
>
> A. いいえ。`remotty telegram configure` が開く入力欄にだけ貼ってください。

> Q. bot を見つけた人が Codex を操作できますか?
>
> A. できません。ペアリング済みで、allowlist に入った送信者だけを受け付けます。

## 関連ドキュメント

- [高度な CLI モード](exec-transport.ja.md)
- [更新時の注意](upgrading.ja.md)
