# sdk-auto-env.ps1
# Dot-source this script from your PowerShell profile to enable auto-env support.
# When sdkman_auto_env=true in the SDKMAN for Windows config, changing into a
# directory that contains a .sdkmanrc file will automatically apply the versions
# defined in that file to the current shell session.
#
# Usage in your $PROFILE:
#   . "$env:USERPROFILE\.sdkman-windows\scripts\sdk-auto-env.ps1"

$ErrorActionPreference = "Stop"

$_sdkAutoEnvRoot = if ($env:SDKMAN_WINDOWS_DIR) { $env:SDKMAN_WINDOWS_DIR } else { Join-Path $env:USERPROFILE ".sdkman-windows" }
$_sdkAutoEnvExe = Join-Path $_sdkAutoEnvRoot "bin\sdk.exe"
$_sdkAutoEnvConfig = Join-Path $_sdkAutoEnvRoot "etc\config"

function _Sdk-IsAutoEnvEnabled {
    if (!(Test-Path $_sdkAutoEnvConfig)) { return $false }
    $content = Get-Content $_sdkAutoEnvConfig -Raw -ErrorAction SilentlyContinue
    return $content -match '(?m)^\s*sdkman_auto_env\s*=\s*true\s*$'
}

function _Sdk-ApplyEnvIfPresent {
    if (!(Test-Path $_sdkAutoEnvExe)) { return }
    if (!(_Sdk-IsAutoEnvEnabled)) { return }
    $rc = Join-Path (Get-Location) ".sdkmanrc"
    if (!(Test-Path $rc)) { return }

    $output = & $_sdkAutoEnvExe "--emit-env" env install 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Warning "sdk auto-env: $($output | Where-Object { $_ -notlike '__SDKMAN_ENV_JSON__*' } | Out-String)"
        return
    }

    $json = $output | Where-Object { $_ -like "__SDKMAN_ENV_JSON__*" } | Select-Object -Last 1
    if (!$json) { return }

    $json = $json.Substring("__SDKMAN_ENV_JSON__".Length)
    $updates = $json | ConvertFrom-Json
    foreach ($var in $updates.set.PSObject.Properties) {
        Set-Item -Path "Env:$($var.Name)" -Value ([string]$var.Value)
    }
    if ($updates.prepend_path) {
        $existing = $env:PATH -split ';' | Where-Object { $_ -and $_.Trim().Length -gt 0 }
        $env:PATH = ((@($updates.prepend_path) + $existing) | Select-Object -Unique) -join ';'
    }
    if ($updates.message) {
        Write-Host $updates.message
    }
}

# Wrap Set-Location / Push-Location / Pop-Location so auto-env fires on cd.
if (!(Get-Command _Sdk-OriginalSetLocation -ErrorAction SilentlyContinue)) {
    $function:_Sdk-OriginalSetLocation = $function:Set-Location
    function global:Set-Location {
        _Sdk-OriginalSetLocation @args
        _Sdk-ApplyEnvIfPresent
    }
}

if (!(Get-Command _Sdk-OriginalPushLocation -ErrorAction SilentlyContinue)) {
    $function:_Sdk-OriginalPushLocation = $function:Push-Location
    function global:Push-Location {
        _Sdk-OriginalPushLocation @args
        _Sdk-ApplyEnvIfPresent
    }
}

if (!(Get-Command _Sdk-OriginalPopLocation -ErrorAction SilentlyContinue)) {
    $function:_Sdk-OriginalPopLocation = $function:Pop-Location
    function global:Pop-Location {
        _Sdk-OriginalPopLocation @args
        _Sdk-ApplyEnvIfPresent
    }
}

# Apply immediately in case the shell starts in a directory with .sdkmanrc.
_Sdk-ApplyEnvIfPresent
