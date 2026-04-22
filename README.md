[English](README.md) | [日本語](README.ja.md)

# remotty

![remotty: Windows bridge for Codex and Telegram](docs/assets/hero.png)

`remotty` is a Windows bridge that lets you talk to a local coding agent from Telegram.

It runs on your Windows machine, receives messages from your Telegram bot, starts `codex`, and sends the result back to the same chat. The project is designed for people who want a simple chat-based control surface without exposing a public webhook server.

Send a message on Telegram, let your Windows PC run `codex`, and get the reply back in the same chat.

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

Open the installed `remotty` package folder in Codex and enable the local plugin so the `/remotty-*` commands are available. `remotty` does not use Claude Code Channels, so you do not start Codex with `--channels`. The Telegram bridge runs as a separate local process and talks to the local `codex` CLI.

## Requirements

- Windows 10 or Windows 11
- Codex app, so you can install and run the local plugin commands
- Node.js and `npm` for the packaged install path
- `codex` CLI available on `PATH`
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
```

The package installs the `remotty` command and downloads the matching Windows binary from the GitHub Release for that package version.

If you need to install directly from the GitHub Release tarball:

```powershell
npm install -g https://github.com/Sora-bluesky/remotty/releases/latest/download/remotty.tgz
```

If you want to work from source instead:

```powershell
git clone https://github.com/Sora-bluesky/remotty.git
cd remotty
cargo build
```

### 2. Create a Telegram bot

1. Open `@BotFather` in Telegram.
2. Send `/newbot`.
3. Choose a bot name and username.
4. Copy the bot token that BotFather returns.

### 3. Install the local plugin

Open the `remotty` package or repository folder in Codex and install the local plugin entry named `remotty`.

The repository already includes:

- `.agents/plugins/marketplace.json`
- `plugins/remotty/.codex-plugin/plugin.json`

### 4. Configure the bot token

Run the plugin command for setup:

```text
/remotty-configure
```

The command asks for the Telegram bot token without printing it back to the terminal and stores it in Windows protected storage.

### 5. Pair your Telegram account

Make sure the bridge is running. Then send any message to the bot from the Telegram account you want to allow.
The bot replies with a `remotty pairing code`.

Run the plugin command with that code:

```text
/remotty-access-pair <code>
```

The command matches the Telegram sender, shows the target `sender_id` and `chat_id`, and adds the sender to the local allowlist automatically.

After pairing, continue to the allowlist check.

### 6. Lock access to the allowlist

Run:

```text
/remotty-policy-allowlist
```

This confirms which Telegram sender IDs are currently allowed to send normal messages and approval decisions.

### 7. Edit `bridge.toml`

The repository already includes `bridge.toml` as a starting point.

Update these values before the first run:

- `workspaces[0].path`: the folder where Codex should work
- `workspaces[0].writable_roots`: folders Codex is allowed to edit

`telegram.admin_sender_ids` may stay empty when you use pairing through the plugin. The plugin stores allowed senders in SQLite instead of asking you to look up IDs by hand.

If you already use a named Codex profile, you can also set `codex.profile`. Otherwise, leave it out and the bridge will follow the local `codex` CLI default.

### 8. Start the bridge

```text
/remotty-start
```

If you prefer the CLI directly, `remotty --config bridge.toml` starts the foreground bridge.
In a source checkout, use `cargo run -- --config bridge.toml`.

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
3. Press `承認` or `非承認`, or use `/approve <request_id>` and `/deny <request_id>`.
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
model = "<your-codex-model>"
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
continue_prompt = "Continue if more work is needed."
checks_profile = "default"
```

## Security

- Bot tokens should stay in local protected storage or environment variables
- You can use `TELEGRAM_BOT_TOKEN` as a fallback when you do not want to store the token in DPAPI
- Do not commit live values such as `LIVE_TELEGRAM_BOT_TOKEN` or `LIVE_WORKSPACE`
- Do not paste bot tokens, `api.telegram.org/bot...` URLs, or full terminal screenshots into chat tools or issues
- Runtime state is ignored by `.gitignore`
- Local secret-scanning hooks are recommended before commit and push

Secret checks are intentionally layered:

| Layer | Implementation | Scope |
| --- | --- | --- |
| pre-commit | Global `~/.git-hooks/pre-commit` with git-guard-style regex checks | staged diff |
| CI | `.github/workflows/gitleaks.yml` with the Gitleaks GitHub Action | push and pull request changes |
| Manual history scan | `gitleaks git --log-opts=--all --redact --verbose .` | full git history |

## CLI Commands

The npm-installed `remotty` command is the packaged CLI path.

Common equivalents are:

- plugin `/remotty-configure` -> `remotty telegram configure --config bridge.toml`
- plugin `/remotty-access-pair <code>` -> `remotty telegram access-pair <code> --config bridge.toml`
- plugin `/remotty-pair` -> `remotty telegram pair --config bridge.toml`
- plugin `/remotty-policy-allowlist` -> `remotty telegram policy allowlist --config bridge.toml`
- plugin `/remotty-status` -> `remotty service status`
- plugin `/remotty-fakechat-demo` -> `remotty demo fakechat`
- plugin `/remotty-live-env-check` -> `remotty telegram live-env-check`
- plugin `/remotty-smoke-approval-accept` -> `remotty telegram smoke approval accept --config bridge.toml`
- plugin `/remotty-smoke-approval-decline` -> `remotty telegram smoke approval decline --config bridge.toml`

If you keep your config in a non-default path, pass the same `--config <path>` to the CLI commands.

## Run as a Windows Service

If you want the bridge to keep running in the background:

Open PowerShell as Administrator before the install step.

```powershell
remotty service install --config bridge.toml
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
