[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$targets = @(
    'README.md',
    'README.ja.md',
    'scripts/release-history.psd1'
)

$targets += @(Get-ChildItem docs -Recurse -File -Include *.md -ErrorAction SilentlyContinue | ForEach-Object {
    Resolve-Path -Relative $_.FullName
})
$targets += @(Get-ChildItem tasks -Recurse -File -Include *.md -ErrorAction SilentlyContinue | ForEach-Object {
    Resolve-Path -Relative $_.FullName
})

$bannedTerms = @(
    'winsmux',
    'remodex',
    'claude-opus',
    'release-doc-reviews',
    'Release documentation review',
    'リリース前の公開文書レビュー'
)

$failures = New-Object System.Collections.Generic.List[string]

foreach ($path in ($targets | Sort-Object -Unique)) {
    if (-not (Test-Path $path)) {
        continue
    }

    $content = Get-Content $path -Raw
    foreach ($term in $bannedTerms) {
        if ($content -match [regex]::Escape($term)) {
            $failures.Add("$path contains banned term '$term'") | Out-Null
        }
    }
}

if ($failures.Count -gt 0) {
    [Console]::Error.WriteLine("documentation terminology audit failed:`n- " + ($failures -join "`n- "))
    exit 1
}

Write-Output 'documentation terminology audit passed'
