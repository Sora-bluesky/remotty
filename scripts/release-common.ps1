[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Resolve-CodexChannelsRepoRoot {
    param(
        [string]$RepoRoot = ''
    )

    if (-not [string]::IsNullOrWhiteSpace($RepoRoot)) {
        return (Resolve-Path $RepoRoot).Path
    }

    return (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
}

function Resolve-CodexChannelsScriptPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot,
        [Parameter(Mandatory = $true)]
        [string]$RelativePath
    )

    return Join-Path $RepoRoot $RelativePath
}

function Normalize-ReleaseVersion {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Version
    )

    $trimmed = $Version.Trim()
    if ($trimmed.StartsWith('v')) {
        $trimmed = $trimmed.Substring(1)
    }

    if ($trimmed -notmatch '^\d+\.\d+\.\d+$') {
        throw "Invalid version format: '$Version'. Expected X.Y.Z or vX.Y.Z."
    }

    return $trimmed
}

function Get-ReleaseTag {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Version
    )

    return "v$(Normalize-ReleaseVersion -Version $Version)"
}

function ConvertFrom-YamlScalar {
    param(
        [AllowNull()]
        [string]$Value
    )

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
    param(
        [Parameter(Mandatory = $true)]
        [string]$Content
    )

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
        id             = $Matches['id']
        title          = ''
        status         = ''
        priority       = ''
        target_version = ''
        repo           = ''
    }

    for ($index = 1; $index -lt $Lines.Count; $index++) {
        $line = $Lines[$index]
        if ($line -match '^[ \t]{4}(?<key>[a-z_]+):[ \t]*(?<value>.*)$') {
            $values[$Matches['key']] = ConvertFrom-YamlScalar -Value $Matches['value']
        }
    }

    return [pscustomobject]@{
        Id            = $values['id']
        Title         = $values['title']
        Status        = $values['status']
        Priority      = $values['priority']
        TargetVersion = $values['target_version']
        Repo          = $values['repo']
    }
}

function Read-PlanningTasks {
    param(
        [Parameter(Mandatory = $true)]
        [string]$BacklogPath
    )

    if (-not (Test-Path -LiteralPath $BacklogPath)) {
        return @()
    }

    $content = Get-Content -LiteralPath $BacklogPath -Raw -Encoding UTF8
    $taskBlocks = @(Get-TaskBlocks -Content $content)
    return @(
        foreach ($taskBlock in $taskBlocks) {
            $task = ConvertFrom-TaskBlock -Lines @($taskBlock.Lines)
            if ($null -ne $task) {
                $task
            }
        }
    )
}

function Read-ReleaseHistory {
    param(
        [Parameter(Mandatory = $true)]
        [string]$HistoryPath
    )

    if (-not (Test-Path -LiteralPath $HistoryPath)) {
        return @()
    }

    $data = Import-PowerShellDataFile -LiteralPath $HistoryPath
    if ($null -eq $data.Releases) {
        return @()
    }

    return @($data.Releases)
}

function Get-ReleaseEntry {
    param(
        [Parameter(Mandatory = $true)]
        [object[]]$History,
        [Parameter(Mandatory = $true)]
        [string]$Version
    )

    $normalized = Normalize-ReleaseVersion -Version $Version
    return $History | Where-Object { (Normalize-ReleaseVersion -Version $_.Version) -eq $normalized } | Select-Object -First 1
}

function Get-PreviousReleaseVersion {
    param(
        [Parameter(Mandatory = $true)]
        [object[]]$History,
        [Parameter(Mandatory = $true)]
        [string]$Version
    )

    $normalized = Normalize-ReleaseVersion -Version $Version
    $ordered = @($History | Sort-Object { [version](Normalize-ReleaseVersion -Version $_.Version) })
    for ($index = 0; $index -lt $ordered.Count; $index++) {
        $current = Normalize-ReleaseVersion -Version $ordered[$index].Version
        if ($current -eq $normalized) {
            if ($index -gt 0) {
                return Normalize-ReleaseVersion -Version $ordered[$index - 1].Version
            }

            return $null
        }
    }

    if ($ordered.Count -eq 0) {
        return $null
    }

    $candidates = @(
        $ordered |
            Where-Object { [version](Normalize-ReleaseVersion -Version $_.Version) -lt [version]$normalized } |
            Sort-Object { [version](Normalize-ReleaseVersion -Version $_.Version) }
    )
    if ($candidates.Count -eq 0) {
        return $null
    }

    return Normalize-ReleaseVersion -Version $candidates[-1].Version
}

function Set-CargoPackageVersion {
    param(
        [Parameter(Mandatory = $true)]
        [string]$CargoTomlPath,
        [Parameter(Mandatory = $true)]
        [string]$Version
    )

    $content = Get-Content -LiteralPath $CargoTomlPath -Raw -Encoding UTF8
    $updated = $content -replace '(?m)^version = "[^"]+"$', ('version = "' + $Version + '"')
    if ($updated -eq $content) {
        throw "Could not update version line in $CargoTomlPath"
    }

    [System.IO.File]::WriteAllText($CargoTomlPath, $updated, [System.Text.UTF8Encoding]::new($false))
}

function Update-ReleaseBacklogStatus {
    param(
        [Parameter(Mandatory = $true)]
        [string]$BacklogPath,
        [Parameter(Mandatory = $true)]
        [string]$Version
    )

    if (-not (Test-Path -LiteralPath $BacklogPath)) {
        return @()
    }

    $targetTag = Get-ReleaseTag -Version $Version
    $content = Get-Content -LiteralPath $BacklogPath -Raw -Encoding UTF8
    $blocks = @(Get-TaskBlocks -Content $content)
    $updatedIds = New-Object System.Collections.Generic.List[string]
    $builder = New-Object System.Text.StringBuilder
    $normalized = $content -replace "`r`n", "`n"
    $cursor = 0

    foreach ($block in $blocks) {
        $blockText = (($block.Lines -join "`n").TrimEnd()) + "`n"
        $index = $normalized.IndexOf($blockText, $cursor, [System.StringComparison]::Ordinal)
        if ($index -lt 0) {
            continue
        }

        [void]$builder.Append($normalized.Substring($cursor, $index - $cursor))
        $task = ConvertFrom-TaskBlock -Lines @($block.Lines)
        $updatedBlockText = $blockText
        if ($null -ne $task) {
            $taskVersion = ConvertFrom-YamlScalar -Value $task.TargetVersion
            if ($taskVersion -eq $targetTag -or $taskVersion -eq (Normalize-ReleaseVersion -Version $Version)) {
                if ($task.Status -ne 'done') {
                    $updatedBlockText = $updatedBlockText -replace '(?m)^(\s*)status:\s*[^\r\n]*$', '${1}status: done'
                    $updatedIds.Add($task.Id) | Out-Null
                }
            }
        }

        [void]$builder.Append($updatedBlockText)
        $cursor = $index + $blockText.Length
    }

    [void]$builder.Append($normalized.Substring($cursor))

    if ($updatedIds.Count -gt 0) {
        [System.IO.File]::WriteAllText($BacklogPath, $builder.ToString(), [System.Text.UTF8Encoding]::new($false))
    }

    return @($updatedIds.ToArray())
}
