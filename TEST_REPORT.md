# TEST_REPORT.md

Manual test checklist for the current SDKMAN for Windows implementation.

## What Should Work

### Basic CLI

- `sdk version` prints the tool name and native version.
- Running `sdk` with no arguments prints generated help.
- `sdk help <command>` prints command-specific help, for example:
  - `sdk help install`
  - `sdk help config`

Examples:

```powershell
sdk
sdk version
sdk help
sdk help install
sdk help config
sdk help env
```

### Initialization

- `sdk init` creates the SDKMAN Windows directory layout:
  - `bin`
  - `candidates`
  - `etc/config`
  - `shims`
  - `tmp`
  - `var`

Examples:

```powershell
$env:SDKMAN_WINDOWS_DIR = "$PWD\.tmp-sdkman"
sdk init
sdk config
```

### Config

- `sdk config` prints the config path and all current config values.
- `sdk config set <key> <value>` updates known config keys.
- Boolean config values accept only `true` or `false`.
- Timeout values accept non-negative integers.
- Unknown config keys should fail clearly.

Good manual checks:

```powershell
sdk config
sdk config set sdkman_auto_answer true
sdk config set sdkman_curl_max_time 12
sdk config set sdkman_missing true
sdk config set sdkman_auto_answer yes
```

More valid examples:

```powershell
sdk config set sdkman_offline_mode false
sdk config set sdkman_insecure_ssl false
sdk config set sdkman_colour_enable true
sdk config set sdkman_debug_mode false
sdk config set sdkman_auto_env false
sdk config set sdkman_curl_connect_timeout 5
```

Expected failures:

```powershell
sdk config set sdkman_unknown true
sdk config set sdkman_offline_mode maybe
sdk config set sdkman_curl_max_time fast
```

### Local Installs

- `sdk install <candidate> <version> <local_path>` registers an existing local SDK directory.
- Local installs should not delete the original SDK directory when uninstalled.
- `sdk default <candidate> <version>` points the candidate's `current` link at that SDK.
- `sdk home <candidate> <version>` prints the registered SDK path.
- `sdk uninstall <candidate> <version>` removes the registration.

Example local SDK setup:

```powershell
$sdkRoot = "$PWD\.tmp-fake-java"
New-Item -ItemType Directory -Force -Path "$sdkRoot\bin" | Out-Null
Set-Content -Path "$sdkRoot\bin\java.cmd" -Value '@echo off
echo fake-java:%*'

sdk install java 21-local $sdkRoot
sdk home java 21-local
sdk default java 21-local
sdk current java
java -version
sdk uninstall java 21-local
```

Equivalent unregister alias:

```powershell
sdk rm java 21-local
```

Important safety expectation:

- Uninstalling a local install removes SDKMAN metadata only.
- It must not delete the original local SDK directory.

### Shims

- Setting a default SDK regenerates command shims.
- Supported executable types in SDK `bin` directories:
  - `.exe`
  - `.cmd`
  - `.bat`
  - `.ps1`
- Unsupported files should not create shims.
- Duplicate command stems should create only one `.cmd` and one `.ps1` shim.
- Generated CMD and PowerShell shims should preserve arguments with spaces.

Good manual checks:

```powershell
sdk default sample 1.0-local
sample "hello world" plain
sdk default sample 2.0-local
sample "hello world" plain
sdk uninstall sample 2.0-local
```

End-to-end shim example:

```powershell
$first = "$PWD\.tmp-sample-1"
$second = "$PWD\.tmp-sample-2"
New-Item -ItemType Directory -Force -Path "$first\bin", "$second\bin" | Out-Null
Set-Content -Path "$first\bin\sample.cmd" -Value '@echo off
echo first:%*'
Set-Content -Path "$second\bin\sample.cmd" -Value '@echo off
echo second:%*'

sdk install sample 1.0-local $first
sdk install sample 2.0-local $second
sdk default sample 1.0-local
sample "hello world"
sdk default sample 2.0-local
sample "hello world"
sdk uninstall sample 2.0-local
```

### `.sdkmanrc`

- `sdk env init` creates `.sdkmanrc` in the current directory.
- `sdk env clear` removes `.sdkmanrc`.
- `sdk env install` applies versions from `.sdkmanrc`.
- PowerShell and CMD wrappers should intercept only:
  - `sdk use ...`
  - `sdk env install`
- Wrappers should let these pass through normally:
  - `sdk env init`
  - `sdk env clear`

`.sdkmanrc` parser expectations:

- Accepts normal entries like `java=21-tem`.
- Accepts inline comments like `java=21-tem # project version`.
- Preserves `#` inside unspaced values like `java=21#tem`.
- Rejects empty candidates like `=21-tem`.
- Rejects empty versions like `java=`.
- Rejects duplicate candidates.

Examples:

```powershell
sdk env init
Set-Content -Path .sdkmanrc -Value 'java=21-local'
sdk env install
sdk env clear
```

Example `.sdkmanrc` contents:

```properties
java=21-local
maven=3.9.9-local # project build tool
```

Expected invalid `.sdkmanrc` examples:

```properties
=21-local
java=
java=21-local
java=17-local
```

Wrapper-specific examples:

```powershell
.\scripts\sdk.ps1 env init
.\scripts\sdk.ps1 env install
.\scripts\sdk.ps1 use java 21-local
.\scripts\sdk.ps1 env clear
```

```cmd
scripts\sdk.cmd env init
scripts\sdk.cmd env install
scripts\sdk.cmd use java 21-local
scripts\sdk.cmd env clear
```

### Offline Mode

- `sdk offline enable` turns offline mode on.
- While offline:
  - `sdk list <candidate>` should show installed versions.
  - local install registration should still work.
  - remote install should fail clearly.
  - `sdk update` should fail clearly.
- `sdk offline disable` turns online mode back on.

Good manual checks:

```powershell
sdk offline enable
sdk list java
sdk install java 21-remote
sdk update
sdk offline disable
```

Local workflow while offline:

```powershell
sdk offline enable
sdk install java 21-local "$PWD\.tmp-fake-java"
sdk list java
sdk home java 21-local
sdk offline disable
```

Expected failures while offline:

```powershell
sdk install java 21.0.4-tem
sdk update
```

### Remote Commands

These require network access and the live SDKMAN API.

Examples:

```powershell
sdk list
sdk list java
sdk install java 21.0.4-tem
sdk default java 21.0.4-tem
sdk use java 21.0.4-tem
sdk current
sdk current java
sdk home java
sdk home java 21.0.4-tem
sdk flush metadata
sdk flush archives
sdk flush tmp
sdk flush all
```

## What Still Needs Careful Testing

- Real remote SDKMAN API calls:
  - `sdk list`
  - `sdk list java`
  - remote `sdk install java <version>`
- Archive extraction from real SDKMAN downloads.
- PowerShell wrapper behavior from an installed terminal session.
- CMD wrapper behavior from an installed terminal session.
- Installer PATH ordering and idempotency.
- Windows link behavior on machines with and without symlink privileges.
- Shim collision behavior when multiple SDKs expose the same command.
- Failure cleanup after interrupted or failed remote installs.

## Standard Verification

Run before treating changes as ready:

```powershell
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```
