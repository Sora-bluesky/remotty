# Upgrade Notes

Use this page if you already have an existing `remotty` config.

New installs should follow the [Telegram Quickstart](telegram-quickstart.md).

## Recommended Setting

Open `%APPDATA%\remotty\bridge.toml`.

For the normal Telegram-to-thread flow, use:

```toml
[codex]
transport = "app_server"
```

This lets Telegram continue the Codex thread you select with:

```text
/remotty-sessions
/remotty-sessions <thread title or ID>
```

## If Your Config Uses `exec`

`exec` still works.
It starts a separate Codex CLI run for Telegram work.

If that is what you want, keep:

```toml
[codex]
transport = "exec"
```

For details, see [Advanced CLI Mode](exec-transport.md).

## Runtime Files

`remotty` stores its own state under `%APPDATA%\remotty`.

It should not create runtime files inside your project repository. Codex itself
may still edit project files when you ask it to.
