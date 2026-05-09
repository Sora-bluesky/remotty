# Telegram Quickstart

This guide sets up `remotty` as a Telegram bridge for Codex CLI on Windows.
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
2. You start `remotty` in a separate PowerShell window with the same Windows user and project directory.
3. `remotty` prints a channel-style startup message.
4. You send a message to your Telegram bot.
5. `remotty` sends that message to the Codex CLI session you started for this project.
6. Codex replies, and `remotty` sends the reply back to Telegram.

The current quickstart uses a local Codex CLI session.
The product direction is to keep `remotty` focused on Telegram-based watching,
approval relay, and short steering even as richer Codex App surfaces evolve.
See [Remote Bridge Direction](remote-companion.md).

You will use these PowerShell windows:

| Window | Keep it open? | Use it for |
| --- | --- | --- |
| Setup PowerShell | No | Install `remotty`, register the project, store the bot token, and pair Telegram. |
| Codex PowerShell | Yes | Run `codex` in the project you want to continue from Telegram. |
| Bridge PowerShell | Yes | Run `remotty --config "$env:APPDATA\remotty\bridge.toml"` for the same project. |

When startup succeeds, the `remotty` terminal shows:

```text
Listening for Telegram channel messages from: remotty:telegram
```

Keep Bridge PowerShell open while you use Telegram.

## What You Need

- Windows 10 or Windows 11
- Codex CLI
- Node.js and `npm`
- Telegram
- A dedicated Telegram bot from `@BotFather`

## 1. Install `remotty`

Run this in Setup PowerShell:

```powershell
npm install -g remotty
```

## 2. Register Your Project

In Setup PowerShell, use the project you want to continue from Telegram:

```powershell
Set-Location C:\path\to\your\project
```

Run this once per project in Setup PowerShell:

```powershell
remotty config workspace upsert --config "$env:APPDATA\remotty\bridge.toml" --path (Get-Location).Path
```

This saves the project to `%APPDATA%\remotty\bridge.toml`.
It does not create files in the project root.
If you want to verify that, run `git status`.

## 3. Prepare a Telegram Bot

If you already have a dedicated `remotty` bot, use its token.
Only create a new bot when you do not have one yet:

1. Open `@BotFather` in Telegram.
2. Send `/newbot`.
3. Choose a display name.
4. Choose a username ending in `bot`.
5. Copy the token that BotFather returns.

Do not post the token in chat, screenshots, issues, or pull requests.

## 4. Store the Bot Token

Run this in Setup PowerShell:

```powershell
remotty telegram configure --config "$env:APPDATA\remotty\bridge.toml"
```

Paste the token when prompted.
The command stores it in Windows protected storage and does not print it back.
The encrypted file is under `%LOCALAPPDATA%\remotty\secrets`.
The default file name is `remotty-telegram-bot.bin`.

## 5. Start Codex CLI

Open Codex PowerShell, navigate to the same project, and start Codex CLI:

```powershell
Set-Location C:\path\to\your\project
codex
```

Keep this Codex CLI window open because `remotty` sends Telegram messages to
this session. After `codex` starts, that window is the Codex prompt, not a
PowerShell prompt. Do not run `remotty ...` commands in this window.

## 6. Start the Telegram Channel

Open Bridge PowerShell, navigate to the same project, and run `remotty`:

```powershell
Set-Location C:\path\to\your\project
remotty --config "$env:APPDATA\remotty\bridge.toml"
```

Run this in Bridge PowerShell, not inside the Codex CLI prompt. If Codex shows
`no matches`, press `Esc` to clear that input, switch to Bridge PowerShell
window, and run the command there.

Startup uses `%APPDATA%\remotty\bridge.toml`.
When startup succeeds, confirm that the terminal shows:

```text
Listening for Telegram channel messages from: remotty:telegram
```

It also shows the Telegram bot, Codex transport, and registered workspaces.
At this point, the Codex CLI session for this project is the Telegram target.
Keep this process running while you use Telegram.

## 7. Pair Telegram

Send any message to your bot in a private Telegram chat.
The bot replies with a `remotty pairing code`.

Use Setup PowerShell.
If you closed it, open a new normal PowerShell window and use that as Setup PowerShell.
Do not type these commands into the Codex CLI window or the Bridge PowerShell window where
`remotty` is already running.

```powershell
remotty telegram access-pair <code> --config "$env:APPDATA\remotty\bridge.toml"
```

Then check the allowlist:

```powershell
remotty telegram policy allowlist --config "$env:APPDATA\remotty\bridge.toml"
```

This prevents other Telegram users from controlling your local Codex setup.

## 8. Send a Test Message

In Telegram, send:

```text
Summarize the current session and suggest the next step.
```

`remotty` sends the text to the Codex CLI session you started in step 5.
The reply appears in Telegram.

## Approval Prompts

When Codex asks for approval, `remotty` posts the prompt to Telegram.
Only allowed senders can approve.
If you do not understand an approval request, do not approve it from Telegram.
Check the local Codex screen first.

## Connection Q&A

> Q. How do I know Telegram is connected?
>
> A. The `remotty` terminal must show `Listening for Telegram channel messages from: remotty:telegram`.
> If that line is missing, restart `remotty --config "$env:APPDATA\remotty\bridge.toml"` in Bridge PowerShell.

> Q. Does `remotty` require Codex App?
>
> A. No. This flow is for Codex CLI. `remotty` uses the local `codex` command.

> Q. Does `remotty` write files into my project?
>
> A. No. Configuration and runtime state are under `%APPDATA%\remotty`.

> Q. The bot does not reply.
>
> A. First confirm the `remotty` terminal is still running.
> Then run `remotty telegram live-env-check --config "$env:APPDATA\remotty\bridge.toml"` in Setup PowerShell.
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
> A. No. Paste it only into the prompt opened by `remotty telegram configure`.

> Q. Can anyone who finds the bot use my Codex setup?
>
> A. No. Only paired senders on the allowlist are accepted.

## Related Docs

- [Advanced CLI Mode](exec-transport.md)
- [Remote Bridge Direction](remote-companion.md)
- [Upgrade Notes](upgrading.md)
