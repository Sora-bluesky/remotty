# /remotty-smoke-approval-accept

Run the manual approval-accept smoke against Telegram without printing any secret values.

## Workflow

1. Work from the repo root.
2. Run `/remotty-live-env-check` first.
3. Confirm the token is `set` or `stored`.
4. Confirm `LIVE_TELEGRAM_CHAT_ID` and `LIVE_TELEGRAM_SENDER_ID` are `set` or `inferred`.
5. Run `cargo run -- telegram smoke approval accept --config bridge.toml`.
6. Follow the local terminal guidance and use Telegram to press `承認` when the pending request appears.
7. Confirm that the smoke finished with a success message.

## Output requirements

- Never print secret values.
- If the smoke stops on a webhook check, explain how to switch back to polling.
- If another poller is already active, tell the user to stop it before retrying.
