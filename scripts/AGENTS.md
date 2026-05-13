# AGENTS.md

## Wrapper Rules
- Keep wrappers small. Core behavior belongs in Rust.
- PowerShell wrapper may mutate `$env:PATH` and candidate-specific environment variables for the current session.
- CMD wrapper should support global/default workflows and delegate session updates through `sdk.exe emit-env`.
- Do not duplicate command validation in scripts.
