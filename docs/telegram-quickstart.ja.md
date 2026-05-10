# Telegram クイックスタート

この手順では、Windows の `Codex CLI` 向けに `remotty remote-control` を設定します。
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
2. 同じプロジェクトで、別の PowerShell ウィンドウを使って `remotty remote-control` を起動します。
3. 初回だけ、`remotty` が `%APPDATA%\remotty\bridge.toml` を作り、現在のプロジェクトを登録し、Telegram bot token の入力を求めます。
4. `remotty` が Remote Control の起動表示を出します。
5. Telegram bot へメッセージを送ります。
6. 初めて使う送信者には、bot がペアリングコードを返します。ペアリング後は、このプロジェクトの `Codex CLI` セッションへ文が渡ります。
7. Codex が返答し、`remotty` が Telegram へ戻します。

現在の手順では、ローカルの `Codex CLI` セッションを使います。
今後も `remotty` は、Telegram からの見守り、承認の中継、短い追加入力に集中します。
詳しくは [Telegram ブリッジとしての方針](remote-companion.ja.md) を参照してください。

この手順では、次の PowerShell 画面を使い分けます。

| 画面 | 開いたままにするか | 使う場面 |
| --- | --- | --- |
| 通常の PowerShell | いいえ | `remotty` のインストールと、必要な時の Telegram ペアリング |
| Codex 用 PowerShell | はい | Telegram から続けたいプロジェクトで `codex` を起動 |
| Remote Control 用 PowerShell | はい | 同じプロジェクトで `remotty remote-control` を起動 |

起動に成功すると、`remotty` のターミナルに次の表示が出ます。

```text
Remote Control active
Listening for Telegram channel messages from: remotty:telegram
```

Telegram から使う間は、Remote Control 用 PowerShell を開いたままにします。

## 必要なもの

- Windows 10 または Windows 11
- `Codex CLI`
- Node.js と `npm`
- Telegram
- `@BotFather` で作った専用 bot

## 1. `remotty` を入れる

通常の PowerShell で実行します。

```powershell
npm install -g remotty
```

## 2. Telegram bot を用意する

すでに `remotty` 用の bot がある場合は、その token を使います。
新しく作る場合だけ、次を行います。

1. Telegram で `@BotFather` を開きます。
2. `/newbot` を送ります。
3. 表示名を決めます。
4. `bot` で終わる username を決めます。
5. BotFather が返した token を控えます。

token をチャット、スクリーンショット、issue、PR に貼らないでください。

## 3. `Codex CLI` を起動する

Codex 用 PowerShell を開き、プロジェクトへ移動してから起動します。

```powershell
Set-Location C:\path\to\your\project
codex
```

この `Codex CLI` の画面は開いたままにします。
`remotty` が Telegram のメッセージをこのセッションへ渡すためです。
`codex` の起動後、その画面は PowerShell ではなく `Codex CLI` のプロンプトになっています。
この画面では `remotty ...` コマンドを実行しません。

## 4. Remote Control を起動する

Remote Control 用 PowerShell を開き、同じプロジェクトへ移動してから実行します。

```powershell
Set-Location C:\path\to\your\project
remotty remote-control
```

これは Remote Control 用 PowerShell で実行します。
`Codex CLI` の入力欄には貼らないでください。
`no matches` が出る場合は、`Codex CLI` のプロンプトへ送っています。
`Esc` で入力を消し、Remote Control 用 PowerShell に切り替えて実行してください。

初回だけ、表示に従って Telegram bot token を貼ります。
このコマンドは token を再表示せず、Windows の保護領域へ保存します。
暗号化されたファイルは `%LOCALAPPDATA%\remotty\secrets` 配下です。
既定のファイル名は `remotty-telegram-bot.bin` です。

起動時は `%APPDATA%\remotty\bridge.toml` の設定を使います。
必要ならこのファイルを作り、現在のプロジェクトを登録します。
プロジェクトのルートにはファイルを作りません。
確認したい場合は、`git status` を実行してください。

起動に成功したら、ターミナルに次の表示があることを確認します。

```text
Remote Control active
Listening for Telegram channel messages from: remotty:telegram
```

同じ起動ログには、Telegram bot、Codex の接続方式、登録済みワークスペースも表示されます。
この時点で、このプロジェクトの `Codex CLI` セッションが Telegram の連携先になります。
Telegram から使う間は、このプロセスを起動したままにしてください。

## 5. Telegram をペアリングする

Telegram の 1 対 1 のチャットで、bot へ任意のメッセージを送ります。
bot は `remotty pairing code` を返します。

通常の PowerShell を使います。
`Codex CLI` の画面や、`remotty` が起動中の Remote Control 用 PowerShell には入力しません。

```powershell
remotty telegram access-pair <code> --config "$env:APPDATA\remotty\bridge.toml"
```

次に、許可済み送信者を確認します。

```powershell
remotty telegram policy allowlist --config "$env:APPDATA\remotty\bridge.toml"
```

これで、他の Telegram ユーザーが手元の Codex を操作できなくなります。

## 6. テストメッセージを送る

Telegram で次を送ります。

```text
このセッションの内容を要約して、次にやることを提案してください。
```

`remotty` は手順 3 で起動した `Codex CLI` セッションへ文を渡します。
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
> A. `remotty` のターミナルに `Remote Control active` が出ていることを確認してください。
> 同じ起動ログに `Listening for Telegram channel messages from: remotty:telegram` も表示されます。
> 出ていない場合は、Remote Control 用 PowerShell で `remotty remote-control` を起動し直します。

> Q. `remotty` に Codex App は必要ですか?
>
> A. いいえ。この手順は `Codex CLI` 向けです。`remotty` はローカルの `codex` コマンドを使います。

> Q. プロジェクトにファイルを書きますか?
>
> A. 書きません。設定と状態は `%APPDATA%\remotty` 配下へ保存します。

> Q. 設定ファイルやプロジェクトのパスを明示できますか?
>
> A. はい。`remotty remote-control --config <bridge.toml> --path <dir>` を使えます。
> 古い `remotty config workspace upsert` コマンドも、高度なスクリプト向けに残しています。

> Q. bot が返答しません。
>
> A. まず `remotty` のターミナルが動いているか確認します。
> 次に、通常の PowerShell で `remotty telegram live-env-check --config "$env:APPDATA\remotty\bridge.toml"` を実行します。
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
> A. いいえ。`remotty remote-control` または `remotty telegram configure` が開く入力欄にだけ貼ってください。

> Q. bot を見つけた人が Codex を操作できますか?
>
> A. できません。ペアリング済みで、allowlist に入った送信者だけを受け付けます。

## 関連ドキュメント

- [高度な CLI モード](exec-transport.ja.md)
- [Telegram ブリッジとしての方針](remote-companion.ja.md)
- [更新時の注意](upgrading.ja.md)
