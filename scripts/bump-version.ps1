[CmdletBinding()]
param(
    [string]$Version,
    [switch]$SyncOnly,
    [string]$RepoRoot = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'release-common.ps1')
. (Join-Path $PSScriptRoot 'planning-paths.ps1')

$resolvedRepoRoot = Resolve-RemottyRepoRoot -RepoRoot $RepoRoot
$versionFile = Join-Path $resolvedRepoRoot 'VERSION'
$cargoTomlPath = Join-Path $resolvedRepoRoot 'Cargo.toml'
$syncRoadmapScript = Join-Path $PSScriptRoot 'sync-roadmap.ps1'
$generateNotesScript = Join-Path $PSScriptRoot 'generate-release-notes.ps1'
$validatePlanningScript = Join-Path $PSScriptRoot 'validate-planning.ps1'
$auditPublicSurfaceScript = Join-Path $PSScriptRoot 'audit-public-surface.ps1'
$auditDocTerminologyScript = Join-Path $PSScriptRoot 'audit-doc-terminology.ps1'
$auditSecretSurfaceScript = Join-Path $PSScriptRoot 'audit-secret-surface.ps1'
$backlogPath = Resolve-RemottyExternalPlanningFilePath -EnvironmentVariable 'REMOTTY_BACKLOG_PATH' -DefaultFileName 'backlog.yaml'
$roadmapPath = Resolve-RemottyExternalPlanningFilePath -EnvironmentVariable 'REMOTTY_ROADMAP_PATH' -DefaultFileName 'ROADMAP.md'
$titlePath = Resolve-RemottyExternalPlanningFilePath -EnvironmentVariable 'REMOTTY_ROADMAP_TITLE_JA_PATH' -DefaultFileName 'roadmap-title-ja.psd1'

function Assert-NativeSuccess {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Command
    )

    if ($LASTEXITCODE -ne 0) {
        throw ("{0} failed with exit code {1}" -f $Command, $LASTEXITCODE)
    }
}

if ([string]::IsNullOrWhiteSpace($Version)) {
    if (-not (Test-Path -LiteralPath $versionFile)) {
        throw "VERSION file not found: $versionFile"
    }

    $Version = (Get-Content -LiteralPath $versionFile -Raw -Encoding UTF8).Trim()
}

$normalizedVersion = Normalize-ReleaseVersion -Version $Version
$tag = Get-ReleaseTag -Version $normalizedVersion

if (-not $SyncOnly) {
    Assert-ReleasePlanningInputsExist -BacklogPath $backlogPath -RoadmapTitleJaPath $titlePath
    & $validatePlanningScript -BacklogPath $backlogPath -RoadmapTitleJaPath $titlePath | Out-Null
    Push-Location $resolvedRepoRoot
    try {
        & $auditPublicSurfaceScript | Out-Null
        & $auditDocTerminologyScript | Out-Null
        & $auditSecretSurfaceScript | Out-Null

        if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
            throw "GitHub CLI (gh) is required for release."
        }

        $status = git status --porcelain 2>&1
        if ($status) {
            throw "Working tree is not clean. Commit or stash changes before releasing."
        }

        $branch = "release/$tag"
        git switch -c $branch | Out-Null
        Assert-NativeSuccess "git switch -c $branch"
    } finally {
        Pop-Location
    }
}

[System.IO.File]::WriteAllText($versionFile, $normalizedVersion, [System.Text.UTF8Encoding]::new($false))
Set-CargoPackageVersion -CargoTomlPath $cargoTomlPath -Version $normalizedVersion

if ($SyncOnly) {
    Write-Output ("Synced version files to {0}" -f $normalizedVersion)
    return
}

Push-Location $resolvedRepoRoot
try {
    git add VERSION Cargo.toml
    Assert-NativeSuccess "git add VERSION Cargo.toml"
    git commit -m "chore: bump version to $normalizedVersion" | Out-Null
    Assert-NativeSuccess "git commit"
    git push -u origin $branch | Out-Null
    Assert-NativeSuccess "git push -u origin $branch"

    $prUrl = gh pr create --base main --head $branch --title "chore: bump version to $normalizedVersion" --body "Automated release via `scripts/bump-version.ps1 -Version $normalizedVersion`."
    Assert-NativeSuccess "gh pr create"
    $prNumber = if ($prUrl -match '/(\d+)$') { $Matches[1] } else { $prUrl }
    gh pr checks $prNumber --watch | Out-Null
    Assert-NativeSuccess "gh pr checks $prNumber --watch"
    gh pr merge $prNumber --merge --delete-branch | Out-Null
    Assert-NativeSuccess "gh pr merge $prNumber"

    git switch main | Out-Null
    Assert-NativeSuccess "git switch main"
    git pull --ff-only origin main | Out-Null
    Assert-NativeSuccess "git pull --ff-only origin main"
    git tag $tag
    Assert-NativeSuccess "git tag $tag"
    git push origin $tag | Out-Null
    Assert-NativeSuccess "git push origin $tag"

    $notesPath = Join-Path $resolvedRepoRoot 'release\release-body.md'
    & $generateNotesScript -Version $normalizedVersion -OutputPath $notesPath -RepoRoot $resolvedRepoRoot
    gh release create $tag --title $tag --notes-file $notesPath --verify-tag --latest | Out-Null
    Assert-NativeSuccess "gh release create $tag"

    $updatedTaskIds = @(Update-ReleaseBacklogStatus -BacklogPath $backlogPath -Version $normalizedVersion)
    if ($updatedTaskIds.Count -gt 0) {
        & $syncRoadmapScript -BacklogPath $backlogPath -RoadmapPath $roadmapPath -RoadmapTitleJaPath $titlePath | Out-Null
    }

    Write-Output ("Released {0}" -f $tag)
} finally {
    Pop-Location
}
