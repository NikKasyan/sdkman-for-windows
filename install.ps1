param(
    [string]$SdkExe = "$PSScriptRoot\target\release\sdk.exe",
    [string]$InstallDir = "$env:USERPROFILE\.sdkman-windows"
)

$ErrorActionPreference = "Stop"

if (!(Test-Path $SdkExe)) {
    throw "sdk.exe not found at $SdkExe. Build with: cargo build --release"
}

$binDir = Join-Path $InstallDir "bin"
$shimDir = Join-Path $InstallDir "shims"
$scriptDir = Join-Path $InstallDir "scripts"

New-Item -ItemType Directory -Force -Path $binDir, $shimDir, $scriptDir, (Join-Path $InstallDir "etc") | Out-Null
Copy-Item -Force $SdkExe (Join-Path $binDir "sdk.exe")
Copy-Item -Force "$PSScriptRoot\scripts\sdk.ps1" (Join-Path $scriptDir "sdk.ps1")
Copy-Item -Force "$PSScriptRoot\scripts\sdk.cmd" (Join-Path $scriptDir "sdk.cmd")

$currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
$parts = @()
if ($currentPath) {
    $parts = $currentPath -split ';' | Where-Object { $_ -and $_.Trim().Length -gt 0 }
}

foreach ($entry in @($scriptDir, $binDir, $shimDir)) {
    if ($parts -notcontains $entry) {
        $parts += $entry
    }
}

[Environment]::SetEnvironmentVariable("Path", ($parts -join ';'), "User")

& (Join-Path $binDir "sdk.exe") init

Write-Host "SDKMAN for Windows installed at $InstallDir"
Write-Host "Open a new terminal, then run: sdk version"
