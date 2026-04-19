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

---

## 2026-04-18 首页端到端对话计划执行记录

Plan: `docs/superpowers/plans/2026-04-18-home-page-end-to-end-chat.md`
Branch: `feature/home-page-e2e-chat` (11 tasks, 13 commits including baseline + review-driven follow-ups)

### 完成概览

| Task | 目标 | 代表 commit |
|------|------|-------------|
| T1 | Vitest + RTL 测试框架 | `c45f730` |
| T2 | BASE_URL 默认 127.0.0.1（含 stubGlobals/stubEnvs 防泄漏）| `5f547a6` → `3630384` |
| T3 | `GET /conversations` 支持无 agent_id（并去重两个 list 函数 + id DESC tiebreak）| `40d50ed` → `9f7da59` |
| T4 | ChatWelcome 按选中 Agent 渲染（setup.ts 提取 cleanup + subtitle truncate/…）| `e3694af` → `a44824c` |
| T5 | ChatInput 去掉 Sonnet 4.6 硬编码，空值显示"未选择"（守卫 guard 补强测试）| `a92d215` → `78d337f` |
| T6 | ChatPage 透传 agent.model_name（scrollIntoView stub 提到 setup.ts；loading/dangling 测试）| `15faf18` → `c0b9ca6` |
| T7 | SSE 去占位符 + 真累积 delta（mid-stream error 不落库 + 持久化失败 tracing::error）| `e9326e6` → `5c6ce64` |
| T8 | 流结束刷新 messages（onDone 改 async/await + setError 一致化 + refreshMessages helper）| `e09ced1` → `f6ffe8f` |
| T9 | AgentSidebar 去掉假 "2 pending" + 本地化为中文 | `98817b4` → `f4fb8cc` |
| T10 | Provider 编辑 UI（updateModel API + AddProviderModal.initial + ModelProviderCard onEdit）| `b95ea4f` |
| T11 | 端到端冒烟 | 本提交 |

每个 task 走 subagent 驱动的 implementer → spec review → code-quality review → fix → re-review 循环，合计 5+ 轮 per task。

### 验证命令

```
cargo test --workspace       # 所有 Rust 测试全绿（含新增 3 个 SSE 测试）
cd apps/clawx-gui && npm test  # 6 文件 18 tests 全绿
cd apps/clawx-gui && npx tsc -b  # 类型干净
```

### 手工 walkthrough（交给用户）

1. 左侧栏不再出现 `Load failed`；默认 Agent 列表显示带中文状态（空闲/运行中）。
2. 设置 → 模型 Provider：既有智谱条目点"编辑"→填真实 API Key→保存。
3. 重启 `clawx-service`（router 只在启动时读 DB）。
4. Agents → 新建 Agent：Provider 下拉能看到那条智谱，选中→创建。
5. 回首页点该 Agent → 欢迎页显示 Agent 真名 + 自定义 subtitle；底部 composer 显示 `glm-4.6`（或所填模型名）。
6. 点击建议卡片 → 自动创建对话 → SSE 流入真实文本 → done 后持久化落库。
7. 刷新/重开窗口，历史消息保留。

---

## 2026-04-19 Phase-1 agentic tool-use loop 执行记录

Plan: `docs/superpowers/plans/2026-04-19-agentic-tool-use-phase1.md`
Branch: `worktree-agentic-tool-use-phase1`（11 tasks，14 commits：11 个 feature + 3 个 code review 后补强）

### 任务交付清单

| Task | 成果 | SHA |
|------|------|-----|
| T1 | `ContentBlock { Text / ToolUse / ToolResult }` + `Tool` / `Approval` 错误变种；`is_error: false` 不上线 | `44ffc9a` → `0fc2993` |
| T2 | Anthropic provider emit top-level `tools` + 解析 `tool_use` content blocks；`stream()` 遇到 tools 直接报错 | `beb30a9` → `c73a8fc` |
| T3 | OpenAI + Zhipu `tool_calls` wire 统一（Zhipu 复用 OpenAI 数据结构）；`is_error` 在 role:"tool" 消息里用 `[tool error] ` 前缀留痕；`sensitive` → `StopSequence` | `a5843be` → `e99a8c4` |
| T4 | `clawx-tools` crate 脚手架：`Tool` trait + `ToolRegistry` + `ToolExecCtx` + `resolve_in_workspace` | `25d1053` |
| T5 | fs 工具四件套 + symlink-escape hardening（canonical ancestor check） | `93a1e4d` → `0b7e087` |
| T6 | `shell_exec` + `sandbox-exec` profile（非 macOS 返回 unsupported） | `22b98b9` |
| T7 | `RuleApprovalGate` 三档 + 极简 shell-glob matcher + `PromptGate` trait | `3fd0862` |
| T8 | `runtime::tool_loop::run_with_tools`；`agent_loop::run_turn` 二分路径；E2E 测试用 scripted LLM 真实创建目录 | `44266bc` |
| T9 | `clawx-service` 新增 `[lib]` target，`build_runtime_for_tests()` 公共助手；`main.rs` 默认装配五件套 + `RuleApprovalGate::default_claw_code_style()` | `9fb0083` |
| T10 | `POST /tools/approval/:id`（allow/deny，204/404/400）+ `ChannelPromptGate` 通过 oneshot 把决策回传给 `PromptGate::ask` | `968528b` |
| T11 | 本条记录 + `decisions.md` ADR-036 | 本提交 |

每个 task 走 subagent 驱动：implementer → spec review → code-quality review → fix → re-review。最终 workspace 200+ 测试全绿、`cargo build --workspace` 绿、所有被修改的文件 clippy/rustfmt 零 drift（预存的无关 drift 按纪律不触碰）。

### 验证命令

- `cargo test --workspace` — 工作区全部测试绿。
- `cargo test -p clawx-runtime --test tool_loop_e2e` — scripted LLM 创建实目录。
- `cargo test -p clawx-tools --test shell_integration` — macOS 上 `sandbox-exec` 实机拦住 `$HOME` 写入。
- `cargo test -p clawx-api --test approval_route` — approval HTTP 端点 3 个用例（allow / 404 / 400）。
- `cargo clippy -p clawx-types -p clawx-llm -p clawx-tools -p clawx-runtime -p clawx-api -p clawx-service --all-targets -- -D warnings` — 本 PR 触碰的 crate 零 warning；工作区内无关 crate 的预存 drift 不在本计划范围内。

### 手工 walkthrough（macOS）

需要先在 Agent 上把 model 设成真实的 Anthropic / OpenAI / Zhipu 模型（都支持 tool wire 了）。

1. `cargo run -p clawx-service` 启动；打开桌面客户端。
2. 对 Agent 说"请在我的 workspace 里创建一个叫 `claw-demo` 的文件夹"。
3. GUI 弹出 approval dialog（`fs_mkdir` 默认 `prompt`）→ 点允许。
4. `ls ~/.clawx/workspace/claw-demo` 可见。
5. 说"在里面创建 `hello.txt` 写入 `hi claw`" → 再次 approval → 文件存在且内容正确。
6. 说"列出这个目录" → Agent 回复 `hello.txt`，无需再次确认（`fs_list` 默认 `auto`）。
7. 说"运行 `pwd`" → approval → 允许 → stdout 返回 workspace 绝对路径。
8. 说"运行 `curl https://example.com`" → `sandbox-exec` 拦截 → Agent 收到带 `Operation not permitted` 的 stderr，并向用户说明网络被沙箱禁止（Phase-1 的安全基线）。
9. 尝试"运行 `touch $HOME/escape.txt`" → 同样被沙箱拦住，`$HOME/escape.txt` 不会被创建。

### Phase-2 待办（已在计划末尾列出，非本 PR 范围）

hook / SubTurn / steering / MCP 客户端 / markdown skills / GUI approval 对话框 / streaming + tool_use。
