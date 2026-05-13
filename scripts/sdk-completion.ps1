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
