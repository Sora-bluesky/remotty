---
name: remotty-status
description: Check remotty bridge status. Use when the user asks whether remotty is running or asks for bridge status.
---

# remotty status

Check local status.
The supported Telegram flow is for Codex CLI.

1. Run:

```powershell
remotty service status
```

2. Also list likely local processes:

```powershell
Get-Process remotty, codex -ErrorAction SilentlyContinue | Select-Object Id,ProcessName,Path
```

3. Check whether Telegram access is locked down:

```powershell
$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"
remotty telegram policy allowlist --config $configPath
```

4. Run the live environment check:

```powershell
$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"
remotty telegram live-env-check --config $configPath
```

5. Summarize whether a bridge is likely running.
6. State whether the allowlist check passed.
7. If the bridge was just started, tell the user to look for:

```text
Listening for Telegram channel messages from: remotty:telegram
```

8. Do not print secrets.
