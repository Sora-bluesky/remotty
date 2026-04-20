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
            Title = "Planning bootstrap"
            Notes = @(
                "Added public-surface guardrails for tracked planning files."
                "Added one-command planning setup and external roadmap generation."
            )
        }
        @{
            Version = "0.1.3"
            Commit = "f45ffce75a13a214d970b5f050e8c010932b2563"
            Title = "Planning hardening"
            Notes = @(
                "Kept the planning marker stable when sync fails."
                "Validated planning inputs and preferred the canonical Obsidian vault during root discovery."
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
    )
}
