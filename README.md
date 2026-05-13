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

Out of scope for v1: `upgrade`, `selfupdate`, completions, and SDKMAN broadcast messages.

## Install From Source

```powershell
cargo build --release
.\install.ps1 -SdkExe .\target\release\sdk.exe
```

Open a new terminal after installation so PATH changes are visible.

PowerShell users should invoke the installed `sdk.ps1` wrapper, and CMD users should invoke `sdk.cmd`. The installer puts the wrapper directory before the raw binary so `sdk use` and `sdk env install` can update the current shell session.

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
