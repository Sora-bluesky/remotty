[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Version,
    [string]$HistoryPath = '',
    [string]$BacklogPath = '',
    [string]$OutputPath = 'release/release-body.md',
    [string]$Repository = 'Sora-bluesky/codex-channels',
    [string]$RepoRoot = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'release-common.ps1')
. (Join-Path $PSScriptRoot 'planning-paths.ps1')

$resolvedRepoRoot = Resolve-CodexChannelsRepoRoot -RepoRoot $RepoRoot
if ([string]::IsNullOrWhiteSpace($HistoryPath)) {
    $HistoryPath = Resolve-CodexChannelsScriptPath -RepoRoot $resolvedRepoRoot -RelativePath 'scripts/release-history.psd1'
}
if ([string]::IsNullOrWhiteSpace($BacklogPath)) {
    $BacklogPath = Resolve-CodexChannelsPlanningFilePath -RepoRoot $resolvedRepoRoot -LocalRelativePath 'tasks/backlog.example.yaml' -EnvironmentVariable 'CODEX_CHANNELS_BACKLOG_PATH' -DefaultFileName 'backlog.yaml'
}
if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = 'release/release-body.md'
}

$normalizedVersion = Normalize-ReleaseVersion -Version $Version
$tag = Get-ReleaseTag -Version $normalizedVersion
$history = @(Read-ReleaseHistory -HistoryPath $HistoryPath)
$entry = Get-ReleaseEntry -History $history -Version $normalizedVersion
$previousVersion = Get-PreviousReleaseVersion -History $history -Version $normalizedVersion

$notes = New-Object System.Collections.Generic.List[string]
if ($null -ne $entry -and $null -ne $entry.Notes) {
    foreach ($note in @($entry.Notes)) {
        if (-not [string]::IsNullOrWhiteSpace([string]$note)) {
            $notes.Add([string]$note) | Out-Null
        }
    }
}

if ($notes.Count -eq 0) {
    $tasks = @(Read-PlanningTasks -BacklogPath $BacklogPath)
    foreach ($task in $tasks) {
        $taskVersion = ConvertFrom-YamlScalar -Value $task.TargetVersion
        if ($taskVersion -eq $tag -or $taskVersion -eq $normalizedVersion) {
            if (-not [string]::IsNullOrWhiteSpace([string]$task.Title)) {
                $notes.Add([string]$task.Title) | Out-Null
            }
        }
    }
}

if ($notes.Count -eq 0) {
    $notes.Add("Release $tag") | Out-Null
}

$outputFullPath = if ([System.IO.Path]::IsPathRooted($OutputPath)) { $OutputPath } else { Join-Path $resolvedRepoRoot $OutputPath }
$outputDirectory = Split-Path -Parent $outputFullPath
if (-not [string]::IsNullOrWhiteSpace($outputDirectory)) {
    New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
}

$builder = [System.Text.StringBuilder]::new()
[void]$builder.AppendLine('## Highlights')
[void]$builder.AppendLine()
foreach ($note in $notes) {
    [void]$builder.AppendLine("- $note")
}
[void]$builder.AppendLine()
[void]$builder.AppendLine('## Full Changelog')
[void]$builder.AppendLine()

if (-not [string]::IsNullOrWhiteSpace($previousVersion)) {
    $previousTag = Get-ReleaseTag -Version $previousVersion
    [void]$builder.AppendLine("- [$previousTag...$tag](https://github.com/$Repository/compare/$previousTag...$tag)")
} else {
    [void]$builder.AppendLine("- [$tag](https://github.com/$Repository/releases/tag/$tag)")
}

[System.IO.File]::WriteAllText($outputFullPath, $builder.ToString(), [System.Text.UTF8Encoding]::new($false))
Write-Output ("Generated release notes: {0}" -f $outputFullPath)
