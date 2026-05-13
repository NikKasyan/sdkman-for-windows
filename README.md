# SDKMAN for Windows

Native Windows SDKMAN-style SDK manager.

This project provides a compiled `sdk.exe` CLI plus PowerShell and CMD wrappers. It stores SDKs under `%USERPROFILE%\.sdkman-windows`, uses stable `current` links per candidate, and generates command shims in `%USERPROFILE%\.sdkman-windows\shims`.

## Status

V1 implements the core SDKMAN-style workflow:

```powershell
sdk list
sdk list java
sdk install java 21.0.4-tem
sdk default java 21.0.4-tem
sdk use java 17.0.12-tem
sdk current
sdk env init
sdk offline enable
sdk flush tmp
```

Out of scope for v1: `upgrade`, `selfupdate`, and SDKMAN broadcast messages.

## Usage

Run `sdk` or `sdk help` to see the command guide. Run `sdk help <command>` for command-specific details and examples.

| Command | What it does | Use it when |
| --- | --- | --- |
| `sdk init` | Creates the SDKMAN for Windows directory layout under `%USERPROFILE%\.sdkman-windows`. | You copied `sdk.exe` manually or want to prepare the home directory before installing SDKs. |
| `sdk list` | Lists available SDKMAN candidates. | You want to see candidate names such as `java` or `maven`. |
| `sdk list <candidate>` | Lists versions for one candidate and marks installed/current versions where possible. | You want to choose or inspect versions for a candidate. |
| `sdk install <candidate> [version]` | Downloads and installs a remote SDK version. If no version is supplied, an arrow-key picker shows available versions. A version prefix such as `25` narrows the picker when it is ambiguous. | You want SDKMAN for Windows to manage the SDK files. |
| `sdk install <candidate> <version> <path>` | Registers an existing local SDK directory without copying it. | You already have an SDK installed somewhere and want SDKMAN-style switching. |
| `sdk uninstall <candidate> <version>` | Removes a downloaded SDK version. For local registrations, it only removes the registration. Alias: `sdk rm`. | You no longer want a version managed by this tool. |
| `sdk use <candidate> [version]` | Selects a version for the current shell session by setting HOME variables and prepending that SDK's `bin` directory to PATH. Omitted versions and ambiguous prefixes open an installed-version picker. | You want a temporary version without changing the default. |
| `sdk default <candidate> [version]` | Sets the default version by updating the candidate `current` link and regenerating shims. Omitted versions and ambiguous prefixes open an installed-version picker. | You want commands such as `java` or `mvn` to resolve to this version by default. |
| `sdk current [candidate]` | Shows the active SDK home for one candidate or all installed candidates. | You want to confirm what version is active. |
| `sdk home <candidate> [version]` | Prints the active or version-specific SDK home path. | You need a path for scripts, troubleshooting, or manual inspection. |
| `sdk env init` | Creates a `.sdkmanrc` in the current directory. | You want a project to declare its required SDK versions. |
| `sdk env install` | Reads `.sdkmanrc` and applies those versions to the current shell. | You enter a project and want its SDK versions active. |
| `sdk env clear` | Removes the current directory's `.sdkmanrc`. | You no longer want project-local SDK declarations there. |
| `sdk offline enable` | Enables offline mode. | You want to block network-backed commands and use installed/local versions only. |
| `sdk offline disable` | Disables offline mode. | You want remote listing, metadata refresh, or downloads again. |
| `sdk update` | Refreshes cached SDKMAN candidate and version metadata. | Listings or installs should use fresh catalog data. |
| `sdk flush <target>` | Clears `archives`, `tmp`, `metadata`, or `all` caches. | You want downloads, extraction scratch data, or metadata rebuilt. |
| `sdk config` | Prints the config path and current values. | You want to inspect settings. |
| `sdk config set <key> <value>` | Updates a supported SDKMAN-style configuration key. | You want to change behavior such as auto-answer, timeouts, or offline mode. |
| `sdk version` | Prints version information. | You want to confirm which build is installed. |

## Install From Source

```powershell
cargo build --release
.\install.ps1 -SdkExe .\target\release\sdk.exe
```

The installer copies `sdk.exe`, installs the PowerShell and CMD wrappers, updates the user PATH, registers PowerShell completions, and runs `sdk init` for the selected install directory automatically. Open a new terminal after installation so PATH changes are visible.

PowerShell users should invoke the installed `sdk.ps1` wrapper, and CMD users should invoke `sdk.cmd`. The installer puts the wrapper directory before the raw binary so `sdk use` and `sdk env install` can update the current shell session.

The installer also registers PowerShell tab completion in the current user's Windows PowerShell and PowerShell profile paths. Completion suggests install versions from SDKMAN metadata, respecting offline mode and cached metadata, and suggests `use` versions from currently installed versions only. Pass `-SkipProfileUpdate` to `install.ps1` if you do not want the installer to edit your PowerShell profiles.

If tab completion falls back to directory names, reload your profile or dot-source the completion script manually:

```powershell
. "$env:USERPROFILE\.sdkman-windows\scripts\sdk-completion.ps1"
```

Running any command through the PowerShell wrapper also loads completions for the rest of that shell session. You can check the wrapper-loaded completion status with:

```powershell
sdk completion status
```

## Uninstall

```powershell
.\uninstall.ps1
```

By default the uninstaller removes SDKMAN for Windows from the user PATH, removes the PowerShell completion profile entry, and deletes the installed wrapper, binary, and generated shim files. It leaves installed SDKs, metadata, archives, and config under `%USERPROFILE%\.sdkman-windows` in place.

To remove the entire SDKMAN for Windows home, including downloaded SDKs and metadata, run:

```powershell
.\uninstall.ps1 -RemoveData
```

External local SDK directories registered with `sdk install <candidate> <version> <path>` are not removed.

## Workspace Git Note

If Git reports dubious ownership in this workspace, run:

```powershell
git config --global --add safe.directory C:/Users/KasyanNikolaus/Desktop/WORK/TOOLS/sdkman-windows
```

## Configuration

Config lives at `%USERPROFILE%\.sdkman-windows\etc\config` and supports SDKMAN-style keys:

```properties
sdkman_auto_answer=false
sdkman_insecure_ssl=false
sdkman_curl_connect_timeout=5
sdkman_curl_max_time=60
sdkman_colour_enable=true
sdkman_debug_mode=false
sdkman_healthcheck_enable=true
sdkman_auto_env=false
sdkman_offline_mode=false
```
