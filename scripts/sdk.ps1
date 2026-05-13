param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$SdkArgs
)

$ErrorActionPreference = "Stop"

$root = if ($env:SDKMAN_WINDOWS_DIR) { $env:SDKMAN_WINDOWS_DIR } else { Join-Path $env:USERPROFILE ".sdkman-windows" }
$exe = Join-Path $root "bin\sdk.exe"
$shimDir = Join-Path $root "shims"

if (!(Test-Path $exe)) {
    throw "sdk.exe not found at $exe"
}

$completion = Join-Path $root "scripts\sdk-completion.ps1"
if ((Test-Path $completion) -and !(Get-Command Register-SdkmanWindowsCompletion -ErrorAction SilentlyContinue)) {
    . $completion
}

if ($SdkArgs.Count -ge 2 -and $SdkArgs[0] -eq "completion" -and $SdkArgs[1] -eq "status") {
    if (Get-Command Register-SdkmanWindowsCompletion -ErrorAction SilentlyContinue) {
        Write-Host "SDKMAN for Windows completion is loaded for this PowerShell session."
    } else {
        Write-Host "SDKMAN for Windows completion is not loaded for this PowerShell session."
    }
    Write-Host "Completion script: $completion"
    Write-Host "Reload manually with: . `"$completion`""
    return
}

if (Test-Path $shimDir) {
    $existing = $env:PATH -split ';' | Where-Object {
        $_ -and $_.Trim().Length -gt 0 -and $_ -ine $shimDir
    }
    $env:PATH = (@($shimDir) + $existing) -join ';'
}

if (
    $SdkArgs.Count -gt 0 -and (
        $SdkArgs[0] -eq "use" -or
        ($SdkArgs[0] -eq "env" -and $SdkArgs.Count -gt 1 -and $SdkArgs[1] -eq "install")
    )
) {
    $output = & $exe "--emit-env" @SdkArgs
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    $json = $output | Where-Object { $_ -like "__SDKMAN_ENV_JSON__*" } | Select-Object -Last 1
    if ($json) {
        $json = $json.Substring("__SDKMAN_ENV_JSON__".Length)
        $updates = $json | ConvertFrom-Json
        foreach ($var in $updates.set.PSObject.Properties) {
            Set-Item -Path "Env:$($var.Name)" -Value ([string]$var.Value)
        }
        if ($updates.prepend_path) {
            $existing = $env:PATH -split ';' | Where-Object { $_ -and $_.Trim().Length -gt 0 }
            $prepend = @($updates.prepend_path)
            $env:PATH = (($prepend + $existing) | Select-Object -Unique) -join ';'
        }
        if ($updates.message) {
            Write-Host $updates.message
        }
    }
    return
}

if (
    $SdkArgs.Count -gt 0 -and (
        $SdkArgs[0] -eq "install" -or
        $SdkArgs[0] -eq "default" -or
        $SdkArgs[0] -eq "uninstall" -or
        $SdkArgs[0] -eq "rm"
    )
) {
    & $exe @SdkArgs
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    return
}

& $exe @SdkArgs
exit $LASTEXITCODE
