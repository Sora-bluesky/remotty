# Remote Companion Direction

`remotty` is a Telegram remote companion for Codex on Windows.
It is not a replacement for the Codex App.

The Codex App or Codex CLI remains the main workspace for full transcripts,
diff review, project selection, and rich task control.
`remotty` stays focused on the smaller surface that is useful away from the desk:
notifications, approval relay, concise status, and short steering messages.
Its future direction is to fit Codex App Server based workflows without
pretending to be the full Codex interface.

## Product Positioning

Use the Codex App or Codex CLI as the main workspace.
Use `remotty` as the pocket remote.

`remotty` should answer a few narrow questions quickly:

- Is Codex still running?
- What is it doing now?
- Is approval required?
- Should the run pause or continue?
- Do I need to send one short steering message?
- Did the work finish or fail?

## Current Scope

The current public flow connects Telegram to a local Codex CLI session on
Windows.
The recommended transport is:

```toml
[codex]
transport = "app_server"
```

In this mode, `remotty` talks to the local `codex` command and uses the local
`app_server` transport for thread continuation, approval relay, and follow-up
messages.

Connecting to an already running Codex App Server is future direction.
It is not required by the current quickstart.

The `exec` transport remains available as an advanced fallback.

## Non-Goals

`remotty` should not become:

- a full transcript UI
- a rich diff viewer
- an editor integration
- a filesystem browser
- a worktree manager
- a project management UI
- a clone of the Codex App

Those belong in richer Codex surfaces.

## Roadmap Shape

Future work should keep the same product boundary:

- connect to an existing Codex App Server when that is available
- spawn a local app server when running unattended as a Windows service
- attach Telegram chats to Codex threads
- maintain a throttled live status card
- relay approval requests with sender allowlist checks
- support short steering messages for running turns
- summarize diffs and recent events without flooding Telegram
- show goal state when the app-server API exposes it safely

When an App Server API is unclear or experimental, `remotty` should use only the
safe, explicit API surface that is available.
It should not inject unknown slash commands into a thread to guess behavior.

## Security Boundary

Telegram access must remain sender based.
`chat_id` alone is not enough, especially in group chats.

Required defaults:

- bot tokens stay in Windows protected storage
- only paired senders on the allowlist can operate Codex
- approval actions require an allowed sender
- runtime state stays under `%APPDATA%\remotty`
- project files stay separate from `remotty` runtime files

## Notification Policy

New Telegram messages should be reserved for important events:

- approval required
- work completed
- turn failed
- app-server disconnected
- long idle
- user input required

Normal progress, output deltas, and repeated status updates should be grouped
into an edited live card or a compact event log.
