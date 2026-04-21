# /remotty-smoke-approval-decline

Run the manual approval-decline smoke against Telegram without printing any secret values.

## Workflow

1. Work from the package or repo root that contains `bridge.toml`.
2. Run `/remotty-live-env-check` first.
3. Confirm the token is `set` or `stored`.
4. Confirm `LIVE_TELEGRAM_CHAT_ID` and `LIVE_TELEGRAM_SENDER_ID` are `set` or `inferred`.
5. Run `remotty telegram smoke approval decline --config bridge.toml`.
6. Follow the local terminal guidance and use Telegram to press `非承認` when the pending request appears.
7. Confirm that the smoke finished with a success message and no target file was created.

Use the installed `remotty` command for this check. In a source checkout, build or install the package before running manual smoke.

## Output requirements

- Never print secret values.
- If the smoke stops on a webhook check, explain how to switch back to polling.
- If another poller is already active, tell the user to stop it before retrying.
