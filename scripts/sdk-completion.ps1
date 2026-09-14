$_sdkmanWindowsRoot = if ($env:SDKMAN_WINDOWS_DIR) { $env:SDKMAN_WINDOWS_DIR } else { Join-Path $env:USERPROFILE ".sdkman-windows" }
$_sdkmanWindowsPathEntries = @(
    (Join-Path $_sdkmanWindowsRoot "scripts"),
    (Join-Path $_sdkmanWindowsRoot "shims"),
    (Join-Path $_sdkmanWindowsRoot "bin")
)
$_sdkmanWindowsPathKeys = $_sdkmanWindowsPathEntries | ForEach-Object { $_.Trim().TrimEnd('\', '/') }
$_sdkmanWindowsExistingPathEntries = $env:PATH -split ';' | Where-Object {
    $_ -and $_.Trim().Length -gt 0 -and $_.Trim().TrimEnd('\', '/') -notin $_sdkmanWindowsPathKeys
}
$env:PATH = (@($_sdkmanWindowsPathEntries) + @($_sdkmanWindowsExistingPathEntries)) -join ';'

$_sdkmanWindowsExe = Join-Path $_sdkmanWindowsRoot "bin\sdk.exe"
if (Test-Path $_sdkmanWindowsExe) {
    $_sdkmanWindowsOutput = & $_sdkmanWindowsExe --emit-env init 2>$null
    if ($LASTEXITCODE -eq 0) {
        $_sdkmanWindowsJson = $_sdkmanWindowsOutput |
            Where-Object { $_ -like "__SDKMAN_ENV_JSON__*" } |
            Select-Object -Last 1
        if ($_sdkmanWindowsJson) {
            $_sdkmanWindowsJson = $_sdkmanWindowsJson.Substring("__SDKMAN_ENV_JSON__".Length)
            $_sdkmanWindowsUpdates = $_sdkmanWindowsJson | ConvertFrom-Json
            foreach ($_sdkmanWindowsVar in $_sdkmanWindowsUpdates.set.PSObject.Properties) {
                Set-Item -Path "Env:$($_sdkmanWindowsVar.Name)" -Value ([string]$_sdkmanWindowsVar.Value)
            }
        }
    }
}
Remove-Variable _sdkmanWindowsRoot, _sdkmanWindowsPathEntries, _sdkmanWindowsPathKeys, _sdkmanWindowsExistingPathEntries -ErrorAction SilentlyContinue
Remove-Variable _sdkmanWindowsExe, _sdkmanWindowsOutput, _sdkmanWindowsJson, _sdkmanWindowsUpdates, _sdkmanWindowsVar -ErrorAction SilentlyContinue

function Register-SdkmanWindowsCompletion {
    Register-ArgumentCompleter -Native -CommandName "sdk", "sdk.exe", "sdk.ps1", "sdk.cmd" -ScriptBlock {
        param($wordToComplete, $commandAst, $cursorPosition)

        $root = if ($env:SDKMAN_WINDOWS_DIR) { $env:SDKMAN_WINDOWS_DIR } else { Join-Path $env:USERPROFILE ".sdkman-windows" }
        $exe = Join-Path $root "bin\sdk.exe"
        if (!(Test-Path $exe)) {
            return
        }

        $tokens = @(
            $commandAst.CommandElements |
                Select-Object -Skip 1 |
                ForEach-Object {
                    if ($_ -is [System.Management.Automation.Language.StringConstantExpressionAst]) {
                        $_.Value
                    } else {
                        $_.Extent.Text.Trim("'`"")
                    }
                }
        )

        $line = $commandAst.Extent.Text
        if ($cursorPosition -lt $line.Length) {
            $line = $line.Substring(0, $cursorPosition)
        }
        if ($line -match '\s$') {
            $tokens += ""
        }

        & $exe complete @tokens 2>$null |
            ForEach-Object {
                [System.Management.Automation.CompletionResult]::new($_, $_, "ParameterValue", $_)
            }
    }
}

Register-SdkmanWindowsCompletion
