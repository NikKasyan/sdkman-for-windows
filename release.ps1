param(
    [string]$Version,
    [ValidateSet("patch", "minor", "major")]
    [string]$Bump = "patch",
    [switch]$Force,
    [switch]$SkipChecks
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Invoke-CheckedCommand {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Command,
        [string[]]$ArgumentList = @()
    )

    & $Command @ArgumentList
    if ($LASTEXITCODE -ne 0) {
        throw "$Command $($ArgumentList -join ' ') failed with exit code $LASTEXITCODE"
    }
}

function Get-CurrentVersion {
    $toml = Get-Content -Raw -LiteralPath "Cargo.toml"
    if ($toml -match '(?m)^\s*version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"\s*$') {
        return $Matches[1]
    }

    throw "Could not find package version in Cargo.toml"
}

function Assert-SemverVersion {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Value
    )

    if ($Value -notmatch '^[0-9]+\.[0-9]+\.[0-9]+$') {
        throw "Version must be semver X.Y.Z, got '$Value'"
    }
}

function Bump-Version {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Value,
        [Parameter(Mandatory = $true)]
        [ValidateSet("patch", "minor", "major")]
        [string]$Kind
    )

    Assert-SemverVersion -Value $Value
    $parts = $Value.Split(".") | ForEach-Object { [int]$_ }

    switch ($Kind) {
        "patch" { $parts[2] += 1 }
        "minor" { $parts[1] += 1; $parts[2] = 0 }
        "major" { $parts[0] += 1; $parts[1] = 0; $parts[2] = 0 }
    }

    return ($parts -join ".")
}

function Update-CargoTomlVersion {
    param(
        [Parameter(Mandatory = $true)]
        [string]$NewVersion
    )

    $path = "Cargo.toml"
    $text = Get-Content -Raw -LiteralPath $path
    $replacement = "version = `"$NewVersion`""
    $updated = [regex]::Replace(
        $text,
        '(?m)^(\s*)version\s*=\s*"[0-9]+\.[0-9]+\.[0-9]+"\s*$',
        "`${1}$replacement",
        1
    )

    if ($text -eq $updated -and $text -match "(?m)^\s*version\s*=\s*`"$NewVersion`"\s*$") {
        return $false
    }

    if ($text -eq $updated) {
        throw "Failed to update Cargo.toml version"
    }

    Set-Content -LiteralPath $path -Value $updated -Encoding UTF8
    return $true
}

function Get-GitStatus {
    $status = git status --porcelain
    if ($LASTEXITCODE -ne 0) {
        throw "git status failed. If Git reports dubious ownership, add this repository as a safe.directory and rerun."
    }

    return @($status)
}

function Confirm-DirtyRelease {
    param(
        [string[]]$Status,
        [string]$CurrentVersion
    )

    if (-not $Status) {
        return $false
    }

    if ($Force) {
        Write-Host "Working tree is dirty but proceeding because -Force was specified." -ForegroundColor Yellow
        return $true
    }

    Write-Host "Working tree is not clean:" -ForegroundColor Yellow
    $Status | ForEach-Object { Write-Host "  $_" -ForegroundColor Yellow }
    $answer = Read-Host "Re-release current version v$CurrentVersion instead of bumping? [Y/n]"
    if ($answer.Trim() -in @("", "y", "Y", "yes", "YES")) {
        Write-Host "Proceeding with current version v$CurrentVersion." -ForegroundColor Yellow
        return $true
    }

    throw "Working tree is not clean. Commit or stash changes before preparing a new bumped release."
}

function Ensure-GitClean {
    param(
        [string[]]$Status,
        [bool]$DirtyConfirmed
    )

    if ($status) {
        if (-not $DirtyConfirmed) {
            throw "Working tree is not clean. Commit or stash changes, or run with -Force."
        }
    }
}


function Ensure-TagDoesNotExist {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Tag
    )

    git rev-parse -q --verify "refs/tags/$Tag" | Out-Null
    if ($LASTEXITCODE -eq 0) {
        throw "Tag '$Tag' already exists locally."
    }

    git ls-remote --exit-code --tags origin "refs/tags/$Tag" | Out-Null
    if ($LASTEXITCODE -eq 0) {
        throw "Tag '$Tag' already exists on origin."
    }
}

Push-Location -LiteralPath $PSScriptRoot
try {
    if (-not (Test-Path -LiteralPath "Cargo.toml")) {
        throw "Cargo.toml not found in $PSScriptRoot"
    }

    $current = Get-CurrentVersion
    $gitStatus = Get-GitStatus
    $dirtyConfirmed = Confirm-DirtyRelease -Status $gitStatus -CurrentVersion $current

    if ($Version) {
        Assert-SemverVersion -Value $Version
    } elseif ($dirtyConfirmed) {
        $Version = $current
    } else {
        $Version = Bump-Version -Value $current -Kind $Bump
    }

    $tag = "v$Version"
    Write-Host "Preparing release $tag (current version: $current)"

    Ensure-GitClean -Status $gitStatus -DirtyConfirmed $dirtyConfirmed
    Ensure-TagDoesNotExist -Tag $tag

    $updatedVersion = Update-CargoTomlVersion -NewVersion $Version

    if (-not $SkipChecks) {
        Invoke-CheckedCommand -Command cargo -ArgumentList @("fmt", "--check")
        Invoke-CheckedCommand -Command cargo -ArgumentList @("test")
        Invoke-CheckedCommand -Command cargo -ArgumentList @("clippy", "--all-targets", "--", "-D", "warnings")
    }

    if ($updatedVersion) {
        Invoke-CheckedCommand -Command git -ArgumentList @("add", "Cargo.toml", "Cargo.lock")
        Invoke-CheckedCommand -Command git -ArgumentList @("commit", "-m", "chore(release): $tag")
        Invoke-CheckedCommand -Command git -ArgumentList @("push")
    } else {
        Write-Host "Cargo.toml is already at $Version; skipping version commit."
    }

    Invoke-CheckedCommand -Command git -ArgumentList @("tag", "-a", $tag, "-m", $tag)
    Invoke-CheckedCommand -Command git -ArgumentList @("push", "origin", $tag)

    Write-Host "Released $tag" -ForegroundColor Green
} catch {
    Write-Error $_.Exception.Message
    exit 1
} finally {
    Pop-Location
}
