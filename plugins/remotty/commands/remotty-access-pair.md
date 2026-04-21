# /remotty-access-pair

Authorize the Telegram sender that received a pairing code from the bot.

## Workflow

1. Ask the user to send any message to the Telegram bot.
2. Wait for the bot to reply with a `remotty pairing code`.
3. Run `cargo run -- telegram access-pair <code> --config bridge.toml`.
4. Confirm that the sender was added to the allowlist.

## Output requirements

- Never print the bot token.
- Report the paired sender ID after success.
- If no matching code exists, ask the user to send a fresh Telegram message and retry.
