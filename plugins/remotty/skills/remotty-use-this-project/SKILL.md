---
name: remotty-use-this-project
description: Register the current Codex workspace as a remotty workspace. Use when the user asks to use this project, register this project, or set the current project for remotty.
---

# remotty use this project

Register the current workspace without writing to the repository.

1. Resolve the config path:

```powershell
$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"
```

2. Resolve the current workspace from the thread.
3. Run:

```powershell
remotty config workspace upsert --config $configPath --path (Get-Location).Path
```

4. Run `git status --short --branch` in the project.
5. Report that config was saved under `%APPDATA%\remotty`.
6. Report whether the repository gained any tracked or untracked files.
