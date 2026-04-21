# remotty plugin

This local plugin wraps the `remotty` Rust bridge with user-facing Codex
commands.

Use it for:

- token setup without echoing the token in the terminal
- pairing a Telegram sender into the allowlist
- bridge start, stop, and status workflows
- live environment checks before manual smoke runs
- approval-accept and approval-decline smoke runs

Available commands:

- `/remotty-configure`
- `/remotty-access-pair`
- `/remotty-pair`
- `/remotty-policy-allowlist`
- `/remotty-start`
- `/remotty-status`
- `/remotty-stop`
- `/remotty-live-env-check`
- `/remotty-smoke-approval-accept`
- `/remotty-smoke-approval-decline`

The plugin lives in `plugins/remotty/` and is listed by the repo-local
marketplace in `.agents/plugins/marketplace.json`.
