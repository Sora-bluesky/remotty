[English](README.md) | [日本語](README.ja.md)

# remotty

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

Today, the preferred path is plugin-first, with the standalone CLI kept as a compatibility layer. If you are comfortable with PowerShell and basic Telegram bot setup, you should be able to get started.

## Plugin-First Setup

The supported setup path is now plugin-first.

Use the local `remotty` plugin to:

- save the bot token without echoing it in the terminal
- pair your Telegram account from a bot-issued code
- start, stop, and inspect the bridge from one place

The Rust bridge still runs as the local core. The plugin is the user-facing layer.

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

Install from the GitHub Release package:

```powershell
npm install -g https://github.com/Sora-bluesky/remotty/releases/latest/download/remotty.tgz
$remottyRoot = Join-Path (npm root -g) "remotty"
Set-Location $remottyRoot
```

The package installs the `remotty` command and downloads the matching Windows binary from the GitHub Release for that package version.

After the package is published to the npm registry, the shorter command will be:

```powershell
npm install -g remotty
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

The older `/remotty-pair` command still works as a compatibility path.

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
In a source checkout, `cargo run -- --config bridge.toml` still works as a compatibility path.

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

## CLI Compatibility

The npm-installed `remotty` command is the packaged CLI path.
The standalone Rust CLI still works from a source checkout.

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

If you keep your config in a non-default path, pass the same `--config <path>` to the compatibility CLI commands.

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

## For Contributors

### Checks

```powershell
cargo fmt --check
cargo test
cargo check
node --check npm/install.js
node --check bin/remotty.js
pwsh -NoProfile -File scripts/audit-public-surface.ps1
pwsh -NoProfile -File scripts/audit-secret-surface.ps1
```

### npm registry publish

GitHub Releases include `remotty.tgz` and a versioned tarball such as `remotty-0.1.15.tgz`.
Publishing to the npm registry is a separate maintainer step:

```powershell
npm publish .\release\remotty.tgz
```

Run this only from an npm account that owns the `remotty` package.

### Optional manual smoke

The manual smoke run is opt-in and does not run in CI.
Use the plugin-first setup before running it:

1. Run `/remotty-configure` to store the Telegram bot token in Windows protected storage.
2. Run `/remotty-access-pair <code>` to add your Telegram sender to the local allowlist.
3. Run `/remotty-live-env-check` to confirm the live smoke can resolve its inputs.

The smoke command can read the bot token from the configured secret and infer a single paired private sender.
If `LIVE_WORKSPACE` is not set, it uses `target/live-smoke-workspace` and creates the `.remotty-live-smoke-ok` marker there.
`/remotty-live-env-check` also checks whether the bot is in polling mode.
It reports `polling-ready` when no webhook is configured and `webhook-configured` when the bot must be switched back before a smoke run.

Only set `LIVE_*` variables when you need to override the plugin-first defaults.
Do not paste secret values into chat, and do not share terminal screenshots that include them.

Optional override environment variables:

- `LIVE_TELEGRAM_BOT_TOKEN`
- `LIVE_TELEGRAM_CHAT_ID`
- `LIVE_TELEGRAM_SENDER_ID`
- `LIVE_WORKSPACE`

Optional environment variables:

- `LIVE_CODEX_BIN`
- `LIVE_CODEX_PROFILE`
- `LIVE_TIMEOUT_SEC`
- `LIVE_APPROVAL_MODE`

Check the environment first:

```powershell
remotty telegram live-env-check
```

For a non-default config file:

```powershell
remotty telegram live-env-check --config bridge.local.toml
```

Then run the approval-accept smoke:

```powershell
remotty telegram smoke approval accept --config bridge.toml
```

For the approval-decline smoke:

```powershell
$env:LIVE_APPROVAL_MODE = "app_server"
remotty telegram smoke approval decline --config bridge.toml
```

Use a dedicated test bot and chat when possible.
Use a dedicated smoke workspace as well.
If a smoke run reports a polling conflict, stop the other `remotty` process that is reading the same bot before retrying.

## Repository Layout

```text
remotty/
├── src/                    # bridge runtime, Telegram client, Codex runner
├── tests/                  # config, mock Telegram, and safety tests
├── scripts/                # maintenance and validation scripts
├── bridge.toml             # local configuration starter
├── README.md               # English README
└── README.ja.md            # Japanese README
```

## License

[MIT](LICENSE)
