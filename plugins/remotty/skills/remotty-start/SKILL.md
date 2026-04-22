---
name: remotty-start
description: Start the remotty bridge for Telegram. Use when the user asks to start remotty, run the bridge, or connect Telegram to Codex.
---

# remotty start

Start the bridge using the user config.

1. Resolve the config path:

```powershell
$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"
```

2. If `remotty service status` reports an installed service, run:

```powershell
remotty service start
```

3. Otherwise start an interactive window:

```powershell
Start-Process pwsh -ArgumentList @(
  "-NoProfile",
  "-NoExit",
  "-Command",
  "remotty --config `"$env:APPDATA\remotty\bridge.toml`"; Read-Host 'Press Enter to close'"
)
```

4. Tell the user to keep the bridge window open while using Telegram.
5. If a polling conflict appears, explain that another bridge is reading the same bot.
