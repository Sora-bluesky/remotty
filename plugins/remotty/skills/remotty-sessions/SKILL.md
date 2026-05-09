---
name: remotty-sessions
description: List Codex threads for remotty and explain how to bind a Telegram chat to one. Use when the user asks to select or list Codex sessions or threads.
---

# remotty sessions

List threads and guide Telegram binding.
The supported Telegram flow is for Codex CLI.

1. Resolve the config path:

```powershell
$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"
```

2. Run:

```powershell
remotty telegram sessions --config $configPath
```

3. Ask the user which thread to bind if needed.
4. Tell the user to send this in the target Telegram chat:

```text
/remotty-sessions <thread_id>
```

5. Explain that bindings are stored under `%APPDATA%\remotty`.
