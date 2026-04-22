# Telegram Quickstart

Use this guide to connect `remotty` to a Telegram bot and send messages to your local Codex session from your phone.

`remotty` is not a replacement for Codex Remote connections. Remote connections let the Codex app work on an SSH target. `remotty` is a Telegram bridge for reaching the Codex workflow that is already available on your Windows machine.

`remotty` also differs from Claude Code Channels. Channels require a channel plugin and a `--channels` launch flag. `remotty` runs its own local bridge process, so Codex does not need a channel flag. The local `remotty` plugin only provides the `/remotty-*` setup and control commands.

## What You Need

- Windows 10 or Windows 11
- Codex app and `codex` CLI
- Node.js and `npm`
- Telegram on your phone or desktop
- A Telegram bot token from `@BotFather`

Use a dedicated bot for `remotty` when possible.

## 1. Install `remotty`

Install from npm:

```powershell
npm install -g remotty
$remottyRoot = Join-Path (npm root -g) "remotty"
Set-Location $remottyRoot
```

This installs the `remotty` command and downloads the matching Windows binary.

If you need to install directly from the GitHub Release tarball:

```powershell
npm install -g https://github.com/Sora-bluesky/remotty/releases/latest/download/remotty.tgz
```

## 2. Create a Telegram Bot

1. Open `@BotFather` in Telegram.
2. Send `/newbot`.
3. Choose a display name.
4. Choose a unique username ending in `bot`.
5. Copy the token that BotFather returns.

Do not post the token in chat, screenshots, issues, or pull requests.

## 3. Configure the Token

Open the `remotty` package folder in Codex and enable the local plugin. Then run:

```text
/remotty-configure
```

Paste the token when prompted. The command does not print it back and stores it in Windows protected storage.

## 4. Start the Bridge

Run:

```text
/remotty-start
```

Keep the bridge running while you use Telegram. If it is not running, the bot cannot reply.

Check status with:

```text
/remotty-status
```

Stop it with:

```text
/remotty-stop
```

## 5. Pair Your Telegram Account

Send any message to your bot in a private chat.

The bot replies with a `remotty pairing code`. In Codex, run:

```text
/remotty-access-pair <code>
```

Then confirm the allowlist:

```text
/remotty-policy-allowlist
```

Only allowlisted Telegram senders can send normal messages and approval decisions.

## 6. Send a Test Message

In Telegram, send a small request such as:

```text
What files are in the current workspace?
```

`remotty` receives the message, runs `codex` locally, and sends the reply back to the same Telegram chat.

## 7. Run Manual Smoke Checks

Manual smoke checks are optional. They use the real Telegram bot and a local temporary workspace.

First check the inputs:

```text
/remotty-live-env-check
```

For a custom config:

```powershell
remotty telegram live-env-check --config bridge.local.toml
```

The webhook line should say `polling-ready`. If it says `webhook-configured`, remove the webhook before running smoke checks.

Run the approval accept check:

```text
/remotty-smoke-approval-accept
```

Run the approval decline check:

```text
/remotty-smoke-approval-decline
```

Follow the terminal guidance and press the Telegram approval button when the smoke command asks for it.

## Troubleshooting

### The Bot Does Not Reply

- Confirm `/remotty-start` is still running.
- Run `/remotty-status`.
- Run `/remotty-live-env-check`.
- If the webhook status is `webhook-configured`, switch the bot back to polling mode.

### Pairing Code Does Not Work

- Send the message in a private chat with the bot.
- Use the newest code.
- Run `/remotty-access-pair <code>` before the code expires.

### Polling Conflict

Only one process can poll the same Telegram bot.

On Windows, list likely processes:

```powershell
Get-Process remotty, codex -ErrorAction SilentlyContinue | Select-Object Id,ProcessName,Path
```

Stop the process that is reading the same bot, then retry:

```powershell
Stop-Process -Id <PID>
```

### How This Differs From Codex Remote Connections

Codex Remote connections connect the Codex app to an SSH development machine. Use that when the code and shell live on a remote host.

Use `remotty` when you want to send prompts from Telegram to the Codex setup on your Windows PC and receive replies in the same chat.
