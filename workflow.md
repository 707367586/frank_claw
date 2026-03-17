# ClawX Development Workflow

> AI agents must follow this workflow for every task. No shortcuts.

## Phase 1: Understand (读)

Before writing any code, read and understand the context:

```
1. Read agents.md         → Project overview, tech stack, quick rules
2. Read docs/v1.1-clawx.md → PRD, understand the feature requirements
3. Read docs/overview.md   → System architecture, module boundaries
4. Read rules/             → Coding constraints for the language you're working in
```

**Checkpoint**: Can you explain what you're about to build and why? If not, keep reading.

## Phase 2: Plan (想)

Design before you code:

1. Identify which modules/crates are affected
2. Define the public API (traits, structs, function signatures) first
3. Consider security implications (sandbox, DLP, network whitelist)
4. Check `docs/decisions.md` for prior decisions that constrain your design
5. If making a new architectural decision, log it in `docs/decisions.md`

**Checkpoint**: Write a brief plan and confirm with the user before proceeding.

## Phase 3: Test First (测)

Write tests before implementation:

1. Write failing unit tests that define the expected behavior
2. Use table-driven tests for combinatorial cases
3. Include adversarial inputs (prompt injection, malformed data, edge cases)
4. For Rust: use `#[cfg(test)]` modules in the same file

**Checkpoint**: Tests exist and they fail (because implementation doesn't exist yet).

## Phase 4: Implement (写)

Write the minimal code to make tests pass:

1. Follow `rules/` constraints strictly
2. One logical change at a time — small, reviewable diffs
3. No `unwrap()` in production code
4. All public APIs get `/// doc comments`
5. Run `cargo clippy` and `cargo fmt` — zero warnings

**Checkpoint**: All tests pass. `cargo clippy` clean. `cargo fmt` clean.

## Phase 5: Review (审)

Self-review before delivering:

1. **Security check**: Any untrusted input? Data leaving local? Secrets in logs?
2. **Performance check**: Blocking the main thread? Memory allocation in hot paths?
3. **API check**: Is the public API minimal and intuitive?
4. **Test coverage**: Core modules ≥ 80%?
5. **No over-engineering**: Did you only build what was asked?

**Checkpoint**: Would you approve this PR from someone else?

## Phase 6: Commit (提)

Follow git conventions:

```bash
git add <specific files>    # Never git add -A
git commit -m "feat: add memory hub trait definitions"
```

- Branch: `feat/`, `fix/`, `refactor/`, `docs/`, `test/`
- Message: imperative mood, English, concise
- One logical change per commit

---

## Role-Based Entry Points

| You are... | Start with | Focus on |
|-----------|-----------|---------|
| **Architect** | Phase 1-2 | `docs/overview.md`, `docs/decisions.md` |
| **Rust Developer** | Phase 1-4 | `rules/rust.md`, `src/core/` |
| **SwiftUI Developer** | Phase 1-4 | `rules/swift.md`, `src/gui/` |
| **Security Reviewer** | Phase 5 | `docs/v1.1-clawx.md §3.4`, all source code |
| **Test Engineer** | Phase 3-4 | `tests/`, adversarial inputs |
| **Task Planner** | Phase 1-2 | `docs/backlog.md`, `docs/v1.1-clawx.md §5` |

## Anti-Patterns (Don't Do This)

- Skip Phase 1 and start coding immediately
- Write implementation before tests
- Add features nobody asked for
- Refactor unrelated code while fixing a bug
- Use `unwrap()`, ignore clippy warnings, skip tests
- Make architectural decisions without logging them
- Commit `.env`, secrets, or credentials
- Send data externally without explicit user consent
