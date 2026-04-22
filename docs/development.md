# Development

This page is for contributors and maintainers.
User setup instructions stay in the main [README](../README.md).

## Checks

```powershell
cargo fmt --check
cargo test
cargo check
node --check npm/install.js
node --check bin/remotty.js
pwsh -NoProfile -File scripts/audit-public-surface.ps1
pwsh -NoProfile -File scripts/audit-secret-surface.ps1
pwsh -NoProfile -File scripts/audit-doc-terminology.ps1
```

Run the full history secret scan before release work:

```powershell
gitleaks git --log-opts=--all --redact --verbose .
```

## npm Registry Publish

GitHub Releases include `remotty.tgz` and a versioned tarball such as `remotty-0.1.21.tgz`.
The release workflow publishes the versioned tarball to npm when the repository has an Actions secret named `NPM_TOKEN`.

Create the token from an npm account that owns the `remotty` package.
Add it in GitHub under **Settings > Secrets and variables > Actions > New repository secret**.

```text
Name: NPM_TOKEN
Secret: npm token value
```

Without that secret, the GitHub Release still succeeds and npm publishing is skipped.

For a manual publish from a maintainer machine, use:

```powershell
npm publish .\release\remotty.tgz
```

Run either publish path only from an npm account that owns the `remotty` package.

## Optional Manual Smoke

The manual smoke run is opt-in and does not run in CI.
Configure `remotty` first:

1. Run `/remotty-configure` to store the Telegram bot token in Windows protected storage.
2. Run `/remotty-access-pair <code>` to add your Telegram sender to the local allowlist.
3. Run `/remotty-live-env-check` to confirm the live smoke can resolve its inputs.

The smoke command can read the bot token from the configured secret and infer a single paired private sender.
If `LIVE_WORKSPACE` is not set, it uses `target/live-smoke-workspace` and creates the `.remotty-live-smoke-ok` marker there.
`/remotty-live-env-check` also checks whether the bot is in polling mode.
It reports `polling-ready` when no webhook is configured and `webhook-configured` when the bot must be switched back before a smoke run.

Only set `LIVE_*` variables when you need to override the defaults.
Do not paste secret values into chat, and do not share terminal screenshots that include them.

Override environment variables:

- `LIVE_TELEGRAM_BOT_TOKEN`
- `LIVE_TELEGRAM_CHAT_ID`
- `LIVE_TELEGRAM_SENDER_ID`
- `LIVE_WORKSPACE`

Optional environment variables:

- `LIVE_CODEX_BIN`
- `LIVE_CODEX_PROFILE`
- `LIVE_TIMEOUT_SEC`
- `LIVE_APPROVAL_MODE`

Check the environment first:

```powershell
remotty telegram live-env-check
```

For a non-default config file:

```powershell
remotty telegram live-env-check --config bridge.local.toml
```

Then run the approval-accept smoke:

```powershell
remotty telegram smoke approval accept --config bridge.toml
```

For the approval-decline smoke:

```powershell
$env:LIVE_APPROVAL_MODE = "app_server"
remotty telegram smoke approval decline --config bridge.toml
```

Use a dedicated test bot, chat, and smoke workspace when possible.
If a smoke run reports a polling conflict, stop the other `remotty` process that is reading the same bot before retrying.

## Repository Layout

```text
remotty/
├── src/                    # bridge runtime, Telegram client, Codex runner
├── tests/                  # config, fake Telegram, and public-surface tests
├── scripts/                # maintenance and validation scripts
├── bridge.toml             # local configuration starter
├── README.md               # English README
└── README.ja.md            # Japanese README
```
