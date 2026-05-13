# TASKs.MD

Small, verifiable tasks for moving this repository from a clean v1 skeleton toward a dependable Windows-native SDKMAN-compatible tool.

## Ground Rules

- Keep each task small enough for one focused commit.
- Add or update tests with every behavior change.
- Prefer Rust behavior in `src/`; keep PowerShell and CMD wrappers thin.
- Use temp `SDKMAN_WINDOWS_DIR` values in tests.
- Do not touch the real user profile during tests.
- Run the full verification block before marking a task done:

```powershell
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

## Wrapper And Installer Tasks

### TASK-001: Only intercept shell-mutating commands in PowerShell wrapper

Status: Done.

Problem: `scripts/sdk.ps1` intercepts every `sdk env ...` command and tries to parse normal command output as JSON.

Work:
- Intercept `sdk use ...`.
- Intercept only `sdk env install`.
- Let `sdk env init` and `sdk env clear` pass through normally.

Verification:
- Add a test or script-level check proving `sdk env init` creates `.sdkmanrc` without JSON parsing.
- Add a test or script-level check proving `sdk env clear` removes `.sdkmanrc`.
- Manually run `.\scripts\sdk.ps1 env init` in a temp directory.
- Run the full verification block.

### TASK-002: Only intercept shell-mutating commands in CMD wrapper

Status: Done.

Problem: `scripts/sdk.cmd` intercepts every `sdk env ...` command and attempts to execute each output line as a CMD command.

Work:
- Intercept `sdk use ...`.
- Intercept only `sdk env install`.
- Let `sdk env init` and `sdk env clear` pass through normally.

Verification:
- Add a Windows integration check for `sdk.cmd env init`.
- Add a Windows integration check for `sdk.cmd env clear`.
- Confirm `sdk.cmd env install` still applies emitted `set` commands.
- Run the full verification block.

### TASK-003: Make installer PATH ordering match README

Problem: README says wrapper paths are placed before the raw binary, but `install.ps1` appends paths to the existing user PATH.

Work:
- Ensure the installed `scripts` directory appears before `bin`.
- Ensure `shims` appears in the intended order.
- Avoid duplicating existing PATH entries.
- Preserve unrelated PATH entries.

Verification:
- Add a PowerShell test script or documented manual check using a temporary PATH value.
- Verify repeated installer runs are idempotent.
- Confirm `sdk` resolves to the wrapper before `sdk.exe`.
- Run the full verification block.

### TASK-004: Add uninstall documentation for installed files and PATH entries

Problem: There is no documented way to remove this tool after `install.ps1`.

Work:
- Document installed directories.
- Document user PATH entries added by the installer.
- Decide whether uninstall is manual documentation or a new `uninstall.ps1`.

Verification:
- If documentation only, verify README instructions match actual installer paths.
- If adding `uninstall.ps1`, test it against a temporary install directory and temporary PATH value.
- Run the full verification block.

## Rust CLI Behavior Tasks

### TASK-005: Add a real `sdk config` display command

Status: Done.

Problem: `sdk config` currently prints only the config file path.

Work:
- Print current config key/value pairs.
- Keep output stable and easy to test.
- Do not edit config values in this task.

Verification:
- Add an integration test using temp `SDKMAN_WINDOWS_DIR`.
- Verify defaults are printed after `sdk init`.
- Verify custom config values are printed when present.
- Run the full verification block.

### TASK-006: Add a focused config set command

Status: Done.

Problem: users cannot update config through the CLI.

Work:
- Add a small command such as `sdk config set <key> <value>`.
- Validate known keys.
- Preserve existing config behavior and defaults.

Verification:
- Test setting a boolean key.
- Test setting an integer key.
- Test rejecting an unknown key.
- Test rejecting invalid values.
- Run the full verification block.

### TASK-007: Make `sdk help` output command-specific help

Status: Done.

Problem: no-arg help is hand-written and does not expose Clap's command details.

Work:
- Use Clap-generated help where practical.
- Ensure `sdk help`, `sdk help install`, and no-arg `sdk` behave predictably.

Verification:
- Add tests for no-arg output.
- Add tests for command-specific help.
- Confirm exit codes match common CLI expectations.
- Run the full verification block.

### TASK-008: Make local install registration clearer

Problem: local installs create an empty version directory containing only metadata, while the actual SDK path lives elsewhere.

Work:
- Keep the current safety rule: never delete the original local SDK path.
- Make `sdk home`, `sdk current`, and records clearly point at the real local path.
- Consider avoiding an unnecessary empty version directory if the record can still be found reliably.

Verification:
- Test local install registration.
- Test `sdk uninstall` removes only registration metadata, not the original local directory.
- Test `sdk default` works for local installs.
- Run the full verification block.

### TASK-009: Regenerate shims idempotently after every default-changing operation

Status: Done.

Problem: shim generation should be demonstrably safe and repeatable.

Work:
- Audit `default`, `uninstall`, and future install paths for shim regeneration.
- Add tests around repeated default changes.

Verification:
- Test running `sdk default` twice produces the same shim files.
- Test switching defaults updates shim targets.
- Test uninstalling the current default removes stale shims.
- Run the full verification block.

### TASK-010: Improve `sdk install` archive handling

Problem: downloads are always saved as `.zip`, even though extraction supports `.tar`, `.tar.gz`, and `.tgz`.

Work:
- Determine archive filename or extension from response headers, URL, or SDKMAN metadata if available.
- Keep fallback behavior deterministic.
- Avoid adding network calls beyond what is necessary.

Verification:
- Unit test extension selection.
- Integration test extraction from `.zip`.
- Integration test extraction from `.tar.gz`.
- Run the full verification block.

### TASK-011: Add checksum or integrity verification if SDKMAN exposes it

Problem: downloaded archives are trusted without integrity checks.

Work:
- Investigate SDKMAN API support for checksums.
- If available, verify downloaded archives before extraction.
- If unavailable, document the limitation.

Verification:
- Test checksum success.
- Test checksum mismatch fails before extraction.
- Test temporary files are left in a known state after failure.
- Run the full verification block.

### TASK-012: Add clearer offline mode behavior

Status: Done.

Problem: offline mode works in basic paths but lacks thorough user-facing and test coverage.

Work:
- Ensure all network-requiring commands fail clearly while offline.
- Ensure installed/local-only commands still work while offline.

Verification:
- Test `sdk list <candidate>` uses installed versions while offline.
- Test `sdk install` without local path fails while offline.
- Test `sdk install <candidate> <version> <local_path>` works while offline.
- Test `sdk update` fails while offline.
- Run the full verification block.

## SDKMAN Compatibility Tasks

### TASK-013: Add command alias coverage

Problem: only `uninstall` has the `rm` alias.

Work:
- Compare common SDKMAN aliases.
- Add the smallest useful alias set.
- Keep command names and semantics predictable on Windows.

Verification:
- Test each alias maps to the intended command.
- Test help output includes or documents aliases.
- Run the full verification block.

### TASK-014: Add `sdk upgrade` placeholder behavior

Problem: README says `upgrade` is out of scope, but users may try it.

Work:
- Decide whether to keep it absent or add a friendly unsupported message.
- Do not implement full upgrade behavior in this task.

Verification:
- Test the chosen behavior and exit code.
- Ensure README matches the CLI behavior.
- Run the full verification block.

### TASK-015: Add `sdk selfupdate` placeholder behavior

Problem: README says `selfupdate` is out of scope, but users may try it.

Work:
- Decide whether to keep it absent or add a friendly unsupported message.
- Do not implement full self-update behavior in this task.

Verification:
- Test the chosen behavior and exit code.
- Ensure README matches the CLI behavior.
- Run the full verification block.

### TASK-016: Add shell completion generation

Problem: completions are out of scope for v1 but useful once command shape stabilizes.

Work:
- Add optional completion generation through Clap.
- Support at least PowerShell.
- Consider CMD only if there is a practical completion story.

Verification:
- Test completion command output is non-empty.
- Manually load generated PowerShell completion in a temporary session.
- Run the full verification block.

## API And Network Tasks

### TASK-017: Mock SDKMAN API calls in tests

Problem: network behavior is not covered and should not hit the real API in tests.

Work:
- Add a small HTTP mock server test dependency.
- Cover candidates, versions, and download URL behavior.
- Keep real SDKMAN URLs centralized in `api.rs`.

Verification:
- Test candidate parsing from mock response.
- Test version parsing from mock response.
- Test fallback URL behavior for Windows version endpoints.
- Run the full verification block.

### TASK-018: Make API base URL overrideable for tests

Problem: `Api` hardcodes `https://api.sdkman.io/2`, making integration tests awkward.

Work:
- Add an environment variable or test-only constructor for API base URL.
- Keep production default unchanged.

Verification:
- Test default base URL behavior.
- Test override base URL behavior using a mock server.
- Run the full verification block.

### TASK-019: Improve API parse tests

Problem: candidate and version parsers support multiple formats but are mostly untested.

Work:
- Add unit tests for JSON arrays.
- Add unit tests for JSON objects containing arrays.
- Add unit tests for line-based fallback formats.

Verification:
- Parser tests cover candidates and versions.
- Parser tests include empty and malformed input.
- Run the full verification block.

### TASK-020: Add better network error messages

Problem: network failures currently bubble up with low-level context.

Work:
- Add command-level context for candidate listing, version listing, metadata refresh, and downloads.
- Keep error messages concise.

Verification:
- Test mocked 404/500 responses.
- Test connection failure against an unused local port.
- Confirm no secrets or full noisy debug output appear by default.
- Run the full verification block.

## Archive And Filesystem Tasks

### TASK-021: Add archive extraction tests

Problem: extraction logic supports several formats but has no direct tests.

Work:
- Test `.zip`.
- Test `.tar`.
- Test `.tar.gz`.
- Test nested root normalization.

Verification:
- Tests create archives in temp directories.
- Tests verify expected files land in the normalized root.
- Run the full verification block.

### TASK-022: Test zip slip protection

Problem: zip extraction uses `enclosed_name`, but this safety behavior should be locked down.

Work:
- Create a malicious zip fixture in a temp test.
- Verify paths cannot escape the target directory.

Verification:
- Test extraction succeeds or safely ignores malicious entries.
- Test no file is created outside the target directory.
- Run the full verification block.

### TASK-023: Add Windows link fallback tests

Problem: `fslink` falls back from symlink to junction, but behavior is not tested.

Work:
- Add Windows-specific tests for default links.
- Avoid requiring elevated privileges.
- Verify canonical paths resolve as expected.

Verification:
- Test `sdk default` creates a usable `current` link.
- Test `sdk home <candidate>` resolves through the link.
- Run tests on Windows.
- Run the full verification block.

### TASK-024: Make link removal robust for junctions and symlinks

Problem: `remove_linkish` is simple and may need stronger handling across Windows link types.

Work:
- Audit removal behavior for directory symlinks, junctions, and normal directories.
- Make sure removing `current` never deletes installed SDK contents.

Verification:
- Test removing a symlink `current`.
- Test removing a junction `current`.
- Test removing a normal directory fails or behaves only if explicitly intended.
- Run the full verification block.

## Shim Tasks

### TASK-025: Add shim generation tests

Problem: shim generation is central to PATH behavior but currently untested.

Work:
- Generate shims from a fake SDK `bin` directory.
- Cover `.exe`, `.cmd`, `.bat`, and `.ps1`.
- Ignore unsupported files.

Verification:
- Test expected `.cmd` and `.ps1` shim files exist.
- Test unsupported files do not create shims.
- Test duplicate stems create one shim pair.
- Run the full verification block.

### TASK-026: Preserve command arguments through shims

Problem: shims need to forward arguments exactly.

Work:
- Verify CMD shims preserve arguments with spaces.
- Verify PowerShell shims preserve arguments with spaces.

Verification:
- Create a fake executable/script that records received arguments.
- Run generated shims against it.
- Test arguments with spaces and quotes.
- Run the full verification block.

### TASK-027: Resolve shim command collisions intentionally

Problem: multiple SDKs may expose the same command names.

Work:
- Define collision policy.
- Document which default SDK wins when command names collide.
- Keep output deterministic.

Verification:
- Test two candidates with the same executable name.
- Test default switch updates collision outcome.
- Run the full verification block.

## Environment File Tasks

### TASK-028: Validate `.sdkmanrc` entries

Problem: `.sdkmanrc` parser accepts any `key=value` pair without validation.

Work:
- Reject empty candidate names.
- Reject empty versions.
- Decide how to handle duplicate candidates.

Verification:
- Test empty candidate failure.
- Test empty version failure.
- Test duplicate candidate behavior.
- Run the full verification block.

### TASK-029: Support comments after `.sdkmanrc` values

Problem: inline comments are common in config-style files.

Work:
- Decide whether `java=21-tem # comment` should be supported.
- Implement only if compatible with SDKMAN behavior.

Verification:
- Test inline comments.
- Test `#` inside a version is handled according to the chosen rule.
- Run the full verification block.

### TASK-030: Add auto-env support for PowerShell

Problem: `sdkman_auto_env` exists but does not do anything.

Work:
- Add opt-in PowerShell prompt or directory-change integration.
- Keep wrapper thin.
- Reuse `sdk env install`.

Verification:
- Manual test in a temporary PowerShell profile/session.
- Test enabling/disabling config changes behavior.
- Confirm no auto-env behavior when disabled.
- Run the full verification block.

## Documentation Tasks

### TASK-031: Document exact command support status

Problem: README lists examples but does not clearly separate complete, partial, and unsupported SDKMAN commands.

Work:
- Add a support matrix.
- Include aliases.
- Include known Windows-specific behavior.

Verification:
- Compare README commands against `sdk help`.
- Confirm unsupported commands are documented.
- Run the full verification block.

### TASK-032: Document local install behavior

Problem: local installs are important and have safety rules.

Work:
- Explain that unregistering local installs does not delete the original SDK directory.
- Show one PowerShell example.
- Show one CMD-compatible example if syntax differs.

Verification:
- Manually run the documented example against a temp directory.
- Confirm README language matches actual output.
- Run the full verification block.

### TASK-033: Document offline mode behavior

Problem: users need to know what works without network access.

Work:
- Document what offline mode blocks.
- Document what offline mode still permits.
- Document metadata cache dependency for listing remote versions.

Verification:
- Compare documentation against offline-mode tests.
- Run the full verification block.

### TASK-034: Add troubleshooting for Windows execution policies

Problem: PowerShell wrappers may be blocked by execution policy.

Work:
- Add troubleshooting guidance.
- Keep advice scoped and safe.
- Do not recommend weakening system-wide policy unnecessarily.

Verification:
- Confirm commands are valid PowerShell.
- Confirm README points users to the installed wrapper path.
- Run the full verification block.

## Release And CI Tasks

### TASK-035: Add CI for format, tests, and clippy

Problem: verification is manual.

Work:
- Add GitHub Actions workflow for Windows.
- Run `cargo fmt --check`.
- Run `cargo test`.
- Run `cargo clippy --all-targets -- -D warnings`.

Verification:
- CI passes on a fresh branch.
- Workflow uses the checked-in Rust toolchain.
- Run the full verification block locally.

### TASK-036: Add release build workflow

Problem: there is no repeatable binary release process.

Work:
- Build release binary on Windows.
- Attach `sdk.exe`, scripts, installer, and checksums.
- Keep signing out of scope unless signing credentials exist.

Verification:
- Dry-run or manual workflow dispatch succeeds.
- Release artifact can be installed into a temp directory.
- Run `sdk version` from the installed artifact.

### TASK-037: Add generated artifact policy

Problem: repository has `target/` locally and all files are currently untracked.

Work:
- Confirm `.gitignore` excludes build artifacts.
- Document which generated files should never be committed.
- Commit `Cargo.lock` because this is an application binary.

Verification:
- Run `git status --short`.
- Confirm build outputs are ignored.
- Run the full verification block.

### TASK-038: Add minimum supported Rust version note

Problem: `rust-toolchain.toml` exists, but README does not explain toolchain expectations.

Work:
- Document required Rust channel/version.
- Confirm it matches `rust-toolchain.toml`.

Verification:
- Run `rustc --version`.
- Confirm README and `rust-toolchain.toml` agree.
- Run the full verification block.

## Quality Bar Tasks

### TASK-039: Add end-to-end local install workflow test

Status: Done.

Problem: core non-network workflow should be proven without SDKMAN API access.

Work:
- Create fake SDK with a `bin` command.
- Register it as local.
- Set it as default.
- Verify shim execution.
- Uninstall registration.

Verification:
- Test original fake SDK directory still exists after uninstall.
- Test current link is removed or updated appropriately.
- Test shim cleanup.
- Run the full verification block.

### TASK-040: Add end-to-end `.sdkmanrc` workflow test

Status: Done.

Problem: `.sdkmanrc` workflow spans parser, install records, emit mode, and wrappers.

Work:
- Create fake installed SDK records.
- Create `.sdkmanrc`.
- Run `sdk env install --emit-env`.
- Validate emitted environment update.

Verification:
- Test PowerShell JSON emit mode.
- Test CMD emit mode.
- Test missing installed version fails clearly.
- Run the full verification block.

### TASK-041: Add failure cleanup tests

Problem: failed installs can leave partial directories or archives.

Work:
- Define expected cleanup behavior after failed download.
- Define expected cleanup behavior after failed extraction.
- Implement cleanup if needed.

Verification:
- Test failed download leaves no installed record.
- Test failed extraction leaves no installed record.
- Test repeated install after failure can succeed.
- Run the full verification block.

### TASK-042: Add smoke test script for manual Windows verification

Problem: there is no simple manual confidence script.

Work:
- Add a PowerShell smoke script under `tests/` or `scripts/`.
- Use a temp `SDKMAN_WINDOWS_DIR`.
- Avoid network unless explicitly requested.

Verification:
- Smoke script runs from a clean checkout.
- Smoke script leaves the real profile untouched.
- Run the full verification block.
