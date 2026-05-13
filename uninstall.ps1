param(
    [string]$InstallDir = "$env:USERPROFILE\.sdkman-windows",
    [ValidateSet("User", "Process")]
    [string]$PathScope = "User",
    [switch]$RemoveData,
    [switch]$SkipProfileUpdate
)

$ErrorActionPreference = "Stop"

function Get-PathEntryKey {
    param([string]$PathEntry)

    $trimmed = $PathEntry.Trim().TrimEnd('\', '/')
    if ($IsWindows -or $env:OS -eq "Windows_NT") {
        return $trimmed.ToLowerInvariant()
    }
    return $trimmed
}

function Remove-SdkmanPathEntries {
    param(
        [string]$Scope,
        [string[]]$ManagedEntries
    )

    $managedKeys = @{}
    foreach ($entry in $ManagedEntries) {
        $managedKeys[(Get-PathEntryKey $entry)] = $true
    }

    $currentPath = [Environment]::GetEnvironmentVariable("Path", $Scope)
    if ($currentPath) {
        $parts = $currentPath -split ';' | Where-Object {
            $_ -and $_.Trim().Length -gt 0 -and !$managedKeys.ContainsKey((Get-PathEntryKey $_))
        }
        [Environment]::SetEnvironmentVariable("Path", ($parts -join ';'), $Scope)
    }
}

$binDir = Join-Path $InstallDir "bin"
$shimDir = Join-Path $InstallDir "shims"
$scriptDir = Join-Path $InstallDir "scripts"
$completionScript = Join-Path $scriptDir "sdk-completion.ps1"

$managedEntries = @($scriptDir, $shimDir, $binDir)
Remove-SdkmanPathEntries -Scope $PathScope -ManagedEntries $managedEntries

if (!$SkipProfileUpdate) {
    $documents = [Environment]::GetFolderPath("MyDocuments")
    $profiles = @(
        $PROFILE,
        (Join-Path $documents "PowerShell\profile.ps1"),
        (Join-Path $documents "WindowsPowerShell\profile.ps1")
    ) | Select-Object -Unique

    foreach ($profilePath in $profiles) {
        if (Test-Path $profilePath) {
            $profileText = Get-Content -Raw $profilePath
            $escapedCompletionScript = [regex]::Escape($completionScript)
            $pattern = "(?m)^\s*# SDKMAN for Windows completions\r?\n\s*\.\s+[`"']?$escapedCompletionScript[`"']?\s*\r?\n?"
            $updatedProfileText = [regex]::Replace($profileText, $pattern, "")
            if ($updatedProfileText -ne $profileText) {
                Set-Content -Path $profilePath -Value $updatedProfileText -NoNewline
            }
        }
    }
}

if ($RemoveData) {
    if (Test-Path $InstallDir) {
        Remove-Item -LiteralPath $InstallDir -Recurse -Force
    }
    Write-Host "SDKMAN for Windows removed from $InstallDir"
    Write-Host "External local SDK directories registered with sdk install <candidate> <version> <path> were not removed."
} else {
    foreach ($path in @(
            (Join-Path $binDir "sdk.exe"),
            (Join-Path $scriptDir "sdk.ps1"),
            (Join-Path $scriptDir "sdk.cmd"),
            $completionScript
        )) {
        if (Test-Path $path) {
            Remove-Item -LiteralPath $path -Force
        }
    }

    if (Test-Path $shimDir) {
        Get-ChildItem -LiteralPath $shimDir -File |
            Where-Object { $_.Extension -in ".cmd", ".ps1" } |
            Remove-Item -Force
    }

    foreach ($dir in @($scriptDir, $binDir, $shimDir)) {
        if ((Test-Path $dir) -and -not (Get-ChildItem -LiteralPath $dir -Force)) {
            Remove-Item -LiteralPath $dir -Force
        }
    }

    Write-Host "SDKMAN for Windows command integration removed from $InstallDir"
    Write-Host "Installed SDKs and metadata were left in place. Run with -RemoveData to delete the SDKMAN for Windows home."
}

Write-Host "Open a new terminal for PATH/profile changes to take effect."
