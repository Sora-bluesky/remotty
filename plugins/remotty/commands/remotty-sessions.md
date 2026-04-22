# /remotty-sessions

List saved Codex threads and bind a Telegram chat to one of them.

## Steps

1. Resolve the user config path: `$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"`.
2. Run `remotty telegram sessions --config $configPath`.
3. Ask the user to choose a thread id.
4. Tell the user to send `/remotty-sessions <thread_id>` in the target Telegram chat.

Only for repo contributors: if the `remotty` command is unavailable in a source checkout, fall back to `cargo run -- telegram sessions --config $configPath`.

## Response

- Do not print raw local secrets.
- Mention that selection happens from Telegram.
- Mention that the binding is stored under the configured remotty state database.
- If a thread id is missing, ask the user to refresh with `/remotty-sessions`.
