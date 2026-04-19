[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

function Get-TrackedFiles {
    $output = git ls-files
    if ($LASTEXITCODE -ne 0) {
        throw 'Failed to list tracked files.'
    }

    return @($output | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
}

function Test-Tracked {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$TrackedFiles,

        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    return ($TrackedFiles -contains $Path)
}

function Test-RepoFileExists {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot,

        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    return (Test-Path -LiteralPath (Join-Path $RepoRoot $Path))
}

$trackedFiles = Get-TrackedFiles
$repoRoot = (Resolve-Path '.').Path
$failures = New-Object System.Collections.Generic.List[string]

$requiredTracked = @(
    'tasks/README.md',
    'tasks/backlog.example.yaml',
    'tasks/roadmap-title-ja.example.psd1',
    'tasks/ROADMAP.example.md',
    'scripts/planning-paths.ps1',
    'scripts/setup-planning.ps1',
    'scripts/sync-roadmap.ps1'
)

$forbiddenTracked = @(
    'tasks/backlog.yaml',
    'tasks/roadmap-title-ja.psd1',
    'docs/project/ROADMAP.md'
)

foreach ($path in $requiredTracked) {
    if (-not (Test-RepoFileExists -RepoRoot $repoRoot -Path $path)) {
        $failures.Add("missing required file: $path") | Out-Null
    }
}

foreach ($path in $forbiddenTracked) {
    if (Test-Tracked -TrackedFiles $trackedFiles -Path $path) {
        $failures.Add("forbidden tracked file: $path") | Out-Null
    }
}

if ($failures.Count -gt 0) {
    Write-Error ("public surface audit failed:`n- " + ($failures -join "`n- "))
    exit 1
}

Write-Output 'public surface audit passed'
