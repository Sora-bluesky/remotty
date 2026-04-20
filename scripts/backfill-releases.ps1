[CmdletBinding()]
param(
    [string]$RepoRoot = '',
    [string]$HistoryPath = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'release-common.ps1')

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
    foreach ($entry in $history) {
        $normalizedVersion = Normalize-ReleaseVersion -Version $entry.Version
        $tag = Get-ReleaseTag -Version $normalizedVersion
        $commit = [string]$entry.Commit

        git rev-parse --verify "refs/tags/$tag" 1>$null 2>$null
        if ($LASTEXITCODE -ne 0) {
            git tag $tag $commit
            git push origin $tag | Out-Null
        }

        gh release view $tag 1>$null 2>$null
        if ($LASTEXITCODE -eq 0) {
            continue
        }

        $notesPath = Join-Path $releaseDirectory "$tag.md"
        & $generateNotesScript -Version $normalizedVersion -HistoryPath $HistoryPath -OutputPath $notesPath -RepoRoot $resolvedRepoRoot | Out-Null
        gh release create $tag --target $commit --title $tag --notes-file $notesPath | Out-Null
    }
} finally {
    Pop-Location
}
