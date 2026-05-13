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
    $json = & $exe "--emit-env" @SdkArgs
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    if ($json) {
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
