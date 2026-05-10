# Telegram クイックスタート

この手順では、`remotty` を Windows の `Codex CLI` 向け Telegram ブリッジとして設定します。
`remotty` は Codex App の画面へ入力しません。
ローカルの `codex` コマンドと `app_server` 接続を通じて Codex とやり取りします。

## 何ができるようになるか

AI に作業を任せると、作業中に承認が必要になることがあります。
席を離れている間にそこで止まると、帰ってくるまで何も進みません。

`remotty` を使うと、Telegram で状態を見たり、承認したり、短い追加入力を送ったりできます。
ただし、Codex App を遠隔操作する公式機能ではありません。
大きな方針変更や差分の細かい確認は、手元の Codex 画面で行います。

## 仕組み

1. 作業したいプロジェクトで、1つ目の PowerShell ウィンドウを使って `Codex CLI` を起動します。
2. 同じ Windows ユーザー、同じプロジェクトで、別の PowerShell ウィンドウを使って `remotty` を起動します。
3. `remotty` がチャンネル型の起動表示を出します。
4. Telegram bot へメッセージを送ります。
5. `remotty` が、このプロジェクトで起動した `Codex CLI` セッションへ文を渡します。
6. Codex が返答し、`remotty` が Telegram へ戻します。

現在の手順では、ローカルの `Codex CLI` セッションを使います。
今後も `remotty` は、Telegram からの見守り、承認の中継、短い追加入力に集中します。
詳しくは [Telegram ブリッジとしての方針](remote-companion.ja.md) を参照してください。

この手順では、次の PowerShell 画面を使い分けます。

| 画面 | 開いたままにするか | 使う場面 |
| --- | --- | --- |
| 設定用 PowerShell | いいえ | `remotty` のインストール、プロジェクト登録、bot token の保存、Telegram のペアリング |
| Codex 用 PowerShell | はい | Telegram から続けたいプロジェクトで `codex` を起動 |
| ブリッジ用 PowerShell | はい | 同じプロジェクトで `remotty --config "$env:APPDATA\remotty\bridge.toml"` を起動 |

起動に成功すると、`remotty` のターミナルに次の表示が出ます。

```text
Listening for Telegram channel messages from: remotty:telegram
```

Telegram から使う間は、ブリッジ用 PowerShell を開いたままにします。

## 必要なもの

- Windows 10 または Windows 11
- `Codex CLI`
- Node.js と `npm`
- Telegram
- `@BotFather` で作った専用 bot

## 1. `remotty` を入れる

設定用 PowerShell で実行します。

```powershell
npm install -g remotty
```

## 2. プロジェクトを登録する

設定用 PowerShell で、Telegram から続けたいプロジェクトへ移動します。

```powershell
Set-Location C:\path\to\your\project
```

設定用 PowerShell で、プロジェクトごとに1回実行します。

```powershell
remotty config workspace upsert --config "$env:APPDATA\remotty\bridge.toml" --path (Get-Location).Path
```

この操作は、プロジェクトを `%APPDATA%\remotty\bridge.toml` へ保存します。
プロジェクトのルートにはファイルを作りません。
確認したい場合は、`git status` を実行してください。

## 3. Telegram bot を用意する

すでに `remotty` 用の bot がある場合は、その token を使います。
新しく作る場合だけ、次を行います。

1. Telegram で `@BotFather` を開きます。
2. `/newbot` を送ります。
3. 表示名を決めます。
4. `bot` で終わる username を決めます。
5. BotFather が返した token を控えます。

token をチャット、スクリーンショット、issue、PR に貼らないでください。

## 4. bot token を保存する

設定用 PowerShell で次を実行します。

```powershell
remotty telegram configure --config "$env:APPDATA\remotty\bridge.toml"
```

表示に従って token を貼ります。
このコマンドは token を再表示せず、Windows の保護領域へ保存します。
暗号化されたファイルは `%LOCALAPPDATA%\remotty\secrets` 配下です。
既定のファイル名は `remotty-telegram-bot.bin` です。

## 5. `Codex CLI` を起動する

Codex 用 PowerShell を開き、同じプロジェクトへ移動してから起動します。

```powershell
Set-Location C:\path\to\your\project
codex
```

この `Codex CLI` の画面は開いたままにします。
`remotty` が Telegram のメッセージをこのセッションへ渡すためです。
`codex` の起動後、その画面は PowerShell ではなく `Codex CLI` のプロンプトになっています。
この画面では `remotty ...` コマンドを実行しません。

## 6. Telegram チャンネルを起動する

ブリッジ用 PowerShell を開き、同じプロジェクトへ移動してから `remotty` を起動します。

```powershell
Set-Location C:\path\to\your\project
remotty --config "$env:APPDATA\remotty\bridge.toml"
```

これはブリッジ用 PowerShell で実行します。
`Codex CLI` の入力欄には貼らないでください。
`no matches` が出る場合は、`Codex CLI` のプロンプトへ送っています。
`Esc` で入力を消し、ブリッジ用 PowerShell に切り替えて実行してください。

起動時は `%APPDATA%\remotty\bridge.toml` の設定を使います。
起動に成功したら、ターミナルに次の表示があることを確認します。

```text
Listening for Telegram channel messages from: remotty:telegram
```

同じ起動ログには、Telegram bot、Codex の接続方式、登録済みワークスペースも表示されます。
この時点で、このプロジェクトの `Codex CLI` セッションが Telegram の連携先になります。
Telegram から使う間は、このプロセスを起動したままにしてください。

## 7. Telegram をペアリングする

Telegram の 1 対 1 のチャットで、bot へ任意のメッセージを送ります。
bot は `remotty pairing code` を返します。

設定用 PowerShell を使います。
閉じている場合は、通常の PowerShell を新しく開き、その画面を設定用 PowerShell として使います。
`Codex CLI` の画面や、`remotty` が起動中のブリッジ用 PowerShell には入力しません。

```powershell
remotty telegram access-pair <code> --config "$env:APPDATA\remotty\bridge.toml"
```

次に、許可済み送信者を確認します。

```powershell
remotty telegram policy allowlist --config "$env:APPDATA\remotty\bridge.toml"
```

これで、他の Telegram ユーザーが手元の Codex を操作できなくなります。

## 8. テストメッセージを送る

Telegram で次を送ります。

```text
このセッションの内容を要約して、次にやることを提案してください。
```

`remotty` は手順 5 で起動した `Codex CLI` セッションへ文を渡します。
返答は Telegram に表示されます。

## 承認待ち

Codex が承認を求めると、`remotty` は Telegram へ中継します。
許可済み送信者だけが承認できます。
分からない承認は、その場で許可しないでください。
手元の Codex 画面で内容を確認してから判断します。

### Codex の追加入力

Codex が短い追加入力を求めた場合は、Telegram 通知に表示された `request_id` を使い、`/answer <request_id> <value>` で返せます。

```text
/answer request-123 docs
```

入力欄が複数ある場合は、Telegram 通知に各入力欄の `id` が表示されます。
1つの Telegram メッセージにまとめ、各行に `id=value` の形で書きます。

```text
/answer request-123 target=docs
mode=review
```

`remotty` は、Codex が秘密入力として示した要求を Telegram からは受け付けません。
Telegram のメッセージはチャット履歴として扱い、パスワード、API キー、リカバリーコードなどの秘密情報を `/answer` で送らないでください。
Codex が秘密入力を求めた場合は、手元の Codex 画面で入力してください。

## 接続の Q&A

> Q. Telegram とつながっているか、どこで分かりますか?
>
> A. `remotty` のターミナルに `Listening for Telegram channel messages from: remotty:telegram` が出ていることを確認してください。
> 出ていない場合は、ブリッジ用 PowerShell で `remotty --config "$env:APPDATA\remotty\bridge.toml"` を起動し直します。

> Q. `remotty` に Codex App は必要ですか?
>
> A. いいえ。この手順は `Codex CLI` 向けです。`remotty` はローカルの `codex` コマンドを使います。

> Q. プロジェクトにファイルを書きますか?
>
> A. 書きません。設定と状態は `%APPDATA%\remotty` 配下へ保存します。

> Q. bot が返答しません。
>
> A. まず `remotty` のターミナルが動いているか確認します。
> 次に、設定用 PowerShell で `remotty telegram live-env-check --config "$env:APPDATA\remotty\bridge.toml"` を実行します。
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
- [Telegram ブリッジとしての方針](remote-companion.ja.md)
- [更新時の注意](upgrading.ja.md)
