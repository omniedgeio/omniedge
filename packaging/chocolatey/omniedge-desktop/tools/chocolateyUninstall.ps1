$ErrorActionPreference = 'Stop'

$packageName = 'omniedge-desktop'

# Get the uninstall registry key
$uninstallKey = Get-UninstallRegistryKey -SoftwareName 'OmniEdge*'

if ($uninstallKey) {
    $uninstallString = $uninstallKey.UninstallString
    
    if ($uninstallString -match 'msiexec') {
        $productCode = $uninstallKey.PSChildName
        $silentArgs = "/qn /norestart /x $productCode"
        
        $packageArgs = @{
            packageName    = $packageName
            fileType       = 'msi'
            silentArgs     = $silentArgs
            validExitCodes = @(0, 1605, 1614, 1641, 3010)
            file           = ''
        }
        
        Uninstall-ChocolateyPackage @packageArgs
    }
}

Write-Host "OmniEdge Desktop has been uninstalled." -ForegroundColor Green
