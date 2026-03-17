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
├── docs/                  # All product & design documents
│   ├── v1.1-clawx.md     # Product Requirements Document (PRD)
│   ├── overview.md        # System architecture design
│   ├── decisions.md       # Architecture Decision Records (ADRs)
│   ├── roles.md           # AI agent role definitions
│   └── backlog.md         # Task backlog & sprint planning
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
1. agents.md               → Project overview & tech stack (you are here)
2. workflow.md              → Step-by-step dev workflow (MUST follow)
3. docs/prd/clawx-v2.0.md      → PRD, understand what to build
4. docs/arch/architecture-v2.1.md → Architecture (融合 OpenClaw/IronClaw/ZeroClaw/OpenFang 精华)
5. rules/                   → Coding constraints for your language
6. docs/backlog.md          → Find what to work on
7. docs/roles.md            → Pick your AI role
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
