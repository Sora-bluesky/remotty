[CmdletBinding()]
param(
    [string]$RepoRoot = '',
    [string]$HistoryPath = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'release-common.ps1')

function Resolve-RemoteTagCommit {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Tag
    )

    $remoteTagRefs = git ls-remote origin "refs/tags/$Tag" "refs/tags/$Tag^{}" 2>$null
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to query remote tag $Tag."
    }

    if ([string]::IsNullOrWhiteSpace($remoteTagRefs)) {
        return $null
    }

    $lines = @($remoteTagRefs -split "`r?`n" | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    $peeled = $lines | Where-Object { $_ -match '\^\{\}$' } | Select-Object -First 1
    if ($null -ne $peeled) {
        return ([string]$peeled -split '\s+')[0].Trim()
    }

    return ([string]$lines[0] -split '\s+')[0].Trim()
}

function Wait-ReleaseWorkflowForTag {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Tag,
        [Parameter(Mandatory = $true)]
        [string]$Commit,
        [bool]$RequireRun = $true,
        [int]$TimeoutSeconds = 1800
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $runId = $null

    while ((Get-Date) -lt $deadline) {
        $runsJson = gh run list --workflow Release --event push --commit $Commit --limit 100 --json databaseId,status,conclusion,headBranch,headSha 2>$null
        if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($runsJson)) {
            Start-Sleep -Seconds 5
            continue
        }

        $runs = @()
        try {
            $runs = @($runsJson | ConvertFrom-Json)
        } catch {
            Start-Sleep -Seconds 5
            continue
        }

        $run = $runs | Where-Object { [string]$_.headBranch -eq $Tag } | Select-Object -First 1
        if ($null -eq $run) {
            $run = $runs | Where-Object { [string]$_.headSha -eq $Commit } | Select-Object -First 1
        }
        if ($null -eq $run -or [string]::IsNullOrWhiteSpace([string]$run.databaseId)) {
            Start-Sleep -Seconds 5
            continue
        }

        $runId = [string]$run.databaseId
        while ((Get-Date) -lt $deadline) {
            $runViewJson = gh run view $runId --json status,conclusion 2>$null
            if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($runViewJson)) {
                Start-Sleep -Seconds 10
                continue
            }

            try {
                $runView = $runViewJson | ConvertFrom-Json
            } catch {
                Start-Sleep -Seconds 10
                continue
            }

            if ([string]$runView.status -ne 'completed') {
                Start-Sleep -Seconds 10
                continue
            }

            if ([string]$runView.conclusion -ne 'success') {
                throw "Release workflow failed for $Tag (run id: $runId, conclusion: $($runView.conclusion))."
            }

            return $true
        }

        if (-not $RequireRun) {
            return $false
        }

        throw "Timed out waiting for the Release workflow for $Tag (run id: $runId)."
    }

    if (-not $RequireRun) {
        return $false
    }

    throw "Timed out waiting for the Release workflow for $Tag."
}

$resolvedRepoRoot = Resolve-CodexChannelsRepoRoot -RepoRoot $RepoRoot
if ([string]::IsNullOrWhiteSpace($HistoryPath)) {
    $HistoryPath = Join-Path $resolvedRepoRoot 'scripts/release-history.psd1'
}

$history = @(Read-ReleaseHistory -HistoryPath $HistoryPath | Sort-Object { [version](Normalize-ReleaseVersion -Version $_.Version) })
$generateNotesScript = Join-Path $resolvedRepoRoot 'scripts/generate-release-notes.ps1'
$releaseDirectory = Join-Path $resolvedRepoRoot 'release'
New-Item -ItemType Directory -Force -Path $releaseDirectory | Out-Null

Push-Location $resolvedRepoRoot
try {
    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
        throw "GitHub CLI (gh) is required for release backfill."
    }
    gh auth status 1>$null 2>$null
    if ($LASTEXITCODE -ne 0) {
        throw "GitHub CLI authentication is required for release backfill."
    }

    foreach ($entry in $history) {
        $normalizedVersion = Normalize-ReleaseVersion -Version $entry.Version
        $tag = Get-ReleaseTag -Version $normalizedVersion
        $commit = [string]$entry.Commit
        $localTagExists = $false
        $tagCreated = $false
        $releaseExists = $false

        git rev-parse --verify "refs/tags/$tag" 1>$null 2>$null
        $localTagExists = ($LASTEXITCODE -eq 0)
        $remoteResolvedCommit = Resolve-RemoteTagCommit -Tag $tag

        if (-not $localTagExists) {
            if ([string]::IsNullOrWhiteSpace($remoteResolvedCommit)) {
                git tag $tag $commit
                if ($LASTEXITCODE -ne 0) {
                    throw "Failed to create local tag $tag."
                }

                git push origin $tag | Out-Null
                if ($LASTEXITCODE -ne 0) {
                    throw "Failed to push tag $tag to origin."
                }

                $tagCreated = $true
            } else {
                git fetch origin "refs/tags/$tag:refs/tags/$tag" | Out-Null
                if ($LASTEXITCODE -ne 0) {
                    throw "Failed to fetch existing remote tag $tag."
                }
            }
        }

        $resolvedTagCommit = git rev-parse "${tag}^{commit}" 2>$null
        if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($resolvedTagCommit)) {
            throw "Failed to resolve tag $tag to a commit."
        }

        $resolvedTagCommit = [string]$resolvedTagCommit
        if ($resolvedTagCommit.Trim() -ne $commit) {
            throw "Tag $tag points to $resolvedTagCommit, expected $commit."
        }

        if (-not [string]::IsNullOrWhiteSpace($remoteResolvedCommit) -and ($remoteResolvedCommit.Trim() -ne $commit)) {
            throw "Remote tag $tag points to $remoteResolvedCommit, expected $commit."
        }

        gh release view $tag 1>$null 2>$null
        $releaseExists = ($LASTEXITCODE -eq 0)

        $notesPath = Join-Path $releaseDirectory "$tag.md"
        & $generateNotesScript -Version $normalizedVersion -HistoryPath $HistoryPath -OutputPath $notesPath -RepoRoot $resolvedRepoRoot | Out-Null

        if ($tagCreated) {
            Wait-ReleaseWorkflowForTag -Tag $tag -Commit $commit | Out-Null
        } elseif (-not $releaseExists) {
            Wait-ReleaseWorkflowForTag -Tag $tag -Commit $commit -RequireRun:$false -TimeoutSeconds 30 | Out-Null
        }

        gh release view $tag 1>$null 2>$null
        $releaseExists = ($LASTEXITCODE -eq 0)

        if ($tagCreated -and $releaseExists) {
            gh release edit $tag --title $tag --notes-file $notesPath | Out-Null
            if ($LASTEXITCODE -ne 0) {
                throw "Failed to update release notes for $tag."
            }
            continue
        }

        if ($releaseExists) {
            continue
        }

        gh release create $tag --target $commit --title $tag --notes-file $notesPath | Out-Null
        if ($LASTEXITCODE -ne 0) {
            gh release view $tag 1>$null 2>$null
            if ($LASTEXITCODE -eq 0) {
                continue
            }

            throw "Failed to create release $tag."
        }
    }
} finally {
    Pop-Location
}
