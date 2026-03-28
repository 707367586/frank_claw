# Dead Code Analysis Report

**Date:** 2026-03-19
**Project:** ClawX (Rust workspace, 20 crates)
**Baseline:** All tests pass, `cargo clippy` clean, `cargo build` zero warnings

---

## Summary

| Category | Count | Severity |
|----------|-------|----------|
| Stub crates (zero functionality, zero dependents) | 6 | SAFE |
| Empty placeholder modules | 3 | SAFE |
| Unused `clawx-hal` dependency refs | 4 crates | CAUTION |
| Unused pub methods | 1 | SAFE |
| Suppressed dead_code warnings | 1 | OK (no action) |

---

## 1. SAFE: Stub Crates — No Implementation, No Dependents

These 6 crates are workspace members with stub implementations (9-11 lines each).
**No other crate in the workspace imports them in .rs code.**

| Crate | Content | Unused Dependencies Declared |
|-------|---------|------------------------------|
| `clawx-skills` | `SkillRegistry { _private: () }` | clawx-security, clawx-eventbus, tokio, async-trait, tracing |
| `clawx-scheduler` | `Scheduler { _private: () }` | clawx-eventbus, tokio, async-trait, tracing |
| `clawx-artifact` | `ArtifactManager { _private: () }` | clawx-eventbus, tokio, tracing |
| `clawx-ota` | `OtaUpdater { _private: () }` | clawx-hal, tokio, tracing |
| `clawx-channel` | `ChannelAdapter` trait (empty) | clawx-types, clawx-eventbus, tokio, tracing |
| `clawx-ffi` | `pub mod bridge;` (empty placeholder) | clawx-controlplane-client |

**Recommendation:** Remove all 6 crate directories and their entries from root `Cargo.toml`.

## 2. SAFE: Empty Placeholder Modules

Files containing only `//! Placeholder module.` (1 line each):

| File | Parent `mod` declaration |
|------|--------------------------|
| `crates/clawx-ffi/src/bridge.rs` | (in clawx-ffi, removed with crate) |
| `crates/clawx-security/src/prompt_defense.rs` | `clawx-security/src/lib.rs` |
| `crates/clawx-vault/src/restore.rs` | `clawx-vault/src/lib.rs` |
| `crates/clawx-memory/src/short_term.rs` | `clawx-memory/src/lib.rs` |

**Recommendation:** Delete files and remove `pub mod` declarations from parent lib.rs.

## 3. CAUTION: Unused `clawx-hal` Dependency

`clawx-hal` has real implementation (FsWatcher, KeychainStore), but is never `use`d in .rs code:

| Cargo.toml | Status |
|-----------|--------|
| `apps/clawx-service/Cargo.toml` | Listed, never imported |
| `crates/clawx-security/Cargo.toml` | Listed, never imported |
| `crates/clawx-kb/Cargo.toml` | Listed, never imported |
| `crates/clawx-ota/Cargo.toml` | (removed with stub crate) |

**Recommendation:** Remove `clawx-hal` dependency from these 3 Cargo.toml files. Keep the `clawx-hal` crate itself for future use.

## 4. SAFE: Unused Public Method

| Location | Item |
|----------|------|
| `clawx-controlplane-client/src/lib.rs:30` | `pub async fn read_token()` — Zero callers across workspace |

## 5. OK: Suppressed Warning (No Action Needed)

| Location | Item |
|----------|------|
| `clawx-llm/src/anthropic.rs:100` | `#[allow(dead_code)] content_type` — Required for serde deserialization |

---

## Proposed Cleanup Plan

### Phase 1: Remove 6 stub crates
- Delete directories: `crates/clawx-{skills,scheduler,artifact,ota,channel,ffi}/`
- Remove from `Cargo.toml` workspace members + workspace.dependencies

### Phase 2: Remove unused clawx-hal dependency refs
- Remove `clawx-hal` from Cargo.toml of: clawx-service, clawx-security, clawx-kb

### Phase 3: Remove empty placeholder modules
- Delete: `prompt_defense.rs`, `restore.rs`, `short_term.rs`
- Remove `pub mod` declarations from parent lib.rs files

### Phase 4: Remove unused public method
- Delete `read_token()` from `clawx-controlplane-client/src/lib.rs`

### Verification
After each phase: `cargo build --workspace && cargo test --workspace`
