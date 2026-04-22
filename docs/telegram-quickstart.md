# Telegram Quickstart

This guide sets up `remotty` so Telegram can send messages to a Codex thread
on your Windows PC.

## How It Works

1. You run `remotty` on your Windows PC.
2. You send a message to your Telegram bot.
3. `remotty` sends that message to the Codex thread you selected.
4. Codex replies, and `remotty` sends the reply back to Telegram.

## What You Need

- Windows 10 or Windows 11
- Codex App and Codex CLI
- Node.js and `npm`
- Telegram
- A dedicated Telegram bot from `@BotFather`

## 1. Install `remotty`

Run this in PowerShell:

```powershell
npm install -g remotty
```

Open the installed package folder:

```powershell
$remottyRoot = Join-Path (npm root -g) "remotty"
Set-Location $remottyRoot
```

Copy the starter config:

```powershell
$configDir = Join-Path $env:APPDATA "remotty"
New-Item -ItemType Directory -Force -Path $configDir | Out-Null
Copy-Item -Force .\bridge.toml (Join-Path $configDir "bridge.toml")
$configPath = Join-Path $configDir "bridge.toml"
```

## 2. Set Your Project Folder

Open `%APPDATA%\remotty\bridge.toml`.

Change these two lines to the project you want Codex to work in:

```toml
path = "C:/Users/you/Documents/project"
writable_roots = ["C:/Users/you/Documents/project"]
```

Use forward slashes in Windows paths.

You do not need to choose a transport for the normal setup.
The included config is already set up for Codex thread relay.

## 3. Create a Telegram Bot

1. Open `@BotFather` in Telegram.
2. Send `/newbot`.
3. Choose a display name.
4. Choose a username ending in `bot`.
5. Copy the token that BotFather returns.

Do not post the token in chat, screenshots, issues, or pull requests.

## 4. Install the Local Plugin

Open the `remotty` package folder in the Codex App.

In the Plugins view:

1. Add `.agents/plugins/marketplace.json`.
2. Install the plugin named `remotty`.
3. Confirm that `remotty` appears in the Plugins view.

## 5. Store the Bot Token

Run this in the Codex App:

```text
/remotty-configure
```

Paste the token when prompted.
The command stores it in Windows protected storage.
It does not print the token back.

## 6. Start the Bridge

Run this in the Codex App:

```text
/remotty-start
```

Keep the bridge running while you use Telegram.
If it stops, the bot cannot reply.

## 7. Pair Telegram

Send any message to your bot in a private Telegram chat.

The bot replies with a `remotty pairing code`.
Run this in the Codex App:

```text
/remotty-access-pair <code>
```

Then lock access to your allowlist:

```text
/remotty-policy-allowlist
```

This prevents other Telegram users from controlling your local Codex setup.

## 8. Select a Codex Thread

Run this in the Codex App:

```text
/remotty-sessions
```

Choose the thread you want Telegram to continue.
Then bind this Telegram chat to it:

```text
/remotty-sessions <thread_id>
```

This binding is stored under `%APPDATA%\remotty`.
It is not written into your project repository.

## 9. Send a Test Message

In Telegram, send:

```text
Summarize the current thread and suggest the next step.
```

`remotty` sends the text to the selected Codex thread.
The reply appears in Telegram.

## Approval Prompts

When Codex asks for approval, `remotty` posts the prompt to Telegram.

You can press `Approve` or `Deny`.
You can also type:

```text
/approve <request_id>
/deny <request_id>
```

The decision is returned to the same Codex turn.

## Troubleshooting

### The Bot Does Not Reply

- Confirm `/remotty-start` is still running.
- Run `/remotty-status` in the Codex App.
- Run `/remotty-live-env-check` in the Codex App.
- If the webhook status is `webhook-configured`, switch the bot back to polling.

### No Codex Threads Appear

- Update Codex CLI, then try again.
- Start at least one Codex App or Codex CLI thread.
- Run `/remotty-sessions` again.

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

Stop the process that reads the same bot:

```powershell
Stop-Process -Id <PID>
```

## Related

- [Fakechat Demo](fakechat-demo.md)
- [Advanced CLI Mode](exec-transport.md)
- [Upgrade Notes](upgrading.md)

Codex Remote connections connect the Codex App to an SSH development machine.
Use them when the code and shell live on a remote host.

Use `remotty` when the Codex setup is on your Windows PC and Telegram should
send work to it.
