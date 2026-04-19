# codex-channels

Windows bridge that recreates Claude Code Channels style workflows with Codex and Telegram.

## Status

This repository is an early foundation build.

Implemented today:

- Rust project and workspace layout
- Telegram long polling client
- SQLite-backed lane and run state
- `codex exec` and `codex exec resume` integration
- attachment parsing and safe local attachment storage
- `completion_checks` execution and automatic repair retry flow
- automatic continuation for `infinite` and `max_turns`
- progress message editing in Telegram
- Telegram control commands for `/help`, `/status`, `/stop`, and `/mode`
- DPAPI-backed local secret storage
- Windows service entry point
- GitHub Actions CI for `cargo fmt --check` and `cargo check`
- focused tests for checks, prompt shaping, and attachment helpers

Not implemented yet:

- end-to-end tests around live Telegram and `codex` execution

## Requirements

- Windows
- Rust toolchain
- `codex` CLI on `PATH`
- Telegram bot token

## Quick Start

1. Set the bot token in the local protected store:

```powershell
cargo run -- secret set codex-telegram-bot <YOUR_TELEGRAM_BOT_TOKEN>
```

2. Review and update [`bridge.toml`](bridge.toml).

3. Run the bridge in console mode:

```powershell
cargo run
```

4. Verify formatting and build checks:

```powershell
cargo fmt --check
cargo check
```

5. Use Telegram commands inside the chat when needed:

```text
/help
/status
/stop
/mode completion_checks
/mode infinite
/mode max_turns 3
```

6. Install the Windows service when you want background execution:

```powershell
cargo run -- service install --config bridge.toml
cargo run -- service start
cargo run -- service status
```

Stop or remove it later:

```powershell
cargo run -- service stop
cargo run -- service uninstall
```

7. Sync the external roadmap view when backlog changes:

```powershell
pwsh -NoProfile -File scripts/sync-roadmap.ps1
```

8. Validate planning inputs before syncing:

```powershell
pwsh -NoProfile -File scripts/validate-planning.ps1
```

9. Initialize the external planning workspace in one step:

```powershell
pwsh -NoProfile -File scripts/setup-planning.ps1
```

## Configuration

The main local config file is [`bridge.toml`](bridge.toml).

Important sections:

- `service`: run mode and shutdown timing
- `telegram`: allowed chat types and admin sender IDs
- `codex`: CLI binary, model, sandbox, and approval mode
- `storage`: SQLite path, temp path, and log path
- `policy`: default lane behavior and output truncation
- `policy.max_turns_limit`: default extra-turn cap for `max_turns`
- `checks`: named completion-check profiles
- `workspaces`: workspace mapping and default continuation prompt

Example `completion_checks` profile:

```toml
[policy]
default_mode = "completion_checks"
progress_edit_interval_ms = 5000
max_output_chars = 12000
max_turns_limit = 3

[checks.profiles.default]
[[checks.profiles.default.commands]]
name = "fmt"
program = "cargo"
args = ["fmt", "--check"]
timeout_sec = 30

[[checks.profiles.default.commands]]
name = "test"
program = "cargo"
args = ["test"]
timeout_sec = 180

[[workspaces]]
id = "main"
path = "C:/path/to/workspace"
writable_roots = ["C:/path/to/workspace"]
default_mode = "completion_checks"
continue_prompt = "失敗した確認を直し、必要ならテストを追加して続けてください。"
checks_profile = "default"
```

Example `max_turns` control command:

```text
/mode max_turns 3
```

This keeps the lane in `waiting_reply` after up to three automatic continuation turns.

## Secret Handling

Local secrets are stored under `LOCALAPPDATA/codex-telegram-bridge/secrets` using DPAPI.

Commands:

```powershell
cargo run -- secret set codex-telegram-bot <TOKEN>
cargo run -- secret delete codex-telegram-bot
```

If no stored secret is found, the bridge falls back to `TELEGRAM_BOT_TOKEN`.

## Git Safety

- Internal handoff files are ignored by `.gitignore`
- Runtime state is ignored by `.gitignore`
- Git hooks and `git-guard` should remain enabled for commit and push protection

## CI

GitHub Actions runs:

- `cargo fmt --check`
- `cargo check`
- `cargo test`
- `pwsh -NoProfile -File scripts/audit-public-surface.ps1`

The `main` branch is protected and requires:

- pull request based changes
- resolved conversations
- passing `ci`

## Planning

Maintainer planning files live outside the repository, following the same pattern as `winsmux`.

- `scripts/sync-roadmap.ps1` reads `backlog.yaml` and writes `ROADMAP.md`
- `scripts/validate-planning.ps1` checks `backlog.yaml` and `roadmap-title-ja.psd1` before sync
- `scripts/setup-planning.ps1` creates the external planning root, writes the marker, copies examples, and runs the first sync
- `scripts/planning-paths.ps1` resolves the planning root from `CODEX_CHANNELS_PLANNING_ROOT` or `%LOCALAPPDATA%\\codex-channels\\planning-root.txt`
- tracked files under `tasks/` are example-only bootstrap files

Files resolved by the planning root:

- `backlog.yaml`
- `ROADMAP.md`
- `roadmap-title-ja.psd1`

## Repository Layout

```text
src/main.rs            startup and CLI entry
src/cli.rs             CLI parsing for secrets and service control
src/config.rs          config types and validation
src/checks.rs          completion-check runner
src/store.rs           SQLite persistence
src/telegram.rs        Telegram Bot API client
src/codex.rs           Codex CLI execution
src/engine.rs          lane execution loop
src/windows_secret.rs  DPAPI secret storage
src/service.rs         Windows service host
tests/checks_runner.rs completion-check tests
tests/roadmap_sync.rs  roadmap sync tests
```
