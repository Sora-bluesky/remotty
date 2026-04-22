# /remotty-smoke-approval-accept

Run the manual approval-accept smoke against Telegram without printing any secret values.

## Workflow

1. Use the user config under `%APPDATA%\remotty`.
2. Run `/remotty-live-env-check` first.
3. Confirm the token is `set` or `stored`.
4. Confirm `LIVE_TELEGRAM_CHAT_ID` and `LIVE_TELEGRAM_SENDER_ID` are `set` or `inferred`.
5. Resolve the user config path: `$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"`.
6. Tell the user that this command creates a temporary `app_server` run and does not require changing their normal `codex.transport`.
7. Run `remotty telegram smoke approval accept --config $configPath`.
8. Follow the local terminal guidance and use Telegram to press `Approve` when the pending request appears.
9. Confirm that the smoke finished with a success message.

Use the installed `remotty` command for this check. In a source checkout, build or install the package before running manual smoke.

## Output requirements

- Never print secret values.
- If the smoke stops on a webhook check, explain how to switch back to polling.
- If another poller is already active, tell the user to stop it before retrying.
