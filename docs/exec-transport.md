# Advanced CLI Mode

Most users should follow the [Telegram Quickstart](telegram-quickstart.md).

This page is for users who want each Telegram request to start a separate
Codex CLI run.

## When To Use This

Use this mode when:

- you do not need to continue a selected Codex thread
- you want a simple one-request, one-run flow
- your local Codex command does not support `app-server`

## Configure It

Open `%APPDATA%\remotty\bridge.toml`.

Set:

```toml
[codex]
transport = "exec"
```

Keep the same `workspaces` settings from the quickstart.

## Behavior

In this mode, `remotty` calls `codex exec` for Telegram work.

The result still returns to Telegram. The work does not attach to a selected
Codex thread.

Approval buttons for the thread-relay flow are not the main path in this mode.
Use the normal quickstart if you want Telegram to continue a Codex thread.
