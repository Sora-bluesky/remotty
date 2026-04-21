# /remotty-live-env-check

Check whether the live smoke can resolve its inputs without printing secret values.

## Workflow

1. Work from the repo root.
2. Run `cargo run -- telegram live-env-check`.
3. Summarize which values are set, stored, inferred, defaulted, missing, or ambiguous.

## Output requirements

- Never print secret values.
- Mention that `stored` means the token is available from `/remotty-configure`.
- Mention that `inferred` means a single paired sender was found.
- Mention that `default` workspace means `target/live-smoke-workspace`.
