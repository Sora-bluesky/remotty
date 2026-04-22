---
name: remotty-configure
description: Store the Telegram bot token for remotty in Windows protected storage. Use when the user asks to configure remotty, save a bot token, replace a token, or run remotty configure.
---

# remotty configure

Store the Telegram bot token without printing it.

1. Resolve the config path:

```powershell
$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"
```

2. Start an interactive PowerShell window for hidden token input:

```powershell
Start-Process pwsh -ArgumentList @(
  "-NoProfile",
  "-NoExit",
  "-Command",
  "remotty telegram configure --config `"$env:APPDATA\remotty\bridge.toml`"; Read-Host 'Press Enter to close'"
)
```

3. Tell the user to enter the token only in that PowerShell window.
4. Do not ask the user to paste the token into Codex chat.
5. After the command finishes, confirm that the token was stored.
