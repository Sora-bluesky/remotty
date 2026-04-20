[English](README.md) | [日本語](README.ja.md)

# codex-channels

`codex-channels` is a Windows bridge that lets you talk to Codex from Telegram.

It runs on your Windows machine, receives messages from your Telegram bot, starts `codex`, and sends the result back to the same chat. The project is designed for people who want a simple chat-based control surface without exposing a public webhook server.

Send a message on Telegram, let your Windows PC run `codex`, and get the reply back in the same chat.

## What It Does

- Connects a Telegram bot to a local Codex workflow
- Keeps conversation state in SQLite so the bridge can resume work cleanly
- Supports reply-driven work and automatic continuation modes
- Stores bot tokens in local protected storage with DPAPI
- Can run in the foreground or as a Windows service

## Who It Is For

This project is best for:

- Windows users who want to trigger Codex from Telegram
- solo builders who want a lightweight remote control surface
- developers who prefer local execution over hosted automation

Today, setup is still command-line based. If you are comfortable with PowerShell and basic Telegram bot setup, you should be able to get started.

## Requirements

- Windows 10 or Windows 11
- Rust toolchain from [rustup.rs](https://rustup.rs/) with `cargo` available on `PATH`
- `codex` CLI available on `PATH`
- a Telegram bot token from `@BotFather`
- your Telegram user ID, so the bridge knows who is allowed to use it

## Quick Start

### 1. Clone the repository

```powershell
git clone https://github.com/Sora-bluesky/codex-channels.git
cd codex-channels
```

### 2. Create a Telegram bot

1. Open `@BotFather` in Telegram.
2. Send `/newbot`.
3. Choose a bot name and username.
4. Copy the bot token that BotFather returns.

### 3. Store the bot token locally

This stores the token in Windows protected storage instead of a tracked file.
The first `cargo run` may take a few minutes because it builds the project.

```powershell
cargo run -- secret set codex-telegram-bot <YOUR_TELEGRAM_BOT_TOKEN>
```

### 4. Edit `bridge.toml`

The repository already includes `bridge.toml` as a starting point.

Update these values before the first run:

- `telegram.admin_sender_ids`: your Telegram user ID
- `workspaces[0].path`: the folder where Codex should work
- `workspaces[0].writable_roots`: folders Codex is allowed to edit

If you do not know your Telegram user ID yet, send a message to your bot and inspect the latest `message.from.id` field with:

```powershell
Invoke-RestMethod "https://api.telegram.org/bot<YOUR_TELEGRAM_BOT_TOKEN>/getUpdates" | ConvertTo-Json -Depth 8
```

If you already use a named Codex profile, you can also set `codex.profile`. Otherwise, leave it out and the bridge will follow the local `codex` CLI default.

### 5. Start the bridge

```powershell
cargo run
```

If startup succeeds, leave the terminal window open.

### 6. Open your bot in Telegram

Send `/help` to the bot. If the bridge is running and your sender ID is allowed, you should see the available commands.

## Common Commands

Inside Telegram, you can use:

```text
/help
/status                  # show the current bridge state
/stop                    # stop the active Codex session
/workspace               # show the current workspace and available IDs
/workspace docs          # switch this chat to another workspace
/mode completion_checks  # continue only after local checks fail
/mode infinite           # keep continuing until Codex stops naturally
/mode max_turns 3        # continue automatically up to 3 times
```

## Configuration

The main config file is `bridge.toml`.

### Important sections

- `service`: run mode and shutdown timing
- `telegram`: allowed chat types and allowed senders
- `codex`: CLI binary, model, sandbox mode, approval mode, and optional profile
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
token_secret_ref = "codex-telegram-bot"
allowed_chat_types = ["private"]
admin_sender_ids = [123456789]

[codex]
binary = "codex"
model = "<your-codex-model>"
sandbox = "workspace-write"
approval = "on-request"

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
- You can use `TELEGRAM_BOT_TOKEN` as a fallback when you do not want to store the token with `cargo run -- secret set`
- Do not commit live values such as `LIVE_TELEGRAM_BOT_TOKEN` or `LIVE_WORKSPACE`
- Runtime state is ignored by `.gitignore`
- Local secret-scanning hooks are recommended before commit and push

## Run as a Windows Service

If you want the bridge to keep running in the background:

Open PowerShell as Administrator before the install step.

```powershell
cargo run -- service install --config bridge.toml
cargo run -- service start
cargo run -- service status
```

To stop or remove it later:

```powershell
cargo run -- service stop
cargo run -- service uninstall
```

## For Contributors

### Checks

```powershell
cargo fmt --check
cargo test
cargo check
pwsh -NoProfile -File scripts/audit-public-surface.ps1
pwsh -NoProfile -File scripts/audit-secret-surface.ps1
```

### Optional live smoke test

The live smoke test is opt-in and does not run in CI.
Keep the `LIVE_*` values in the current shell only. Do not write them into tracked files.

Required environment variables:

- `LIVE_TELEGRAM_BOT_TOKEN`
- `LIVE_TELEGRAM_CHAT_ID`
- `LIVE_TELEGRAM_SENDER_ID`
- `LIVE_WORKSPACE`

Optional environment variables:

- `LIVE_CODEX_BIN`
- `LIVE_CODEX_PROFILE`
- `LIVE_TIMEOUT_SEC`

Run:

```powershell
cargo test --features live-e2e --test live_end_to_end -- --ignored --nocapture
```

Use a dedicated test bot and chat when possible.

## Repository Layout

```text
codex-channels/
├── src/                    # bridge runtime, Telegram client, Codex runner
├── tests/                  # config, smoke, and safety tests
├── scripts/                # maintenance and validation scripts
├── bridge.toml             # local configuration starter
├── README.md               # English README
└── README.ja.md            # Japanese README
```

## License

[MIT](LICENSE)
