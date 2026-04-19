[CmdletBinding()]
param()

function Get-CodexChannelsPlanningRootMarkerPath {
    $explicitMarkerPath = [Environment]::GetEnvironmentVariable('CODEX_CHANNELS_PLANNING_ROOT_MARKER')
    if (-not [string]::IsNullOrWhiteSpace($explicitMarkerPath)) {
        return $explicitMarkerPath
    }

    $localAppData = if ($env:LOCALAPPDATA) { $env:LOCALAPPDATA } else { [Environment]::GetFolderPath('LocalApplicationData') }
    return Join-Path $localAppData 'codex-channels\planning-root.txt'
}

function Get-CodexChannelsPlanningRootFromMarker {
    $markerPath = Get-CodexChannelsPlanningRootMarkerPath
    if (-not (Test-Path -LiteralPath $markerPath)) {
        return $null
    }

    try {
        $markerValue = (Get-Content -LiteralPath $markerPath -Raw -ErrorAction Stop).Trim()
        if (-not [string]::IsNullOrWhiteSpace($markerValue)) {
            return $markerValue
        }
    } catch {
        return $null
    }

    return $null
}

function Find-CodexChannelsPlanningRoot {
    param([Parameter(Mandatory = $true)][string]$UserProfile)

    try {
        $backlogFiles = Get-ChildItem -LiteralPath $UserProfile -Filter 'backlog.yaml' -File -Recurse -Depth 8 -ErrorAction SilentlyContinue
        foreach ($file in $backlogFiles) {
            $directory = $file.DirectoryName
            if ([string]::IsNullOrWhiteSpace($directory)) {
                continue
            }

            if ($directory -notmatch '[\\/]codex-channels[\\/]planning$') {
                continue
            }

            if (Test-Path -LiteralPath (Join-Path $directory 'ROADMAP.md')) {
                return $directory
            }
        }
    } catch {
        return $null
    }

    return $null
}

function Get-CodexChannelsDefaultPlanningRoot {
    $cachedPlanningRoot = $null
    $cachedVariable = Get-Variable -Scope Script -Name CodexChannelsDefaultPlanningRoot -ErrorAction SilentlyContinue
    if ($cachedVariable) {
        $cachedPlanningRoot = [string]$cachedVariable.Value
    }

    if (-not [string]::IsNullOrWhiteSpace($cachedPlanningRoot)) {
        return $cachedPlanningRoot
    }

    $userProfile = if ($env:USERPROFILE) { $env:USERPROFILE } else { [Environment]::GetFolderPath('UserProfile') }
    $markerRoot = Get-CodexChannelsPlanningRootFromMarker
    if (-not [string]::IsNullOrWhiteSpace($markerRoot)) {
        $script:CodexChannelsDefaultPlanningRoot = $markerRoot
        return $script:CodexChannelsDefaultPlanningRoot
    }

    $discoveredRoot = Find-CodexChannelsPlanningRoot -UserProfile $userProfile
    if (-not [string]::IsNullOrWhiteSpace($discoveredRoot)) {
        $script:CodexChannelsDefaultPlanningRoot = $discoveredRoot
        return $script:CodexChannelsDefaultPlanningRoot
    }

    $script:CodexChannelsDefaultPlanningRoot = Join-Path $userProfile '.codex-channels\planning'
    return $script:CodexChannelsDefaultPlanningRoot
}

function Get-CodexChannelsPlanningRoot {
    if (-not [string]::IsNullOrWhiteSpace($env:CODEX_CHANNELS_PLANNING_ROOT)) {
        return $env:CODEX_CHANNELS_PLANNING_ROOT
    }

    return Get-CodexChannelsDefaultPlanningRoot
}

function Resolve-CodexChannelsPlanningFilePath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot,

        [Parameter(Mandatory = $true)]
        [string]$LocalRelativePath,

        [Parameter(Mandatory = $true)]
        [string]$EnvironmentVariable,

        [Parameter(Mandatory = $true)]
        [string]$DefaultFileName
    )

    $explicitPath = [Environment]::GetEnvironmentVariable($EnvironmentVariable)
    if (-not [string]::IsNullOrWhiteSpace($explicitPath)) {
        return $explicitPath
    }

    $externalPath = Join-Path (Get-CodexChannelsPlanningRoot) $DefaultFileName
    $localPath = Join-Path $RepoRoot $LocalRelativePath

    if ((Test-Path -LiteralPath $externalPath) -or -not (Test-Path -LiteralPath $localPath)) {
        return $externalPath
    }

    return $localPath
}
