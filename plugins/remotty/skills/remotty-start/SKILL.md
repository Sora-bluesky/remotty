---
name: remotty-start
description: Start the remotty bridge for Telegram. Use when the user asks to start remotty, run the bridge, or connect Telegram to Codex CLI.
---

# remotty start

Start Remote Control from the current project.
The supported Telegram flow is for Codex CLI.

1. Resolve the config path for checks and troubleshooting:

```powershell
$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"
```

2. If `remotty service status` reports an installed service, run:

```powershell
remotty service start
```

3. Otherwise start an interactive window in the project directory:

```powershell
Start-Process pwsh -ArgumentList @(
  "-NoProfile",
  "-NoExit",
  "-Command",
  "remotty remote-control; Read-Host 'Press Enter to close'"
)
```

4. Confirm that the Remote Control window shows:

```text
Remote Control active
Listening for Telegram channel messages from: remotty:telegram
```

5. Tell the user to keep the Remote Control window open while using Telegram.
6. If a polling conflict appears, explain that another bridge is reading the same bot.
