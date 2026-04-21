@{
    Releases = @(
        @{
            Version = "0.1.0"
            Commit = "4d4c2d39d0302b9113f165d8c6e970cfebfb2cac"
            Title = "Bridge foundation"
            Notes = @(
                "Initial Windows bridge foundation with Telegram polling, SQLite lane state, and Codex execution."
                "Added DPAPI-backed secret storage and the first Windows service entry points."
            )
        }
        @{
            Version = "0.1.1"
            Commit = "c08379ec95f27e046dfa8148422f69e71510c10e"
            Title = "Follow-up checks and operator controls"
            Notes = @(
                "Added completion checks, attachment handling, and Telegram-side control commands."
                "Added Windows service management commands so the bridge can be installed and operated as a service."
            )
        }
        @{
            Version = "0.1.2"
            Commit = "52eb8965b44b4346f0a459ab34dd9307920fd90a"
            Title = "Repository hygiene safeguards"
            Notes = @(
                "Added checks that keep private operational files out of published source."
                "Improved release preparation scripts for repeatable publishing."
            )
        }
        @{
            Version = "0.1.3"
            Commit = "f45ffce75a13a214d970b5f050e8c010932b2563"
            Title = "Repository hygiene hardening"
            Notes = @(
                "Improved validation around release preparation inputs."
                "Reduced local-environment assumptions in repository automation."
            )
        }
        @{
            Version = "0.1.4"
            Commit = "1ac53b5f1c0c8cbad4ffad1f965ae3a8cf21d776"
            Title = "Automatic turn modes"
            Notes = @(
                "Added automatic continuation modes, including completion checks, infinite loops, and max-turn budgets."
                "Tightened non-zero exit handling and auto-turn completion messaging."
            )
        }
        @{
            Version = "0.1.5"
            Commit = "b8e7340b79ebec9b3b9a706cde30b59c95c43646"
            Title = "Live smoke foundation"
            Notes = @(
                "Added an opt-in live end-to-end smoke test against Telegram and Codex."
                "Made the Codex profile optional for live runs."
            )
        }
        @{
            Version = "0.1.6"
            Commit = "4895915fc61159fccb7e0a13b6894fc882b3f5a5"
            Title = "Public docs and secret guardrails"
            Notes = @(
                "Refreshed the public README pair for non-engineer readers."
                "Added secret-surface audits in both CI and local git hooks."
            )
        }
        @{
            Version = "0.1.7"
            Commit = "ba9aeaf48be2fcb5c7bd82fbd19490359c83b995"
            Title = "Workspace switching"
            Notes = @(
                "Added per-chat workspace switching through `/workspace`."
                "Rounded out the operator command set for day-to-day bridge operation."
            )
        }
        @{
            Version = "0.1.8"
            Commit = "4575d0c053b4f2aa88030d9430abf2644077b2a0"
            Title = "Release automation"
            Notes = @(
                "Added scripted version bumps, release note generation, and historical release backfill."
                "Published Windows x64 and arm64 binaries through tag-driven GitHub Actions releases."
            )
        }
        @{
            Version = "0.1.9"
            Commit = "051981b0db4c71b8141c9b0099316985cdd09a70"
            Title = "Approval notification foundation"
            Notes = @(
                "Added the `app_server` transport and approval request storage for Telegram-driven approval loops."
                "Sent pending approvals to Telegram and added the first live approval end-to-end harness."
            )
        }
        @{
            Version = "0.1.10"
            Commit = "333c8530f736f70eef084ba4950580fbce5c23cc"
            Title = "Telegram approval actions"
            Notes = @(
                "Added Telegram-side approve and deny actions, callback handling, and approval resume flow."
                "Hardened approval state recovery, persistence, and operator-facing status updates."
            )
        }
        @{
            Version = "0.1.11"
            Commit = "f4eb46dd2ba67ca193e54b485d6bb4d2e78ca71e"
            Title = "Approval callback hardening"
            Notes = @(
                "Improved Telegram approval callback feedback, restart invalidation, and approval state tests."
                "Kept approval history intact while making stale or repeated approval actions safer."
            )
        }
        @{
            Version = "0.1.12"
            Commit = "927774a512b5fe3b9ace0cd378458b6a7ab2a6af"
            Title = "Tool input approval summaries"
            Notes = @(
                "Sanitized tool-input approval summaries so sensitive prompt details are not forwarded to Telegram."
                "Tightened text truncation behavior and added coverage for summary length boundaries."
            )
        }
        @{
            Version = "0.1.13"
            Commit = "3640e9ab0a4b23c595592809f515f496a70ea246"
            Title = "Plugin-first Telegram pairing"
            Notes = @(
                "Added bot-issued Telegram pairing codes for the Codex plugin setup flow."
                "Allowed live approval smoke runs to infer saved bot credentials, paired sender IDs, and a default live workspace."
                "Updated Codex exec JSON parsing and suppressed extra Windows terminal windows during bridge runs."
                "Hardened release and public-surface audit automation for Windows output handling."
            )
        }
        @{
            Version = "0.1.14"
            Commit = "adf7640e8692ac23c99cbdcbf57abee4d10f55be"
            Title = "Guarded Telegram polling"
            Notes = @(
                "Centralized Telegram update reads behind a guarded poller that resolves bot identity before polling."
                "Routed bridge runtime, legacy pairing, and live smoke guard checks through the same poller path."
                "Added CI-backed Gitleaks scanning and documented the layered secret checks."
                "Hardened release automation so Cargo.lock and empty planning updates are handled consistently."
            )
        }
        @{
            Version = "0.1.15"
            Commit = "678693b2c67fd3a443e36ccf656780a52844570c"
            Title = "npm installation package"
            Notes = @(
                "Added an npm package entry point that installs the matching Windows release binary."
                "Included the local Codex plugin, bridge starter config, and npm installer in the package tarball."
                "Attached both a stable remotty.tgz package and a versioned package tarball to GitHub Releases."
                "Updated public setup docs to use the GitHub Release package before npm registry publishing."
            )
        }
    )
}
