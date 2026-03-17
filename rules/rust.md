# Rust Coding Rules

## Style
- Follow `rustfmt` defaults — no custom config overrides
- Run `cargo clippy -- -W clippy::all` with zero warnings
- Prefer `impl Trait` over `dyn Trait` when possible for performance
- Use `#[must_use]` on functions that return important values

## Async
- Use `tokio` as the async runtime (multi-threaded)
- Prefer `tokio::spawn` over `std::thread::spawn`
- CPU-intensive work goes to `tokio::task::spawn_blocking`
- Use `tokio::select!` for concurrent operations with cancellation

## Traits & Abstractions
- All pluggable backends (LLM, Memory, Tools) defined as traits in `clawx-core`
- Traits should be object-safe when possible (for `dyn` dispatch)
- Use `async_trait` macro for async trait methods

## Dependencies
- Prefer Rust-native crates over FFI bindings
- Key dependencies:
  - `tokio` — async runtime
  - `serde` / `serde_json` — serialization
  - `sqlx` — database (SQLite/PostgreSQL)
  - `qdrant-client` — vector DB
  - `tantivy` — full-text search
  - `wasmtime` — WASM sandbox
  - `axum` — HTTP API
  - `tracing` — structured logging
  - `thiserror` / `anyhow` — error handling

## Module Organization
- One crate per major module (workspace layout)
- `lib.rs` exports public API only
- Internal types stay in private submodules
- Shared types in `clawx-types` crate
