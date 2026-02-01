# Test script for OmniEdge Helper service
# Usage: powershell -ExecutionPolicy Bypass -File scripts\test-helper.ps1

param(
    [int]$ConcurrentCount = 5,
    [switch]$Verbose
)

function Send-HelperCommand {
    param([string]$Command, [hashtable]$CommandArgs = @{})
    
    try {
        $pipe = New-Object System.IO.Pipes.NamedPipeClientStream('.', 'omniedge-helper', 'InOut')
        $pipe.Connect(5000)
        
        $writer = New-Object System.IO.StreamWriter($pipe)
        $reader = New-Object System.IO.StreamReader($pipe)
        $writer.AutoFlush = $true
        
        $request = @{
            command = $Command
            args = $CommandArgs
        } | ConvertTo-Json -Compress
        
        $writer.Write($request)
        $response = $reader.ReadLine()
        $pipe.Close()
        
        $parsed = $response | ConvertFrom-Json
        return $parsed
    }
    catch {
        return [PSCustomObject]@{ success = $false; error = $_.Exception.Message }
    }
}

Write-Host "=== OmniEdge Helper Test ===" -ForegroundColor Cyan
Write-Host ""

# Test 1: Single version request
Write-Host "Test 1: Single version request..." -ForegroundColor Yellow
$result = Send-HelperCommand -Command "version"
if ($result.success) {
    Write-Host "  PASS: Version = $($result.version)" -ForegroundColor Green
} else {
    Write-Host "  FAIL: $($result.error)" -ForegroundColor Red
    exit 1
}

# Test 2: Single status request
Write-Host "Test 2: Single status request..." -ForegroundColor Yellow
$result = Send-HelperCommand -Command "status"
if ($result.success) {
    Write-Host "  PASS: Connected = $($result.connected)" -ForegroundColor Green
} else {
    Write-Host "  FAIL: $($result.error)" -ForegroundColor Red
    exit 1
}

# Test 3: Single ping request
Write-Host "Test 3: Single ping request..." -ForegroundColor Yellow
$result = Send-HelperCommand -Command "ping"
if ($result.success) {
    Write-Host "  PASS: Pong received" -ForegroundColor Green
} else {
    Write-Host "  FAIL: $($result.error)" -ForegroundColor Red
    exit 1
}

# Test 4: Concurrent requests
Write-Host "Test 4: $ConcurrentCount concurrent version requests..." -ForegroundColor Yellow

$jobs = @()
for ($i = 1; $i -le $ConcurrentCount; $i++) {
    $jobs += Start-Job -ScriptBlock {
        param($index)
        try {
            $pipe = New-Object System.IO.Pipes.NamedPipeClientStream('.', 'omniedge-helper', 'InOut')
            $pipe.Connect(10000)
            
            $writer = New-Object System.IO.StreamWriter($pipe)
            $reader = New-Object System.IO.StreamReader($pipe)
            $writer.AutoFlush = $true
            
            $request = '{"command":"version","args":{}}'
            $writer.Write($request)
            $response = $reader.ReadLine()
            $pipe.Close()
            
            return @{ index = $index; success = $true; response = $response }
        }
        catch {
            return @{ index = $index; success = $false; error = $_.Exception.Message }
        }
    } -ArgumentList $i
}

# Wait for all jobs
$results = $jobs | Wait-Job | Receive-Job
$jobs | Remove-Job

$passed = 0
$failed = 0
foreach ($r in $results) {
    if ($r.success) {
        $passed++
        if ($Verbose) {
            Write-Host "    Request $($r.index): OK" -ForegroundColor Gray
        }
    } else {
        $failed++
        Write-Host "    Request $($r.index): FAILED - $($r.error)" -ForegroundColor Red
    }
}

if ($failed -eq 0) {
    Write-Host "  PASS: All $passed concurrent requests succeeded" -ForegroundColor Green
} else {
    Write-Host "  FAIL: $failed of $ConcurrentCount requests failed" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "=== All tests passed ===" -ForegroundColor Green
