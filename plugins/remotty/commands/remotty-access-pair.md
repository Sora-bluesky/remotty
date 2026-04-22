# /remotty-access-pair

Authorize the Telegram sender that received a pairing code from the bot.

## Workflow

1. Ask the user to send any message to the Telegram bot.
2. Wait for the bot to reply with a `remotty pairing code`.
3. Resolve the user config path: `$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"`.
4. Run `remotty telegram access-pair <code> --config $configPath`.
5. Confirm that the sender was added to the allowlist.

Only for repo contributors: if the `remotty` command is unavailable in a source checkout, fall back to
`cargo run -- telegram access-pair <code> --config $configPath`.

## Output requirements

- Never print the bot token.
- Report the paired sender ID after success.
- If no matching code exists, ask the user to send a fresh Telegram message and retry.
