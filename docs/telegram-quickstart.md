# Telegram Quickstart

This guide sets up `remotty remote-control` for Codex CLI on Windows.
`remotty` does not type into a Codex App window.
It talks to local Codex through the local `codex` command and the local
`app_server` transport.

## What This Gives You

AI work can stop while you are away from the keyboard.
Codex may need approval, hit an error, or need one short follow-up instruction.

`remotty` lets you check status, approve, deny, stop, or send a short follow-up
from Telegram.
It is not an official remote-control surface for the Codex App.
Use the Codex App or Codex CLI for rich task control and detailed diff review.

## How It Works

1. You start Codex CLI in one PowerShell window for the project you want to work on.
2. You run `remotty remote-control` in a separate PowerShell window for the same project.
3. On first run, `remotty` creates `%APPDATA%\remotty\bridge.toml`, registers the current project, and asks for your Telegram bot token.
4. `remotty` prints a remote-control startup message.
5. You send a message to your Telegram bot.
6. First-time senders get a pairing code. After pairing, messages go to the Codex CLI session for this project.
7. Codex replies, and `remotty` sends the reply back to Telegram.

The current quickstart uses a local Codex CLI session.
The product direction is to keep `remotty` focused on Telegram-based watching,
approval relay, and short follow-ups even as richer Codex App surfaces evolve.
See [Telegram Bridge Direction](remote-companion.md).

You will use these PowerShell windows:

| Window | Keep it open? | Use it for |
| --- | --- | --- |
| Normal PowerShell | No | Install `remotty` and finish Telegram pairing when needed. |
| Codex PowerShell | Yes | Run `codex` in the project you want to continue from Telegram. |
| Remote Control PowerShell | Yes | Run `remotty remote-control` in the same project. |

When startup succeeds, the `remotty` terminal shows:

```text
Remote Control active
Listening for Telegram channel messages from: remotty:telegram
```

Keep Remote Control PowerShell open while you use Telegram.

## What You Need

- Windows 10 or Windows 11
- Codex CLI
- Node.js and `npm`
- Telegram
- A dedicated Telegram bot from `@BotFather`

## 1. Install `remotty`

Run this in Normal PowerShell:

```powershell
npm install -g remotty
```

## 2. Prepare a Telegram Bot

If you already have a dedicated `remotty` bot, use its token.
Only create a new bot when you do not have one yet:

1. Open `@BotFather` in Telegram.
2. Send `/newbot`.
3. Choose a display name.
4. Choose a username ending in `bot`.
5. Copy the token that BotFather returns.

Do not post the token in chat, screenshots, issues, or pull requests.

## 3. Start Codex CLI

Open Codex PowerShell, navigate to the project, and start Codex CLI:

```powershell
Set-Location C:\path\to\your\project
codex
```

Keep this Codex CLI window open because `remotty` sends Telegram messages to
this session. After `codex` starts, that window is the Codex prompt, not a
PowerShell prompt. Do not run `remotty ...` commands in this window.

## 4. Start Remote Control

Open Remote Control PowerShell, navigate to the same project, and run:

```powershell
Set-Location C:\path\to\your\project
remotty remote-control
```

Run this in Remote Control PowerShell, not inside the Codex CLI prompt. If Codex shows
`no matches`, press `Esc` to clear that input, switch to Remote Control PowerShell,
and run the command there.

On first run, paste the Telegram bot token when prompted.
The command stores it in Windows protected storage and does not print it back.
The encrypted file is under `%LOCALAPPDATA%\remotty\secrets`.
The default file name is `remotty-telegram-bot.bin`.

Startup uses `%APPDATA%\remotty\bridge.toml`.
The command creates that file if needed and registers the current project.
It does not create files in the project root.
If you want to verify that, run `git status`.

When startup succeeds, confirm that the terminal shows:

```text
Remote Control active
Listening for Telegram channel messages from: remotty:telegram
```

It also shows the Telegram bot, Codex transport, and registered workspaces.
At this point, the Codex CLI session for this project is the Telegram target.
Keep this process running while you use Telegram.

## 5. Pair Telegram

Send any message to your bot in a private Telegram chat.
The bot replies with a `remotty pairing code`.

Use Normal PowerShell.
Do not type these commands into the Codex CLI window or the Remote Control PowerShell window where
`remotty` is already running.

```powershell
remotty telegram access-pair <code> --config "$env:APPDATA\remotty\bridge.toml"
```

Then check the allowlist:

```powershell
remotty telegram policy allowlist --config "$env:APPDATA\remotty\bridge.toml"
```

This prevents other Telegram users from controlling your local Codex setup.

## 6. Send a Test Message

In Telegram, send:

```text
Summarize the current session and suggest the next step.
```

`remotty` sends the text to the Codex CLI session you started in step 3.
The reply appears in Telegram.

## Approval Prompts

When Codex asks for approval, `remotty` posts the prompt to Telegram.
Only allowed senders can approve.
If you do not understand an approval request, do not approve it from Telegram.
Check the local Codex screen first.

### Codex Input Requests

Some Codex prompts ask for one short piece of additional input.
Use the `request_id` shown in the Telegram notification and reply with
`/answer <request_id> <value>`:

```text
/answer request-123 docs
```

If the prompt has several fields, the Telegram notification shows each field
ID. Send one Telegram message and put each answer on its own `id=value` line:

```text
/answer request-123 target=docs
mode=review
```

`remotty` rejects Codex input requests marked as secret.
Treat Telegram messages as chat history: do not send passwords, API keys,
recovery codes, or other secrets with `/answer`.
If Codex asks for secret input, use the local Codex screen.

## Connection Q&A

> Q. How do I know Telegram is connected?
>
> A. The `remotty` terminal must show `Remote Control active`.
> It also prints `Listening for Telegram channel messages from: remotty:telegram`.
> If those lines are missing, restart `remotty remote-control` in Remote Control PowerShell.

> Q. Does `remotty` require Codex App?
>
> A. No. This flow is for Codex CLI. `remotty` uses the local `codex` command.

> Q. Does `remotty` write files into my project?
>
> A. No. Configuration and runtime state are under `%APPDATA%\remotty`.

> Q. Can I use an explicit config or workspace path?
>
> A. Yes. Use `remotty remote-control --config <bridge.toml> --path <dir>`.
> The older `remotty config workspace upsert` command remains available for advanced scripts.

> Q. The bot does not reply.
>
> A. First confirm the `remotty` terminal is still running.
> Then run `remotty telegram live-env-check --config "$env:APPDATA\remotty\bridge.toml"` in Normal PowerShell.
> If the webhook status is `webhook-configured`, switch the bot back to polling.

> Q. Telegram reports a polling conflict.
>
> A. Only one process can poll the same Telegram bot.
> Stop the other `remotty` process, live smoke run, or bot worker.

## Security Q&A

> Q. Where is the bot token stored?
>
> A. It is stored in Windows protected storage under `%LOCALAPPDATA%\remotty\secrets`.
> It is not stored in your project repository, GitHub, or Telegram chat.

> Q. Should I paste the token into Codex CLI?
>
> A. No. Paste it only into the prompt opened by `remotty remote-control` or `remotty telegram configure`.

> Q. Can anyone who finds the bot use my Codex setup?
>
> A. No. Only paired senders on the allowlist are accepted.

## Related Docs

- [Advanced CLI Mode](exec-transport.md)
- [Telegram Bridge Direction](remote-companion.md)
- [Upgrade Notes](upgrading.md)
