# remotty plugin

This local plugin wraps the `remotty` bridge with Codex skills.
In Codex App, type `@`, select `remotty`, then describe the setup task.

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
