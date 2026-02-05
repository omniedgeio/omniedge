$ErrorActionPreference = 'Stop'

$packageName = 'omniedge-desktop'
$version = '2.3.0'
$url64 = "https://github.com/omniedgeio/omniedge/releases/download/v$version/omniedge-desktop-$version-windows-x64.msi"
$checksum64 = 'cc7019d214de89baf478756191e4b30f14248dd74012ed8a9f06a75dbedd3680'
$checksumType64 = 'sha256'

$packageArgs = @{
    packageName    = $packageName
    fileType       = 'msi'
    url64bit       = $url64
    checksum64     = $checksum64
    checksumType64 = $checksumType64
    silentArgs     = '/quiet /norestart'
    validExitCodes = @(0, 1641, 3010)
}

Install-ChocolateyPackage @packageArgs

Write-Host ""
Write-Host "OmniEdge Desktop has been installed!" -ForegroundColor Green
Write-Host ""
Write-Host "You can launch it from the Start Menu." -ForegroundColor Cyan
Write-Host ""
