# Dead Code Analysis Report

**Date:** 2026-04-14
**Project:** ClawX (Rust workspace, 16 crates + Tauri/React GUI)
**Baseline:** `cargo test --workspace` green before and after; `pnpm build` green before and after.
**Tooling:** `cargo check --workspace --all-targets`, `knip@6.4.1`, `ts-prune@0.10.3`, `depcheck`.

---

## Summary of This Pass

| Category | Items | Action |
|----------|-------|--------|
| Rust unused imports | 5 | DELETED |
| Rust dead test helpers | 2 methods + 1 field | DELETED |
| Rust orphan WIP block | 1 | DELETED |
| TS unused files | 6 | DELETED |
| TS unused dependencies | 3 | REMOVED from package.json |
| Rust placeholder crate (`clawx-ffi`) | 1 | FLAGGED (CAUTION) |
| Rust orphan crate (`clawx-hal`) | 1 | FLAGGED (CAUTION) |
| TS unused api.ts exports | 54 | FLAGGED (CAUTION) |

---

## 1. DELETED: Rust — unused imports (5)

| File | Import |
|------|--------|
| `crates/clawx-memory/tests/vector_index_test.rs:6` | `clawx_kb::reciprocal_rank_fusion` |
| `crates/clawx-api/tests/integration_test.rs:1021` | `RecoveryReport` |
| `crates/clawx-api/tests/integration_test.rs:1648` | `failed_notification` |
| `crates/clawx-runtime/src/autonomy/executor.rs:848` | `ExecutionSummary` |
| `crates/clawx-runtime/src/run_recovery.rs:137` | `clawx_types::autonomy::*` |

## 2. DELETED: Rust — dead test helpers in `executor.rs`

| Location | Item | Notes |
|----------|------|-------|
| `StubToolExecutor::with_risk` | method (lines 873–877 pre-edit) | No callers anywhere |
| `StubPermissionGate::with_override` | method (lines 928–931 pre-edit) | No callers anywhere |
| `RunUpdateRecord::run_id` | field | Written at line 1043 but never read; all consumers access only `.update` |
| `step_limit_exceeded` test body | orphan `let step = ExecutionStep {...}` + WIP comments | Redundant with `step_limit_triggers_at_max`; removed ≈ 12 lines |

## 3. DELETED: TS — unused files (6)

| File | Reason |
|------|--------|
| `src/components/PermissionModal.tsx` | Orphan from commit `2ebc7ab` that removed the demo mount in AppLayout |
| `src/components/ui/Card.tsx` | No importers |
| `src/components/ui/Separator.tsx` | No importers |
| `src/components/ui/Switch.tsx` | No importers |
| `src/lib/agentTemplates.ts` | No importers |
| `src/lib/constants.ts` | No importers (entire color palette never wired up) |

## 4. REMOVED: TS — unused dependencies (3)

| Package | Scope |
|---------|-------|
| `@tauri-apps/api` | dependency — no `from '@tauri-apps/api'` anywhere in `src/` |
| `dompurify` | dependency — sanitization handled by `react-markdown` |
| `@types/dompurify` | devDependency — no longer needed |

`pnpm install` run; `pnpm-lock.yaml` regenerated.

---

## 5. CAUTION — Not deleted, needs user decision

### `crates/clawx-ffi`
- **Content:** `lib.rs` declaring `pub mod bridge;` + `bridge.rs` with a single `//! Placeholder module.` line.
- **Callers:** Zero. Not listed as a dependency of any other crate or app.
- **Comment in lib.rs:** "UniFFI bridge to Swift for ClawX" — forward-looking placeholder.
- **Recommendation:** If the SwiftUI frontend has been dropped from the roadmap, delete the crate (remove `crates/clawx-ffi/` + workspace entries). If it's still planned, leave as-is and accept the cost of a compile unit with no code.

### `crates/clawx-hal`
- **Content:** 260 lines — `FsWatcher` (fs_watcher.rs, 165 lines) + `KeychainStore` (keychain.rs, 84 lines).
- **Callers:** Zero. Not listed in any Cargo.toml anywhere except its own.
- **Note:** Unlike `clawx-ffi`, this crate has real implementation. Either it's premature (merge cost paid, usage deferred) or the calling site was removed and the crate was overlooked.
- **Recommendation:** Check whether FsWatcher/KeychainStore are intended to be wired into `clawx-security` / `clawx-vault`. If yes, add the dep and `use` path. If no, delete the crate.

### `apps/clawx-gui/src/lib/api.ts` — 54 unused exports
- These look like a mostly-complete HTTP/Tauri client surface where only a subset of endpoints are currently wired into React views.
- **Recommendation:** Do NOT delete unilaterally — they likely belong to planned but unfinished UI features (agents CRUD, channels, triggers, skills, feedback). A sweep is best done after a view-by-view audit.

---

## 6. OK — Suppressed/required, no action needed

| Location | Item | Reason |
|----------|------|--------|
| `clawx-llm/src/anthropic.rs` | `content_type` field | serde deserialization |
| `clawx-kb/src/embedding.rs` | `index` field | serde deserialization |
| `clawx-channel/src/lib.rs` | `channel_manager` field | carries Arc for planned MessageRouter wiring |
| `clawx-api/src/routes/tasks.rs` | `TaskQuery` struct | serde via axum Query extractor |

---

## 7. Resolved since 2026-04-13 report

| Item | Status |
|------|--------|
| Stub crates `clawx-skills`, `clawx-artifact`, `clawx-ota` | Removed |
| Placeholder `clawx-vault/src/restore.rs`, `clawx-memory/src/short_term.rs` | Removed |
| Unused `clawx-hal` entries in `clawx-service` / `clawx-security` / `clawx-kb` Cargo.toml | Removed (crate is now fully orphan — see §5) |
| Clippy style batches in `clawx-types` / `clawx-runtime` / `clawx-service` | Resolved (no warnings on current `cargo check`) |
| SSE `execution_step_event` / `confirmation_required_event` dead-code warnings | No longer present on current tree |

---

## Verification

- `cargo check --workspace --all-targets` — zero warnings after changes.
- `cargo test --workspace --no-fail-fast` — all suites pass (same counts as baseline; no skipped/failing tests introduced).
- `pnpm build` (in `apps/clawx-gui`) — vite build succeeds; bundle size unchanged gzip (157 kB).
