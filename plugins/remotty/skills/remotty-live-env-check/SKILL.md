---
name: remotty-live-env-check
description: Check the Telegram environment for remotty. Use when the bot does not reply, setup is unclear, or the user asks for a live environment check.
---

# remotty live env check

Check Telegram setup without printing secrets.

1. Resolve the config path:

```powershell
$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"
```

2. Run:

```powershell
remotty telegram live-env-check --config $configPath
```

3. Explain `stored` as token available in Windows protected storage.
4. Explain webhook status as `polling-ready` or `webhook-configured`.
5. Do not print the token.
