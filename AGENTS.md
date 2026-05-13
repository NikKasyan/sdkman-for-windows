# AGENTS.md

## Project Intent
This repository implements a native Windows SDKMAN-compatible tool. The core is a compiled Rust CLI named `sdk`, with PowerShell and CMD wrappers for shell-local environment changes.

## Engineering Rules
- Keep Windows native behavior first; WSL compatibility is not a goal for v1.
- Preserve SDKMAN command names and user-facing semantics where practical.
- Prefer durable, testable Rust behavior in `src/`; keep shell wrappers thin.
- Do not delete installed SDKs when unregistering local installs.
- Avoid mutating user PATH except in `install.ps1` or an explicit install command.

## Layout
- `src/`: Rust CLI, state management, SDKMAN API client, archive extraction, shims.
- `scripts/`: PowerShell/CMD wrappers that call the compiled CLI.
- `tests/`: integration tests and fixtures.

## Verification
Run these before shipping when Rust is available:

```powershell
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```
