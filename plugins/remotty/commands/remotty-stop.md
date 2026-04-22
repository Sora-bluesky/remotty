# /remotty-stop

Stop the local bridge.

## Workflow

1. If the Windows service is installed and running, run `remotty service stop`.
2. Otherwise explain that the foreground process must be stopped in its own terminal with Ctrl+C or by closing that terminal.

Only for repo contributors: if the `remotty` command is unavailable in a source checkout, fall back to
`cargo run -- service stop`.

## Output requirements

- State whether the service was stopped.
- If only a foreground process exists, say so clearly.
