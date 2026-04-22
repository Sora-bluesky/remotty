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

$trackedFiles = Get-TrackedFiles
$failures = New-Object System.Collections.Generic.List[string]

$requiredTracked = @(
    'scripts/audit-doc-terminology.ps1',
    'scripts/audit-secret-surface.ps1',
    'scripts/planning-paths.ps1',
    'scripts/setup-planning.ps1',
    'scripts/sync-roadmap.ps1',
    'scripts/validate-planning.ps1'
)

$forbiddenTracked = @(
    'tasks/backlog.yaml',
    'tasks/roadmap-title-ja.psd1',
    '.github/release-doc-reviews',
    'docs/project/ROADMAP.md'
)

foreach ($path in $requiredTracked) {
    if (-not (Test-Tracked -TrackedFiles $trackedFiles -Path $path)) {
        $failures.Add("missing tracked file: $path") | Out-Null
    }
}

foreach ($path in $forbiddenTracked) {
    if ((Test-Tracked -TrackedFiles $trackedFiles -Path $path) -or
        @($trackedFiles | Where-Object { $_ -like "$path/*" }).Count -gt 0) {
        $failures.Add("forbidden tracked file: $path") | Out-Null
    }
}

$trackedTaskFiles = @($trackedFiles | Where-Object { $_ -like 'tasks/*' })
foreach ($path in $trackedTaskFiles) {
    $failures.Add("unexpected tracked task file: $path") | Out-Null
}

$forbiddenPresent = @(
    'tasks/backlog.yaml',
    'tasks/roadmap-title-ja.psd1',
    'tasks/ROADMAP.md',
    '.github/release-doc-reviews',
    'docs/project/ROADMAP.md'
)

foreach ($path in $forbiddenPresent) {
    if (Test-Path -LiteralPath $path) {
        $failures.Add("forbidden live file present in repo: $path") | Out-Null
    }
}

$forbiddenLiveNames = @('backlog.yaml', 'roadmap-title-ja.psd1', 'ROADMAP.md')
$forbiddenPresentAnywhere = Get-ChildItem -LiteralPath (Get-Location).Path -Recurse -File -Force |
    Where-Object {
        $_.FullName -notmatch '[\\/]\.git([\\/]|$)' -and
        $_.FullName -notmatch '[\\/]target([\\/]|$)' -and
        ($forbiddenLiveNames -contains $_.Name)
    }

foreach ($item in $forbiddenPresentAnywhere) {
    $relativePath = [System.IO.Path]::GetRelativePath((Get-Location).Path, $item.FullName)
    if ($relativePath -notin $forbiddenPresent) {
        $failures.Add("forbidden live file present in repo: $relativePath") | Out-Null
    }
}

function Assert-FileContains {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,

        [Parameter(Mandatory = $true)]
        [string]$Needle
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        $failures.Add("missing public doc: $Path") | Out-Null
        return
    }

    $content = Get-Content -LiteralPath $Path -Raw
    if (-not $content.Contains($Needle)) {
        $failures.Add("public doc $Path must mention: $Needle") | Out-Null
    }
}

Assert-FileContains -Path 'README.md' -Needle 'Codex thread'
Assert-FileContains -Path 'README.md' -Needle 'Telegram Quickstart'
Assert-FileContains -Path 'README.md' -Needle 'Advanced CLI Mode'
Assert-FileContains -Path 'README.ja.md' -Needle 'Codex スレッド'
Assert-FileContains -Path 'README.ja.md' -Needle 'Telegram クイックスタート'
Assert-FileContains -Path 'README.ja.md' -Needle '高度な CLI モード'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle '/remotty-sessions <thread_id>'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle '/remotty-use-this-project'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Codex CLI users run'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'remotty config workspace upsert'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Codex App chat box'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Windows protected storage'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'remotty local plugins'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'one-time setup'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'How Often Each Step Is Needed'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Do these when you use a new project'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Do this for each Telegram chat'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle '/remotty-sessions <thread_id>'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle '/remotty-use-this-project'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'Codex CLI では'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'remotty config workspace upsert'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'Codex App のチャット欄'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'Windows の保護領域'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'remotty local plugins'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle '初回だけ'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle '手順の分け方'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle '新しいプロジェクトを使う時'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'Telegram チャットごと'
Assert-FileContains -Path 'docs/exec-transport.md' -Needle 'transport = "exec"'
Assert-FileContains -Path 'docs/exec-transport.ja.md' -Needle 'transport = "exec"'
Assert-FileContains -Path 'docs/upgrading.md' -Needle 'transport = "app_server"'
Assert-FileContains -Path 'docs/upgrading.ja.md' -Needle 'transport = "app_server"'

if (Test-Path -LiteralPath 'docs/telegram-quickstart.md') {
    $quickstart = Get-Content -LiteralPath 'docs/telegram-quickstart.md' -Raw
    if ($quickstart.Contains('writable_roots') -or
        $quickstart.Contains('path = "C:/Users/you/Documents/project"') -or
        $quickstart.Contains('.agents/plugins/marketplace.json')) {
        $failures.Add('Telegram quickstart must not use bridge.toml workspace editing in the main path.') | Out-Null
    }
}

if (Test-Path -LiteralPath 'docs/telegram-quickstart.ja.md') {
    $quickstartJa = Get-Content -LiteralPath 'docs/telegram-quickstart.ja.md' -Raw
    if ($quickstartJa.Contains('writable_roots') -or
        $quickstartJa.Contains('path = "C:/Users/you/Documents/project"') -or
        $quickstartJa.Contains('.agents/plugins/marketplace.json')) {
        $failures.Add('Japanese Telegram quickstart must not use bridge.toml workspace editing in the main path.') | Out-Null
    }
}

if ($failures.Count -gt 0) {
    [Console]::Error.WriteLine("public surface audit failed:`n- " + ($failures -join "`n- "))
    exit 1
}

Write-Output 'public surface audit passed'
