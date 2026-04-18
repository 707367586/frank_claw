# ClawX 首页端到端对话 — 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让用户从 Tauri 客户端首页选中 Agent（绑定智谱 GLM provider）后，通过对话框/建议卡/chips 发送消息，拿到真实 SSE 流式回复并持久化——一条端到端路径全绿。

**Architecture:**
- 前端沿用现有 React+Vite 结构。新增 Vitest + React Testing Library 作为 TDD 骨架。核心修复集中在 `AgentSidebar`→`ChatPage`→`ChatWelcome`/`ChatInput` 这条链路。
- 后端 axum SSE 路径 `POST /conversations/:id/messages` 已经能调 LLM，但结束时固化成 "[streamed response]" 字面量——必须在流期间累积 delta，完成后写入真实内容。
- Zhipu provider 由 `build_llm_router` 从 DB 读取（前一次已落地），本计划只补缺失的"DB 行缺 api_key"的编辑能力，以及保证 127.0.0.1 走 IPv4 路径在 Tauri webview 下不断线。

**Tech Stack:** React 19, TypeScript, Vite, Vitest, @testing-library/react, jsdom, Rust (axum, sqlx, tokio, futures).

---

## 文件分工

**Frontend（需要修改）**
- `apps/clawx-gui/package.json` — 加 Vitest/RTL devDeps 与 `test` 脚本
- `apps/clawx-gui/vitest.config.ts` — 新建，jsdom、setupFiles
- `apps/clawx-gui/src/test/setup.ts` — 新建，全局 mock 与 `@testing-library/jest-dom`
- `apps/clawx-gui/src/lib/api.ts` — 默认 base url 改为 `http://127.0.0.1:9090`；`listConversations(agentId)` 必填
- `apps/clawx-gui/src/components/ChatWelcome.tsx` — 根据 agent 自定义标题/副标题/图标
- `apps/clawx-gui/src/components/ChatInput.tsx` — 去掉 `Sonnet 4.6` 默认，接收必填 `model` prop
- `apps/clawx-gui/src/pages/ChatPage.tsx` — 把 agent 的 `model_name` 透传给 `ChatInput`；欢迎页建议点击 → `handleWelcomeSend`；流结束后不再额外 `listMessages`（后端已持久化）
- `apps/clawx-gui/src/components/AddProviderModal.tsx` — 新增"编辑"模式，调 `updateModel`
- `apps/clawx-gui/src/components/ModelProviderCard.tsx` — 加"编辑"按钮
- `apps/clawx-gui/src/pages/SettingsPage.tsx` — 串联 modal 编辑流

**Frontend（新建测试）**
- `apps/clawx-gui/src/components/__tests__/ChatWelcome.test.tsx`
- `apps/clawx-gui/src/components/__tests__/ChatInput.test.tsx`
- `apps/clawx-gui/src/components/__tests__/AgentSidebar.test.tsx`
- `apps/clawx-gui/src/lib/__tests__/api.test.ts`
- `apps/clawx-gui/src/pages/__tests__/ChatPage.test.tsx`

**Backend**
- `crates/clawx-api/src/lib.rs` — `serve_tcp` 监听 `::` 双栈（可选，作为 IPv6 故障后备）
- `crates/clawx-api/src/routes/conversations.rs` — SSE handler 累积 delta，结束后写入真实内容；`list_conversations` 允许不带 `agent_id`
- `crates/clawx-api/src/routes/models.rs` — 补 `PATCH /models/:id`（或沿用已有 PUT）做局部更新（目前已支持 PUT + ProviderUpdate，仅验证前端调用）

---

## Task 1: 铺 Vitest + RTL 测试框架

**Files:**
- Modify: `apps/clawx-gui/package.json`
- Create: `apps/clawx-gui/vitest.config.ts`
- Create: `apps/clawx-gui/src/test/setup.ts`
- Create: `apps/clawx-gui/src/test/__smoke__.test.tsx`

- [ ] **Step 1: 写一条会失败的 smoke 测试**

文件 `apps/clawx-gui/src/test/__smoke__.test.tsx`：

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

describe("vitest smoke", () => {
  it("renders plain JSX", () => {
    render(<h1>hello-claw</h1>);
    expect(screen.getByText("hello-claw")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: 运行确认"没有 vitest" 失败**

```bash
cd apps/clawx-gui && npm test 2>&1 | head
```

Expected: `npm ERR! Missing script: "test"` 或等价。

- [ ] **Step 3: 安装测试依赖**

```bash
cd apps/clawx-gui && npm i -D vitest @vitest/ui @testing-library/react \
  @testing-library/jest-dom @testing-library/user-event jsdom
```

- [ ] **Step 4: 写 `vitest.config.ts`**

```ts
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: false,
    setupFiles: ["./src/test/setup.ts"],
    css: false,
  },
});
```

- [ ] **Step 5: 写 setup 文件**

文件 `apps/clawx-gui/src/test/setup.ts`：

```ts
import "@testing-library/jest-dom/vitest";
```

- [ ] **Step 6: 在 `package.json` 加 `test` 脚本**

把 `scripts` 改为：

```json
"scripts": {
  "dev": "vite",
  "build": "tsc -b && vite build",
  "preview": "vite preview",
  "tauri": "tauri",
  "test": "vitest run",
  "test:watch": "vitest"
}
```

- [ ] **Step 7: 跑测试确认 smoke 绿**

```bash
cd apps/clawx-gui && npm test
```

Expected: `1 passed`.

- [ ] **Step 8: commit**

```bash
git add apps/clawx-gui/package.json apps/clawx-gui/package-lock.json \
  apps/clawx-gui/vitest.config.ts apps/clawx-gui/src/test
git commit -m "test(gui): bootstrap vitest + react-testing-library"
```

---

## Task 2: 修复 API base url：IPv6 → IPv4

**Files:**
- Modify: `apps/clawx-gui/src/lib/api.ts`
- Create: `apps/clawx-gui/src/lib/__tests__/api.test.ts`

背景：macOS 下 `localhost` 优先解析到 `::1`，而服务端只监听 `127.0.0.1`，Tauri webview 直接 `TypeError: Load failed`。把默认 URL 换到 `http://127.0.0.1:9090`。

- [ ] **Step 1: 写失败测试**

文件 `apps/clawx-gui/src/lib/__tests__/api.test.ts`：

```ts
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";

describe("api base url", () => {
  const ORIGINAL = import.meta.env.VITE_API_URL;
  beforeEach(() => {
    vi.resetModules();
    // @ts-expect-error overwrite for test
    import.meta.env.VITE_API_URL = undefined;
  });
  afterEach(() => {
    // @ts-expect-error restore
    import.meta.env.VITE_API_URL = ORIGINAL;
  });

  it("defaults to 127.0.0.1 to avoid macOS IPv6 localhost resolution", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response("[]", { status: 200, headers: { "content-type": "application/json" } }),
    );
    vi.stubGlobal("fetch", fetchMock);
    const { listAgents } = await import("../api");
    await listAgents();
    expect(fetchMock).toHaveBeenCalledWith(
      "http://127.0.0.1:9090/agents",
      expect.any(Object),
    );
  });
});
```

- [ ] **Step 2: 运行确认 fail**

```bash
cd apps/clawx-gui && npx vitest run src/lib/__tests__/api.test.ts
```

Expected: 断言命中 `http://localhost:9090/agents`，失败。

- [ ] **Step 3: 改 `api.ts` 默认 URL**

`apps/clawx-gui/src/lib/api.ts` 第 22 行：

```ts
const BASE_URL = import.meta.env.VITE_API_URL ?? "http://127.0.0.1:9090";
```

- [ ] **Step 4: 跑测试绿**

```bash
cd apps/clawx-gui && npx vitest run src/lib/__tests__/api.test.ts
```

Expected: PASS.

- [ ] **Step 5: commit**

```bash
git add apps/clawx-gui/src/lib/api.ts apps/clawx-gui/src/lib/__tests__/api.test.ts
git commit -m "fix(gui/api): default BASE_URL to 127.0.0.1 to bypass IPv6 resolution"
```

---

## Task 3: `listConversations` 允许无 agentId 调用

**Files:**
- Modify: `crates/clawx-api/src/routes/conversations.rs:75-88`

背景：`ChatPage.tsx` 调 `listConversations()`（不带 agentId）时，后端 `Query<ListConversationsQuery>` 必填 agent_id，直接 400。要么前端强制传，要么后端改为 optional。选后端更省事。

- [ ] **Step 1: 写失败测试（Rust）**

在 `crates/clawx-api/src/routes/conversations.rs` 文件 `tests` 模块末尾追加：

```rust
#[tokio::test]
async fn list_conversations_without_agent_id_returns_200() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    let state = crate::tests::make_state().await;
    let app = crate::build_router(state.clone());
    let req = Request::builder()
        .uri("/conversations")
        .header("Authorization", "Bearer test-token")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
```

（注：`crate::tests::make_state` 若当前可见性不够，用 `pub(crate)` 放开；或改走 local helper。）

- [ ] **Step 2: 运行 fail**

```bash
cargo test -p clawx-api list_conversations_without_agent_id_returns_200
```

Expected: FAIL（400 Bad Request）。

- [ ] **Step 3: 改 handler 签名**

替换 `list_conversations`：

```rust
#[derive(Debug, Deserialize)]
struct ListConversationsQuery {
    #[serde(default)]
    agent_id: Option<String>,
}

async fn list_conversations(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(query): axum::extract::Query<ListConversationsQuery>,
) -> ApiResult<Json<Vec<Value>>> {
    let convs = match query.agent_id {
        Some(agent_id) => {
            conversation_repo::list_conversations(&state.runtime.db.main, &agent_id)
                .await
                .map_err(internal_err)?
        }
        None => conversation_repo::list_all_conversations(&state.runtime.db.main)
            .await
            .map_err(internal_err)?,
    };
    Ok(Json(convs))
}
```

在 `crates/clawx-runtime/src/conversation_repo.rs` 加 `list_all_conversations`（若未存在）：

```rust
pub async fn list_all_conversations(pool: &SqlitePool) -> Result<Vec<serde_json::Value>> {
    let rows = sqlx::query("SELECT id, agent_id, title, status, created_at, updated_at FROM conversations ORDER BY updated_at DESC")
        .fetch_all(pool)
        .await
        .map_err(|e| ClawxError::Database(format!("list_all_conversations: {}", e)))?;
    Ok(rows.into_iter().map(|r| {
        use sqlx::Row;
        serde_json::json!({
            "id": r.get::<String, _>("id"),
            "agent_id": r.get::<String, _>("agent_id"),
            "title": r.get::<Option<String>, _>("title"),
            "status": r.get::<String, _>("status"),
            "created_at": r.get::<String, _>("created_at"),
            "updated_at": r.get::<String, _>("updated_at"),
        })
    }).collect())
}
```

- [ ] **Step 4: 跑测试绿**

```bash
cargo test -p clawx-api list_conversations_without_agent_id_returns_200
cargo test -p clawx-runtime list_all_conversations
```

Expected: 都 PASS。

- [ ] **Step 5: commit**

```bash
git add crates/clawx-api/src/routes/conversations.rs crates/clawx-runtime/src/conversation_repo.rs
git commit -m "fix(api): list_conversations accepts no agent_id filter"
```

---

## Task 4: ChatWelcome 按选中 Agent 定制标题

**Files:**
- Modify: `apps/clawx-gui/src/components/ChatWelcome.tsx`
- Create: `apps/clawx-gui/src/components/__tests__/ChatWelcome.test.tsx`

当前 hero 永远显示 "MaxClaw"。改为 agent.name，图标/角色描述随角色切换。

- [ ] **Step 1: 失败测试**

文件 `apps/clawx-gui/src/components/__tests__/ChatWelcome.test.tsx`：

```tsx
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import ChatWelcome from "../ChatWelcome";
import type { Agent } from "../../lib/types";

const agent: Agent = {
  id: "a1",
  name: "编程助手",
  role: "Developer",
  system_prompt: "",
  model_id: "m1",
  status: "idle",
  created_at: "",
  updated_at: "",
};

describe("ChatWelcome", () => {
  it("shows the selected agent's name, not a hardcoded MaxClaw", () => {
    render(<ChatWelcome agent={agent} />);
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("编程助手");
    expect(screen.queryByText("MaxClaw")).toBeNull();
  });

  it("forwards chip clicks through onSend", async () => {
    const onSend = vi.fn();
    render(<ChatWelcome agent={agent} onSend={onSend} />);
    await userEvent.click(screen.getByRole("button", { name: "对话" }));
    expect(onSend).toHaveBeenCalledWith("对话");
  });
});
```

- [ ] **Step 2: 跑 fail**

```bash
cd apps/clawx-gui && npx vitest run src/components/__tests__/ChatWelcome.test.tsx
```

Expected: 第一条断言失败（显示 MaxClaw）。

- [ ] **Step 3: 改 `ChatWelcome.tsx`**

```tsx
import { Sparkles, MessageSquare, FileText, Code, Search, ChevronRight } from "lucide-react";
import type { Agent } from "../lib/types";

const TAGS = ["对话", "文件创建", "代码编写", "分析研究", "总结", "文献检索", "任务规划", "代码审查"];

const SUGGESTIONS = [
  { icon: MessageSquare, text: "智能分析业务流程并提出建议" },
  { icon: FileText, text: "快速生成高质量技术文档" },
  { icon: Code, text: "为移动端设计一个技术方案" },
  { icon: Search, text: "研究并汇总行业最新动态" },
];

interface Props {
  agent?: Agent;
  onSend?: (t: string) => void | Promise<void>;
}

export default function ChatWelcome({ agent, onSend }: Props) {
  const title = agent?.name ?? "ClawX";
  const subtitle = agent?.system_prompt?.slice(0, 80)
    || "选中一个 Agent 开始对话，或在下方输入问题。";

  const handleSuggest = async (text: string) => {
    if (onSend) await onSend(text);
  };

  return (
    <div className="chat-welcome">
      <div className="chat-welcome__hero">
        <div className="chat-welcome__icon"><Sparkles size={30} /></div>
        <h1 className="chat-welcome__title">{title}</h1>
        <p className="chat-welcome__subtitle">{subtitle}</p>
      </div>
      <div className="chat-welcome__tags">
        {TAGS.map((t) => (
          <button key={t} className="chat-welcome__tag" onClick={() => handleSuggest(t)}>
            {t}
          </button>
        ))}
      </div>
      <ul className="chat-welcome__suggestions">
        {SUGGESTIONS.map((s) => (
          <li key={s.text}>
            <button className="chat-welcome__suggestion" onClick={() => handleSuggest(s.text)}>
              <s.icon size={16} className="chat-welcome__suggestion-icon" />
              <span>{s.text}</span>
              <ChevronRight size={14} className="chat-welcome__suggestion-chevron" />
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
```

- [ ] **Step 4: 测试绿**

```bash
cd apps/clawx-gui && npx vitest run src/components/__tests__/ChatWelcome.test.tsx
```

Expected: 2 passed。

- [ ] **Step 5: commit**

```bash
git add apps/clawx-gui/src/components/ChatWelcome.tsx \
  apps/clawx-gui/src/components/__tests__/ChatWelcome.test.tsx
git commit -m "feat(gui/chat): welcome hero reflects selected agent"
```

---

## Task 5: ChatInput 显示 agent 当前模型

**Files:**
- Modify: `apps/clawx-gui/src/components/ChatInput.tsx`
- Create: `apps/clawx-gui/src/components/__tests__/ChatInput.test.tsx`

取消 `Sonnet 4.6` fallback —— ChatPage 必须传 `model`，否则显示 "未选择"。

- [ ] **Step 1: 失败测试**

文件 `apps/clawx-gui/src/components/__tests__/ChatInput.test.tsx`：

```tsx
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import ChatInput from "../ChatInput";

describe("ChatInput", () => {
  it("renders the model passed in, not a hardcoded default", () => {
    render(<ChatInput onSend={() => {}} model="glm-4.6" />);
    expect(screen.getByText("glm-4.6")).toBeInTheDocument();
    expect(screen.queryByText("Sonnet 4.6")).toBeNull();
  });

  it("shows `未选择` placeholder when no model supplied", () => {
    render(<ChatInput onSend={() => {}} />);
    expect(screen.getByText("未选择")).toBeInTheDocument();
  });

  it("sends on Enter and clears the input", async () => {
    const onSend = vi.fn();
    render(<ChatInput onSend={onSend} model="glm-4.6" />);
    const field = screen.getByPlaceholderText("输入任何问题...");
    await userEvent.type(field, "你好{enter}");
    expect(onSend).toHaveBeenCalledWith("你好");
    expect(field).toHaveValue("");
  });
});
```

- [ ] **Step 2: fail**

```bash
cd apps/clawx-gui && npx vitest run src/components/__tests__/ChatInput.test.tsx
```

Expected: 第一条命中 "Sonnet 4.6" 失败。

- [ ] **Step 3: 改 `ChatInput.tsx`**

```tsx
import { useState, type KeyboardEvent } from "react";
import { Plus, Zap, ArrowUp, ChevronDown } from "lucide-react";
import IconButton from "./ui/IconButton";

interface Props {
  onSend: (text: string) => void;
  disabled?: boolean;
  model?: string;
  onPickModel?: () => void;
}

export default function ChatInput({ onSend, disabled, model, onPickModel }: Props) {
  const [value, setValue] = useState("");
  const label = model && model.length > 0 ? model : "未选择";

  function submit() {
    const t = value.trim();
    if (!t || disabled) return;
    onSend(t);
    setValue("");
  }
  function onKey(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      submit();
    }
  }

  return (
    <div className="chat-input">
      <IconButton icon={<Plus size={16} />} aria-label="附件" variant="ghost" size="sm" />
      <button className="chat-input__skill" type="button">
        <Zap size={14} />
        <span>技能</span>
      </button>
      <input
        className="chat-input__field"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={onKey}
        placeholder="输入任何问题..."
        disabled={disabled}
      />
      <button className="chat-input__model" type="button" onClick={onPickModel}>
        <span>{label}</span>
        <ChevronDown size={14} />
      </button>
      <IconButton
        icon={<ArrowUp size={16} />}
        aria-label="发送"
        variant="default"
        size="sm"
        onClick={submit}
        disabled={disabled || !value.trim()}
      />
    </div>
  );
}
```

- [ ] **Step 4: 测试绿**

```bash
cd apps/clawx-gui && npx vitest run src/components/__tests__/ChatInput.test.tsx
```

Expected: 3 passed。

- [ ] **Step 5: commit**

```bash
git add apps/clawx-gui/src/components/ChatInput.tsx \
  apps/clawx-gui/src/components/__tests__/ChatInput.test.tsx
git commit -m "feat(gui/chat): composer shows agent's real model, removes stale default"
```

---

## Task 6: ChatPage 把 agent 的 model_name 透传给 ChatInput

**Files:**
- Modify: `apps/clawx-gui/src/pages/ChatPage.tsx`
- Modify: `apps/clawx-gui/src/lib/store.tsx` — 扩展 agent 带 model_name（或新 hook）
- Modify: `apps/clawx-gui/src/lib/types.ts` — 增加 ProviderLookup 约束
- Create: `apps/clawx-gui/src/pages/__tests__/ChatPage.test.tsx`

最简方案：ChatPage 自己在 agent 变化时 `getProviderById(agent.model_id)` 拿 `model_name`，缓存在 state。

- [ ] **Step 1: 失败测试**

文件 `apps/clawx-gui/src/pages/__tests__/ChatPage.test.tsx`：

```tsx
import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import ChatPage from "../ChatPage";
import { AgentProvider } from "../../lib/store";

vi.mock("../../lib/api", () => ({
  listAgents: vi.fn().mockResolvedValue([
    {
      id: "a1", name: "编程助手", role: "Developer",
      system_prompt: "helper", model_id: "p1",
      status: "idle", created_at: "", updated_at: "",
    },
  ]),
  listModels: vi.fn().mockResolvedValue([
    { id: "p1", name: "智谱", provider_type: "zhipu",
      base_url: "", model_name: "glm-4.6", parameters: {},
      is_default: true, created_at: "", updated_at: "" },
  ]),
  listConversations: vi.fn().mockResolvedValue([]),
  listMessages: vi.fn().mockResolvedValue([]),
  sendMessageStream: vi.fn(() => new AbortController()),
  createConversation: vi.fn(),
}));

describe("ChatPage model surface", () => {
  it("shows the model bound to the selected agent", async () => {
    render(
      <MemoryRouter initialEntries={["/?agent=a1"]}>
        <AgentProvider>
          <Routes>
            <Route path="/" element={<ChatPage />} />
          </Routes>
        </AgentProvider>
      </MemoryRouter>,
    );
    await waitFor(() => expect(screen.getByText("glm-4.6")).toBeInTheDocument());
  });
});
```

- [ ] **Step 2: fail**

```bash
cd apps/clawx-gui && npx vitest run src/pages/__tests__/ChatPage.test.tsx
```

Expected: 找不到 `glm-4.6`。

- [ ] **Step 3: 在 `ChatPage.tsx` 加 provider 解析**

在现有 imports 下追加：

```tsx
import { listModels } from "../lib/api";
import type { ModelProvider } from "../lib/types";
```

在 `useAgents()` 后追加：

```tsx
const [providers, setProviders] = useState<ModelProvider[]>([]);

useEffect(() => {
  let cancelled = false;
  listModels()
    .then((p) => { if (!cancelled) setProviders(p); })
    .catch(() => { /* silent; composer falls back to 未选择 */ });
  return () => { cancelled = true; };
}, []);

const modelName = agent
  ? providers.find((p) => p.id === agent.model_id)?.model_name
  : undefined;
```

把两处 `<ChatInput onSend=... />` 改为：

```tsx
<ChatInput onSend={handleSend} disabled={isStreaming || loading} model={modelName} />
```

`ChatWelcome` 的使用保持不变（已经接收 agent）。

- [ ] **Step 4: 测试绿**

```bash
cd apps/clawx-gui && npx vitest run src/pages/__tests__/ChatPage.test.tsx
```

Expected: PASS。

- [ ] **Step 5: commit**

```bash
git add apps/clawx-gui/src/pages/ChatPage.tsx \
  apps/clawx-gui/src/pages/__tests__/ChatPage.test.tsx
git commit -m "feat(gui/chat): resolve agent's provider to surface model in composer"
```

---

## Task 7: 后端 SSE 真正持久化 assistant 回复

**Files:**
- Modify: `crates/clawx-api/src/routes/conversations.rs:220-340`
- Modify (test): 同文件 tests 模块

- [ ] **Step 1: 失败测试**

在 `crates/clawx-api/src/routes/conversations.rs` 的 `#[cfg(test)] mod tests` 里追加（StubLlmProvider 每次产生 `[stub] hello` 这样的内容，具体 token 取决实现——测试里我们只断言不是 `[streamed response]`）：

```rust
#[tokio::test]
async fn sse_persists_accumulated_content_not_placeholder() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    let state = crate::tests::make_state().await;
    let app = crate::build_router(state.clone());

    // 1. 建 agent + conversation
    let agent_id = clawx_runtime::agent_repo::create_agent(
        &state.runtime.db.main,
        &clawx_types::agent::AgentConfig {
            id: clawx_types::ids::AgentId::new(),
            name: "t".into(),
            role: "t".into(),
            system_prompt: None,
            model_id: clawx_types::ids::ProviderId::new(),
            icon: None,
            status: clawx_types::agent::AgentStatus::Idle,
            capabilities: vec![],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_active_at: None,
        },
    )
    .await
    .unwrap()
    .id;
    let conv_id = clawx_runtime::conversation_repo::create_conversation(
        &state.runtime.db.main,
        &agent_id.to_string(),
        None,
    )
    .await
    .unwrap();

    // 2. 发消息 + stream
    let body = serde_json::json!({
        "role": "user", "content": "ping", "stream": true,
    });
    let req = Request::builder()
        .method("POST")
        .uri(format!("/conversations/{}/messages", conv_id))
        .header("Authorization", "Bearer test-token")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    // 消费完整 SSE 流
    let _ = resp.into_body().collect().await.unwrap();

    // 3. 断言存下来的 assistant 消息不是 placeholder
    let messages =
        clawx_runtime::conversation_repo::list_messages(&state.runtime.db.main, &conv_id)
            .await
            .unwrap();
    let assistant = messages
        .iter()
        .find(|m| m["role"] == "assistant")
        .expect("assistant message must exist");
    let content = assistant["content"].as_str().unwrap();
    assert!(
        !content.contains("[streamed response]"),
        "assistant content must be accumulated deltas, got: {}",
        content
    );
    assert!(!content.is_empty(), "assistant content must not be empty");
}
```

- [ ] **Step 2: 运行 fail**

```bash
cargo test -p clawx-api sse_persists_accumulated_content_not_placeholder
```

Expected: FAIL（断言 `[streamed response]`）。

- [ ] **Step 3: 改 `stream_agent_response` 的持久化逻辑**

替换 `stream_agent_response` 里从 `match state.runtime.llm.stream(request).await { Ok(llm_stream) => { ... } }` 开始那段：

```rust
match state.runtime.llm.stream(request).await {
    Ok(llm_stream) => {
        let state_clone = state.clone();
        let conv_id = conversation_id.clone();
        let accumulator: Arc<tokio::sync::Mutex<String>> =
            Arc::new(tokio::sync::Mutex::new(String::new()));

        let acc_for_stream = accumulator.clone();
        let sse_stream = llm_stream.map(move |chunk_result| {
            let acc = acc_for_stream.clone();
            match chunk_result {
                Ok(chunk) => {
                    let delta = chunk.delta.clone();
                    if !delta.is_empty() {
                        // push to accumulator non-blocking
                        tokio::task::spawn(async move {
                            let mut guard = acc.lock().await;
                            guard.push_str(&delta);
                        });
                    }
                    Ok(Event::default()
                        .event("delta")
                        .data(
                            serde_json::to_string(&json!({
                                "delta": chunk.delta,
                                "stop_reason": chunk.stop_reason,
                            }))
                            .unwrap_or_default(),
                        ))
                }
                Err(e) => Ok(Event::default()
                    .event("error")
                    .data(json!({"error": e.to_string()}).to_string())),
            }
        });

        let acc_for_done = accumulator.clone();
        let done_event = stream::once(async move {
            let final_content = acc_for_done.lock().await.clone();
            let _ = conversation_repo::add_message(
                &state_clone.runtime.db.main,
                &conv_id,
                "assistant",
                if final_content.is_empty() {
                    "(empty response)"
                } else {
                    &final_content
                },
            )
            .await;

            Ok::<_, std::convert::Infallible>(
                Event::default()
                    .event("done")
                    .data(json!({"status": "complete"}).to_string()),
            )
        });

        Sse::new(Box::pin(sse_stream.chain(done_event)) as _)
    }
    Err(e) => {
        // unchanged
        ...
    }
}
```

> **注意：** `tokio::task::spawn` 在每个 delta 里累积是有顺序风险的——替换为同步累积更稳妥：
> ```rust
> let sse_stream = llm_stream.then(move |chunk_result| {
>     let acc = acc_for_stream.clone();
>     async move {
>         match chunk_result {
>             Ok(chunk) => {
>                 if !chunk.delta.is_empty() {
>                     acc.lock().await.push_str(&chunk.delta);
>                 }
>                 Ok(Event::default().event("delta").data(
>                     serde_json::to_string(&json!({
>                         "delta": chunk.delta,
>                         "stop_reason": chunk.stop_reason,
>                     })).unwrap_or_default(),
>                 ))
>             }
>             Err(e) => Ok(Event::default().event("error").data(
>                 json!({"error": e.to_string()}).to_string())),
>         }
>     }
> });
> ```
> 用 `futures::StreamExt::then` 代替 `.map`，配合 `async move`，就能同步 await 锁而不丢序。`use futures::StreamExt;` 已在文件头。

- [ ] **Step 4: 跑测试绿**

```bash
cargo test -p clawx-api sse_persists_accumulated_content_not_placeholder
```

Expected: PASS（内容非空且不含 `[streamed response]`）。

- [ ] **Step 5: commit**

```bash
git add crates/clawx-api/src/routes/conversations.rs
git commit -m "fix(api/sse): persist accumulated assistant response instead of literal placeholder"
```

---

## Task 8: 前端流结束后不再重复 listMessages

**Files:**
- Modify: `apps/clawx-gui/src/pages/ChatPage.tsx`

Task 7 之后，后端已保证 `done` 事件前写入真实内容。`onDone` 直接 `listMessages(convId)` 就能拿到。当前代码已经这么做了，主要是保证 `streamingContent` 进完整消息流后不会闪烁。

- [ ] **Step 1: 失败测试（行为验证）**

在 `src/pages/__tests__/ChatPage.test.tsx` 追加：

```tsx
it("appends streamed text to messages on done", async () => {
  const api = await import("../../lib/api");
  (api.listMessages as any).mockReset();
  (api.listMessages as any)
    .mockResolvedValueOnce([])                                     // initial
    .mockResolvedValueOnce([
      { id: "m1", conversation_id: "c1", role: "user", content: "ping", created_at: "" },
      { id: "m2", conversation_id: "c1", role: "assistant", content: "pong-from-llm", created_at: "" },
    ]);                                                            // post-stream refresh
  (api.listConversations as any).mockResolvedValue([
    { id: "c1", agent_id: "a1", title: "", created_at: "", updated_at: "" },
  ]);

  let capturedOnDone: (() => void) | undefined;
  (api.sendMessageStream as any).mockImplementation(
    (_c: string, _msg: string, _onMsg: any, onDone: any) => {
      capturedOnDone = onDone;
      return new AbortController();
    },
  );

  render(
    <MemoryRouter initialEntries={["/?conv=c1&agent=a1"]}>
      <AgentProvider>
        <Routes><Route path="/" element={<ChatPage />} /></Routes>
      </AgentProvider>
    </MemoryRouter>,
  );

  // wait for initial load
  await waitFor(() => expect(screen.queryByText("加载中…")).toBeNull());

  // simulate send + stream complete
  const input = screen.getByPlaceholderText("输入任何问题...");
  await userEvent.type(input, "ping{enter}");
  capturedOnDone?.();

  await waitFor(() =>
    expect(screen.getByText("pong-from-llm")).toBeInTheDocument(),
  );
});
```

- [ ] **Step 2: 跑测试**

```bash
cd apps/clawx-gui && npx vitest run src/pages/__tests__/ChatPage.test.tsx
```

如果已 PASS，跳 Step 3—4；若 FAIL，按下方补逻辑。

- [ ] **Step 3: （若 fail）确保 onDone 里 `listMessages` 被 await 前先清 `streamingContent`**

ChatPage 的 `onDone` 已有 `setIsStreaming(false); setStreamingContent(""); listMessages(convId).then(...)`。如果测试因顺序失败，改用 async/await：

```tsx
onDone: async () => {
  setIsStreaming(false);
  setStreamingContent("");
  try {
    const msgs = await listMessages(convId!);
    setMessages(msgs);
  } catch (e) {
    console.error("refresh after stream:", e);
  }
},
```

- [ ] **Step 4: 测试绿**

```bash
cd apps/clawx-gui && npx vitest run src/pages/__tests__/ChatPage.test.tsx
```

Expected: 全部 PASS。

- [ ] **Step 5: commit**

```bash
git add apps/clawx-gui/src/pages/ChatPage.tsx \
  apps/clawx-gui/src/pages/__tests__/ChatPage.test.tsx
git commit -m "test(gui/chat): verify stream done path refreshes with final assistant message"
```

---

## Task 9: AgentSidebar 状态映射 + 测试

**Files:**
- Modify: `apps/clawx-gui/src/components/AgentSidebar.tsx`
- Create: `apps/clawx-gui/src/components/__tests__/AgentSidebar.test.tsx`

当前 `STATUS_DESC.working` 硬编码 "Running · 2 pending"。Agent 真实数据不带 pending 数；简化为：
- `idle` → `Idle`
- `working` → `Running`
- `error` → `Error`
- `offline` → `Offline`

- [ ] **Step 1: 失败测试**

```tsx
// src/components/__tests__/AgentSidebar.test.tsx
import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";
import AgentSidebar from "../AgentSidebar";
import { AgentProvider } from "../../lib/store";

vi.mock("../../lib/api", () => ({
  listAgents: vi.fn().mockResolvedValue([
    { id: "a1", name: "编程助手", role: "Developer", system_prompt: "",
      model_id: "m1", status: "idle", created_at: "", updated_at: "" },
    { id: "a2", name: "研究助手", role: "Researcher", system_prompt: "",
      model_id: "m1", status: "working", created_at: "", updated_at: "" },
  ]),
}));

describe("AgentSidebar", () => {
  it("renders plain status labels without fake counts", async () => {
    render(
      <MemoryRouter>
        <AgentProvider><AgentSidebar /></AgentProvider>
      </MemoryRouter>,
    );
    await waitFor(() => expect(screen.getByText("编程助手")).toBeInTheDocument());
    expect(screen.getByText("Idle")).toBeInTheDocument();
    expect(screen.getByText("Running")).toBeInTheDocument();
    expect(screen.queryByText("2 pending")).toBeNull();
  });
});
```

- [ ] **Step 2: fail**

```bash
cd apps/clawx-gui && npx vitest run src/components/__tests__/AgentSidebar.test.tsx
```

Expected: 找到 `Running · 2 pending`，断言 `"2 pending"` 为 null 失败。

- [ ] **Step 3: 修 `STATUS_DESC`**

`apps/clawx-gui/src/components/AgentSidebar.tsx` 第 11-16 行替换：

```tsx
const STATUS_DESC: Record<Agent["status"], string> = {
  working: "Running",
  idle:    "Idle",
  error:   "Error",
  offline: "Offline",
};
```

- [ ] **Step 4: 绿**

```bash
cd apps/clawx-gui && npx vitest run src/components/__tests__/AgentSidebar.test.tsx
```

- [ ] **Step 5: commit**

```bash
git add apps/clawx-gui/src/components/AgentSidebar.tsx \
  apps/clawx-gui/src/components/__tests__/AgentSidebar.test.tsx
git commit -m "fix(gui/sidebar): drop fabricated `2 pending` suffix in status badge"
```

---

## Task 10: Provider 编辑能力（补 api_key）

**Files:**
- Modify: `apps/clawx-gui/src/lib/api.ts` — 加 `updateModel`
- Modify: `apps/clawx-gui/src/components/ModelProviderCard.tsx` — 加"编辑"按钮
- Modify: `apps/clawx-gui/src/components/AddProviderModal.tsx` — 支持 `initial` prop，复用为编辑
- Modify: `apps/clawx-gui/src/pages/SettingsPage.tsx` — 打开编辑 modal

后端 PUT `/models/:id` 已支持 ProviderUpdate，前端只需调用。目前用户已有一条智谱行但 `parameters=null`，无 UI 补 api_key 的路径——必须补上，否则 `build_llm_router` 永远只注册 stub。

- [ ] **Step 1: 失败测试**

`src/lib/__tests__/api.test.ts` 末尾追加：

```ts
it("updateModel PUTs to /models/:id with partial payload", async () => {
  const fetchMock = vi.fn().mockResolvedValue(
    new Response(JSON.stringify({ id: "p1" }), {
      status: 200,
      headers: { "content-type": "application/json" },
    }),
  );
  vi.stubGlobal("fetch", fetchMock);
  const { updateModel } = await import("../api");
  await updateModel("p1", { parameters: { api_key: "k" } });
  expect(fetchMock).toHaveBeenCalledWith(
    "http://127.0.0.1:9090/models/p1",
    expect.objectContaining({ method: "PUT" }),
  );
  const call = fetchMock.mock.calls[0];
  expect(JSON.parse(call[1].body as string)).toEqual({
    parameters: { api_key: "k" },
  });
});
```

- [ ] **Step 2: fail**

```bash
cd apps/clawx-gui && npx vitest run src/lib/__tests__/api.test.ts
```

Expected: `updateModel is not a function`。

- [ ] **Step 3: 加 `updateModel`**

在 `apps/clawx-gui/src/lib/api.ts` `deleteModel` 后追加：

```ts
export function updateModel(
  id: string,
  data: Partial<Omit<ModelProvider, "id" | "created_at" | "updated_at">>,
): Promise<ModelProvider> {
  return fetchApi(`/models/${encodeURIComponent(id)}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}
```

- [ ] **Step 4: 绿**

```bash
cd apps/clawx-gui && npx vitest run src/lib/__tests__/api.test.ts
```

- [ ] **Step 5: 把 `AddProviderModal` 改造为"可编辑"**

在 `AddProviderModal.tsx` 顶部加入：

```tsx
import { createModel, updateModel } from "../lib/api";

interface Props {
  open: boolean;
  onClose: () => void;
  onSaved: (provider: ModelProvider) => void;
  initial?: ModelProvider;
}
```

`useEffect(... [open])` 里如果 `initial` 存在就用它回填字段；`handleSubmit` 分支：

```tsx
const saved = initial
  ? await updateModel(initial.id, {
      name: name.trim(),
      provider_type: type,
      base_url: baseUrl.trim(),
      model_name: modelName.trim(),
      parameters: apiKey.trim() ? { api_key: apiKey.trim() } : {},
      is_default: isDefault,
    })
  : await createModel({ ...same payload });
onSaved(saved);
```

卡片的"编辑"按钮（`ModelProviderCard.tsx`）：

```tsx
<Button size="sm" variant="ghost" onClick={() => onEdit?.(provider)} disabled={busy}>
  编辑
</Button>
```

`SettingsPage.tsx` 维护 `editing` state：

```tsx
const [editing, setEditing] = useState<ModelProvider | null>(null);
...
<ModelProviderCard
  key={p.id}
  provider={p}
  onEdit={(prov) => { setEditing(prov); setModalOpen(true); }}
  onDelete={handleDelete}
  busy={deletingId === p.id}
/>
...
<AddProviderModal
  open={modalOpen}
  initial={editing ?? undefined}
  onClose={() => { setModalOpen(false); setEditing(null); }}
  onSaved={(p) => {
    setProviders((prev) => {
      const exists = prev.some((x) => x.id === p.id);
      return exists ? prev.map((x) => (x.id === p.id ? p : x)) : [p, ...prev];
    });
    setEditing(null);
  }}
/>
```

同时把 modal 旧 prop 名 `onCreated` 改为 `onSaved`；SettingsPage 与 modal 保持一致。

- [ ] **Step 6: commit**

```bash
git add apps/clawx-gui/src/lib/api.ts \
  apps/clawx-gui/src/lib/__tests__/api.test.ts \
  apps/clawx-gui/src/components/AddProviderModal.tsx \
  apps/clawx-gui/src/components/ModelProviderCard.tsx \
  apps/clawx-gui/src/pages/SettingsPage.tsx
git commit -m "feat(gui/settings): edit existing providers (fill api_key without delete+recreate)"
```

---

## Task 11: 端到端冒烟验证（手动 + 命令）

**Files:** 无代码改动；验证所有任务组合可用。

- [ ] **Step 1: 后端/前端所有测试全绿**

```bash
cargo test --workspace
cd apps/clawx-gui && npm test
```

Expected: 全部 0 failures。

- [ ] **Step 2: 重启 service**

```bash
pgrep -f target/debug/clawx-service | xargs -I {} kill {} 2>/dev/null
cargo run -p clawx-service > /tmp/clawx-service.log 2>&1 &
```

- [ ] **Step 3: 同步 token**

```bash
echo "VITE_AUTH_TOKEN=$(cat ~/.clawx/run/control_token)" > apps/clawx-gui/.env.local
```

- [ ] **Step 4: 起 Tauri**

```bash
cd apps/clawx-gui && npm run tauri dev &
```

- [ ] **Step 5: UI 手动 walkthrough**

1. 首页左边栏不再显示 "Load failed"；至少一个 Agent 出现。
2. 设置 → 模型 Provider：点已有智谱条目的"编辑"，填入真实 API Key，保存。
3. 再次重启 service（让 router 加载新 key）。
4. Agents → 新建 Agent（模板"编程助手"，Provider 选智谱）。
5. 回首页点击这个 Agent → 欢迎页显示 Agent 名字；composer 底部显示 `glm-4.6`（或你填的模型名）。
6. 点一个建议卡 "为移动端设计一个技术方案"，自动创建对话 + 流式回复真实内容。
7. 关闭窗口重开，刷新还能看到刚才的消息（说明持久化）。

- [ ] **Step 6: 把验证结果记录到 PR/workflow 文档**

```bash
cat >> workflow.md <<'EOF'

## 2026-04-18 首页端到端对话验证
- Agent 侧栏加载正常
- 智谱 provider (model `glm-4.6`) 已生效
- 建议卡 → 对话 → 流式回复 → 持久化，链路闭环
EOF
git add workflow.md
git commit -m "docs(workflow): log e2e chat validation"
```

---

## Self-review

- **Spec coverage:**
  - 目标"能和 agent 对话" → Task 3/7/8 覆盖后端 SSE 修复与前端刷新；Task 2 解决连通性；Task 10 保证 Zhipu provider 能带 api_key。
  - 首页 UI（image #2）细节：sidebar Load-failed → Task 2；Welcome hero 个性化 → Task 4；Composer 模型显示 → Task 5/6；Agent status → Task 9；新建 Agent modal（前一轮已落地，不重做）。
- **Placeholder scan:** 所有步骤含具体代码或命令；仅 Task 11 的"UI walkthrough"是人工步骤（明确了清单），不属 TBD。
- **Type consistency:** `AddProviderModal` 的 prop 从 `onCreated` 统一改为 `onSaved`，Task 10 对此做了显式 rename；`updateModel` 签名和 `ModelProvider` 对齐。

---

# 执行切换

**Plan complete and saved to `docs/superpowers/plans/2026-04-18-home-page-end-to-end-chat.md`.**

两种执行方式：

1. **Subagent-Driven（推荐）** — 我为每个 task 派新的 subagent，task 间两阶段 review，快速迭代。
2. **Inline Execution** — 当前会话里按 `superpowers:executing-plans` 批执行，关键点停顿 review。

要哪种？
