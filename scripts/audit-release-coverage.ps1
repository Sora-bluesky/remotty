[CmdletBinding()]
param(
    [string]$RepoRoot = '',
    [string]$HistoryPath = '',
    [string]$RequiredReleasePath = '',
    [string]$VersionPath = '',
    [string]$Remote = 'origin',
    [switch]$SkipRemoteTagCheck
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'release-common.ps1')

$resolvedRepoRoot = Resolve-RemottyRepoRoot -RepoRoot $RepoRoot
if ([string]::IsNullOrWhiteSpace($HistoryPath)) {
    $HistoryPath = Join-Path $resolvedRepoRoot 'scripts/release-history.psd1'
}
if ([string]::IsNullOrWhiteSpace($RequiredReleasePath)) {
    $RequiredReleasePath = Join-Path $resolvedRepoRoot 'scripts/release-required.psd1'
}
if ([string]::IsNullOrWhiteSpace($VersionPath)) {
    $VersionPath = Join-Path $resolvedRepoRoot 'VERSION'
}

if (-not (Test-Path -LiteralPath $RequiredReleasePath)) {
    Write-Output 'release coverage audit passed'
    return
}

$requiredData = Import-PowerShellDataFile -LiteralPath $RequiredReleasePath
$requiredReleases = @()
if ($null -ne $requiredData.RequiredReleases) {
    $requiredReleases = @($requiredData.RequiredReleases)
}

$history = @(Read-ReleaseHistory -HistoryPath $HistoryPath)
$historyVersions = New-Object System.Collections.Generic.HashSet[string]
foreach ($entry in $history) {
    [void]$historyVersions.Add((Normalize-ReleaseVersion -Version ([string]$entry.Version)))
}

$sourceVersion = ''
if (Test-Path -LiteralPath $VersionPath) {
    $sourceVersion = Normalize-ReleaseVersion -Version ((Get-Content -LiteralPath $VersionPath -Raw -Encoding UTF8).Trim())
}

$failures = New-Object System.Collections.Generic.List[string]
foreach ($required in $requiredReleases) {
    $normalized = Normalize-ReleaseVersion -Version ([string]$required)
    $tag = Get-ReleaseTag -Version $normalized

    if (-not $historyVersions.Contains($normalized)) {
        $failures.Add("required release $tag is missing from scripts/release-history.psd1") | Out-Null
    }

    $isCurrentSourceVersion = (-not [string]::IsNullOrWhiteSpace($sourceVersion)) -and ($normalized -eq $sourceVersion)
    if ((-not $SkipRemoteTagCheck) -and (-not $isCurrentSourceVersion)) {
        $remoteTag = git -C $resolvedRepoRoot ls-remote --tags $Remote "refs/tags/$tag" "refs/tags/$tag^{}" 2>$null
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to query remote tag $tag from $Remote."
        }
        if (@($remoteTag).Count -eq 0) {
            $failures.Add("required release tag $tag is missing from $Remote") | Out-Null
        }
    }
}

if ($failures.Count -gt 0) {
    Write-Error ("release coverage audit failed:`n- " + ($failures -join "`n- "))
}

Write-Output 'release coverage audit passed'
