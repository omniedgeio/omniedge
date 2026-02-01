#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Installs or updates the OmniEdge Helper Windows Service.

.DESCRIPTION
    This script:
    1. Stops and removes any existing OmniEdge Helper service
    2. Builds the omni-helper binary (if -Build is specified)
    3. Copies the binary to Program Files
    4. Creates and starts the Windows service

.PARAMETER Build
    Build the omni-helper binary before installation.

.PARAMETER HelperPath
    Path to the omni-helper.exe binary. If not specified, uses the default build path.

.PARAMETER Uninstall
    Remove the service instead of installing.

.EXAMPLE
    .\install-helper.ps1 -Build
    Builds and installs the helper service.

.EXAMPLE
    .\install-helper.ps1 -HelperPath "C:\path\to\omni-helper.exe"
    Installs using a specific binary.

.EXAMPLE
    .\install-helper.ps1 -Uninstall
    Removes the helper service.
#>

param(
    [switch]$Build,
    [string]$HelperPath,
    [switch]$Uninstall
)

$ErrorActionPreference = "Stop"
$ServiceName = "OmniEdgeHelper"
$DisplayName = "OmniEdge Helper Service"
$Description = "Facilitates secure P2P networking for OmniEdge VPN connections."
$InstallDir = "$env:ProgramFiles\OmniEdge"

function Write-Status {
    param([string]$Message, [string]$Color = "Cyan")
    Write-Host "[$((Get-Date).ToString('HH:mm:ss'))] " -NoNewline -ForegroundColor DarkGray
    Write-Host $Message -ForegroundColor $Color
}

function Stop-ExistingService {
    Write-Status "Checking for existing $ServiceName service..."
    
    $service = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    if ($service) {
        Write-Status "Found existing service (Status: $($service.Status))" -Color Yellow
        
        if ($service.Status -eq 'Running') {
            Write-Status "Stopping service..."
            Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
            
            # Wait for service to stop (max 30 seconds)
            $timeout = 30
            $elapsed = 0
            while ((Get-Service -Name $ServiceName -ErrorAction SilentlyContinue).Status -eq 'Running' -and $elapsed -lt $timeout) {
                Start-Sleep -Seconds 1
                $elapsed++
            }
            
            if ($elapsed -ge $timeout) {
                Write-Status "Warning: Service did not stop gracefully, forcing..." -Color Yellow
                # Kill the process if still running
                $process = Get-Process -Name "omni-helper*" -ErrorAction SilentlyContinue
                if ($process) {
                    $process | Stop-Process -Force
                }
            }
        }
        
        Write-Status "Removing existing service..."
        # Use sc.exe for more reliable deletion
        $result = & sc.exe delete $ServiceName 2>&1
        
        # Wait for service to be fully removed
        $timeout = 10
        $elapsed = 0
        while ((Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) -and $elapsed -lt $timeout) {
            Start-Sleep -Seconds 1
            $elapsed++
        }
        
        if (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) {
            throw "Failed to remove existing service. Please reboot and try again."
        }
        
        Write-Status "Existing service removed successfully" -Color Green
    } else {
        Write-Status "No existing service found" -Color Green
    }
}

function Build-Helper {
    Write-Status "Building omni-helper..."
    
    $projectRoot = Split-Path -Parent $PSScriptRoot
    Push-Location $projectRoot
    
    try {
        $result = & cargo build --release -p omni-helper 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw "Build failed: $result"
        }
        Write-Status "Build completed successfully" -Color Green
    } finally {
        Pop-Location
    }
}

function Install-HelperService {
    param([string]$BinaryPath)
    
    # Validate binary exists
    if (-not (Test-Path $BinaryPath)) {
        throw "Helper binary not found at: $BinaryPath"
    }
    
    Write-Status "Using binary: $BinaryPath"
    
    # Create install directory if needed
    if (-not (Test-Path $InstallDir)) {
        Write-Status "Creating install directory: $InstallDir"
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }
    
    # Copy binary to install location
    $targetPath = Join-Path $InstallDir "omni-helper.exe"
    Write-Status "Copying binary to: $targetPath"
    Copy-Item -Path $BinaryPath -Destination $targetPath -Force
    
    # Create the service
    Write-Status "Creating Windows service..."
    
    # Use New-Service cmdlet
    try {
        New-Service -Name $ServiceName `
                    -BinaryPathName "`"$targetPath`"" `
                    -DisplayName $DisplayName `
                    -Description $Description `
                    -StartupType Automatic `
                    -ErrorAction Stop | Out-Null
        
        Write-Status "Service created successfully" -Color Green
    } catch {
        # If New-Service fails, try sc.exe as fallback
        Write-Status "New-Service failed, trying sc.exe..." -Color Yellow
        
        $result = & sc.exe create $ServiceName binPath= "`"$targetPath`"" DisplayName= "$DisplayName" start= auto 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to create service: $result"
        }
        
        # Set description separately
        & sc.exe description $ServiceName "$Description" | Out-Null
        
        Write-Status "Service created via sc.exe" -Color Green
    }
    
    # Configure service recovery options (restart on failure)
    Write-Status "Configuring service recovery options..."
    & sc.exe failure $ServiceName reset= 86400 actions= restart/5000/restart/10000/restart/30000 | Out-Null
    
    # Start the service
    Write-Status "Starting service..."
    Start-Service -Name $ServiceName
    
    # Verify service is running
    Start-Sleep -Seconds 2
    $service = Get-Service -Name $ServiceName
    if ($service.Status -eq 'Running') {
        Write-Status "Service is running!" -Color Green
    } else {
        Write-Status "Warning: Service status is $($service.Status)" -Color Yellow
    }
    
    # Show service info
    Write-Host ""
    Write-Status "Installation complete!" -Color Green
    Write-Host ""
    Write-Host "  Service Name:  $ServiceName"
    Write-Host "  Binary Path:   $targetPath"
    Write-Host "  Status:        $($service.Status)"
    Write-Host "  Startup Type:  Automatic"
    Write-Host ""
    Write-Host "  Log files:     $env:ProgramData\OmniEdge\logs\"
    Write-Host ""
}

function Uninstall-HelperService {
    Stop-ExistingService
    
    # Remove binary
    $targetPath = Join-Path $InstallDir "omni-helper.exe"
    if (Test-Path $targetPath) {
        Write-Status "Removing binary: $targetPath"
        Remove-Item -Path $targetPath -Force
    }
    
    Write-Status "Uninstallation complete!" -Color Green
}

# Main execution
Write-Host ""
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "  OmniEdge Helper Service Installer  " -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""

try {
    if ($Uninstall) {
        Uninstall-HelperService
        exit 0
    }
    
    # Stop/remove existing service first
    Stop-ExistingService
    
    # Build if requested
    if ($Build) {
        Build-Helper
    }
    
    # Determine binary path
    if (-not $HelperPath) {
        $projectRoot = Split-Path -Parent $PSScriptRoot
        $HelperPath = Join-Path $projectRoot "target\release\omni-helper.exe"
        
        if (-not (Test-Path $HelperPath)) {
            # Try debug build
            $HelperPath = Join-Path $projectRoot "target\debug\omni-helper.exe"
        }
    }
    
    Install-HelperService -BinaryPath $HelperPath
    
} catch {
    Write-Host ""
    Write-Status "ERROR: $_" -Color Red
    Write-Host ""
    exit 1
}
