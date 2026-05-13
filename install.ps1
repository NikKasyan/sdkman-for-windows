param(
    [string]$SdkExe = "$PSScriptRoot\target\release\sdk.exe",
    [string]$InstallDir = "$env:USERPROFILE\.sdkman-windows",
    [switch]$SkipProfileUpdate
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
Copy-Item -Force "$PSScriptRoot\scripts\sdk-completion.ps1" (Join-Path $scriptDir "sdk-completion.ps1")

$currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
$parts = @()
if ($currentPath) {
    $parts = $currentPath -split ';' | Where-Object { $_ -and $_.Trim().Length -gt 0 }
}

$managedEntries = @($scriptDir, $shimDir, $binDir)
$parts = @($managedEntries) + @($parts | Where-Object { $managedEntries -notcontains $_ })

[Environment]::SetEnvironmentVariable("Path", ($parts -join ';'), "User")

$previousSdkmanWindowsDir = $env:SDKMAN_WINDOWS_DIR
try {
    $env:SDKMAN_WINDOWS_DIR = $InstallDir
    & (Join-Path $binDir "sdk.exe") init
} finally {
    if ($null -eq $previousSdkmanWindowsDir) {
        Remove-Item Env:SDKMAN_WINDOWS_DIR -ErrorAction SilentlyContinue
    } else {
        $env:SDKMAN_WINDOWS_DIR = $previousSdkmanWindowsDir
    }
}

if (!$SkipProfileUpdate) {
    $completionScript = Join-Path $scriptDir "sdk-completion.ps1"
    $completionLine = ". `"$completionScript`""
    $documents = [Environment]::GetFolderPath("MyDocuments")
    $profiles = @(
        $PROFILE,
        (Join-Path $documents "PowerShell\profile.ps1"),
        (Join-Path $documents "WindowsPowerShell\profile.ps1")
    ) | Select-Object -Unique

    foreach ($profilePath in $profiles) {
        $profileDir = Split-Path -Parent $profilePath
        if ($profileDir) {
            New-Item -ItemType Directory -Force -Path $profileDir | Out-Null
        }
        $profileText = if (Test-Path $profilePath) { Get-Content -Raw $profilePath } else { "" }
        if ($profileText -notlike "*$completionScript*") {
            Add-Content -Path $profilePath -Value "`n# SDKMAN for Windows completions`n$completionLine"
        }
    }
}

Write-Host "SDKMAN for Windows installed at $InstallDir"
Write-Host "Open a new terminal, then run: sdk version"
if (!$SkipProfileUpdate) {
    Write-Host "PowerShell tab completion will be available in new PowerShell sessions."
}
