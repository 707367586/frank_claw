# ClawX - macOS Local Claw Agent Platform

> This file is the entry point for all AI coding agents (Claude, Cursor, Windsurf, Copilot, etc.)

## Project Overview

ClawX is a macOS-native local AI agent platform built with **Rust** (core) + **SwiftUI** (GUI).
All data processing, memory, and knowledge base run locally for maximum privacy and security.

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Core Runtime | Rust (tokio async) |
| GUI | SwiftUI (macOS native) |
| Database | SQLite (default) / PostgreSQL (optional) |
| Vector DB | Qdrant (embedded mode) |
| Full-text Search | Tantivy (BM25) |
| Sandbox | Wasmtime (WASM) |
| Mobile | SwiftUI (iOS) / Jetpack Compose (Android) |
| Communication | Tailscale / iCloud CloudKit |

## Project Structure

```
frank_claw/
├── agents.md              # ← You are here. AI agent entry point.
├── workflow.md            # Development workflow (AI must follow this)
├── rules/                 # Coding rules & constraints
│   ├── general.md         # Global rules (security, errors, testing)
│   ├── rust.md            # Rust-specific rules
│   └── swift.md           # SwiftUI-specific rules
├── docs/
│   ├── prd/               # Product Requirements Documents
│   │   └── clawx-v2.0.md # PRD v2.0 — 产品需求文档
│   └── arch/              # Architecture Design Documents
│       ├── README.md              # 架构文档索引
│       ├── architecture.md        # 系统架构总览
│       ├── api-design.md          # API 设计
│       ├── data-model.md          # 数据模型
│       ├── memory-architecture.md # 记忆架构
│       ├── security-architecture.md # 安全架构
│       ├── autonomy-architecture.md # 自主性架构
│       ├── crate-dependency-graph.md # Crate 依赖图
│       └── decisions.md           # 架构决策记录 (ADR)
├── src/
│   ├── core/              # Rust core runtime (workspace crates)
│   ├── gui/               # SwiftUI macOS GUI
│   └── mobile/            # iOS / Android apps
├── tests/                 # Test files
├── scripts/               # Build / deploy scripts
└── config/                # Configuration files
```

## Key Commands

```bash
cargo build               # Build Rust core
cargo test                # Run tests
cargo clippy              # Lint (zero warnings required)
cargo fmt                 # Format
```

## How to Start (Read Order)

```
1. agents.md                    → Project overview & tech stack (you are here)
2. workflow.md                  → Step-by-step dev workflow (MUST follow)
3. docs/prd/clawx-v2.0.md      → PRD, understand what to build
4. docs/arch/architecture.md    → 系统架构总览
5. docs/arch/README.md          → 架构文档索引, 按需深入阅读
6. rules/                       → Coding constraints for your language
```

## Core Rules (Quick Reference)

- **Local-first**: Never send data externally without user consent
- **No `unwrap()` in prod**: Use `?` or explicit error handling
- **Test coverage >= 80%** for core modules
- **One logical change per commit**: imperative mood, English
- **Trait-driven design**: All backends behind abstract traits
- **Security by default**: WASM sandbox, network whitelist, DLP scanning
- Follow `rustfmt` + `clippy` with zero warnings
- All public APIs must have `/// doc comments`
- Full rules in `rules/` directory
