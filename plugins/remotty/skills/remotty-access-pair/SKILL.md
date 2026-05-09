---
name: remotty-access-pair
description: Pair a Telegram account with remotty using a pairing code. Use when the user gives a remotty pairing code or asks to pair Telegram.
---

# remotty access pair

Pair Telegram with the local config.
The supported Telegram flow is for Codex CLI.

1. Ask for the pairing code if it was not provided.
2. Resolve the config path:

```powershell
$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"
```

3. Run:

```powershell
remotty telegram access-pair <code> --config $configPath
```

4. Confirm the sender was added.
5. Do not print the bot token.
