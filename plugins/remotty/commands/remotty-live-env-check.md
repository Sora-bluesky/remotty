# /remotty-live-env-check

Check whether the live smoke can resolve its inputs without printing secret values.

## Workflow

1. Resolve the user config path: `$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"`.
2. Run `remotty telegram live-env-check --config $configPath`.
3. If a different config is needed, run `remotty telegram live-env-check --config <path>`.
4. Summarize which values are set, stored, inferred, defaulted, missing, or ambiguous.
5. Summarize the webhook status as `polling-ready`, `webhook-configured`, or `unknown`.

Only for repo contributors: if the `remotty` command is unavailable in a source checkout, fall back to
`cargo run -- telegram live-env-check --config $configPath`.

## Output requirements

- Never print secret values.
- Mention that `stored` means the token is available from `/remotty-configure`.
- Mention that `inferred` means a single paired sender was found.
- Mention that `default` workspace means `target/live-smoke-workspace`.
- Mention that `webhook-configured` must be resolved before running manual smoke.
