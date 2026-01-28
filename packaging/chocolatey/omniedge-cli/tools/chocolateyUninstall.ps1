$ErrorActionPreference = 'Stop'

$packageName = 'omniedge-cli'
$toolsDir = "$(Split-Path -Parent $MyInvocation.MyCommand.Definition)"

# Clean up any remaining files
$filesToRemove = @(
    "omniedge.exe",
    "omniedge-cli-*.exe"
)

foreach ($pattern in $filesToRemove) {
    Get-ChildItem -Path $toolsDir -Filter $pattern -ErrorAction SilentlyContinue | ForEach-Object {
        Remove-Item $_.FullName -Force -ErrorAction SilentlyContinue
    }
}

Write-Host "OmniEdge CLI has been uninstalled." -ForegroundColor Green
