param(
    [string]$Version,
    [ValidateSet("patch","minor","major")]
    [string]$Bump = "patch",
    [switch]$Force
)

Set-StrictMode -Version Latest

function Get-CurrentVersion {
    $toml = Get-Content -Raw Cargo.toml
    if ($toml -match 'version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"') {
        return $Matches[1]
    }
    throw 'Could not find version in Cargo.toml'
}

function Bump-Version([string]$ver, [string]$kind) {
    $parts = $ver.Split('.') | ForEach-Object { [int]$_ }
    if ($parts.Count -ne 3) { throw 'Version must be semver X.Y.Z' }
    switch ($kind) {
        'patch' { $parts[2] += 1 }
        'minor' { $parts[1] += 1; $parts[2] = 0 }
        'major' { $parts[0] += 1; $parts[1] = 0; $parts[2] = 0 }
        default { throw "Unknown bump kind: $kind" }
    }
    return ($parts -join '.')
}

function Update-CargoTomlVersion([string]$new) {
    $path = 'Cargo.toml'
    $text = Get-Content -Raw $path
    $updated = [regex]::Replace($text, 'version\s*=\s*"[0-9]+\.[0-9]+\.[0-9]+"', "version = \"$new\"")
    if ($text -eq $updated) { throw 'Failed to update Cargo.toml version' }
    Set-Content -LiteralPath $path -Value $updated -Encoding UTF8
}

function Ensure-GitClean {
    $status = git status --porcelain
    if ($status) {
        if ($Force) {
            Write-Host 'Working tree is dirty but proceeding because -Force was specified.' -ForegroundColor Yellow
        } else {
            throw 'Working tree is not clean. Commit or stash changes, or run with -Force.'
        }
    }
}

try {
    if (-not (Test-Path 'Cargo.toml')) { throw 'Cargo.toml not found in current directory' }

    $current = Get-CurrentVersion
    if (-not $Version) {
        $Version = Bump-Version $current $Bump
    }

    Write-Host "Releasing version: $Version"

    Ensure-GitClean

    Update-CargoTomlVersion -new $Version

    git add Cargo.toml
    git commit -m "chore(release): v$Version"
    git push

    $tag = "v$Version"
    git tag -a $tag -m $tag
    git push origin $tag

    Write-Host "Released $tag" -ForegroundColor Green
} catch {
    Write-Error $_.Exception.Message
    exit 1
}
