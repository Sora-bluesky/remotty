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
    'docs/project/ROADMAP.md',
    'docs/development.md',
    'docs/development.ja.md'
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
    'docs/project/ROADMAP.md',
    'docs/development.md',
    'docs/development.ja.md'
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

function Test-FileContains {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,

        [Parameter(Mandatory = $true)]
        [string]$Needle
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        return $false
    }

    return (Get-Content -LiteralPath $Path -Raw).Contains($Needle)
}

$isRemottyRepo = (Test-FileContains -Path 'package.json' -Needle '"name": "remotty"') -or
    (Test-FileContains -Path 'Cargo.toml' -Needle 'name = "remotty"')

if ($isRemottyRepo) {
Assert-FileContains -Path 'README.md' -Needle 'Codex CLI session you connected'
Assert-FileContains -Path 'README.md' -Needle 'Telegram Quickstart'
Assert-FileContains -Path 'README.md' -Needle 'Advanced CLI Mode'
Assert-FileContains -Path 'README.md' -Needle 'remotty telegram configure'
Assert-FileContains -Path 'README.md' -Needle 'remotty remote-control'
Assert-FileContains -Path 'README.md' -Needle 'Remote Control active'
Assert-FileContains -Path 'README.md' -Needle 'Listening for Telegram channel messages from: remotty:telegram'
Assert-FileContains -Path 'README.ja.md' -Needle '連携した `Codex CLI` セッション'
Assert-FileContains -Path 'README.ja.md' -Needle 'Telegram クイックスタート'
Assert-FileContains -Path 'README.ja.md' -Needle '高度な CLI モード'
Assert-FileContains -Path 'README.ja.md' -Needle 'remotty telegram configure'
Assert-FileContains -Path 'README.ja.md' -Needle 'remotty remote-control'
Assert-FileContains -Path 'README.ja.md' -Needle 'Remote Control active'
Assert-FileContains -Path 'README.ja.md' -Needle 'Listening for Telegram channel messages from: remotty:telegram'

foreach ($readmePath in @('README.md', 'README.ja.md')) {
    if (Test-Path -LiteralPath $readmePath) {
        $readmeContent = Get-Content -LiteralPath $readmePath -Raw
        if ($readmeContent.Contains('docs/development')) {
            $failures.Add("$readmePath must not link internal development docs.") | Out-Null
        }
    }
}
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'remotty config workspace upsert'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'remotty remote-control'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle '## 3. Start Codex CLI'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'You will use these PowerShell windows:'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Normal PowerShell'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Remote Control PowerShell'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle '"$env:APPDATA\remotty\bridge.toml"'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Keep Remote Control PowerShell open while you use Telegram.'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Run this in Normal PowerShell:'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Open Codex PowerShell'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Open Remote Control PowerShell'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Do not run `remotty ...` commands in this'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'not inside the Codex CLI prompt'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Do not type these commands into the Codex CLI window'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Codex CLI session for this project is the Telegram target'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Remote Control active'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Windows protected storage'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle '%LOCALAPPDATA%\remotty\secrets'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'remotty-telegram-bot.bin'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Listening for Telegram channel messages from: remotty:telegram'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'does not create files in the project root'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Security Q&A'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Connection Q&A'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Windows protected storage'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'paired senders'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Does `remotty` require Codex App?'
Assert-FileContains -Path 'docs/telegram-quickstart.md' -Needle 'Only paired senders on the allowlist are accepted.'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'remotty config workspace upsert'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'remotty remote-control'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle '## 3. `Codex CLI` を起動する'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'この手順では、次の PowerShell 画面を使い分けます。'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle '通常の PowerShell'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'Remote Control 用 PowerShell'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle '"$env:APPDATA\remotty\bridge.toml"'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'Telegram から使う間は、Remote Control 用 PowerShell を開いたままにします。'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle '通常の PowerShell で実行します。'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'Codex 用 PowerShell を開き'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'Remote Control 用 PowerShell を開き'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'この画面では `remotty ...` コマンドを実行しません'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle '`Codex CLI` の入力欄には貼らないでください'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle '`remotty` が起動中の Remote Control 用 PowerShell には入力しません'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'このプロジェクトの `Codex CLI` セッションが Telegram の連携先'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'Remote Control active'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'Windows の保護領域'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle '%LOCALAPPDATA%\remotty\secrets'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'remotty-telegram-bot.bin'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'Listening for Telegram channel messages from: remotty:telegram'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'プロジェクトのルートに'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle '安全性の Q&A'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle '接続の Q&A'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'Windows の保護領域'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle '許可済み送信者'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle '`remotty` に Codex App は必要ですか?'
Assert-FileContains -Path 'docs/telegram-quickstart.ja.md' -Needle 'ペアリング済みで、allowlist に入った送信者だけ'
Assert-FileContains -Path 'docs/exec-transport.md' -Needle 'transport = "exec"'
Assert-FileContains -Path 'docs/exec-transport.ja.md' -Needle 'transport = "exec"'
Assert-FileContains -Path 'docs/upgrading.md' -Needle 'transport = "app_server"'
Assert-FileContains -Path 'docs/upgrading.ja.md' -Needle 'transport = "app_server"'
Assert-FileContains -Path 'plugins/remotty/.codex-plugin/plugin.json' -Needle '"skills": "./skills/"'
Assert-FileContains -Path 'plugins/remotty/skills/remotty-configure/SKILL.md' -Needle 'PowerShell window'
Assert-FileContains -Path 'plugins/remotty/skills/remotty-start/SKILL.md' -Needle 'remotty service start'
Assert-FileContains -Path 'plugins/remotty/skills/remotty-start/SKILL.md' -Needle 'remotty remote-control'
Assert-FileContains -Path 'plugins/remotty/skills/remotty-status/SKILL.md' -Needle 'remotty telegram policy allowlist'

if (-not (Test-Path -LiteralPath 'package.json')) {
    $failures.Add("package.json must exist for version audit.") | Out-Null
} elseif (-not (Test-Path -LiteralPath 'plugins/remotty/.codex-plugin/plugin.json')) {
    $failures.Add("plugin manifest must exist for version audit.") | Out-Null
} else {
    $packageJson = Get-Content -LiteralPath 'package.json' -Raw | ConvertFrom-Json
    $pluginJson = Get-Content -LiteralPath 'plugins/remotty/.codex-plugin/plugin.json' -Raw | ConvertFrom-Json
    $packageVersion = [string]$packageJson.version
    $pluginVersion = [string]$pluginJson.version
    if ([string]::IsNullOrWhiteSpace($packageVersion) -or
        [string]::IsNullOrWhiteSpace($pluginVersion) -or
        $packageVersion -ne $pluginVersion) {
        $failures.Add("plugin manifest version must match package version.") | Out-Null
    }
}

if (Test-Path -LiteralPath 'plugins/remotty/README.md') {
    $pluginReadme = Get-Content -LiteralPath 'plugins/remotty/README.md' -Raw
    foreach ($forbidden in @('fakechat', '/remotty-fakechat-demo', '/remotty-smoke')) {
        if ($pluginReadme.Contains($forbidden)) {
            $failures.Add("plugins/remotty/README.md must not mention removed command wording: $forbidden") | Out-Null
        }
    }
}

if (Test-Path -LiteralPath 'bridge.toml') {
    $bridgeToml = Get-Content -LiteralPath 'bridge.toml' -Raw
    if ($bridgeToml -match '(?m)^\s*model\s*=') {
        $failures.Add('bridge.toml must not pin a Codex model by default.') | Out-Null
    }
}

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
}

if ($failures.Count -gt 0) {
    [Console]::Error.WriteLine("public surface audit failed:`n- " + ($failures -join "`n- "))
    exit 1
}

Write-Output 'public surface audit passed'
