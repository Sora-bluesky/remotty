---
name: remotty-policy-allowlist
description: Apply or check the Telegram allowlist policy for remotty. Use after pairing or when the user asks to lock down Telegram access.
---

# remotty policy allowlist

Apply the allowlist workflow and show the allowed sender state.

1. Resolve the config path:

```powershell
$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"
```

2. Run:

```powershell
remotty telegram policy allowlist --config $configPath
```

3. Explain that only listed senders are allowed after pairing.
4. Do not print secrets.
