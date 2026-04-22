# /remotty-pair

Fallback pairing path for cases where the running bridge cannot reply with a pairing code.

## Workflow

1. Prefer `/remotty-access-pair <code>` when the running bridge can reply with a code.
2. Resolve the user config path: `$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"`.
3. Confirm the bridge is not already running. If it is running, stop it before pairing.
4. Run `remotty telegram pair --config $configPath`.
5. Read the one-time pairing code shown in the local terminal.
6. Ask the user to send `/pair <code>` to the bot from Telegram.
7. Wait for the local terminal to show the matched `sender_id` and `chat_id`.
8. Confirm that the sender was added to the allowlist.

Only for repo contributors: if the `remotty` command is unavailable in a source checkout, fall back to
`cargo run -- telegram pair --config $configPath`.

## Output requirements

- Report the paired sender ID after success.
- Do not print the bot token.
- If pairing fails because another poller is active, tell the user to stop the running bridge and retry.
- If no matching Telegram message exists, tell the user to send `/pair <code>` and retry.
