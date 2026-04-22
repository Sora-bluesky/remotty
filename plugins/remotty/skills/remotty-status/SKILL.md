---
name: remotty-status
description: Check remotty bridge status. Use when the user asks whether remotty is running or asks for bridge status.
---

# remotty status

Check local status.

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

4. Summarize whether a bridge is likely running.
5. State whether the allowlist check passed.
6. Do not print secrets.
