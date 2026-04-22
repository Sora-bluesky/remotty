[English](README.md) | [日本語](README.ja.md)

# remotty

![remotty: Windows bridge for Codex and Telegram](docs/assets/hero.png)

`remotty` is a Windows bridge that lets you talk to local Codex from Telegram.

It runs on your Windows machine, receives messages from your Telegram bot, starts the Codex CLI, and sends the result back to the same chat. The project is designed for people who want a simple chat-based control surface without exposing a public webhook server.

> [!WARNING]
> **Disclaimer**
>
> This is an unofficial community project and is not affiliated with, endorsed by, or sponsored by OpenAI.
> `Codex`, `ChatGPT`, and related marks are trademarks of OpenAI.
> They are referenced here only to describe the target CLI or app that this tool works with.
> All other trademarks belong to their respective owners.

## What It Does

- Connects a Telegram bot to a local Codex workflow
- Keeps conversation state in SQLite so the bridge can resume work cleanly
- Supports reply-driven work and automatic continuation modes
- Sends approval requests back to Telegram when Codex needs confirmation
- Stores bot tokens in local protected storage with DPAPI
- Can run in the foreground or as a Windows service

## Who It Is For

This project is best for:

- Windows users who want to trigger Codex from Telegram
- solo builders who want a lightweight remote control surface
- developers who prefer local execution over hosted automation

If you are comfortable with PowerShell and basic Telegram bot setup, you should be able to get started.

## Configure With the Plugin

Use the local `remotty` plugin to:

- save the bot token without echoing it in the terminal
- pair your Telegram account from a bot-issued code
- start, stop, and inspect the bridge from one place

Open the installed `remotty` package folder in the Codex App and enable the local plugin so the `/remotty-*` commands are available. No special Codex launch flag is required. The Telegram bridge runs as a separate local process and talks to the local Codex CLI.

## Requirements

- Windows 10 or Windows 11
- Codex App, so you can install and run the local plugin commands
- Node.js and `npm`
- Codex CLI available on `PATH`
- a Telegram bot token from `@BotFather`

Rust is only required when you build from a source checkout instead of using the npm package.

## Quick Start

Want to try the local chat loop before creating a Telegram bot?
Use the [Fakechat Demo](docs/fakechat-demo.md). It runs only on `localhost` and does not require a token.

For the step-by-step Telegram setup, use the dedicated [Telegram Quickstart](docs/telegram-quickstart.md).
It also explains how `remotty` differs from Codex Remote connections.

### 1. Install `remotty`

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

The package installs the `remotty` command and downloads the matching Windows binary from the GitHub Release for that package version.
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

For source builds, see [Development](docs/development.md).

### 2. Create a Telegram bot

1. Open `@BotFather` in Telegram.
2. Send `/newbot`.
3. Choose a bot name and username.
4. Copy the bot token that BotFather returns.

### 3. Install the local plugin

Open the `remotty` package folder in the Codex App. In the Plugins view, add the local marketplace file at `.agents/plugins/marketplace.json`, then install the plugin entry named `remotty`.
Confirm that `remotty` appears in the Plugins view before continuing.

The installed package already includes:

- `.agents/plugins/marketplace.json`
- `plugins/remotty/.codex-plugin/plugin.json`

### 4. Configure the bot token

Run the plugin command for setup:

```text
/remotty-configure
```

The command asks for the Telegram bot token without printing it back to the terminal and stores it in Windows protected storage.

### 5. Edit `bridge.toml`

Edit the copied config at `%APPDATA%\remotty\bridge.toml`.

Update these values before the first run:

- `workspaces[0].path`: the folder where Codex should work
- `workspaces[0].writable_roots`: folders Codex is allowed to edit
- `codex.model`: keep `gpt-5.4` or replace it with the model name your Codex CLI should use
- `codex.transport`: keep `exec` for the simple CLI path, or use `app_server` when you want Telegram approval buttons

Use forward slashes in Windows paths, such as `C:/Users/you/Documents/project`.

`telegram.admin_sender_ids` may stay empty when you use pairing through the plugin. The plugin stores allowed senders in SQLite instead of asking you to look up IDs by hand.

If you already use a named Codex profile, you can also set `codex.profile`. Otherwise, leave it out and the bridge will follow the local `codex` CLI default.

Relative `state/` paths in this config are resolved next to the copied file, under `%APPDATA%\remotty`.

### 6. Start the bridge

```text
/remotty-start
```

If you prefer the CLI directly, `remotty --config $configPath` starts the foreground bridge.
The foreground bridge occupies that PowerShell window until it stops. Keep it open, and run pairing commands from the Codex App or another shell.
If you open a new PowerShell window later, define it again first:

```powershell
$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"
remotty --config $configPath
```

### 7. Pair your Telegram account

Keep the bridge running. Then send any message to the bot from the Telegram account you want to allow.
The bot replies with a `remotty pairing code`.

Run the plugin command with that code:

```text
/remotty-access-pair <code>
```

The command matches the Telegram sender, shows the target `sender_id` and `chat_id`, and adds the sender to the local allowlist automatically.

If the bridge cannot reply with a code, stop the running bridge and use `/remotty-pair` instead. That older pairing path shows a code locally and asks you to send `/pair <code>` to the bot.

### 8. Lock access to the allowlist

Run:

```text
/remotty-policy-allowlist
```

This confirms which Telegram sender IDs are currently allowed to send normal messages and approval decisions.

### 9. Open your bot in Telegram

Send `/help` to the bot. If the bridge is running and your sender ID is allowed, you should see the available commands.

## Common Commands

Inside Telegram, you can use:

```text
/help
/status                  # show the current bridge state
/stop                    # stop the active Codex session
/approve <request_id>    # approve a pending request from chat text
/deny <request_id>       # deny a pending request from chat text
/workspace               # show the current workspace and available IDs
/workspace docs          # switch this chat to another workspace
/mode completion_checks  # continue only after local checks fail
/mode infinite           # keep continuing until Codex stops naturally
/mode max_turns 3        # continue automatically up to 3 times
```

When `codex.transport = "app_server"`, the bridge also sends inline approval buttons for pending requests. You can approve or deny them directly in Telegram without switching back to the Windows PC.

## Approval Flow

Use `codex.transport = "app_server"` when you want Telegram-driven approvals.

The flow is:

1. Send a request that makes Codex ask for approval.
2. Wait for the bridge to post an approval message in Telegram.
3. Press `Approve` or `Deny`, or use `/approve <request_id>` and `/deny <request_id>`.
4. The same Codex turn continues on the Windows machine.

If you prefer the older CLI-only path, keep `codex.transport = "exec"`.

## Configuration

The main config file is `bridge.toml`.

### Important sections

- `service`: run mode and shutdown timing
- `telegram`: allowed chat types and allowed senders
- `codex`: CLI binary, model, sandbox mode, approval mode, transport, and optional profile
- `storage`: SQLite path, state directory, temp directory, and logs
- `policy`: default lane mode and output limits
- `checks`: optional post-run verification commands
- `workspaces`: where Codex runs and what it may edit

### Minimal example

This example shows the keys most people change first. The included `bridge.toml` already contains the remaining defaults.

```toml
[service]
run_mode = "console"
poll_timeout_sec = 30
shutdown_grace_sec = 15

[telegram]
token_secret_ref = "remotty-telegram-bot"
allowed_chat_types = ["private"]
admin_sender_ids = []

[codex]
binary = "codex"
model = "gpt-5.4"
sandbox = "workspace-write"
approval = "on-request"
transport = "exec"

[storage]
db_path = "state/bridge.db"
state_dir = "state"
temp_dir = "state/tmp"
log_dir = "state/logs"

[policy]
default_mode = "await_reply"
progress_edit_interval_ms = 5000
max_output_chars = 12000
max_turns_limit = 3

[[workspaces]]
id = "main"
path = "C:/path/to/workspace"
writable_roots = ["C:/path/to/workspace"]
default_mode = "await_reply"
continue_prompt = "Continue with the needed checks. If you must stop, reply with the short reason."
checks_profile = "default"
```

## Security

- For interactive use, prefer `/remotty-configure` so the bot token stays in Windows protected storage
- Use `TELEGRAM_BOT_TOKEN` only as a fallback for CI or short-lived local checks where DPAPI is not practical
- Do not paste bot tokens, `api.telegram.org/bot...` URLs, or full terminal screenshots into chat tools or issues

## CLI Commands

The `remotty` command installed via npm is the packaged CLI.
If you copy these commands into a new PowerShell window, define the config path first:

```powershell
$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"
```

Common equivalents are:

- plugin `/remotty-configure` -> `remotty telegram configure --config $configPath`
- plugin `/remotty-access-pair <code>` -> `remotty telegram access-pair <code> --config $configPath`
- plugin `/remotty-pair` -> `remotty telegram pair --config $configPath`
- plugin `/remotty-policy-allowlist` -> `remotty telegram policy allowlist --config $configPath`
- plugin `/remotty-start` -> `remotty --config $configPath`
- plugin `/remotty-stop` -> stop the Windows service when installed; for a foreground bridge, close or interrupt its terminal
- plugin `/remotty-status` -> `remotty service status`; this reports the Windows service state, not a foreground bridge in another terminal
- plugin `/remotty-fakechat-demo` -> `remotty demo fakechat`
- plugin `/remotty-live-env-check` -> `remotty telegram live-env-check`
- plugin `/remotty-smoke-approval-accept` -> `remotty telegram smoke approval accept --config $configPath`
- plugin `/remotty-smoke-approval-decline` -> `remotty telegram smoke approval decline --config $configPath`

If you keep your config somewhere else, pass that path to `--config`.

## Run as a Windows Service

If you want the bridge to keep running in the background:

Open PowerShell as Administrator before the install step.

```powershell
$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"
remotty service install --config $configPath
remotty service start
remotty service status
```

To stop or remove it later:

```powershell
remotty service stop
remotty service uninstall
```

## Related Docs

- [Telegram Quickstart](docs/telegram-quickstart.md)
- [Fakechat Demo](docs/fakechat-demo.md)
- [Development](docs/development.md)

## License

[MIT](LICENSE)
