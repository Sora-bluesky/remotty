[CmdletBinding()]
param(
    [string]$BacklogPath = '',
    [string]$RoadmapPath = '',
    [string]$RoadmapTitleJaPath = ''
)

. (Join-Path $PSScriptRoot 'planning-paths.ps1')

function Resolve-WorkspacePath {
    param([Parameter(Mandatory = $true)][string]$Path)

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return $Path
    }

    return Join-Path (Get-Location).Path $Path
}

function ConvertFrom-YamlScalar {
    param([AllowNull()][string]$Value)

    if ($null -eq $Value) {
        return $null
    }

    $trimmed = $Value.Trim()
    if ($trimmed.Length -ge 2) {
        if (($trimmed.StartsWith('"') -and $trimmed.EndsWith('"')) -or ($trimmed.StartsWith("'") -and $trimmed.EndsWith("'"))) {
            return $trimmed.Substring(1, $trimmed.Length - 2)
        }
    }

    return $trimmed
}

function Get-TaskBlocks {
    param([Parameter(Mandatory = $true)][string]$Content)

    $normalized = $Content -replace "`r`n", "`n"
    $lines = $normalized -split "`n"
    $blocks = New-Object System.Collections.Generic.List[object]
    $current = $null

    foreach ($line in $lines) {
        if ($line -match '^[ \t]*-[ \t]+id:[ \t]*(?<id>\S+)[ \t]*$') {
            if ($null -ne $current -and $current.Count -gt 0) {
                $blocks.Add([pscustomobject]@{ Lines = @($current.ToArray()) })
            }

            $current = New-Object System.Collections.Generic.List[string]
            $current.Add($line)
            continue
        }

        if ($null -ne $current) {
            $current.Add($line)
        }
    }

    if ($null -ne $current -and $current.Count -gt 0) {
        $blocks.Add([pscustomobject]@{ Lines = @($current.ToArray()) })
    }

    return $blocks
}

function Get-VersionTitleMap {
    param([Parameter(Mandatory = $true)][string]$Content)

    $normalized = $Content -replace "`r`n", "`n"
    $versionTitles = [ordered]@{}

    foreach ($line in ($normalized -split "`n")) {
        if ($line -match '^[ \t]*#[ \t]*===[ \t]*(?<version>[^:]+):[ \t]*(?<title>.+?)[ \t]*===[ \t]*$') {
            $versionTitles[$Matches['version'].Trim()] = $Matches['title'].Trim()
        }
    }

    return $versionTitles
}

function Get-RoadmapLocalizationMap {
    param([Parameter(Mandatory = $true)][string]$Path)

    if ([string]::IsNullOrWhiteSpace($Path) -or -not (Test-Path -LiteralPath $Path)) {
        return @{
            VersionTitles = @{}
            TaskTitles = @{}
        }
    }

    $data = Import-PowerShellDataFile -LiteralPath $Path
    return @{
        VersionTitles = if ($null -ne $data.VersionTitles) { $data.VersionTitles } else { @{} }
        TaskTitles = if ($null -ne $data.TaskTitles) { $data.TaskTitles } else { @{} }
    }
}

function Convert-TitleFallbackToJapanese {
    param([AllowNull()][string]$Title)

    if ([string]::IsNullOrWhiteSpace($Title)) {
        return ''
    }

    $converted = $Title
    $converted = $converted -replace '^Add (.+)$', '$1 を追加'
    $converted = $converted -replace '^Create (.+)$', '$1 を作成'
    $converted = $converted -replace '^Implement (.+)$', '$1 を実装'
    $converted = $converted -replace '^Document (.+)$', '$1 を文書化'
    $converted = $converted -replace '^Write (.+)$', '$1 を作成'
    $converted = $converted -replace '^Run (.+)$', '$1 を実行'
    $converted = $converted -replace '^Update (.+)$', '$1 を更新'
    $converted = $converted -replace '^Fix (.+)$', '$1 を修正'
    $converted = $converted -replace '^Integrate (.+)$', '$1 を統合'
    return $converted
}

function ConvertFrom-TaskBlock {
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyString()]
        [AllowEmptyCollection()]
        [string[]]$Lines
    )

    if ($Lines[0] -notmatch '^[ \t]*-[ \t]+id:[ \t]*(?<id>\S+)[ \t]*$') {
        return $null
    }

    $values = @{
        id = $Matches['id']
        title = ''
        status = ''
        priority = ''
        target_version = ''
        repo = ''
    }

    for ($index = 1; $index -lt $Lines.Count; $index++) {
        $line = $Lines[$index]
        if ($line -match '^[ \t]{4}(?<key>[a-z_]+):[ \t]*(?<value>.*)$') {
            $key = $Matches['key']
            $value = $Matches['value']
            $values[$key] = ConvertFrom-YamlScalar -Value $value
        }
    }

    $taskIdNumber = [int]::MaxValue
    if ($values['id'] -match 'TASK-(?<number>\d+)$') {
        $taskIdNumber = [int]$Matches['number']
    }

    return [pscustomobject]@{
        Id = $values['id']
        IdNumber = $taskIdNumber
        Title = $values['title']
        Status = $values['status']
        Priority = $values['priority']
        TargetVersion = $values['target_version']
        Repo = $values['repo']
    }
}

function Get-StatusSymbol {
    param([AllowNull()][string]$Status)

    switch (($Status ?? '').ToLowerInvariant()) {
        'done' { return '[x]' }
        'review' { return '[R]' }
        'in-progress' { return '[-]' }
        'in_progress' { return '[-]' }
        'doing' { return '[-]' }
        'active' { return '[-]' }
        'cancelled' { return '[~]' }
        default { return '[ ]' }
    }
}

function Get-PriorityRank {
    param([AllowNull()][string]$Priority)

    switch (($Priority ?? '').ToUpperInvariant()) {
        'P0' { return 0 }
        'P1' { return 1 }
        'P2' { return 2 }
        'P3' { return 3 }
        default { return 9 }
    }
}

function Get-VersionSortKey {
    param([AllowNull()][string]$Version)

    if ($Version -match '^v(?<major>\d+)\.(?<minor>\d+)\.(?<patch>\d+)$') {
        return [pscustomobject]@{
            Major = [int]$Matches['major']
            Minor = [int]$Matches['minor']
            Patch = [int]$Matches['patch']
            Raw = $Version
        }
    }

    return [pscustomobject]@{
        Major = [int]::MaxValue
        Minor = [int]::MaxValue
        Patch = [int]::MaxValue
        Raw = ($Version ?? '')
    }
}

function New-ProgressBar {
    param([int]$DoneCount, [int]$TotalCount)

    if ($TotalCount -le 0) {
        return '[--------------------] 0% (0/0)'
    }

    $percentage = [int][math]::Round(($DoneCount / $TotalCount) * 100, 0, [System.MidpointRounding]::AwayFromZero)
    $filledCount = [int][math]::Round(($DoneCount / $TotalCount) * 20, 0, [System.MidpointRounding]::AwayFromZero)
    $filledCount = [Math]::Min([Math]::Max($filledCount, 0), 20)
    $emptyCount = 20 - $filledCount
    $bar = ('=' * $filledCount) + ('-' * $emptyCount)
    return '[{0}] {1}% ({2}/{3})' -f $bar, $percentage, $DoneCount, $TotalCount
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if ([string]::IsNullOrWhiteSpace($BacklogPath)) {
    $BacklogPath = Resolve-CodexChannelsPlanningFilePath -RepoRoot $repoRoot -LocalRelativePath 'tasks/backlog.example.yaml' -EnvironmentVariable 'CODEX_CHANNELS_BACKLOG_PATH' -DefaultFileName 'backlog.yaml'
}
if ([string]::IsNullOrWhiteSpace($RoadmapPath)) {
    $RoadmapPath = Resolve-CodexChannelsPlanningFilePath -RepoRoot $repoRoot -LocalRelativePath 'tasks/ROADMAP.example.md' -EnvironmentVariable 'CODEX_CHANNELS_ROADMAP_PATH' -DefaultFileName 'ROADMAP.md'
}
if ([string]::IsNullOrWhiteSpace($RoadmapTitleJaPath)) {
    $RoadmapTitleJaPath = Resolve-CodexChannelsPlanningFilePath -RepoRoot $repoRoot -LocalRelativePath 'tasks/roadmap-title-ja.example.psd1' -EnvironmentVariable 'CODEX_CHANNELS_ROADMAP_TITLE_JA_PATH' -DefaultFileName 'roadmap-title-ja.psd1'
}

$resolvedBacklogPath = Resolve-WorkspacePath -Path $BacklogPath
$resolvedRoadmapPath = Resolve-WorkspacePath -Path $RoadmapPath
$resolvedRoadmapTitleJaPath = Resolve-WorkspacePath -Path $RoadmapTitleJaPath

if (-not (Test-Path -LiteralPath $resolvedBacklogPath)) {
    Write-Warning "Backlog not found: $resolvedBacklogPath"
    exit 0
}

$utf8NoBom = [System.Text.UTF8Encoding]::new($false)
$backlogContent = [System.IO.File]::ReadAllText($resolvedBacklogPath, $utf8NoBom)
$versionTitles = Get-VersionTitleMap -Content $backlogContent
$roadmapLocalization = Get-RoadmapLocalizationMap -Path $resolvedRoadmapTitleJaPath
$taskBlocks = @(Get-TaskBlocks -Content $backlogContent)
$tasks = @(
    foreach ($taskBlock in $taskBlocks) {
        $parsedTask = ConvertFrom-TaskBlock -Lines @($taskBlock.Lines)
        if ($null -ne $parsedTask) {
            $parsedTask
        }
    }
)

$tasksWithTargetVersion = @($tasks | Where-Object { -not [string]::IsNullOrWhiteSpace($_.TargetVersion) })
$versionGroups = $tasksWithTargetVersion |
    Group-Object -Property TargetVersion |
    Sort-Object @{ Expression = { (Get-VersionSortKey -Version $_.Name).Major } }, @{ Expression = { (Get-VersionSortKey -Version $_.Name).Minor } }, @{ Expression = { (Get-VersionSortKey -Version $_.Name).Patch } }, @{ Expression = { (Get-VersionSortKey -Version $_.Name).Raw } }

$builder = [System.Text.StringBuilder]::new()
[void]$builder.AppendLine('# ロードマップ')
[void]$builder.AppendLine()
[void]$builder.AppendLine('> planning backlog から自動生成 — 手動編集禁止')
[void]$builder.AppendLine(('> 最終同期: {0}' -f (Get-Date -Format 'yyyy-MM-dd HH:mm (zzz)')))
[void]$builder.AppendLine()
[void]$builder.AppendLine('## バージョン概要')
[void]$builder.AppendLine()
[void]$builder.AppendLine('| バージョン | タスク数 | 進捗 |')
[void]$builder.AppendLine('|-----------|---------|------|')

foreach ($versionGroup in $versionGroups) {
    $versionTasks = @($versionGroup.Group)
    $totalCount = $versionTasks.Count
    $doneCount = @($versionTasks | Where-Object { $_.Status -eq 'done' }).Count
    [void]$builder.AppendLine(('| {0} | {1} | {2} |' -f $versionGroup.Name, $totalCount, (New-ProgressBar -DoneCount $doneCount -TotalCount $totalCount)))
}

[void]$builder.AppendLine()
[void]$builder.AppendLine('## タスク詳細')
[void]$builder.AppendLine()

foreach ($versionGroup in $versionGroups) {
    $defaultVersionTitle = if ($versionTitles.Contains($versionGroup.Name)) { [string]$versionTitles[$versionGroup.Name] } else { '' }
    $localizedVersionTitle = if ($roadmapLocalization.VersionTitles.ContainsKey($versionGroup.Name)) { [string]$roadmapLocalization.VersionTitles[$versionGroup.Name] } else { $defaultVersionTitle }
    $titleSuffix = if (-not [string]::IsNullOrWhiteSpace($localizedVersionTitle)) { ': ' + $localizedVersionTitle } else { '' }
    [void]$builder.AppendLine(('### {0}{1}' -f $versionGroup.Name, $titleSuffix))
    [void]$builder.AppendLine()
    [void]$builder.AppendLine('| | ID | Title | Priority | Repo | Status |')
    [void]$builder.AppendLine('|-|-----|-------|----------|------|--------|')

    $sortedTasks = @($versionGroup.Group | Sort-Object @{ Expression = { Get-PriorityRank -Priority $_.Priority } }, @{ Expression = { $_.IdNumber } }, @{ Expression = { $_.Id } })
    foreach ($task in $sortedTasks) {
        $localizedTaskTitle = if ($roadmapLocalization.TaskTitles.ContainsKey($task.Id)) { [string]$roadmapLocalization.TaskTitles[$task.Id] } else { Convert-TitleFallbackToJapanese -Title $task.Title }
        [void]$builder.AppendLine(('| {0} | {1} | {2} | {3} | {4} | {5} |' -f (Get-StatusSymbol -Status $task.Status), $task.Id, $localizedTaskTitle, $task.Priority, $task.Repo, $task.Status))
    }

    [void]$builder.AppendLine()
}

[void]$builder.AppendLine('## 凡例')
[void]$builder.AppendLine()
[void]$builder.AppendLine('| 記号 | 意味 |')
[void]$builder.AppendLine('|------|------|')
[void]$builder.AppendLine('| [x] | 完了 |')
[void]$builder.AppendLine('| [-] | 作業中 |')
[void]$builder.AppendLine('| [R] | レビュー中 |')
[void]$builder.AppendLine('| [ ] | 未着手 |')
[void]$builder.AppendLine()
[void]$builder.AppendLine('| 優先度 | 意味 |')
[void]$builder.AppendLine('|--------|------|')
[void]$builder.AppendLine('| P0 | 最重要 |')
[void]$builder.AppendLine('| P1 | 高 |')
[void]$builder.AppendLine('| P2 | 中 |')
[void]$builder.AppendLine('| P3 | 低 |')

$roadmapDirectory = Split-Path -Parent $resolvedRoadmapPath
if (-not [string]::IsNullOrWhiteSpace($roadmapDirectory)) {
    New-Item -ItemType Directory -Force -Path $roadmapDirectory 1>$null
}

[System.IO.File]::WriteAllText($resolvedRoadmapPath, $builder.ToString(), $utf8NoBom)
Write-Output ("Generated roadmap: {0}" -f $resolvedRoadmapPath)
