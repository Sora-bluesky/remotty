# remotty plugin

This local plugin wraps the `remotty` bridge with user-facing Codex commands.

Use it for:

- token setup without echoing the token in the terminal
- pairing a Telegram sender into the allowlist
- bridge start, stop, and status workflows
- live environment checks before manual smoke runs
- approval-accept and approval-decline smoke runs
- local fakechat demo before Telegram setup

Available commands:

- `/remotty-configure`
- `/remotty-access-pair`: pair from a code returned by the running Telegram bridge
- `/remotty-pair`: fallback pairing when the bridge cannot reply with a code
- `/remotty-policy-allowlist`
- `/remotty-start`
- `/remotty-status`
- `/remotty-stop`
- `/remotty-sessions`
- `/remotty-fakechat-demo`
- `/remotty-live-env-check`
- `/remotty-smoke-approval-accept`
- `/remotty-smoke-approval-decline`

The plugin lives in `plugins/remotty/` and is listed by the package-local
marketplace in `.agents/plugins/marketplace.json`.
