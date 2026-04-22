# /remotty-start

Start the local bridge.

## Workflow

1. Resolve the user config path: `$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"`.
2. Prefer the Windows service when it is already installed:
   `remotty service start`
3. Otherwise run the foreground bridge with `remotty --config $configPath`.
4. If you use the foreground path, make it clear that the command occupies that terminal until the bridge stops. Tell the user to keep it open and run pairing from the Codex App or another shell.
5. Confirm whether the bridge is running in the foreground or as a service.

Only for repo contributors: if the `remotty` command is unavailable in a source checkout, fall back to
`cargo run -- service start` or `cargo run -- --config $configPath`.

## Output requirements

- State which start path you used.
- If startup fails, report the blocking error clearly.
