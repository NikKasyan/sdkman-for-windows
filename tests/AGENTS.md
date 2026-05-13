# AGENTS.md

## Test Rules
- Prefer temp directories through `SDKMAN_WINDOWS_DIR`.
- Do not use a real user profile in tests.
- Mock network behavior when adding integration tests for SDKMAN API calls.
- Include Windows-specific tests for shims and link fallback where possible.
