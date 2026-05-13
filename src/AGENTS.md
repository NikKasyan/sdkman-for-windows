# AGENTS.md

## Rust CLI Rules
- Keep command parsing in `cli.rs`; command effects belong in `commands.rs`.
- Keep filesystem layout centralized in `state.rs`.
- Keep SDKMAN API URL construction in `api.rs`.
- Preserve local install safety: never delete the original path for locally registered SDKs.
- Any operation that changes shims must be idempotent.
