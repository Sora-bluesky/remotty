# Telegram Quickstart

Use this guide to connect `remotty` to a Telegram bot and send messages to local Codex from your phone.

`remotty` is not a replacement for Codex Remote connections. Remote connections let the Codex App work on an SSH target. `remotty` is a Telegram bridge for reaching the Codex CLI workflow that is already available on your Windows machine.

`remotty` runs its own local bridge process, so Codex does not need a special launch flag. The local `remotty` plugin only provides the `/remotty-*` setup and control commands.

## What You Need

- Windows 10 or Windows 11
- Codex App and Codex CLI
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
$configDir = Join-Path $env:APPDATA "remotty"
New-Item -ItemType Directory -Force -Path $configDir | Out-Null
Copy-Item -Force .\bridge.toml (Join-Path $configDir "bridge.toml")
$configPath = Join-Path $configDir "bridge.toml"
```

This installs the `remotty` command and downloads the matching Windows binary.
`npm root -g` returns the global npm package folder. The next two lines move PowerShell into the installed `remotty` folder so `Copy-Item` can read the bundled `bridge.toml`. Open the same folder in the Codex App in step 3.
The remaining lines copy the starter config to `%APPDATA%\remotty\bridge.toml`, so your settings and runtime state are not stored inside the global npm package folder.

If you need to install directly from the GitHub Release tarball:

```powershell
npm install -g https://github.com/Sora-bluesky/remotty/releases/latest/download/remotty.tgz
$remottyRoot = Join-Path (npm root -g) "remotty"
Set-Location $remottyRoot
$configDir = Join-Path $env:APPDATA "remotty"
New-Item -ItemType Directory -Force -Path $configDir | Out-Null
Copy-Item -Force .\bridge.toml (Join-Path $configDir "bridge.toml")
$configPath = Join-Path $configDir "bridge.toml"
```

## 2. Create a Telegram Bot

1. Open `@BotFather` in Telegram.
2. Send `/newbot`.
3. Choose a display name.
4. Choose a unique username ending in `bot`.
5. Copy the token that BotFather returns.

Do not post the token in chat, screenshots, issues, or pull requests.

## 3. Configure the Token

Open the `remotty` package folder in the Codex App. In the Plugins view, add the local marketplace file at `.agents/plugins/marketplace.json`, then install the plugin entry named `remotty`.
Confirm that `remotty` appears in the Plugins view before continuing.

Then run:

```text
/remotty-configure
```

Paste the token when prompted. The command does not print it back and stores it in Windows protected storage.

## 4. Edit `bridge.toml`

Edit `%APPDATA%\remotty\bridge.toml` before the first start:

- `workspaces[0].path`: the folder where Codex should work
- `workspaces[0].writable_roots`: folders Codex is allowed to edit
- `codex.model`: keep `gpt-5.4` or replace it with the model name your Codex CLI should use
- `codex.transport`: keep `exec` for the simple CLI path, or use `app_server` when you want Telegram approval buttons

Use forward slashes in Windows paths, such as `C:/Users/you/Documents/project`.

`telegram.admin_sender_ids` may stay empty when you use pairing through the plugin.

Relative `state/` paths in this config are resolved next to the copied file, under `%APPDATA%\remotty`.

## 5. Start the Bridge

Run:

```text
/remotty-start
```

Keep the bridge running while you use Telegram. If it is not running, the bot cannot reply.
If the bridge runs in the foreground, it occupies that PowerShell window until it stops. Keep it open, and run pairing commands from the Codex App or another shell.

Check status with:

```text
/remotty-status
```

Stop it with:

```text
/remotty-stop
```

## 6. Pair Your Telegram Account

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

If the bot cannot reply with a pairing code, stop the bridge and run `/remotty-pair` as the fallback pairing path.

## 7. Send a Test Message

In Telegram, send a small request such as:

```text
What files are in the current workspace?
```

`remotty` receives the message, runs the Codex CLI locally, and sends the reply back to the same Telegram chat.

## 8. Run Manual Smoke Checks

Manual smoke checks are optional. They use the real Telegram bot and a local temporary workspace.
The smoke commands create a temporary `app_server` run. You do not need to change your normal `codex.transport = "exec"` setting for regular use.

First check the inputs:

```text
/remotty-live-env-check
```

For a custom config:

```powershell
remotty telegram live-env-check --config C:/path/to/custom-bridge.toml
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

Codex Remote connections connect the Codex App to an SSH development machine. Use that when the code and shell live on a remote host.

Use `remotty` when you want to send prompts from Telegram to the Codex setup on your Windows PC and receive replies in the same chat.
