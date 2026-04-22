# /remotty-policy-allowlist

Show the active Telegram allowlist for this repo.

## Workflow

1. Resolve the user config path: `$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"`.
2. Run `remotty telegram policy allowlist --config $configPath`.
3. Summarize the allowed sender IDs.

Only for repo contributors: if the `remotty` command is unavailable in a source checkout, fall back to
`cargo run -- telegram policy allowlist --config $configPath`.

## Output requirements

- State that allowlist mode is enforced.
- Show the allowed sender IDs only.
- If no senders are allowed yet, recommend `/remotty-access-pair <code>` when the bridge can reply with a code.
- If the bridge cannot reply with a code, recommend `/remotty-pair`.
