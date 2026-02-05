$ErrorActionPreference = 'Stop'

$packageName = 'omniedge-cli'
$version = '2.3.0'
$url64 = "https://github.com/omniedgeio/omniedge/releases/download/v$version/omniedge-cli-$version-windows-x64.zip"
$checksum64 = '07A1641D76C92CC9237F1E807026389FE3D758A98AA8D4635CEA9D7F77943ECD'
$checksumType64 = 'sha256'

$toolsDir = "$(Split-Path -Parent $MyInvocation.MyCommand.Definition)"

$packageArgs = @{
    packageName    = $packageName
    unzipLocation  = $toolsDir
    url64bit       = $url64
    checksum64     = $checksum64
    checksumType64 = $checksumType64
}

Install-ChocolateyZipPackage @packageArgs

# Rename the binary to omniedge.exe
$binaryPath = Join-Path $toolsDir "omniedge-cli-$version-windows-x64.exe"
$targetPath = Join-Path $toolsDir "omniedge.exe"

if (Test-Path $binaryPath) {
    Move-Item -Path $binaryPath -Destination $targetPath -Force
}

# Create shim ignore for the versioned binary name
$shimIgnore = Join-Path $toolsDir "omniedge-cli-$version-windows-x64.exe.ignore"
if (Test-Path (Join-Path $toolsDir "omniedge-cli-$version-windows-x64.exe")) {
    New-Item -ItemType File -Path $shimIgnore -Force | Out-Null
}

Write-Host ""
Write-Host "OmniEdge CLI has been installed!" -ForegroundColor Green
Write-Host ""
Write-Host "To get started, run:" -ForegroundColor Cyan
Write-Host "  omniedge login" -ForegroundColor White
Write-Host "  omniedge join" -ForegroundColor White
Write-Host ""
