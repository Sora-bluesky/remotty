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
        @{
            Version = "0.1.16"
            Commit = "8d3cb74088955e5531918f89f36ca4be748dc90a"
            Title = "Fake Telegram bridge integration tests"
            Notes = @(
                "Added fake Telegram integration coverage that runs the real bridge against a local Telegram-compatible test server."
                "Verified normal message round trips through a mocked Codex executable."
                "Covered Telegram approval callback accept and decline flows through the fake server."
                "Kept the integration harness local-only so release checks do not require real Telegram credentials."
            )
        }
        @{
            Version = "0.1.17"
            Commit = "60ab6ca122a4d9a2eb48ee3e53dd28080b310dfb"
            Title = "Manual Telegram smoke checks"
            Notes = @(
                "Added ``telegram live-env-check --config <path>`` to validate saved credentials, paired senders, live workspace readiness, and Telegram webhook state before manual smoke runs."
                "Improved polling conflict errors with Windows process discovery and stop-command hints."
                "Updated plugin smoke commands to favor the installed ``remotty`` command path."
                "Prepared the Telegram setup docs and Codex Remote connections positioning for the next release."
            )
        }
        @{
            Version = "0.1.18"
            Commit = "9b93f50f3cdf41fc6619ac287b9ef501d1b5c22b"
            Title = "Telegram quickstart documentation"
            Notes = @(
                "Added English and Japanese Telegram quickstart guides covering BotFather setup, installation, bridge startup, pairing, allowlists, and troubleshooting."
                "Linked the quickstart from both README files so new users have a shorter setup path."
                "Clarified that Codex Remote connections are for working on SSH targets while remotty is a Telegram bridge into the current working environment."
                "Published the quickstart release with both stable and versioned npm tarballs attached to GitHub Releases."
            )
        }
        @{
            Version = "0.1.19"
            Commit = "213a8222ac049f067ffeaa913b0e8422fa45a9c3"
            Title = "Local fakechat demo"
            Notes = @(
                "Added ``remotty demo fakechat`` for a localhost browser chat UI that calls the local Codex CLI without Telegram credentials."
                "Added English and Japanese fakechat demo docs and linked them from both README files."
                "Added the ``/remotty-fakechat-demo`` plugin command guide."
                "Included ``docs/`` in the npm package so quickstart and demo links work after installation."
            )
        }
        @{
            Version = "0.1.23"
            Commit = "1d81d548fca096b9bedc941cc20e5b1deb5dd3be"
            Title = "Codex Telegram setup polish"
            Notes = @(
                "Clarified the npm install, config copy, Codex App plugin setup, pairing, and allowlist flow across public docs."
                "Updated Telegram approval notices and buttons to use global-friendly Approve and Deny labels."
                "Resolved relative storage paths from the copied config file location so npm installs keep runtime state under the user config directory."
                "Reviewed shipped public docs and bridge.toml with Opus, then verified the package contents and secret scans before release."
            )
        }
        @{
            Version = "0.1.24"
            Commit = "687dc5e8eaac798d1054a22488c383780fb41ce5"
            Title = "Simpler npm install docs"
            Notes = @(
                "Removed the GitHub Release tarball fallback from user-facing setup docs so the npm registry install path stays primary."
                "Split the npm install, package-folder, and config-copy commands into shorter labeled steps."
                "Updated public-surface tests to keep the README focused on the short npm install command."
            )
        }
        @{
            Version = "0.2.0"
            Commit = "4195c31a1a027e01fc02220aeed469d5a6f527bc"
            Title = "Codex App session feasibility"
            Notes = @(
                "Documented the product shift toward returning to an existing Codex App session instead of starting a separate Codex CLI run."
                "Added guarded app-server version checks and thread listing support as the first supported local session interface."
                "Added Telegram session-selection commands so chats can bind to an available Codex App thread when the interface is present."
                "Added a release coverage audit that blocks the next source version until the completed release is recorded and tagged."
            )
        }
        @{
            Version = "0.2.1"
            Commit = "74ed8f4fd48b9ed8ce7584981633afff707bdcfe"
            Title = "Saved Codex thread selection"
            Notes = @(
                "Released saved Codex thread listing through the app-server interface."
                "Added Telegram commands for selecting the saved thread used by a chat."
                "Stored chat-to-thread bindings under the user config directory instead of the project directory."
                "Marked v0.2.1 as a required release before later source versions can pass CI."
            )
        }
        @{
            Version = "0.2.2"
            Commit = "18276cf551f757e20198a90ef7692f58cbfa61fb"
            Title = "Saved Codex thread message relay"
            Notes = @(
                "Resumed the selected saved Codex thread before each Telegram turn."
                "Relayed Telegram text through app-server turn/start on the resumed thread."
                "Returned the resumed thread reply to the same Telegram chat."
                "Added turn/steer follow-up input while an app-server turn is active."
                "Extended fakechat with saved-thread relay checks that do not need Telegram credentials."
            )
        }
        @{
            Version = "0.2.3"
            Commit = "1a3b628db78fe136a5116cecd75db1e0cd0a191d"
            Title = "Permission relay and worktree safety"
            Notes = @(
                "Kept app-server permission requests relayed to Telegram."
                "Kept Telegram approval and denial decisions routed back to the same turn."
                "Kept session control limited to paired and allowlisted Telegram senders."
                "Warned before relaying work into a Git repository with uncommitted changes."
            )
        }
        @{
            Version = "0.2.4"
            Commit = "897deaa418617b14ea26c68cea50b8692f92572d"
            Title = "Saved-thread setup documentation"
            Notes = @(
                "Reworked the README files around returning to a saved Codex thread from Telegram."
                "Updated the Telegram quickstart guides for app-server setup, pairing, thread selection, and approvals."
                "Added migration notes for moving from the v0.1 separate-run bridge to the v0.2 saved-thread relay."
                "Added public-surface checks that keep the saved-thread setup and migration docs present before release."
            )
        }
        @{
            Version = "0.2.5"
            Commit = "c622a700b508f3cdb19f4f362c6dbfd934e4e60b"
            Title = "Final-state public docs"
            Notes = @(
                "Reframed the README files around the current user flow instead of release history."
                "Moved the advanced exec transport and upgrade notes out of the main setup path."
                "Simplified the Telegram quickstart so normal users do not choose between transports."
                "Changed the starter config to use the Codex thread relay transport by default."
            )
        }
        @{
            Version = "0.2.6"
            Commit = "733536e0c98779282070e7135be7dabfa7448c1b"
            Title = "App-server relay hardening"
            Notes = @(
                "Declined unsupported app-server requests by default so unknown approval prompts do not hang silently."
                "Added timeouts around app-server control writes and request-response calls."
                "Limited active app-server work to one turn per selected Codex thread while still allowing follow-up steering."
                "Split long Telegram replies into message-sized chunks when Codex returns a large answer."
            )
        }
        @{
            Version = "0.2.7"
            Commit = "5b2dd7c2962d64696c3c5898e7965ffddbab04f5"
            Title = "Codex App project registration"
            Notes = @(
                "Added a config workspace upsert command that registers the current project without hand-editing bridge.toml."
                "Added a remotty plugin command for saving the current Codex App project as the default workspace."
                "Stopped bridge startup when the starter workspace is still unconfigured."
                "Reworked the Telegram quickstart around opening the target project before configuring remotty."
            )
        }
        @{
            Version = "0.2.8"
            Commit = "ea3858e98e0b0cca88e5a3ee59d6162b2aad3b8d"
            Title = "Plugin-first setup documentation"
            Notes = @(
                "Added Codex plugin setup guidance for app and CLI users."
                "Clarified which setup steps are one-time, per project, and per Telegram chat."
                "Added quickstart screenshots and Q&A sections for security and troubleshooting."
                "Removed internal development documents from the public package surface."
            )
        }
        @{
            Version = "0.2.9"
            Commit = "5cdfbe048126207b2f7aaf1bdf752bd198e45d23"
            Title = "Plugin version refresh fix"
            Notes = @(
                "Fixed the bundled Codex plugin manifest that still reported the previous version after npm install -g remotty."
                "Added release checks that block a release when package and plugin versions drift."
                "Documented the PowerShell fallback when @remotty is not visible in the current Codex App chat."
                "Documented update steps in README.md so installed plugins can pick up the new version."
                "Updated the release bump script so future plugin versions are changed with the package version."
            )
        }
        @{
            Version = "0.2.10"
            Commit = "218720c3e952d7efa1ca28cf0a84c5486ff5c944"
            Title = "Active follow-up queueing"
            Notes = @(
                "Queued Telegram text follow-ups while the selected app-server turn is still active."
                "Recovered stale running lanes after a bridge restart instead of blocking the next message."
                "Added tests for saved-thread follow-ups, live turn lookup precedence, and stale lane recovery."
                "Published matching Cargo, npm, and Codex plugin metadata for v0.2.10."
            )
        }
        @{
            Version = "0.2.11"
            Commit = "23f49782eca0a88a13e28fd71541836f9c233d09"
            Title = "Visible active follow-ups"
            Notes = @(
                "Sent Telegram follow-ups into the visible Codex app-server pending input."
                "Waited for app-server acknowledgement before reporting follow-up success."
                "Rejected stale stored turn ids after bridge restart instead of reporting false success."
                "Added tests for rejected steer responses, stale stored turns, and run ownership."
            )
        }
        @{
            Version = "0.2.13"
            Commit = "529c21ff2bbfcc385f1ac45d30fa91ab017169b2"
            Title = "CLI Telegram channel startup"
            Notes = @(
                "Persisted Telegram follow-up input so messages sent while Codex is working are queued for the next turn."
                "Printed a CLI-visible Telegram listening banner when the bridge starts successfully."
                "Made the supported Telegram flow Codex CLI plus a running remotty process, with Codex App setup kept optional."
                "Removed the duplicate manual Codex thread selection step from the public Telegram quickstart."
            )
        }
        @{
            Version = "0.2.14"
            Commit = "ed17a4566c67621ea057eea3310860bdfcb7b7e9"
            Title = "Telegram bridge documentation and dependency refresh"
            Notes = @(
                "Reframed the public README and quickstart docs around Telegram bridge wording for non-engineer readers."
                "Clarified that the main Codex work surface is not a specific mobile app and is not replaced by remotty."
                "Updated the public hero image and public-facing checks to keep the Telegram bridge positioning consistent."
                "Refreshed underlying dependency libraries to their latest patch versions."
            )
        }
    )
}
