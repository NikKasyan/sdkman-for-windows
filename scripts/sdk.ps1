$ErrorActionPreference = "Stop"

$root = if ($env:SDKMAN_WINDOWS_DIR) { $env:SDKMAN_WINDOWS_DIR } else { Join-Path $env:USERPROFILE ".sdkman-windows" }
$exe = Join-Path $root "bin\sdk.exe"

if (!(Test-Path $exe)) {
    throw "sdk.exe not found at $exe"
}

if (
    $args.Count -gt 0 -and (
        $args[0] -eq "use" -or
        ($args[0] -eq "env" -and $args.Count -gt 1 -and $args[1] -eq "install")
    )
) {
    $json = & $exe "--emit-env" @args
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

& $exe @args
exit $LASTEXITCODE
