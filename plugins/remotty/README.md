# remotty plugin

This local plugin is optional.
The supported Telegram flow is the Codex CLI quickstart in the project README.
The plugin only exposes the same setup tasks as Codex App skills for Codex App users.
The bridge itself talks to your local `codex` CLI.

In Codex App, type `@`, select `remotty`, then describe the setup task.
Codex CLI users should follow the [Telegram Quickstart](../../docs/telegram-quickstart.md)
directly from PowerShell instead.

Use it for:

- token setup without echoing the token in the terminal
- current project registration without hand-editing `bridge.toml`
- pairing a Telegram sender into the allowlist
- bridge start, stop, and status workflows
- live environment checks before manual smoke runs

Example requests:

- `Store the Telegram bot token`
- `Register this project with remotty`
- `Start the bridge`
- `Pair with the code shown in Telegram`
- `Lock down Telegram access to the allowlist`
- `Check status`
- `List Codex threads`

The plugin lives in `plugins/remotty/` and is listed by the package-local
marketplace in `.agents/plugins/marketplace.json`.
