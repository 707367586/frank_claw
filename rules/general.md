# General Coding Rules

## Principles
- **Local-first**: Never send data externally without explicit user consent
- **Security by default**: All tool execution in WASM sandbox; all network blocked unless whitelisted
- **Minimal footprint**: Idle memory < 50MB, cold start < 100ms, idle CPU < 1%

## Code Quality
- No `unwrap()` in production code — use `?` or explicit error handling
- No `unsafe` blocks without a safety comment explaining why it's sound
- All public APIs must have `/// doc comments`
- Functions should be < 50 lines; if longer, decompose
- No magic numbers — use named constants or config values

## Error Handling
- Use `thiserror` for library errors, `anyhow` for application errors
- Errors must carry context: what operation failed + why
- Never silently swallow errors — log at minimum

## Security Rules
- Never log secrets, API keys, or PII
- All external input is untrusted — validate at boundaries
- File operations: always scope to user-approved paths
- Network requests: always go through the proxy/whitelist layer

## Testing
- Unit test coverage ≥ 80% for core modules (runtime, security, memory)
- Integration tests for all user-facing flows
- Use `#[cfg(test)]` modules alongside source code
- Prefer table-driven tests for combinatorial cases

## Git
- Branch: `feat/`, `fix/`, `refactor/`, `docs/`, `test/`
- Commits: imperative mood, English, concise
- One logical change per commit
