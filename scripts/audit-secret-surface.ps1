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

function Test-TextFile {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $extension = [System.IO.Path]::GetExtension($Path)
    return $extension -notin @('.png', '.jpg', '.jpeg', '.gif', '.ico', '.pdf', '.lock')
}

function Test-PlaceholderValue {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Value
    )

    $trimmed = $Value.Trim().Trim('"', "'")
    if ([string]::IsNullOrWhiteSpace($trimmed)) {
        return $true
    }

    $patterns = @(
        '^\<.+\>$',
        '^\$\{\{.+\}\}$',
        '^(?i)(your|example|placeholder)[-_a-z0-9 ]*$',
        '^(?i)(chat id|sender id|bot token|workspace)$',
        '^(?i)c:/path/to/.+'
    )

    foreach ($pattern in $patterns) {
        if ($trimmed -match $pattern) {
            return $true
        }
    }

    return $false
}

$trackedFiles = Get-TrackedFiles
$failures = New-Object System.Collections.Generic.List[string]
$assignmentPattern = '(?i)^\s*(?<name>LIVE_[A-Z0-9_]+|TELEGRAM_BOT_TOKEN)\s*=\s*(?<value>.+?)\s*$'
$telegramTokenPattern = '\b\d{7,12}:[A-Za-z0-9_-]{20,}\b'

foreach ($path in $trackedFiles) {
    if (-not (Test-TextFile -Path $path)) {
        continue
    }

    $fullPath = Join-Path (Get-Location) $path
    if (-not (Test-Path -LiteralPath $fullPath)) {
        continue
    }

    $lines = @(Get-Content -LiteralPath $fullPath -ErrorAction Stop)
    for ($index = 0; $index -lt $lines.Count; $index++) {
        $line = $lines[$index]
        if ($line -match $assignmentPattern) {
            $name = $Matches['name']
            $value = $Matches['value']
            if (-not (Test-PlaceholderValue -Value $value)) {
                $failures.Add("${path}:$($index + 1) contains a tracked assignment for $name") | Out-Null
            }
        }

        if ($line -match $telegramTokenPattern) {
            $failures.Add("${path}:$($index + 1) contains a Telegram bot token-like value") | Out-Null
        }
    }
}

if ($failures.Count -gt 0) {
    Write-Error ("secret surface audit failed:`n- " + ($failures -join "`n- "))
    exit 1
}

Write-Output 'secret surface audit passed'
