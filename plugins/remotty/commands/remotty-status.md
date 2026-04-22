# /remotty-status

Inspect the local bridge state.

## Workflow

1. Resolve the user config path: `$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"`.
2. Run `remotty service status`.
3. Also run `remotty telegram policy allowlist --config $configPath`.
4. Summarize the current service state and allowed senders.
5. Mention that service status does not detect a foreground bridge running in another terminal.

Only for repo contributors: if the `remotty` command is unavailable in a source checkout, run the same commands through `cargo run --`.

## Output requirements

- Report service state.
- Report whether allowlist has at least one sender.
- Keep the summary short.
