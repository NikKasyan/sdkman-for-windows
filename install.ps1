param(
    [string]$SdkExe = "$PSScriptRoot\target\release\sdk.exe",
    [string]$InstallDir = "$env:USERPROFILE\.sdkman-windows",
    [ValidateSet("User", "Process")]
    [string]$PathScope = "User",
    [switch]$SkipProfileUpdate,
    [switch]$SkipLocalSdkDiscovery
    ,[switch]$UnblockScripts
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

function Set-SdkmanPathEntries {
    param(
        [string]$Scope,
        [string[]]$ManagedEntries
    )

    $currentPath = [Environment]::GetEnvironmentVariable("Path", $Scope)
    $existingEntries = @()
    if ($currentPath) {
        $existingEntries = $currentPath -split ';' | Where-Object { $_ -and $_.Trim().Length -gt 0 }
    }

    $managedKeys = @{}
    foreach ($entry in $ManagedEntries) {
        $managedKeys[(Get-PathEntryKey $entry)] = $true
    }

    $preservedEntries = foreach ($entry in $existingEntries) {
        if (!$managedKeys.ContainsKey((Get-PathEntryKey $entry))) {
            $entry
        }
    }

    # Build final entries with managed entries first, then preserved entries.
    $combined = @($ManagedEntries + $preservedEntries)

    # Normalize and deduplicate while preserving order
    $seen = @{}
    $unique = New-Object System.Collections.Generic.List[string]
    foreach ($entry in $combined) {
        if (!$entry) { continue }
        $key = Get-PathEntryKey $entry
        if (-not $seen.ContainsKey($key)) {
            $seen[$key] = $true
            $unique.Add($entry)
        }
    }

    # Detect common candidate bin references to avoid duplicate-ish entries
    $candidates = @('java','maven','gradle','kotlin')
    $foundCandidates = @()
    foreach ($entry in $unique) {
        $lower = $entry.Trim().ToLowerInvariant()
        foreach ($c in $candidates) {
            if ($lower -like "*\$c\*\bin*" -or $lower -like "*\$c\*\\bin*") {
                if (-not ($foundCandidates -contains $c)) { $foundCandidates += $c }
            }
        }
    }
    if ($foundCandidates.Count -gt 0) {
        Write-Host "Detected existing candidate bin paths in PATH: $($foundCandidates -join ', '). Duplicates were removed." -ForegroundColor Cyan
    }

    [Environment]::SetEnvironmentVariable("Path", ($unique -join ';'), $Scope)
}

function Get-ReleaseValue {
    param(
        [string]$ReleaseFile,
        [string]$Key
    )

    if (!(Test-Path -LiteralPath $ReleaseFile)) {
        return $null
    }

    $line = Get-Content -LiteralPath $ReleaseFile | Where-Object { $_ -match "^$([regex]::Escape($Key))=" } | Select-Object -First 1
    if (!$line) {
        return $null
    }

    return ($line -replace "^[^=]+=", "").Trim().Trim('"')
}

function Get-JavaVendorSlug {
    param(
        [string]$Implementor,
        [string]$Path
    )

    $text = "$Implementor $Path"
    if ($text -match "Adoptium|Temurin") { return "tem" }
    if ($text -match "Microsoft") { return "ms" }
    if ($text -match "Amazon|Corretto") { return "amzn" }
    if ($text -match "Azul|Zulu") { return "zulu" }
    if ($text -match "BellSoft|Liberica") { return "librca" }
    if ($text -match "Oracle") { return "oracle" }
    if ($text -match "GraalVM") { return "graal" }
    if ($text -match "JetBrains") { return "jbr" }
    return "local"
}

function Get-DirectoryVersionId {
    param(
        [string]$SdkHome,
        [string]$Candidate
    )

    $leaf = Split-Path -Leaf $SdkHome
    if (!$leaf) {
        $leaf = $Candidate
    }

    $patterns = @(
        "^$([regex]::Escape($Candidate))[-_ ]?(?<version>[0-9][A-Za-z0-9._+-]*)",
        "^(apache-)?$([regex]::Escape($Candidate))[-_ ]?(?<version>[0-9][A-Za-z0-9._+-]*)",
        "^(?<version>[0-9][A-Za-z0-9._+-]*)$"
    )

    foreach ($pattern in $patterns) {
        $match = [regex]::Match($leaf, $pattern, [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
        if ($match.Success) {
            return Convert-ToSafeVersionId "$($match.Groups["version"].Value)-local"
        }
    }

    return Convert-ToSafeVersionId "$leaf-local"
}

function Convert-ToSafeVersionId {
    param([string]$Value)

    $safe = $Value.Trim().ToLowerInvariant()
    $safe = $safe -replace "\s+", "-"
    $safe = $safe -replace "[\\/:*?`"<>|]", "-"
    $safe = $safe -replace "[^a-z0-9._+-]", "-"
    $safe = $safe -replace "-+", "-"
    return $safe.Trim("-")
}

function Get-LocalSdkVersionId {
    param(
        [string]$Candidate,
        [string]$SdkHome
    )

    if (!$Candidate.Equals("java", [System.StringComparison]::OrdinalIgnoreCase)) {
        return Get-DirectoryVersionId -SdkHome $SdkHome -Candidate $Candidate
    }

    $releaseFile = Join-Path $SdkHome "release"
    $javaVersion = Get-ReleaseValue -ReleaseFile $releaseFile -Key "JAVA_VERSION"
    $implementor = Get-ReleaseValue -ReleaseFile $releaseFile -Key "IMPLEMENTOR"
    $vendor = Get-JavaVendorSlug -Implementor $implementor -Path $SdkHome

    if ($javaVersion) {
        return Convert-ToSafeVersionId "$javaVersion-$vendor-local"
    }

    return Get-DirectoryVersionId -SdkHome $SdkHome -Candidate "java"
}

function Find-ExistingSdkHomes {
    param(
        [string]$Candidate,
        [string[]]$EnvironmentVariables,
        [string[]]$SearchRoots,
        [string[]]$ExecutableRelativePaths
    )

    $roots = New-Object System.Collections.Generic.List[string]

    foreach ($variable in $EnvironmentVariables) {
        $path = [Environment]::GetEnvironmentVariable($variable)
        if ($path) {
            $roots.Add($path)
        }
    }

    foreach ($base in $SearchRoots) {
        if ($base -and (Test-Path -LiteralPath $base)) {
            Get-ChildItem -LiteralPath $base -Directory -ErrorAction SilentlyContinue | ForEach-Object {
                $roots.Add($_.FullName)
            }
        }
    }

    # Also attempt to locate common executables on PATH (java, mvn, gradle, etc.)
    foreach ($relative in $ExecutableRelativePaths) {
        $exeName = Split-Path -Leaf $relative
        try {
            $cmd = Get-Command $exeName -ErrorAction SilentlyContinue
        } catch {
            $cmd = $null
        }
        if ($cmd -and $cmd.Path) {
            # exe path -> ..\bin -> parent = SDK home
            $exePath = $cmd.Path
            $binDir = Split-Path -Parent $exePath
            $homeDir = Split-Path -Parent $binDir
            if ($homeDir -and (Test-Path -LiteralPath $homeDir)) {
                $roots.Add($homeDir)
            }
        }
    }

    $seen = @{}
    foreach ($path in $roots) {
        if (!$path -or !(Test-Path -LiteralPath $path)) {
            continue
        }

        $resolvedHome = (Resolve-Path -LiteralPath $path).Path
        $key = Get-PathEntryKey $resolvedHome
        if ($seen.ContainsKey($key)) {
            continue
        }
        $seen[$key] = $true

        $hasExecutable = $false
        foreach ($relative in $ExecutableRelativePaths) {
            if (Test-Path -LiteralPath (Join-Path $resolvedHome $relative)) {
                $hasExecutable = $true
                break
            }
        }

        if ($hasExecutable) {
            # TrimEnd prevents trailing backslash from escaping the closing quote
            # when passed to native executables (PowerShell quoting bug).
            $resolvedHome.TrimEnd('\', '/')
        }
    }
}

function Get-LocalSdkDiscoverySpecs {
    $programFiles = $env:ProgramFiles
    $programFilesX86 = ${env:ProgramFiles(x86)}
    $localAppData = $env:LOCALAPPDATA
    $userProfile = $env:USERPROFILE

    @(
        [pscustomobject]@{
            Candidate = "java"
            EnvironmentVariables = @("JAVA_HOME", "JDK_HOME")
            SearchRoots = @(
                "$programFiles\Java",
                "$programFiles\Eclipse Adoptium",
                "$programFiles\Microsoft",
                "$programFiles\Amazon Corretto",
                "$programFiles\BellSoft",
                "$programFiles\Zulu",
                "$programFiles\Azul",
                "$programFiles\Oracle",
                "$programFilesX86\Java"
            )
            Executables = @("bin\java.exe")
        },
        [pscustomobject]@{
            Candidate = "maven"
            EnvironmentVariables = @("MAVEN_HOME", "M2_HOME")
            SearchRoots = @(
                "$programFiles\Apache\maven",
                "$programFiles\Apache Maven",
                "$programFiles\maven",
                "$programFilesX86\Apache\maven",
                "$userProfile\scoop\apps\maven"
            )
            Executables = @("bin\mvn.cmd", "bin\mvn.bat")
        },
        [pscustomobject]@{
            Candidate = "gradle"
            EnvironmentVariables = @("GRADLE_HOME")
            SearchRoots = @(
                "$programFiles\Gradle",
                "$programFilesX86\Gradle",
                "$userProfile\scoop\apps\gradle"
            )
            Executables = @("bin\gradle.bat", "bin\gradle.cmd")
        },
        [pscustomobject]@{
            Candidate = "kotlin"
            EnvironmentVariables = @("KOTLIN_HOME")
            SearchRoots = @(
                "$programFiles\Kotlin",
                "$programFilesX86\Kotlin",
                "$localAppData\Programs\Kotlin",
                "$userProfile\scoop\apps\kotlin"
            )
            Executables = @("bin\kotlinc.bat", "bin\kotlinc.cmd")
        }
    )
}

function Register-ExistingLocalSdks {
    param([string]$SdkExePath)

    foreach ($spec in Get-LocalSdkDiscoverySpecs) {
        $homes = @(Find-ExistingSdkHomes `
                -Candidate $spec.Candidate `
                -EnvironmentVariables $spec.EnvironmentVariables `
                -SearchRoots $spec.SearchRoots `
                -ExecutableRelativePaths $spec.Executables)
        foreach ($sdkHome in $homes) {
            $version = Get-LocalSdkVersionId -Candidate $spec.Candidate -SdkHome $sdkHome
            if (!$version) {
                continue
            }

            Write-Host "Registering existing $($spec.Candidate) at $sdkHome as $($spec.Candidate) $version"
            $output = "n" | & $SdkExePath install $spec.Candidate $version $sdkHome 2>&1
            if ($LASTEXITCODE -ne 0) {
                Write-Warning "Could not register $($spec.Candidate) at $sdkHome`: $($output -join "`n")"
            }
        }
    }
}

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
Copy-Item -Force "$PSScriptRoot\scripts\sdk-auto-env.ps1" (Join-Path $scriptDir "sdk-auto-env.ps1")
$managedEntries = @($scriptDir, $shimDir, $binDir)
Set-SdkmanPathEntries -Scope $PathScope -ManagedEntries $managedEntries

# Optionally unblock downloaded script files so they can be executed without an
# extra user action. This is gated behind the -UnblockScripts switch or an
# interactive prompt. If skipped, we print instructions the user can run later.
function Do-UnblockScripts {
    param([string]$Dir)

    $files = @(
        (Join-Path $Dir "sdk.ps1"),
        (Join-Path $Dir "sdk.cmd"),
        (Join-Path $Dir "sdk-completion.ps1"),
        (Join-Path $Dir "sdk-auto-env.ps1")
    )

    foreach ($f in $files) {
        if (Test-Path -LiteralPath $f) {
            try {
                Unblock-File -LiteralPath $f -ErrorAction Stop
            } catch {
                Write-Warning ("Failed to unblock {0}: {1}" -f $f, $_.Exception.Message)
            }
        }
    }
}

if ($UnblockScripts) {
    Do-UnblockScripts -Dir $scriptDir
    Write-Host "Unblocked installer scripts in $scriptDir"
} else {
    if ($Host.UI.RawUI.KeyAvailable -or $PSBoundParameters.ContainsKey('UnblockScripts')) {
        # If running non-interactively we skip prompting; fall through to info below.
    } else {
        $answer = Read-Host "Unblock installed scripts in $scriptDir so they can be executed? [y/N]"
        if ($answer -and $answer.Trim().ToLower() -eq 'y') {
            Do-UnblockScripts -Dir $scriptDir
            Write-Host "Unblocked installer scripts in $scriptDir"
        } else {
            Write-Host "Scripts were left blocked. To allow execution later run:`n  Unblock-File -LiteralPath '$scriptDir\sdk.ps1'" -ForegroundColor Yellow
            Write-Host "Or unblock all provided scripts:`n  Unblock-File -LiteralPath '$scriptDir\*'" -ForegroundColor Yellow
        }
    }
}

$previousSdkmanWindowsDir = $env:SDKMAN_WINDOWS_DIR
try {
    $env:SDKMAN_WINDOWS_DIR = $InstallDir
    & (Join-Path $binDir "sdk.exe") init
    if (!$SkipLocalSdkDiscovery) {
        Register-ExistingLocalSdks -SdkExePath (Join-Path $binDir "sdk.exe")
    }
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
