# Dead Code Analysis Report

**Date:** 2026-04-13
**Project:** ClawX (Rust workspace, 20 crates)
**Baseline:** All tests pass, `cargo test --workspace` green

---

## Summary

| Category | Count | Severity |
|----------|-------|----------|
| Stub crates (zero functionality) | 3 | SAFE |
| Empty placeholder modules | 2 | SAFE |
| Unused `clawx-hal` dependency refs | 3 crates | CAUTION |
| Dead functions (compiler warning) | 2 | SAFE |
| Suppressed `dead_code` warnings | 4 | OK (no action) |
| Clippy style warnings | 16 | SAFE (auto-fixable) |

---

## 1. SAFE: Stub Crates — No Implementation

These 3 crates are workspace members with stub implementations only.

| Crate | Content | Notes |
|-------|---------|-------|
| `clawx-skills` | `SkillRegistry { _private: () }` (10 lines) | No callers in workspace |
| `clawx-artifact` | `ArtifactManager { _private: () }` (10 lines) | No callers in workspace |
| `clawx-ota` | `OtaUpdater { _private: () }` (10 lines) | No callers in workspace |

**Note:** `clawx-ffi` is also a stub (`pub mod bridge;` + empty `bridge.rs`) but may be needed for future SwiftUI FFI bridge.

**Recommendation:** Remove `clawx-skills`, `clawx-artifact`, `clawx-ota` directories and entries from root `Cargo.toml`.

## 2. SAFE: Empty Placeholder Modules

Files containing only `//! Placeholder module.` (1 line each):

| File | Parent `mod` declaration |
|------|--------------------------|
| `crates/clawx-vault/src/restore.rs` | `clawx-vault/src/lib.rs` |
| `crates/clawx-memory/src/short_term.rs` | `clawx-memory/src/lib.rs` |
| `crates/clawx-ffi/src/bridge.rs` | `clawx-ffi/src/lib.rs` |

**Recommendation:** Delete `restore.rs` and `short_term.rs`, remove `pub mod` declarations from parent lib.rs files.

## 3. CAUTION: Unused `clawx-hal` Dependency

`clawx-hal` has real implementation (FsWatcher, KeychainStore), but is never `use`d in any .rs code:

| Cargo.toml | Status |
|-----------|--------|
| `apps/clawx-service/Cargo.toml` | Listed, never imported |
| `crates/clawx-security/Cargo.toml` | Listed, never imported |
| `crates/clawx-kb/Cargo.toml` | Listed, never imported |

**Recommendation:** Remove `clawx-hal` dependency from these 3 Cargo.toml files. Keep the `clawx-hal` crate itself.

## 4. SAFE: Dead Functions (Compiler Warning)

Functions flagged by `rustc` as never used (only called from `#[cfg(test)]`):

| Location | Function |
|----------|----------|
| `crates/clawx-api/src/routes/conversations.rs:192` | `execution_step_event()` |
| `crates/clawx-api/src/routes/conversations.rs:200` | `confirmation_required_event()` |

**Recommendation:** These are SSE event constructors intended for future Agent Loop integration. Either annotate with `#[allow(dead_code)]` or mark as `pub(crate)` if only used in tests.

## 5. OK: Suppressed `dead_code` Warnings (No Action Needed)

| Location | Item | Reason |
|----------|------|--------|
| `clawx-llm/src/anthropic.rs:100` | `content_type` field | Required for serde deserialization |
| `clawx-kb/src/embedding.rs:50` | `index` field | Required for serde deserialization |
| `clawx-channel/src/lib.rs:208` | `channel_manager` field | Stored for future use in MessageRouter |
| `clawx-api/src/routes/tasks.rs:123` | `TaskQuery` struct | Used for serde deserialization (fields accessed via Query extractor) |

## 6. Clippy Warnings (Auto-fixable)

| Crate | Count | Type |
|-------|-------|------|
| `clawx-types` | 6 | `derivable_impls` — manual Default impls that can be `#[derive(Default)]` |
| `clawx-security` | 1 | `new_without_default` — `InMemorySecretStore::new()` needs Default impl |
| `clawx-runtime` | 8 | Mixed: `match_like_matches_macro`, `collapsible_if`, `derivable_impls`, `field_reassign_with_default`, `redundant_closure`, `map_flatten` |
| `clawx-service` | 1 | `needless_borrow` |

**Recommendation:** Run `cargo clippy --fix --workspace` to auto-fix most of these.

---

## Resolved Since Last Report (2026-03-19)

| Item | Status |
|------|--------|
| `clawx-scheduler` stub crate | Now has full implementation (TaskScheduler, cron, event triggers) |
| `clawx-channel` stub crate | Now has full implementation (ChannelManager, adapters, MessageRouter) |
| `prompt_defense.rs` empty module | Now has full implementation (PatternMatchGuard, ContentSanitizer) |
| `read_token()` unused method | Now called from `apps/clawx-cli/src/main.rs:264` |
