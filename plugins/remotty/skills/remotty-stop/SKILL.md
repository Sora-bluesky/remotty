---
name: remotty-stop
description: Stop a running remotty bridge or service. Use when the user asks to stop remotty or fix a Telegram polling conflict.
---

# remotty stop

Stop the bridge or identify the process to stop.
The supported Telegram flow is for Codex CLI.

1. Try:

```powershell
remotty service stop
```

2. If no service is active, list likely processes:

```powershell
Get-Process remotty, codex -ErrorAction SilentlyContinue | Select-Object Id,ProcessName,Path
```

3. Ask before stopping an unrelated process.
4. If the process is clearly the bridge window, stop only that process.
