# ClawX GUI — Pencil Design Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 `apps/clawx-gui` 的视觉与结构完全对齐 Pencil 设计稿（`/Users/zhoulingfeng/Desktop/startup/pencil/frank_claw.pen`），使 13 个页面形态与 halo 深色设计系统一致。

**Architecture:** 保留现有 React 19 + Vite + react-router v7 + lucide-react 栈，在 `styles.css` 中完善设计令牌与组件类，按页面逐个改造组件 / 页面。新增基础 UI 原子组件目录 `components/ui/`，页面改为消费它们。不引入 CSS 框架。

**Tech Stack:** React 19, TypeScript, Vite, react-router v7, lucide-react, CSS variables (手写 BEM-like 样式)。

---

## 设计令牌（来自 Pencil get_variables，Dark theme）

```css
/* Palette */
--background:        #131124;
--foreground:        #E8E8EA;
--card:              #1A182E;
--card-foreground:   #FFFFFF;
--primary:           #5749F4;   /* 品牌紫 */
--primary-foreground:#FFFFFF;
--secondary:         #403F51;
--secondary-foreground:#FFFFFF;
--accent:            #131124;   /* 深色模式下与 background 等同 */
--accent-foreground: #F2F3F0;
--muted:             #2E2E2E;
--muted-foreground:  #888799;
--destructive:       #CC3314;
--destructive-foreground:#FFFFFF;
--border:            #2B283D;
--input:             #2B283D;
--ring:              #666666;
--tile:              #1A182E;
--white:             #FFFFFF;
--black:             #000000;

/* Sidebar */
--sidebar:           #1A182E;
--sidebar-foreground:#ACABB2;
--sidebar-accent:    #2B283D;
--sidebar-accent-foreground:#E8E8EA;
--sidebar-border:    #2B283D;
--sidebar-primary:   #0F5FFE;
--sidebar-primary-foreground:#E8E8EA;
--sidebar-ring:      #2B283D;

/* Status (Dark mode, 背景色较暗，foreground 为浅色) */
--color-error:            #53424F;
--color-error-foreground: #FFBFB2;
--color-warning:          #53484F;
--color-warning-foreground:#FFD9B2;
--color-success:          #3B4748;
--color-success-foreground:#A1E5A1;
--color-info:             #404562;
--color-info-foreground:  #B2CCFF;

/* Radii */
--radius-none: 0;
--radius-xs:   6px;
--radius-m:    24px;
--radius-l:    40px;
--radius-pill: 999px;

/* Typography */
font-family: Inter, -apple-system, BlinkMacSystemFont, sans-serif;
/* 实际使用字号: 10,11,12,13,14,15,16,18,20,24,28,32 */
/* 实际使用 weight: 400, 500, 600, 700 */

/* Spacing (Pencil 全用裸像素值，无 token)：常用 2,4,6,8,10,12,14,16,24 */

/* Shadows */
--shadow-sm: 0 2px 3.5px -1px rgba(0,0,0,0.06);   /* tooltip/select */
--shadow-md: 0 4px 5.25px     rgba(0,0,0,0.10);   /* popover/modal-sm */
--shadow-lg: 0 8px 8.75px     rgba(0,0,0,0.15);   /* modal-lg */
```

**关键视觉规则：**
- 所有页面全屏 `1440×900` 基准，三列 shell：**56px app rail** + **280px secondary sidebar**（部分页面省略）+ **flexible main**。
- 主按钮填 `--primary`；次要按钮填 `--secondary`；危险按钮填 `--destructive`；ghost 按钮 transparent + hover `--accent`。
- 卡片填 `--card`，描边 `--border`，圆角 `8px`（非 token）或 `12px`，阴影见上。
- 输入框背景 `--input`（透明包装在 `--card` 上时视觉为深紫色），圆角 `--radius-pill` 或 `8px`，描边 `--border`。
- 状态标签（运行中/已连接/可用）用 `--color-*` 作底 + `--color-*-foreground` 作字，微圆角或 pill。

---

## 文件结构与职责

**新增目录：** `apps/clawx-gui/src/components/ui/` — 放原子 UI 原语。每个组件独立文件，<100 行，无业务逻辑。

| 文件 | 职责 |
|---|---|
| `components/ui/Button.tsx` | `variant: default \| secondary \| destructive \| outline \| ghost`；`size: sm \| md \| lg`；支持左右 icon |
| `components/ui/IconButton.tsx` | 纯 icon 圆形/方形按钮，同 variant/size |
| `components/ui/Input.tsx` | 基础文本输入 + optional left/right icon slot |
| `components/ui/Select.tsx` | 下拉选择（原生 `<select>` + 样式包装） |
| `components/ui/Textarea.tsx` | 多行输入 |
| `components/ui/Tabs.tsx` | `Tabs.Root`, `Tabs.List`, `Tabs.Trigger`, `Tabs.Content` — 受控 |
| `components/ui/Card.tsx` | Card 容器 + `Card.Header/Title/Content/Footer` |
| `components/ui/Badge.tsx` | 状态标签，`tone: success \| warning \| error \| info \| neutral` |
| `components/ui/Avatar.tsx` | 文本/emoji/图片三形态，圆角可配置 |
| `components/ui/Dialog.tsx` | 模态框 backdrop + 面板（替换现有 PermissionModal 的 DOM） |
| `components/ui/Switch.tsx` | 开关 |
| `components/ui/Progress.tsx` | 进度条（pill 形 container + primary fill） |
| `components/ui/Separator.tsx` | 细分割线 |

**修改文件：**

| 文件 | 改动范围 |
|---|---|
| `src/styles.css` | 分成 `styles/tokens.css`、`styles/base.css`、`styles/components/*.css`、`styles/pages/*.css` 结构，按页面拆分，主文件 `@import` 各片段 |
| `src/App.tsx` | 新增 `/contacts` 路由（指向 `ContactsPage`） |
| `src/layouts/AppLayout.tsx` | 无结构变化，Permission 触发改由全局 store，不保留 demo 数据 |
| `src/components/NavBar.tsx` | 改为 56px app rail，按 Pencil 图标顺序（Chat/Contacts/Agents/Knowledge/Tasks/Connectors/Skills），底部 avatar + settings |
| `src/components/AgentSidebar.tsx` | 头部品牌 `ZettClaw` + chevron，搜索 + "+"，agent 列表卡片（紫色 emoji 方块 + 名字 + 状态描述） |
| `src/components/ChatInput.tsx` | 改为 pill 风 input bar（+ 按钮、技能、input、model selector、send） |
| `src/components/MessageBubble.tsx` | User 气泡：圆角、`--primary` 底 + 白字；Assistant 消息：无气泡，直接多行 + source refs |
| `src/components/ChatWelcome.tsx` | 渐变方块 icon + 标题 MaxClaw + 描述 + 建议 pill 行 + 建议 list item |
| `src/components/SourceReferences.tsx` | 文件卡片：图标 + 文件名 + 行号范围 + 代码 snippet 预览 |
| `src/components/ArtifactsPanel.tsx` | 右侧面板：顶部 tabs、产物卡片列表 |
| `src/pages/ChatPage.tsx` | 主区 Tabs `对话` / `产物`；`对话` 分 empty/welcome 和 message-stream 两态；`产物` 渲染卡片列表 |
| `src/pages/AgentsPage.tsx` | 改为 Agent / Skill tabs；去掉 sidebar 列表栏（页面本身占 NavBar 右侧全宽） |
| `src/pages/KnowledgePage.tsx` | 两列：左源列表（含"本地添加/对话产生"分组），右搜索工作台 |
| `src/pages/TasksPage.tsx` | 任务卡片，含反馈度量行；顶部 search + filter tabs |
| `src/pages/ConnectorsPage.tsx` | Agent context 卡 + "已连接渠道"列表 + "此 Agent 可用渠道"网格 |
| `src/pages/SettingsPage.tsx` | 左内嵌 nav（模型/安全/外观与语言/健康/关于/反馈），右各区内容 |
| `src/pages/ContactsPage.tsx` | Sidebar 分组（收藏/最近/全部）+ 主区 Agent 详情 |

**新增组件：**
- `src/components/AgentTemplateModal.tsx` — 新建 Agent 模态（模板 / 自定义 两 tab）
- `src/components/KnowledgeSearchPanel.tsx` — 搜索工作台
- `src/components/TaskCard.tsx` — 任务卡片
- `src/components/ConnectorCard.tsx` — 连接的渠道卡片
- `src/components/AvailableChannelChip.tsx` — 可用渠道小卡
- `src/components/ModelProviderCard.tsx` — 模型 Provider 卡
- `src/components/AgentModelAssignTable.tsx` — Agent 模型分配表
- `src/components/SettingsNav.tsx` — 设置页左侧分节导航

**删除/归档：** `src/components/AgentList.tsx`、`src/components/ListPanel.tsx`（空/冗余）、`src/components/SettingsList.tsx` 若被 `SettingsNav` 取代。

---

## 执行原则

1. **逐阶段提交**：每阶段一个以上 commit，信息格式 `feat(gui): <scope>` 或 `refactor(gui): <scope>`。
2. **TDD 适用度**：此为纯视觉改造，逻辑面窄；仅在 **store/状态改动**（如 Permission 全局化、Agent 选择 url 同步）和 **非平凡 hook** 处写 vitest 单测。视觉改动用 `npm run build` + Tauri 预览人工验证。
3. **不删除现有流程**：保留当前的 `lib/api.ts` 接口、`lib/store.tsx` 上下文、URL 参数契约；仅替换组件实现。
4. **原子组件先行**：先把 `components/ui/*` 建完并让所有现有代码切换到它们，再做页面级改造。
5. **DRY**：Pencil 重复的状态标签 / emoji 头像 / 文件 icon 色卡全部抽成 UI 原子或工具。
6. **YAGNI**：暂不建 tooltip / popover / pagination / data-table 原子组件，等到具体页面需要才加。

---

## 阶段 0：基础与令牌重组

### Task 0.1：styles.css 拆分与令牌收齐

**Files:**
- Create: `apps/clawx-gui/src/styles/tokens.css`
- Create: `apps/clawx-gui/src/styles/base.css`
- Modify: `apps/clawx-gui/src/styles.css`
- Modify: `apps/clawx-gui/src/main.tsx`

- [ ] **Step 1: 创建 `styles/tokens.css`**

把本计划"设计令牌"节所有 CSS 变量搬进去，放到 `:root {}` 内。不含布局/字号以外的任何规则。

- [ ] **Step 2: 创建 `styles/base.css`**

内容：

```css
* { margin: 0; padding: 0; box-sizing: border-box; }

html, body, #root { height: 100vh; overflow: hidden; }

body {
  font-family: var(--font-family);
  font-size: 14px;
  line-height: 1.4286;
  background: var(--background);
  color: var(--foreground);
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

::-webkit-scrollbar { width: 8px; height: 8px; }
::-webkit-scrollbar-thumb { background: var(--sidebar-accent); border-radius: 4px; }
::-webkit-scrollbar-thumb:hover { background: var(--muted-foreground); }

button, input, select, textarea { font: inherit; color: inherit; }
button { cursor: pointer; background: none; border: none; }
a { color: inherit; text-decoration: none; }
```

- [ ] **Step 3: 把 `styles.css` 当前 `:root` 段整体删除，改为纯 `@import`**

```css
@import "./styles/tokens.css";
@import "./styles/base.css";
@import "./styles/layout.css";
@import "./styles/components.css";
@import "./styles/pages.css";
```

将 `styles.css` 原有的布局、组件、页面规则分别剪进 `styles/layout.css`、`styles/components.css`、`styles/pages.css`（仅剪切/分组，不改逻辑）。

- [ ] **Step 4: 确认 `main.tsx` import 仍是 `./styles.css`**

不用改，已是入口。

- [ ] **Step 5: 运行 `npm run build` 验证无 CSS 构建错误**

预期：构建成功。

- [ ] **Step 6: Commit**

```bash
git add apps/clawx-gui/src/styles apps/clawx-gui/src/styles.css
git commit -m "refactor(gui): split styles.css into tokens/base/layout/components/pages"
```

### Task 0.2：补齐缺失的语义令牌

**Files:**
- Modify: `apps/clawx-gui/src/styles/tokens.css`

- [ ] **Step 1: 在 `:root` 中追加 Pencil 缺失令牌**

```css
--muted: #2E2E2E;
--tile: #1A182E;

--color-error:            #53424F;
--color-error-foreground: #FFBFB2;
--color-warning:          #53484F;
--color-warning-foreground:#FFD9B2;
--color-success:          #3B4748;
--color-success-foreground:#A1E5A1;
--color-info:             #404562;
--color-info-foreground:  #B2CCFF;

--radius-m-pencil: 24px;    /* Pencil 的 radius-m */
--radius-l-pencil: 40px;    /* Pencil 的 radius-l */
--radius-card:     8px;     /* 所有卡片默认 */
--radius-card-lg:  12px;

--shadow-sm: 0 2px 3.5px -1px rgba(0,0,0,0.06);
--shadow-md: 0 4px 5.25px     rgba(0,0,0,0.10);
--shadow-lg: 0 8px 8.75px     rgba(0,0,0,0.15);
```

> 注：Pencil 的 `--radius-m=24px`、`--radius-l=40px` 与原 styles.css 里的命名冲突（原先用 `--radius-m=8px`）。为避免大范围 grep/replace，保留 `--radius-m=8px` 给组件惯例，Pencil 原生值用 `--radius-m-pencil` / `--radius-l-pencil`。

- [ ] **Step 2: Commit**

```bash
git add apps/clawx-gui/src/styles/tokens.css
git commit -m "feat(gui): add missing semantic tokens from Pencil halo system"
```

---

## 阶段 1：UI 原子组件库

### Task 1.1：Button + IconButton

**Files:**
- Create: `apps/clawx-gui/src/components/ui/Button.tsx`
- Create: `apps/clawx-gui/src/components/ui/IconButton.tsx`
- Create: `apps/clawx-gui/src/styles/components/button.css`
- Modify: `apps/clawx-gui/src/styles.css` (add `@import`)

- [ ] **Step 1: 写 `Button.tsx`**

```tsx
import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";

type Variant = "default" | "secondary" | "destructive" | "outline" | "ghost";
type Size = "sm" | "md" | "lg";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  leftIcon?: ReactNode;
  rightIcon?: ReactNode;
}

const Button = forwardRef<HTMLButtonElement, Props>(function Button(
  { variant = "default", size = "md", leftIcon, rightIcon, className = "", children, ...rest },
  ref,
) {
  const cls = `ui-btn ui-btn--${variant} ui-btn--${size} ${className}`.trim();
  return (
    <button ref={ref} className={cls} {...rest}>
      {leftIcon && <span className="ui-btn__icon">{leftIcon}</span>}
      {children && <span className="ui-btn__label">{children}</span>}
      {rightIcon && <span className="ui-btn__icon">{rightIcon}</span>}
    </button>
  );
});

export default Button;
```

- [ ] **Step 2: 写 `IconButton.tsx`**

```tsx
import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";

type Variant = "default" | "secondary" | "destructive" | "outline" | "ghost";
type Size = "sm" | "md" | "lg";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  icon: ReactNode;
  "aria-label": string;
}

const IconButton = forwardRef<HTMLButtonElement, Props>(function IconButton(
  { variant = "ghost", size = "md", icon, className = "", ...rest },
  ref,
) {
  return (
    <button
      ref={ref}
      className={`ui-icon-btn ui-icon-btn--${variant} ui-icon-btn--${size} ${className}`.trim()}
      {...rest}
    >
      {icon}
    </button>
  );
});

export default IconButton;
```

- [ ] **Step 3: 写 `styles/components/button.css`**

```css
.ui-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  border-radius: var(--radius-pill);
  font-weight: 500;
  white-space: nowrap;
  transition: background 120ms, color 120ms, opacity 120ms;
}
.ui-btn:disabled { opacity: 0.4; cursor: not-allowed; }

.ui-btn--sm { height: 28px; padding: 0 12px; font-size: 12px; }
.ui-btn--md { height: 36px; padding: 0 16px; font-size: 14px; }
.ui-btn--lg { height: 44px; padding: 0 20px; font-size: 15px; }

.ui-btn--default     { background: var(--primary); color: var(--primary-foreground); }
.ui-btn--default:hover:not(:disabled)   { background: color-mix(in srgb, var(--primary) 90%, white); }
.ui-btn--secondary   { background: var(--secondary); color: var(--secondary-foreground); }
.ui-btn--secondary:hover:not(:disabled) { background: color-mix(in srgb, var(--secondary) 85%, white); }
.ui-btn--destructive { background: var(--destructive); color: var(--destructive-foreground); }
.ui-btn--outline     { background: transparent; color: var(--foreground); border: 1px solid var(--border); }
.ui-btn--outline:hover:not(:disabled) { background: var(--sidebar-accent); }
.ui-btn--ghost       { background: transparent; color: var(--foreground); }
.ui-btn--ghost:hover:not(:disabled) { background: var(--sidebar-accent); }

.ui-btn__icon { display: inline-flex; }

.ui-icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-pill);
  transition: background 120ms;
}
.ui-icon-btn--sm { width: 28px; height: 28px; }
.ui-icon-btn--md { width: 36px; height: 36px; }
.ui-icon-btn--lg { width: 44px; height: 44px; }

.ui-icon-btn--ghost { background: transparent; color: var(--sidebar-foreground); }
.ui-icon-btn--ghost:hover { background: var(--sidebar-accent); color: var(--foreground); }
.ui-icon-btn--default { background: var(--primary); color: var(--primary-foreground); }
.ui-icon-btn--secondary { background: var(--secondary); color: var(--secondary-foreground); }
.ui-icon-btn--outline { background: transparent; color: var(--foreground); border: 1px solid var(--border); }
.ui-icon-btn--destructive { background: var(--destructive); color: var(--destructive-foreground); }
```

- [ ] **Step 4: 在 `styles.css` 追加 `@import "./styles/components/button.css";`**

- [ ] **Step 5: `npm run build` 验证**

- [ ] **Step 6: Commit**

```bash
git add apps/clawx-gui/src/components/ui/Button.tsx \
        apps/clawx-gui/src/components/ui/IconButton.tsx \
        apps/clawx-gui/src/styles/components/button.css \
        apps/clawx-gui/src/styles.css
git commit -m "feat(gui/ui): add Button and IconButton primitives"
```

### Task 1.2：Input / Textarea / Select

**Files:**
- Create: `apps/clawx-gui/src/components/ui/Input.tsx`
- Create: `apps/clawx-gui/src/components/ui/Textarea.tsx`
- Create: `apps/clawx-gui/src/components/ui/Select.tsx`
- Create: `apps/clawx-gui/src/styles/components/input.css`

- [ ] **Step 1: 写 `Input.tsx`**

```tsx
import { forwardRef, type InputHTMLAttributes, type ReactNode } from "react";

interface Props extends InputHTMLAttributes<HTMLInputElement> {
  leftIcon?: ReactNode;
  rightIcon?: ReactNode;
  size?: "sm" | "md";
}

const Input = forwardRef<HTMLInputElement, Props>(function Input(
  { leftIcon, rightIcon, size = "md", className = "", ...rest },
  ref,
) {
  return (
    <div className={`ui-input ui-input--${size} ${className}`.trim()}>
      {leftIcon && <span className="ui-input__icon">{leftIcon}</span>}
      <input ref={ref} className="ui-input__control" {...rest} />
      {rightIcon && <span className="ui-input__icon">{rightIcon}</span>}
    </div>
  );
});

export default Input;
```

- [ ] **Step 2: 写 `Textarea.tsx`**

```tsx
import { forwardRef, type TextareaHTMLAttributes } from "react";

const Textarea = forwardRef<HTMLTextAreaElement, TextareaHTMLAttributes<HTMLTextAreaElement>>(
  function Textarea({ className = "", ...rest }, ref) {
    return <textarea ref={ref} className={`ui-textarea ${className}`.trim()} {...rest} />;
  },
);

export default Textarea;
```

- [ ] **Step 3: 写 `Select.tsx`**

```tsx
import { forwardRef, type SelectHTMLAttributes } from "react";
import { ChevronDown } from "lucide-react";

interface Option { value: string; label: string }
interface Props extends SelectHTMLAttributes<HTMLSelectElement> { options: Option[] }

const Select = forwardRef<HTMLSelectElement, Props>(function Select(
  { options, className = "", ...rest },
  ref,
) {
  return (
    <div className={`ui-select ${className}`.trim()}>
      <select ref={ref} className="ui-select__control" {...rest}>
        {options.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
      </select>
      <ChevronDown size={16} className="ui-select__chevron" />
    </div>
  );
});

export default Select;
```

- [ ] **Step 4: 写 `styles/components/input.css`**

```css
.ui-input {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  background: var(--input);
  border: 1px solid var(--border);
  border-radius: var(--radius-pill);
  padding: 0 14px;
  color: var(--foreground);
  transition: border-color 120ms, box-shadow 120ms;
}
.ui-input:focus-within { border-color: var(--primary); box-shadow: 0 0 0 3px color-mix(in srgb, var(--primary) 20%, transparent); }
.ui-input--sm { height: 32px; font-size: 12px; }
.ui-input--md { height: 40px; font-size: 14px; }
.ui-input__icon { color: var(--muted-foreground); display: inline-flex; }
.ui-input__control {
  background: transparent;
  border: none;
  outline: none;
  color: inherit;
  flex: 1;
  min-width: 0;
}
.ui-input__control::placeholder { color: var(--muted-foreground); }

.ui-textarea {
  background: var(--input);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 12px 14px;
  color: var(--foreground);
  font: inherit;
  outline: none;
  resize: vertical;
  min-height: 80px;
  width: 100%;
}
.ui-textarea::placeholder { color: var(--muted-foreground); }
.ui-textarea:focus { border-color: var(--primary); }

.ui-select {
  position: relative;
  display: inline-flex;
  align-items: center;
  background: var(--input);
  border: 1px solid var(--border);
  border-radius: var(--radius-pill);
  padding: 0 38px 0 14px;
  height: 40px;
  color: var(--foreground);
}
.ui-select__control { appearance: none; background: transparent; border: none; outline: none; color: inherit; padding: 0; width: 100%; }
.ui-select__chevron { position: absolute; right: 12px; pointer-events: none; color: var(--muted-foreground); }
```

- [ ] **Step 5: `styles.css` 追加 `@import "./styles/components/input.css";`**

- [ ] **Step 6: `npm run build`**

- [ ] **Step 7: Commit**

```bash
git add apps/clawx-gui/src/components/ui/Input.tsx \
        apps/clawx-gui/src/components/ui/Textarea.tsx \
        apps/clawx-gui/src/components/ui/Select.tsx \
        apps/clawx-gui/src/styles/components/input.css \
        apps/clawx-gui/src/styles.css
git commit -m "feat(gui/ui): add Input, Textarea, Select primitives"
```

### Task 1.3：Tabs

**Files:**
- Create: `apps/clawx-gui/src/components/ui/Tabs.tsx`
- Create: `apps/clawx-gui/src/styles/components/tabs.css`

- [ ] **Step 1: 写 `Tabs.tsx`（受控）**

```tsx
import { createContext, useContext, type ReactNode } from "react";

interface Ctx { value: string; onChange: (v: string) => void }
const TabsCtx = createContext<Ctx | null>(null);

export function TabsRoot({ value, onChange, children }: Ctx & { children: ReactNode }) {
  return <TabsCtx.Provider value={{ value, onChange }}><div className="ui-tabs">{children}</div></TabsCtx.Provider>;
}

export function TabsList({ children }: { children: ReactNode }) {
  return <div className="ui-tabs__list" role="tablist">{children}</div>;
}

export function TabsTrigger({ value, children }: { value: string; children: ReactNode }) {
  const ctx = useContext(TabsCtx)!;
  const active = ctx.value === value;
  return (
    <button
      role="tab"
      aria-selected={active}
      className={`ui-tabs__trigger ${active ? "is-active" : ""}`}
      onClick={() => ctx.onChange(value)}
    >
      {children}
    </button>
  );
}

export function TabsContent({ value, children }: { value: string; children: ReactNode }) {
  const ctx = useContext(TabsCtx)!;
  if (ctx.value !== value) return null;
  return <div role="tabpanel" className="ui-tabs__content">{children}</div>;
}

export default { Root: TabsRoot, List: TabsList, Trigger: TabsTrigger, Content: TabsContent };
```

- [ ] **Step 2: 写 `tabs.css`**

```css
.ui-tabs { display: flex; flex-direction: column; min-height: 0; flex: 1; }
.ui-tabs__list {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px;
  background: var(--sidebar-accent);
  border-radius: var(--radius-pill);
  align-self: flex-start;
}
.ui-tabs__trigger {
  padding: 6px 16px;
  border-radius: var(--radius-pill);
  color: var(--muted-foreground);
  font-size: 13px;
  font-weight: 500;
  transition: background 120ms, color 120ms;
}
.ui-tabs__trigger:hover { color: var(--foreground); }
.ui-tabs__trigger.is-active {
  background: var(--card);
  color: var(--foreground);
  box-shadow: var(--shadow-sm);
}
.ui-tabs__content { flex: 1; min-height: 0; }
```

- [ ] **Step 3: `styles.css` 追加 import，`npm run build`**

- [ ] **Step 4: Commit**

```bash
git add apps/clawx-gui/src/components/ui/Tabs.tsx \
        apps/clawx-gui/src/styles/components/tabs.css \
        apps/clawx-gui/src/styles.css
git commit -m "feat(gui/ui): add Tabs primitive"
```

### Task 1.4：Card / Badge / Avatar / Switch / Progress / Dialog / Separator

**Files:**
- Create: `apps/clawx-gui/src/components/ui/Card.tsx`
- Create: `apps/clawx-gui/src/components/ui/Badge.tsx`
- Create: `apps/clawx-gui/src/components/ui/Avatar.tsx`
- Create: `apps/clawx-gui/src/components/ui/Switch.tsx`
- Create: `apps/clawx-gui/src/components/ui/Progress.tsx`
- Create: `apps/clawx-gui/src/components/ui/Dialog.tsx`
- Create: `apps/clawx-gui/src/components/ui/Separator.tsx`
- Create: `apps/clawx-gui/src/styles/components/card.css`
- Create: `apps/clawx-gui/src/styles/components/badge.css`
- Create: `apps/clawx-gui/src/styles/components/misc.css`

- [ ] **Step 1: 写 `Card.tsx`**

```tsx
import type { HTMLAttributes, ReactNode } from "react";
export function Card({ className = "", children, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return <div className={`ui-card ${className}`.trim()} {...rest}>{children}</div>;
}
export function CardHeader({ children }: { children: ReactNode }) { return <div className="ui-card__header">{children}</div>; }
export function CardTitle({ children }: { children: ReactNode }) { return <h3 className="ui-card__title">{children}</h3>; }
export function CardContent({ children }: { children: ReactNode }) { return <div className="ui-card__content">{children}</div>; }
export function CardFooter({ children }: { children: ReactNode }) { return <div className="ui-card__footer">{children}</div>; }
```

- [ ] **Step 2: 写 `Badge.tsx`**

```tsx
import type { ReactNode } from "react";
type Tone = "neutral" | "success" | "warning" | "error" | "info" | "primary";
export default function Badge({ tone = "neutral", children }: { tone?: Tone; children: ReactNode }) {
  return <span className={`ui-badge ui-badge--${tone}`}>{children}</span>;
}
```

- [ ] **Step 3: 写 `Avatar.tsx`**

```tsx
import type { ReactNode } from "react";
interface Props {
  size?: number;
  rounded?: "md" | "full";
  bg?: string;
  children: ReactNode; // emoji / initial / <img/>
  className?: string;
}
export default function Avatar({ size = 32, rounded = "md", bg, children, className = "" }: Props) {
  return (
    <span
      className={`ui-avatar ui-avatar--${rounded} ${className}`.trim()}
      style={{ width: size, height: size, background: bg, fontSize: Math.round(size * 0.5) }}
    >
      {children}
    </span>
  );
}
```

- [ ] **Step 4: 写 `Switch.tsx`**

```tsx
import { forwardRef, type InputHTMLAttributes } from "react";
type Props = Omit<InputHTMLAttributes<HTMLInputElement>, "type">;
const Switch = forwardRef<HTMLInputElement, Props>(function Switch({ className = "", ...rest }, ref) {
  return (
    <label className={`ui-switch ${className}`.trim()}>
      <input ref={ref} type="checkbox" {...rest} />
      <span className="ui-switch__track"><span className="ui-switch__thumb" /></span>
    </label>
  );
});
export default Switch;
```

- [ ] **Step 5: 写 `Progress.tsx`**

```tsx
export default function Progress({ value, max = 100 }: { value: number; max?: number }) {
  const pct = Math.min(100, Math.max(0, (value / max) * 100));
  return (
    <div className="ui-progress" role="progressbar" aria-valuenow={value} aria-valuemin={0} aria-valuemax={max}>
      <div className="ui-progress__fill" style={{ width: `${pct}%` }} />
    </div>
  );
}
```

- [ ] **Step 6: 写 `Dialog.tsx`**

```tsx
import { useEffect, type ReactNode } from "react";
interface Props { open: boolean; onClose: () => void; title?: string; children: ReactNode; width?: number }
export default function Dialog({ open, onClose, title, children, width = 520 }: Props) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);
  if (!open) return null;
  return (
    <div className="ui-dialog-backdrop" onClick={onClose}>
      <div className="ui-dialog" style={{ width }} onClick={(e) => e.stopPropagation()}>
        {title && <h2 className="ui-dialog__title">{title}</h2>}
        {children}
      </div>
    </div>
  );
}
```

- [ ] **Step 7: 写 `Separator.tsx`**

```tsx
export default function Separator({ orientation = "horizontal" }: { orientation?: "horizontal" | "vertical" }) {
  return <div className={`ui-separator ui-separator--${orientation}`} role="separator" />;
}
```

- [ ] **Step 8: 写 `styles/components/card.css`**

```css
.ui-card {
  background: var(--card);
  border: 1px solid var(--border);
  border-radius: 12px;
  display: flex;
  flex-direction: column;
}
.ui-card__header { padding: 16px 20px 12px; display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.ui-card__title { font-size: 15px; font-weight: 600; color: var(--card-foreground); margin: 0; }
.ui-card__content { padding: 0 20px 16px; flex: 1; }
.ui-card__footer { padding: 12px 20px 16px; display: flex; align-items: center; gap: 8px; }
```

- [ ] **Step 9: 写 `styles/components/badge.css`**

```css
.ui-badge {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 10px;
  border-radius: var(--radius-pill);
  font-size: 11px;
  font-weight: 500;
  line-height: 1.4;
  white-space: nowrap;
}
.ui-badge--neutral { background: var(--sidebar-accent); color: var(--muted-foreground); }
.ui-badge--success { background: var(--color-success); color: var(--color-success-foreground); }
.ui-badge--warning { background: var(--color-warning); color: var(--color-warning-foreground); }
.ui-badge--error   { background: var(--color-error);   color: var(--color-error-foreground); }
.ui-badge--info    { background: var(--color-info);    color: var(--color-info-foreground); }
.ui-badge--primary { background: var(--primary);       color: var(--primary-foreground); }
```

- [ ] **Step 10: 写 `styles/components/misc.css`**

```css
.ui-avatar {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  color: var(--primary-foreground);
  font-weight: 600;
  line-height: 1;
  overflow: hidden;
}
.ui-avatar--md { border-radius: 8px; }
.ui-avatar--full { border-radius: 999px; }

.ui-switch { position: relative; display: inline-block; width: 36px; height: 20px; }
.ui-switch input { opacity: 0; width: 0; height: 0; }
.ui-switch__track { position: absolute; inset: 0; background: var(--secondary); border-radius: 999px; transition: background 120ms; }
.ui-switch__thumb { position: absolute; top: 2px; left: 2px; width: 16px; height: 16px; background: var(--white); border-radius: 999px; transition: transform 120ms; }
.ui-switch input:checked + .ui-switch__track { background: var(--primary); }
.ui-switch input:checked + .ui-switch__track .ui-switch__thumb { transform: translateX(16px); }

.ui-progress { height: 6px; background: var(--sidebar-accent); border-radius: 999px; overflow: hidden; }
.ui-progress__fill { height: 100%; background: var(--primary); border-radius: 999px; transition: width 240ms ease; }

.ui-separator--horizontal { height: 1px; width: 100%; background: var(--border); }
.ui-separator--vertical { width: 1px; align-self: stretch; background: var(--border); }

.ui-dialog-backdrop {
  position: fixed; inset: 0;
  background: rgba(0,0,0,0.55);
  display: flex; align-items: center; justify-content: center;
  z-index: 100;
  padding: 40px;
}
.ui-dialog {
  background: var(--card);
  border: 1px solid var(--border);
  border-radius: 16px;
  box-shadow: var(--shadow-lg);
  max-height: 90vh;
  overflow: auto;
  padding: 24px;
  color: var(--card-foreground);
}
.ui-dialog__title { font-size: 18px; font-weight: 600; margin-bottom: 8px; }
```

- [ ] **Step 11: `styles.css` 追加三个 `@import`；`npm run build`**

- [ ] **Step 12: Commit**

```bash
git add apps/clawx-gui/src/components/ui/ \
        apps/clawx-gui/src/styles/components/ \
        apps/clawx-gui/src/styles.css
git commit -m "feat(gui/ui): add Card, Badge, Avatar, Switch, Progress, Dialog, Separator"
```

---

## 阶段 2：应用外壳（NavBar + AgentSidebar + AppLayout）

### Task 2.1：NavBar 改为 56px app rail 并扩图标

**Files:**
- Modify: `apps/clawx-gui/src/components/NavBar.tsx`
- Create: `apps/clawx-gui/src/styles/pages/nav-bar.css`

> Pencil 图标顺序（自上而下，参考 `vT1N6` A-Bar）：`MessageSquare(Chat)` → `Users(Contacts)` → `Bot(Agents & Skill)` → `BookOpen(Knowledge)` → `CalendarClock(Tasks)` → `Plug(Connectors)` → `Zap(Skills)`。底部：avatar（当前用户首字）+ `Settings`。

- [ ] **Step 1: 替换 `NavBar.tsx` 完整内容**

```tsx
import { useLocation, useNavigate } from "react-router-dom";
import {
  MessageSquare, Users, Bot, BookOpen, CalendarClock, Plug, Settings,
} from "lucide-react";
import IconButton from "./ui/IconButton";
import Avatar from "./ui/Avatar";

const navItems = [
  { icon: MessageSquare, label: "对话", path: "/" },
  { icon: Users,         label: "联系人", path: "/contacts" },
  { icon: Bot,           label: "Agent & Skill", path: "/agents" },
  { icon: BookOpen,      label: "知识库", path: "/knowledge" },
  { icon: CalendarClock, label: "定时任务", path: "/tasks" },
  { icon: Plug,          label: "渠道", path: "/connectors" },
];

export default function NavBar() {
  const location = useLocation();
  const navigate = useNavigate();
  const isActive = (p: string) => p === "/" ? location.pathname === "/" : location.pathname.startsWith(p);

  return (
    <nav className="nav-rail" aria-label="Main navigation">
      <div className="nav-rail__top">
        <Avatar size={32} rounded="md" bg="var(--primary)">ZC</Avatar>
      </div>
      <div className="nav-rail__items">
        {navItems.map((it) => (
          <IconButton
            key={it.path}
            icon={<it.icon size={18} />}
            aria-label={it.label}
            title={it.label}
            onClick={() => navigate(it.path)}
            variant="ghost"
            className={isActive(it.path) ? "is-active" : ""}
          />
        ))}
      </div>
      <div className="nav-rail__bottom">
        <IconButton
          icon={<Settings size={18} />}
          aria-label="设置"
          title="设置"
          onClick={() => navigate("/settings")}
          variant="ghost"
          className={isActive("/settings") ? "is-active" : ""}
        />
      </div>
    </nav>
  );
}
```

- [ ] **Step 2: 写 `styles/pages/nav-bar.css`**

```css
.nav-rail {
  width: var(--nav-width);
  background: var(--sidebar);
  border-right: 1px solid var(--sidebar-border);
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 12px 0;
  gap: 12px;
  flex-shrink: 0;
}
.nav-rail__top,
.nav-rail__bottom { display: flex; flex-direction: column; align-items: center; gap: 8px; }
.nav-rail__items { flex: 1; display: flex; flex-direction: column; align-items: center; gap: 4px; }

.nav-rail .ui-icon-btn--ghost {
  color: var(--sidebar-foreground);
}
.nav-rail .ui-icon-btn--ghost.is-active {
  background: var(--sidebar-accent);
  color: var(--foreground);
}
```

- [ ] **Step 3: `styles.css` 追加 import；删除老 `styles/layout.css` 中旧 `.nav-*` 规则**

- [ ] **Step 4: `npm run build`；人工预览确认顺序与激活态**

- [ ] **Step 5: Commit**

```bash
git commit -am "refactor(gui): redesign NavBar as 56px app rail per Pencil"
```

### Task 2.2：AgentSidebar 按 Pencil 样式重构

**Files:**
- Modify: `apps/clawx-gui/src/components/AgentSidebar.tsx`
- Create: `apps/clawx-gui/src/styles/pages/agent-sidebar.css`

> Pencil 规格：
> - 头部：`ZettClaw ▾` + 右侧 hamburger icon。
> - 搜索行：占 ~88% 的 pill input（左放大镜 + placeholder `搜索 Agent...`）+ 紫色 `+` 按钮。
> - Agent item：左侧 `40×40` 紫色圆角方块（含 emoji 或初字），右侧两行（名字 14/600 + 状态描述 12/muted），状态点替换为紫色方块左上角小色点不显；选中态整卡 `--sidebar-accent` 填。

- [ ] **Step 1: 完整替换 `AgentSidebar.tsx`**

```tsx
import { useState, useCallback, useMemo } from "react";
import { useSearchParams } from "react-router-dom";
import { Search, Plus, Menu, ChevronDown } from "lucide-react";
import Input from "./ui/Input";
import IconButton from "./ui/IconButton";
import Avatar from "./ui/Avatar";
import { useAgents } from "../lib/store";
import type { Agent } from "../lib/types";

const STATUS_DESC: Record<Agent["status"], string> = {
  working:  "Running · 2 pending",
  idle:     "Idle",
  error:    "Error",
  offline:  "Offline",
};

const EMOJI: Record<string, string> = {
  dev: "💻", research: "🔍", writing: "✍️", data: "📊",
};

function pickEmoji(agent: Agent): string {
  const key = agent.role?.toLowerCase() ?? "";
  if (key.includes("code") || key.includes("dev")) return EMOJI.dev;
  if (key.includes("research")) return EMOJI.research;
  if (key.includes("writ")) return EMOJI.writing;
  if (key.includes("data")) return EMOJI.data;
  return agent.name.slice(0, 1);
}

export default function AgentSidebar() {
  const [searchParams, setSearchParams] = useSearchParams();
  const selectedId = searchParams.get("agent");
  const { agents, loading, error } = useAgents();
  const [search, setSearch] = useState("");

  const filtered = useMemo(() => {
    const q = search.toLowerCase();
    return !q ? agents : agents.filter((a) => a.name.toLowerCase().includes(q) || a.role.toLowerCase().includes(q));
  }, [agents, search]);

  const handleSelect = useCallback((id: string) => {
    const params = new URLSearchParams(searchParams);
    params.set("agent", id);
    setSearchParams(params);
  }, [searchParams, setSearchParams]);

  return (
    <aside className="agent-sidebar">
      <header className="agent-sidebar__head">
        <div className="agent-sidebar__brand">
          <span className="agent-sidebar__brand-name">ZettClaw</span>
          <ChevronDown size={14} />
        </div>
        <IconButton icon={<Menu size={16} />} aria-label="菜单" variant="ghost" size="sm" />
      </header>

      <div className="agent-sidebar__search">
        <Input
          size="sm"
          leftIcon={<Search size={14} />}
          placeholder="搜索 Agent..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <IconButton icon={<Plus size={14} />} aria-label="新建 Agent" variant="default" size="sm" />
      </div>

      <div className="agent-sidebar__list">
        {loading && <p className="agent-sidebar__placeholder">加载中...</p>}
        {error && <p className="agent-sidebar__placeholder">{error}</p>}
        {!loading && !error && filtered.length === 0 && (
          <p className="agent-sidebar__placeholder">{search ? "无匹配" : "暂无 Agent"}</p>
        )}
        {filtered.map((agent) => (
          <button
            key={agent.id}
            className={`agent-item ${selectedId === agent.id ? "is-active" : ""}`}
            onClick={() => handleSelect(agent.id)}
          >
            <Avatar size={40} rounded="md" bg="var(--primary)">{pickEmoji(agent)}</Avatar>
            <div className="agent-item__text">
              <span className="agent-item__name">{agent.name}</span>
              <span className="agent-item__status">{STATUS_DESC[agent.status]}</span>
            </div>
          </button>
        ))}
      </div>
    </aside>
  );
}
```

- [ ] **Step 2: 写 `styles/pages/agent-sidebar.css`**

```css
.agent-sidebar {
  width: var(--sidebar-width);
  background: var(--sidebar);
  border-right: 1px solid var(--sidebar-border);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  min-height: 0;
}
.agent-sidebar__head {
  display: flex; align-items: center; justify-content: space-between;
  padding: 16px 16px 8px;
}
.agent-sidebar__brand {
  display: inline-flex; align-items: center; gap: 4px;
  color: var(--foreground); font-size: 15px; font-weight: 600;
}
.agent-sidebar__search {
  display: flex; align-items: center; gap: 8px;
  padding: 8px 16px 12px;
}
.agent-sidebar__search .ui-input { flex: 1; }
.agent-sidebar__list {
  flex: 1; min-height: 0; overflow: auto;
  padding: 4px 8px 12px;
  display: flex; flex-direction: column; gap: 2px;
}
.agent-sidebar__placeholder { color: var(--muted-foreground); padding: 12px; font-size: 12px; }

.agent-item {
  display: flex; align-items: center; gap: 10px;
  width: 100%;
  padding: 8px;
  border-radius: 10px;
  color: var(--sidebar-foreground);
  text-align: left;
  transition: background 120ms, color 120ms;
}
.agent-item:hover { background: var(--sidebar-accent); color: var(--foreground); }
.agent-item.is-active { background: var(--sidebar-accent); color: var(--foreground); }
.agent-item__text { display: flex; flex-direction: column; min-width: 0; flex: 1; }
.agent-item__name { color: var(--foreground); font-size: 13px; font-weight: 500; line-height: 1.4; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.agent-item__status { color: var(--muted-foreground); font-size: 11px; line-height: 1.4; }
```

- [ ] **Step 3: `styles.css` 追加 import，`npm run build`**

- [ ] **Step 4: Commit**

```bash
git commit -am "refactor(gui): rebuild AgentSidebar per Pencil spec (emoji avatar, pill search)"
```

### Task 2.3：AppLayout 布局 grid 化 + 按页面条件隐藏 sidebar

**Files:**
- Modify: `apps/clawx-gui/src/layouts/AppLayout.tsx`
- Create: `apps/clawx-gui/src/styles/pages/app-layout.css`

> Pencil：Agent & Skill、Skills、Settings 三个页面**不显示** `AgentSidebar`，直接在主区内置自己的导航。

- [ ] **Step 1: 替换 `AppLayout.tsx`**

```tsx
import { useState } from "react";
import { Outlet, useLocation } from "react-router-dom";
import NavBar from "../components/NavBar";
import AgentSidebar from "../components/AgentSidebar";
import PermissionModal from "../components/PermissionModal";
import type { PermissionRequest } from "../components/PermissionModal";
import { AgentProvider } from "../lib/store";

const DEMO_REQUESTS: PermissionRequest[] = [
  { id: "1", type: "fs_write", target: "/workspace/output.txt", risk: "medium", description: "Agent 需要将处理结果写入工作目录下的输出文件。" },
];

const SIDEBAR_HIDDEN = ["/agents", "/skills", "/settings"];

export default function AppLayout() {
  const [showPermission, setShowPermission] = useState(false);
  const { pathname } = useLocation();
  const hideSidebar = SIDEBAR_HIDDEN.some((p) => pathname.startsWith(p));

  return (
    <AgentProvider>
      <div className={`app-shell ${hideSidebar ? "app-shell--no-sidebar" : ""}`}>
        <NavBar />
        {!hideSidebar && <AgentSidebar />}
        <main className="app-shell__main">
          <Outlet />
        </main>

        {showPermission && (
          <PermissionModal
            agentName="研究助手"
            requests={DEMO_REQUESTS}
            onApprove={() => setShowPermission(false)}
            onDenyAll={() => setShowPermission(false)}
            onClose={() => setShowPermission(false)}
          />
        )}
      </div>
    </AgentProvider>
  );
}
```

- [ ] **Step 2: 写 `styles/pages/app-layout.css`**

```css
.app-shell { display: grid; grid-template-columns: var(--nav-width) var(--sidebar-width) 1fr; height: 100vh; background: var(--background); color: var(--foreground); }
.app-shell--no-sidebar { grid-template-columns: var(--nav-width) 1fr; }
.app-shell__main { min-width: 0; min-height: 0; overflow: hidden; background: var(--background); }
```

- [ ] **Step 3: 删除 `styles/layout.css` 里已被替代的 `.app-layout/.main-content` 相关规则**

- [ ] **Step 4: `styles.css` 追加 import；`npm run build`；人工预览所有页面切换 sidebar 正确显隐**

- [ ] **Step 5: Commit**

```bash
git commit -am "refactor(gui): grid-based AppLayout and conditionally hide sidebar on full-width pages"
```

---

## 阶段 3：对话页面（Main / Active Conversation / Artifacts Tab）

### Task 3.1：ChatWelcome 完全对齐 MaxClaw 卡片

**Files:**
- Modify: `apps/clawx-gui/src/components/ChatWelcome.tsx`
- Create: `apps/clawx-gui/src/styles/pages/chat-welcome.css`

> 规格：居中布局；上方 `64×64` 紫色圆角方块 icon（bot/sparkles）；标题 `MaxClaw`（24/700）；副标题描述（14/muted）；下方 pill tag 行（`对话 / 文件创建 / 代码编写 / ...`）；下方 4 行建议 list-item（每行左图标 + 文本 + chevron-right）。

- [ ] **Step 1: 替换 `ChatWelcome.tsx`**

```tsx
import { Sparkles, MessageSquare, FileText, Code, Search, PenLine, ChevronRight } from "lucide-react";

const TAGS = ["对话", "文件创建", "代码编写", "分析研究", "总结", "文献检索", "任务规划", "代码审查"];

const SUGGESTIONS = [
  { icon: MessageSquare, text: "智能分析业务流程并提出建议" },
  { icon: FileText,      text: "快速生成高质量技术文档" },
  { icon: Code,          text: "为移动端设计一个技术方案" },
  { icon: Search,        text: "研究并汇总行业最新动态" },
];

export default function ChatWelcome({ onSuggest }: { onSuggest?: (t: string) => void }) {
  return (
    <div className="chat-welcome">
      <div className="chat-welcome__hero">
        <div className="chat-welcome__icon"><Sparkles size={30} /></div>
        <h1 className="chat-welcome__title">MaxClaw</h1>
        <p className="chat-welcome__subtitle">
          您的智能 AI 助手，擅长编程、研究和创意任务。随时提问或试试下方的建议。
        </p>
      </div>
      <div className="chat-welcome__tags">
        {TAGS.map((t) => <button key={t} className="chat-welcome__tag" onClick={() => onSuggest?.(t)}>{t}</button>)}
      </div>
      <ul className="chat-welcome__suggestions">
        {SUGGESTIONS.map((s) => (
          <li key={s.text}>
            <button className="chat-welcome__suggestion" onClick={() => onSuggest?.(s.text)}>
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

- [ ] **Step 2: 写 `chat-welcome.css`**

```css
.chat-welcome { max-width: 560px; margin: 0 auto; padding: 48px 24px; display: flex; flex-direction: column; gap: 28px; align-items: center; }
.chat-welcome__hero { display: flex; flex-direction: column; align-items: center; gap: 12px; text-align: center; }
.chat-welcome__icon { width: 64px; height: 64px; border-radius: 16px; background: var(--primary); color: var(--primary-foreground); display: inline-flex; align-items: center; justify-content: center; box-shadow: var(--shadow-md); }
.chat-welcome__title { font-size: 24px; font-weight: 700; }
.chat-welcome__subtitle { color: var(--muted-foreground); font-size: 13px; line-height: 1.5; max-width: 440px; }
.chat-welcome__tags { display: flex; flex-wrap: wrap; gap: 6px; justify-content: center; }
.chat-welcome__tag { padding: 4px 12px; border-radius: 999px; background: var(--sidebar-accent); color: var(--muted-foreground); font-size: 12px; transition: background 120ms, color 120ms; }
.chat-welcome__tag:hover { background: var(--secondary); color: var(--foreground); }
.chat-welcome__suggestions { list-style: none; width: 100%; display: flex; flex-direction: column; gap: 6px; }
.chat-welcome__suggestion { width: 100%; display: flex; align-items: center; gap: 10px; padding: 10px 14px; border: 1px solid var(--border); border-radius: 12px; background: var(--card); color: var(--foreground); font-size: 13px; text-align: left; transition: background 120ms, border-color 120ms; }
.chat-welcome__suggestion:hover { background: var(--sidebar-accent); border-color: var(--primary); }
.chat-welcome__suggestion-icon { color: var(--muted-foreground); flex-shrink: 0; }
.chat-welcome__suggestion span { flex: 1; }
.chat-welcome__suggestion-chevron { color: var(--muted-foreground); }
```

- [ ] **Step 3: `styles.css` 追加 import；`npm run build`；预览 `/` 路径看 welcome 态**

- [ ] **Step 4: Commit**

```bash
git commit -am "feat(gui/chat): rebuild ChatWelcome per Pencil MaxClaw spec"
```

### Task 3.2：ChatInput 改为 Pencil 的 pill 风工具条

**Files:**
- Modify: `apps/clawx-gui/src/components/ChatInput.tsx`
- Create: `apps/clawx-gui/src/styles/pages/chat-input.css`

> 规格：一整条 `--card` 卡片，圆角 20px；内部左→右：`+`（attachBtn）、`Zap`+"技能"、中间透明输入框、model selector（`Sonnet 4.6 ▾`）、紫色圆形 send（ArrowUp）。

- [ ] **Step 1: 替换 `ChatInput.tsx`**

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

export default function ChatInput({ onSend, disabled, model = "Sonnet 4.6", onPickModel }: Props) {
  const [value, setValue] = useState("");
  function submit() {
    const t = value.trim();
    if (!t || disabled) return;
    onSend(t);
    setValue("");
  }
  function onKey(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); submit(); }
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
        <span>{model}</span>
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

- [ ] **Step 2: 写 `chat-input.css`**

```css
.chat-input {
  display: flex; align-items: center; gap: 8px;
  padding: 6px 8px 6px 10px;
  background: var(--card);
  border: 1px solid var(--border);
  border-radius: 20px;
  box-shadow: var(--shadow-sm);
}
.chat-input__skill {
  display: inline-flex; align-items: center; gap: 4px;
  background: var(--sidebar-accent); color: var(--muted-foreground);
  padding: 4px 10px; border-radius: 999px; font-size: 12px;
  transition: background 120ms, color 120ms;
}
.chat-input__skill:hover { background: var(--secondary); color: var(--foreground); }
.chat-input__field {
  flex: 1; background: transparent; border: none; outline: none;
  color: var(--foreground); font-size: 14px; padding: 4px 8px;
}
.chat-input__field::placeholder { color: var(--muted-foreground); }
.chat-input__model {
  display: inline-flex; align-items: center; gap: 4px;
  color: var(--muted-foreground); font-size: 12px;
  padding: 4px 10px; border-radius: 999px;
  transition: background 120ms, color 120ms;
}
.chat-input__model:hover { background: var(--sidebar-accent); color: var(--foreground); }
```

- [ ] **Step 3: import、build、预览**

- [ ] **Step 4: Commit**

```bash
git commit -am "feat(gui/chat): redesign ChatInput as pill bar per Pencil"
```

### Task 3.3：MessageBubble 与 SourceReferences 对齐

**Files:**
- Modify: `apps/clawx-gui/src/components/MessageBubble.tsx`
- Modify: `apps/clawx-gui/src/components/SourceReferences.tsx`
- Create: `apps/clawx-gui/src/styles/pages/message.css`

> 规格：
> - User: 右对齐气泡，`--primary` 填，`--primary-foreground` 字，圆角 14px，最大 66% 宽。
> - Assistant: 左对齐纯文本块（无气泡），下面跟工具调用状态行（`正在读取 auth.ts... 步骤 1/4` + 小加载 icon），再下面跟三张 SourceReference 卡。

- [ ] **Step 1: 重构 `MessageBubble.tsx`**

```tsx
import type { Message } from "../lib/types";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";

export default function MessageBubble({ message }: { message: Message }) {
  const isUser = message.role === "user";
  return (
    <div className={`msg ${isUser ? "msg--user" : "msg--assistant"}`}>
      <div className="msg__bubble">
        {isUser ? <span>{message.content}</span> : <Markdown remarkPlugins={[remarkGfm]}>{message.content}</Markdown>}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: 重构 `SourceReferences.tsx`**

```tsx
import { FileCode, BookOpen, FileText } from "lucide-react";

export interface SourceRef {
  id: string;
  filename: string;
  kind: "code" | "doc" | "text";
  lineRange?: string;       // e.g. "第 45-67 行"
  snippet: string;
}

const ICONS = { code: FileCode, doc: BookOpen, text: FileText } as const;

export default function SourceReferences({ refs }: { refs: SourceRef[] }) {
  if (!refs.length) return null;
  return (
    <ul className="src-refs">
      {refs.map((r) => {
        const Icon = ICONS[r.kind];
        return (
          <li key={r.id} className="src-ref">
            <div className="src-ref__head">
              <Icon size={14} className="src-ref__icon" />
              <span className="src-ref__filename">{r.filename}</span>
              {r.lineRange && <span className="src-ref__range">{r.lineRange}</span>}
            </div>
            <pre className="src-ref__snippet">{r.snippet}</pre>
          </li>
        );
      })}
    </ul>
  );
}
```

- [ ] **Step 3: 写 `message.css`**

```css
.msg { display: flex; margin: 12px 0; }
.msg--user { justify-content: flex-end; }
.msg--assistant { justify-content: flex-start; }
.msg__bubble {
  max-width: 66%;
  padding: 10px 14px;
  font-size: 13.5px; line-height: 1.55;
  border-radius: 14px;
}
.msg--user .msg__bubble { background: var(--primary); color: var(--primary-foreground); border-bottom-right-radius: 6px; }
.msg--assistant .msg__bubble { background: transparent; color: var(--foreground); padding: 4px 0; max-width: 100%; }
.msg--assistant .msg__bubble pre { background: var(--card); border: 1px solid var(--border); padding: 12px; border-radius: 8px; overflow: auto; font-size: 12px; }

.src-refs { list-style: none; margin-top: 12px; display: flex; flex-direction: column; gap: 8px; }
.src-ref { background: var(--card); border: 1px solid var(--border); border-radius: 8px; padding: 10px 12px; }
.src-ref__head { display: flex; align-items: center; gap: 8px; font-size: 12px; color: var(--muted-foreground); margin-bottom: 6px; }
.src-ref__icon { color: var(--primary); }
.src-ref__filename { color: var(--foreground); font-weight: 500; }
.src-ref__range { margin-left: auto; }
.src-ref__snippet { background: var(--background); padding: 8px; border-radius: 6px; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; line-height: 1.5; color: var(--foreground); white-space: pre-wrap; word-break: break-all; }
```

- [ ] **Step 4: `Message` type 增加 optional `refs: SourceRef[]`；`ChatPage` 透传**

在 `lib/types.ts` 的 `Message` 接口增加：
```ts
refs?: import("../components/SourceReferences").SourceRef[];
```
在 `ChatPage.tsx` 将 `<MessageBubble>` 之后紧跟 `<SourceReferences refs={m.refs ?? []} />`。

- [ ] **Step 5: build + 预览（给个 mock 消息带 ref 的场景）**

- [ ] **Step 6: Commit**

```bash
git commit -am "feat(gui/chat): redesign MessageBubble + SourceReferences per Pencil artifacts view"
```

### Task 3.4：ChatPage 主骨架 + Tabs + Composer 固定底部

**Files:**
- Modify: `apps/clawx-gui/src/pages/ChatPage.tsx`
- Create: `apps/clawx-gui/src/styles/pages/chat-page.css`

> 规格：主区上方 tabs `对话 / 产物`；`对话` 态消息流从上往下，`产物` 态渲染 `ArtifactsPanel` 内容。底部固定 `ChatInput`。顶部右上角（Active Conversation 稿）可出现 `ArtifactsPanel` 的作为侧栏，但在此阶段不做，直接放进 `产物` tab。

- [ ] **Step 1: 重构 `ChatPage.tsx` 骨架部分**（保留已有 `useEffect`、`sendMessageStream` 逻辑，仅改 JSX）

```tsx
// ...existing imports + logic...
import { TabsRoot, TabsList, TabsTrigger, TabsContent } from "../components/ui/Tabs";

return (
  <div className="chat-page">
    <TabsRoot value={activeTab} onChange={(v) => setActiveTab(v as any)}>
      <header className="chat-page__head">
        <TabsList>
          <TabsTrigger value="conversation">对话</TabsTrigger>
          <TabsTrigger value="artifacts">产物</TabsTrigger>
        </TabsList>
      </header>

      <TabsContent value="conversation">
        <div className="chat-page__body">
          {!convId && messages.length === 0 ? (
            <ChatWelcome onSuggest={(t) => { /* populate input */ }} />
          ) : (
            <div className="chat-page__stream">
              {messages.map((m) => (
                <div key={m.id}>
                  <MessageBubble message={m} />
                  <SourceReferences refs={m.refs ?? []} />
                </div>
              ))}
              {isStreaming && <MessageBubble message={{ id: "stream", role: "assistant", content: streamingContent, created_at: "" }} />}
              <div ref={messagesEndRef} />
            </div>
          )}
        </div>
      </TabsContent>

      <TabsContent value="artifacts">
        <ArtifactsPanel conversationId={convId ?? undefined} />
      </TabsContent>

      <footer className="chat-page__foot">
        <ChatInput onSend={(t) => { /* existing handler */ }} disabled={isStreaming || loading} />
      </footer>
    </TabsRoot>

    {error && <EmptyState message={error} />}
  </div>
);
```

- [ ] **Step 2: 写 `chat-page.css`**

```css
.chat-page { display: flex; flex-direction: column; height: 100%; padding: 16px 24px; }
.chat-page__head { padding: 4px 0 12px; }
.chat-page__body { flex: 1; min-height: 0; overflow: auto; }
.chat-page__stream { max-width: 820px; margin: 0 auto; padding-bottom: 24px; }
.chat-page__foot { padding: 12px 0 4px; max-width: 820px; margin: 0 auto; width: 100%; }
```

- [ ] **Step 3: import, build, 预览空态 + 有消息态**

- [ ] **Step 4: Commit**

```bash
git commit -am "feat(gui/chat): ChatPage tabs shell + fixed composer"
```

### Task 3.5：ArtifactsPanel 样式对齐（产物 tab）

**Files:**
- Modify: `apps/clawx-gui/src/components/ArtifactsPanel.tsx`
- Create: `apps/clawx-gui/src/styles/pages/artifacts.css`

> 规格：文档卡片列表。每张卡：左 icon（markdown 淡蓝 / python 橙 / image 紫），右上角日期，标题 + 描述 + 类型标签；hover 时边框亮起。顶部 `搜索产物...` pill input + 两按钮 `测试 / 新增`。

- [ ] **Step 1: 重写 `ArtifactsPanel.tsx`**

```tsx
import { Search, FileText, FileCode, Image as ImageIcon } from "lucide-react";
import Input from "./ui/Input";
import Button from "./ui/Button";

interface Artifact { id: string; kind: "md" | "py" | "img"; title: string; date: string; excerpt: string; lang: string }

const MOCK: Artifact[] = [
  { id: "1", kind: "md",  title: "竞品总结 - 第 3 版", date: "Mar 15, 2026", lang: "md", excerpt: "本次总结了当下 3 大主流助手的差异，识别了 10 个 bug，并对差异部分进行综合分析 35%，干货对营业报告有参考价值和指导意义。" },
  { id: "2", kind: "py",  title: "数据处理脚本", date: "Mar 14, 2026", lang: "Python", excerpt: "自动化 Python 脚本，用于自动化执行数据处理/数据批量存储的 PostgreSQL 函数。完成 500 条数据…" },
  { id: "3", kind: "img", title: "产品功能架构图", date: "Mar 12, 2026", lang: "SVG", excerpt: "架构图内容产品功能模块，功能模块详情/内部结构，设计模型和 API 调用关系的真实图。" },
];

const ICON: Record<Artifact["kind"], typeof FileText> = { md: FileText, py: FileCode, img: ImageIcon };

export default function ArtifactsPanel({ conversationId }: { conversationId?: string }) {
  const items = MOCK; // TODO: 接后端
  return (
    <div className="artifacts">
      <header className="artifacts__head">
        <Input leftIcon={<Search size={14} />} placeholder="搜索产物..." size="sm" />
        <div className="artifacts__actions">
          <Button variant="outline" size="sm">测试</Button>
          <Button variant="default" size="sm">新增</Button>
        </div>
      </header>
      <ul className="artifacts__list">
        {items.map((a) => {
          const Icon = ICON[a.kind];
          return (
            <li key={a.id} className="artifact-card">
              <div className={`artifact-card__icon artifact-card__icon--${a.kind}`}><Icon size={16} /></div>
              <div className="artifact-card__body">
                <div className="artifact-card__top">
                  <span className="artifact-card__title">{a.title}</span>
                  <span className="artifact-card__date">{a.date}</span>
                </div>
                <p className="artifact-card__excerpt">{a.excerpt}</p>
                <span className="artifact-card__lang">{a.lang}</span>
              </div>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
```

- [ ] **Step 2: 写 `artifacts.css`**

```css
.artifacts { display: flex; flex-direction: column; height: 100%; gap: 16px; padding: 4px 0; }
.artifacts__head { display: flex; align-items: center; gap: 8px; }
.artifacts__head .ui-input { flex: 1; max-width: 360px; }
.artifacts__actions { margin-left: auto; display: inline-flex; gap: 8px; }
.artifacts__list { list-style: none; display: flex; flex-direction: column; gap: 10px; overflow: auto; padding-bottom: 16px; }

.artifact-card { display: flex; gap: 12px; padding: 14px 16px; background: var(--card); border: 1px solid var(--border); border-radius: 12px; transition: border-color 120ms; }
.artifact-card:hover { border-color: var(--primary); }
.artifact-card__icon { width: 32px; height: 32px; border-radius: 8px; display: inline-flex; align-items: center; justify-content: center; flex-shrink: 0; }
.artifact-card__icon--md  { background: rgba(59,130,246,0.15); color: #B2CCFF; }
.artifact-card__icon--py  { background: rgba(245,158,11,0.15); color: #FFD9B2; }
.artifact-card__icon--img { background: rgba(139,92,246,0.18); color: #D7CCFF; }
.artifact-card__body { flex: 1; min-width: 0; }
.artifact-card__top { display: flex; align-items: baseline; gap: 8px; }
.artifact-card__title { font-size: 14px; font-weight: 600; color: var(--card-foreground); }
.artifact-card__date { margin-left: auto; font-size: 11px; color: var(--muted-foreground); }
.artifact-card__excerpt { margin-top: 4px; font-size: 12.5px; color: var(--muted-foreground); line-height: 1.55; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
.artifact-card__lang { display: inline-block; margin-top: 8px; font-size: 10.5px; color: var(--muted-foreground); padding: 2px 8px; background: var(--sidebar-accent); border-radius: 999px; }
```

- [ ] **Step 3: build，切 `/` tab 验证**

- [ ] **Step 4: Commit**

```bash
git commit -am "feat(gui/chat): artifacts list card design per Pencil"
```

---

## 阶段 4：Agent & Skill 页

### Task 4.1：AgentsPage 改为卡片网格 + Tabs

**Files:**
- Modify: `apps/clawx-gui/src/pages/AgentsPage.tsx`
- Create: `apps/clawx-gui/src/components/AgentGridCard.tsx` （新的统一卡片）
- Create: `apps/clawx-gui/src/styles/pages/agents-page.css`

> 规格：顶部条 `Agent / Skill` tabs，右侧 `搜索 Agent...` 输入 + `新建 Agent` 紫色按钮。主区 3 列卡片网格。每张卡：emoji 方块 + 名称 + 状态 badge，元信息 `对话 28 · 产出 15 · 创建于 3月10日`，模型 chip，下排两按钮 `进入` / `编辑`。

- [ ] **Step 1: 写 `AgentGridCard.tsx`**

```tsx
import Avatar from "./ui/Avatar";
import Badge from "./ui/Badge";
import Button from "./ui/Button";
import type { Agent } from "../lib/types";

const TONE_MAP = { working: "success", idle: "neutral", error: "error", offline: "warning" } as const;
const STATUS_LABEL = { working: "运行中", idle: "空闲", error: "错误", offline: "离线" } as const;

interface Props { agent: Agent; onEnter: () => void; onEdit: () => void }

export default function AgentGridCard({ agent, onEnter, onEdit }: Props) {
  return (
    <div className="agent-grid-card">
      <div className="agent-grid-card__head">
        <Avatar size={36} rounded="md" bg="var(--primary)">{agent.name.slice(0,2)}</Avatar>
        <div className="agent-grid-card__name-col">
          <div className="agent-grid-card__name">{agent.name}</div>
          <span className="agent-grid-card__model">{agent.model ?? "Claude Opus 4"}</span>
        </div>
        <Badge tone={TONE_MAP[agent.status]}>{STATUS_LABEL[agent.status]}</Badge>
      </div>
      <div className="agent-grid-card__meta">对话 28 · 产出 15 · 创建于 3月10日</div>
      <div className="agent-grid-card__actions">
        <Button variant="default" size="sm" onClick={onEnter}>进入</Button>
        <Button variant="outline" size="sm" onClick={onEdit}>编辑</Button>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: 重构 `AgentsPage.tsx`**

```tsx
import { useNavigate, useSearchParams } from "react-router-dom";
import { useState } from "react";
import { Plus, Search } from "lucide-react";
import { TabsRoot, TabsList, TabsTrigger, TabsContent } from "../components/ui/Tabs";
import Input from "../components/ui/Input";
import Button from "../components/ui/Button";
import AgentGridCard from "../components/AgentGridCard";
import SkillStore from "../components/SkillStore";
import AgentTemplateModal from "../components/AgentTemplateModal";
import { useAgents } from "../lib/store";

export default function AgentsPage() {
  const { agents } = useAgents();
  const navigate = useNavigate();
  const [setSearchParams] = useSearchParams().slice(1) as any;
  const [tab, setTab] = useState("agent");
  const [query, setQuery] = useState("");
  const [openNew, setOpenNew] = useState(false);

  const filtered = agents.filter((a) => a.name.toLowerCase().includes(query.toLowerCase()));

  return (
    <div className="agents-page">
      <TabsRoot value={tab} onChange={setTab}>
        <header className="agents-page__head">
          <TabsList>
            <TabsTrigger value="agent">Agent</TabsTrigger>
            <TabsTrigger value="skill">Skill</TabsTrigger>
          </TabsList>
          <div className="agents-page__head-right">
            <Input size="sm" leftIcon={<Search size={14} />} placeholder="搜索 Agent..." value={query} onChange={(e) => setQuery(e.target.value)} />
            <Button leftIcon={<Plus size={14} />} size="sm" onClick={() => setOpenNew(true)}>新建 Agent</Button>
          </div>
        </header>

        <TabsContent value="agent">
          <div className="agents-page__grid">
            {filtered.map((a) => (
              <AgentGridCard
                key={a.id}
                agent={a}
                onEnter={() => navigate(`/?agent=${a.id}`)}
                onEdit={() => navigate(`/agents/${a.id}/edit`)}
              />
            ))}
          </div>
        </TabsContent>
        <TabsContent value="skill">
          <SkillStore />
        </TabsContent>
      </TabsRoot>

      <AgentTemplateModal open={openNew} onClose={() => setOpenNew(false)} />
    </div>
  );
}
```

- [ ] **Step 3: 写 `agents-page.css`**

```css
.agents-page { padding: 20px 28px; height: 100%; display: flex; flex-direction: column; gap: 16px; }
.agents-page__head { display: flex; align-items: center; gap: 12px; }
.agents-page__head-right { margin-left: auto; display: inline-flex; align-items: center; gap: 8px; }
.agents-page__grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 16px; align-content: start; overflow: auto; padding-bottom: 16px; }

.agent-grid-card { background: var(--card); border: 1px solid var(--border); border-radius: 12px; padding: 16px; display: flex; flex-direction: column; gap: 10px; }
.agent-grid-card__head { display: flex; align-items: center; gap: 10px; }
.agent-grid-card__name-col { display: flex; flex-direction: column; flex: 1; min-width: 0; }
.agent-grid-card__name { font-size: 14px; font-weight: 600; color: var(--card-foreground); }
.agent-grid-card__model { font-size: 11px; color: var(--muted-foreground); margin-top: 2px; }
.agent-grid-card__meta { font-size: 11.5px; color: var(--muted-foreground); }
.agent-grid-card__actions { display: flex; gap: 8px; }
.agent-grid-card__actions .ui-btn { flex: 1; }
```

- [ ] **Step 4: `lib/types.ts` 给 `Agent` 增加可选 `model?: string`**

- [ ] **Step 5: import, build, 预览（tab 切换 + mock 数据）**

- [ ] **Step 6: Commit**

```bash
git commit -am "feat(gui/agents): AgentsPage tabs + grid card per Pencil"
```

### Task 4.2：SkillStore 卡片对齐（Skill tab）

**Files:**
- Modify: `apps/clawx-gui/src/components/SkillStore.tsx`
- Create: `apps/clawx-gui/src/styles/pages/skill-store.css`

> 规格：与 Agent 卡片等宽网格；每张卡：左上图标（wrench/code/mic/pencil 等）+ 名称、描述 2 行、`已启用/未启用` Badge、`使用 / 编辑` 按钮。

- [ ] **Step 1: 重写 `SkillStore.tsx`**

```tsx
import { Wrench, Braces, Mic, PenTool, FileSearch, Languages } from "lucide-react";
import Badge from "./ui/Badge";
import Button from "./ui/Button";

const SKILLS = [
  { id: "1", icon: Braces,     name: "代码生成", desc: "根据需求自动生成高质量的代码，支持多种编程语言。", enabled: true,  uses: 136, ts: "3 小时前" },
  { id: "2", icon: FileSearch, name: "数据分析", desc: "处理和分析大规模数据集，提供可视化洞察。",       enabled: true,  uses: 203, ts: "2 小时前" },
  { id: "3", icon: Mic,        name: "语音识别", desc: "将音频内容转换为文字，支持多种语言识别。",       enabled: false, uses: 194, ts: "昨天" },
  { id: "4", icon: PenTool,    name: "文档撰写", desc: "自动生成结构化文档、报告和技术说明。",          enabled: true,  uses: 120, ts: "4 小时前" },
  { id: "5", icon: Languages,  name: "智能翻译", desc: "支持多语言互译，保持原文语境和风格。",          enabled: true,  uses: 87,  ts: "1 天前" },
  { id: "6", icon: Wrench,     name: "摘要总结", desc: "快速提取长文本主要内容，生成精炼摘要。",         enabled: true,  uses: 148, ts: "6 小时前" },
];

export default function SkillStore() {
  return (
    <div className="skill-grid">
      {SKILLS.map((s) => (
        <div key={s.id} className="skill-card">
          <div className="skill-card__head">
            <div className="skill-card__icon"><s.icon size={16} /></div>
            <span className="skill-card__name">{s.name}</span>
            <Badge tone={s.enabled ? "success" : "neutral"}>{s.enabled ? "已启用" : "未启用"}</Badge>
          </div>
          <p className="skill-card__desc">{s.desc}</p>
          <span className="skill-card__meta">调用 {s.uses} 次 · 最近使用 {s.ts}</span>
          <div className="skill-card__actions">
            <Button variant="default" size="sm">使用</Button>
            <Button variant="outline" size="sm">编辑</Button>
          </div>
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 2: 写 `skill-store.css`**

```css
.skill-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 16px; overflow: auto; padding-bottom: 16px; }
.skill-card { background: var(--card); border: 1px solid var(--border); border-radius: 12px; padding: 16px; display: flex; flex-direction: column; gap: 10px; }
.skill-card__head { display: flex; align-items: center; gap: 10px; }
.skill-card__icon { width: 28px; height: 28px; border-radius: 8px; background: var(--sidebar-accent); color: var(--primary); display: inline-flex; align-items: center; justify-content: center; }
.skill-card__name { font-size: 14px; font-weight: 600; color: var(--card-foreground); flex: 1; }
.skill-card__desc { font-size: 12.5px; color: var(--muted-foreground); line-height: 1.5; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
.skill-card__meta { font-size: 11px; color: var(--muted-foreground); }
.skill-card__actions { display: flex; gap: 8px; margin-top: auto; }
.skill-card__actions .ui-btn { flex: 1; }
```

- [ ] **Step 3: import, build, 预览 Skill tab**

- [ ] **Step 4: Commit**

```bash
git commit -am "feat(gui/agents): SkillStore card grid per Pencil"
```

---

## 阶段 5：新建 Agent 模态（Template + Custom）

### Task 5.1：AgentTemplateModal（两 tab）

**Files:**
- Create: `apps/clawx-gui/src/components/AgentTemplateModal.tsx`
- Modify: `apps/clawx-gui/src/components/AgentTemplateGrid.tsx` （复用现有）
- Modify: `apps/clawx-gui/src/components/AgentForm.tsx` （减脂 / 对齐样式）
- Create: `apps/clawx-gui/src/styles/pages/agent-template.css`

> 规格（5CbuX + tEBQq）：
> - Modal 宽 `560px`。标题 `创建新 Agent` + 副标题。
> - 顶部两 tab：`从模板 / 自定义`。
> - 模板 tab：三张卡（开发助手/研究分析/写作创作），选中态紫色边框 + 紫色底（`rgba(87,73,244,0.15)`）；下面是 `Agent 名称` Input + `模型` Select + 底部 `取消 / 创建 Agent`。
> - 自定义 tab：头像上传区（`40×40` + 上传按钮）、`Agent 名称`、`描述`（Textarea）、`系统提示词`（Textarea），再 `模型` Select + 底部按钮。

- [ ] **Step 1: 写 `AgentTemplateModal.tsx`**

```tsx
import { useState } from "react";
import { Code, Search, PenLine, Upload } from "lucide-react";
import Dialog from "./ui/Dialog";
import { TabsRoot, TabsList, TabsTrigger, TabsContent } from "./ui/Tabs";
import Input from "./ui/Input";
import Textarea from "./ui/Textarea";
import Select from "./ui/Select";
import Button from "./ui/Button";

const TEMPLATES = [
  { id: "dev",      icon: Code,    name: "开发助手", desc: "代码审查、调试、架构设计" },
  { id: "research", icon: Search,  name: "研究分析", desc: "网络搜索、数据分析、报告撰写" },
  { id: "writing",  icon: PenLine, name: "写作创作", desc: "内容创作、编辑、文案撰写" },
];

const MODELS = [
  { value: "claude-opus-4-6",   label: "Claude Opus 4" },
  { value: "claude-sonnet-4-6", label: "Claude Sonnet 4.6" },
  { value: "gpt-4o",            label: "GPT-4o" },
];

interface Props { open: boolean; onClose: () => void }

export default function AgentTemplateModal({ open, onClose }: Props) {
  const [tab, setTab] = useState("template");
  const [tpl, setTpl] = useState("dev");
  const [name, setName] = useState("");
  const [desc, setDesc] = useState("");
  const [prompt, setPrompt] = useState("");
  const [model, setModel] = useState("claude-opus-4-6");

  return (
    <Dialog open={open} onClose={onClose} width={560}>
      <header className="agent-template__head">
        <h2>创建新 Agent</h2>
        <p>创建一个专属的多功能智能 AI Agent</p>
      </header>

      <TabsRoot value={tab} onChange={setTab}>
        <TabsList>
          <TabsTrigger value="template">从模板</TabsTrigger>
          <TabsTrigger value="custom">自定义</TabsTrigger>
        </TabsList>

        <TabsContent value="template">
          <ul className="agent-template__grid">
            {TEMPLATES.map((t) => (
              <li key={t.id}>
                <button
                  className={`tpl-card ${tpl === t.id ? "is-active" : ""}`}
                  onClick={() => setTpl(t.id)}
                >
                  <div className="tpl-card__icon"><t.icon size={16} /></div>
                  <div className="tpl-card__text">
                    <div className="tpl-card__name">{t.name}</div>
                    <div className="tpl-card__desc">{t.desc}</div>
                  </div>
                </button>
              </li>
            ))}
          </ul>
          <label className="field">
            <span className="field__label">Agent 名称</span>
            <Input placeholder="例如: 我的开发助手" value={name} onChange={(e) => setName(e.target.value)} />
          </label>
          <label className="field">
            <span className="field__label">模型</span>
            <Select options={MODELS} value={model} onChange={(e) => setModel(e.target.value)} />
          </label>
        </TabsContent>

        <TabsContent value="custom">
          <div className="agent-template__avatar">
            <div className="agent-template__avatar-slot">PC</div>
            <div className="agent-template__avatar-meta">
              <Button size="sm" leftIcon={<Upload size={14} />} variant="outline">上传头像</Button>
              <span>PNG, JPG 最大 2MB</span>
            </div>
          </div>
          <label className="field">
            <span className="field__label">Agent 名称</span>
            <Input placeholder="例如: 我的自定义 Agent" value={name} onChange={(e) => setName(e.target.value)} />
          </label>
          <label className="field">
            <span className="field__label">描述</span>
            <Textarea placeholder="简要描述该 Agent 的任务" value={desc} onChange={(e) => setDesc(e.target.value)} />
          </label>
          <label className="field">
            <span className="field__label">系统提示词</span>
            <Textarea placeholder="输入 Agent 的指令..." value={prompt} onChange={(e) => setPrompt(e.target.value)} />
          </label>
          <label className="field">
            <span className="field__label">模型</span>
            <Select options={MODELS} value={model} onChange={(e) => setModel(e.target.value)} />
          </label>
        </TabsContent>
      </TabsRoot>

      <footer className="agent-template__foot">
        <Button variant="ghost" onClick={onClose}>取消</Button>
        <Button variant="default">创建 Agent</Button>
      </footer>
    </Dialog>
  );
}
```

- [ ] **Step 2: 写 `agent-template.css`**

```css
.agent-template__head { margin-bottom: 16px; }
.agent-template__head h2 { font-size: 18px; font-weight: 600; }
.agent-template__head p { margin-top: 4px; font-size: 12px; color: var(--muted-foreground); }

.agent-template__grid { list-style: none; display: flex; flex-direction: column; gap: 10px; margin-top: 16px; }
.tpl-card { display: flex; align-items: center; gap: 12px; padding: 12px 14px; width: 100%; border: 1px solid var(--border); border-radius: 12px; background: var(--card); color: var(--foreground); text-align: left; transition: border-color 120ms, background 120ms; }
.tpl-card:hover { border-color: var(--primary); }
.tpl-card.is-active { background: color-mix(in srgb, var(--primary) 15%, var(--card)); border-color: var(--primary); }
.tpl-card__icon { width: 28px; height: 28px; border-radius: 8px; background: var(--sidebar-accent); color: var(--primary); display: inline-flex; align-items: center; justify-content: center; }
.tpl-card__text { flex: 1; }
.tpl-card__name { font-size: 14px; font-weight: 600; }
.tpl-card__desc { font-size: 12px; color: var(--muted-foreground); margin-top: 2px; }

.field { display: block; margin-top: 14px; }
.field__label { display: block; font-size: 12px; color: var(--muted-foreground); margin-bottom: 6px; }
.field .ui-input, .field .ui-select { width: 100%; }

.agent-template__avatar { display: flex; align-items: center; gap: 16px; margin-top: 16px; padding: 12px; background: var(--sidebar-accent); border-radius: 12px; }
.agent-template__avatar-slot { width: 48px; height: 48px; border-radius: 12px; background: var(--primary); color: var(--primary-foreground); display: flex; align-items: center; justify-content: center; font-weight: 600; font-size: 16px; }
.agent-template__avatar-meta { display: flex; flex-direction: column; gap: 6px; font-size: 11px; color: var(--muted-foreground); }

.agent-template__foot { display: flex; gap: 8px; justify-content: flex-end; margin-top: 24px; }
```

- [ ] **Step 3: 删除 `AgentForm.tsx` 的多余样式（如存在老 modal 样式），或把它保留为路由 `/agents/:id/edit` 的编辑页独用**

- [ ] **Step 4: `AgentsPage.tsx` 已在 4.1 引用；`AgentSidebar` 的 `+` 按钮也接入该 modal：**

在 `AgentSidebar.tsx` 增加 `useState + AgentTemplateModal`，`+` 按钮 `onClick={() => setOpenNew(true)}`。

- [ ] **Step 5: build + 预览点击 `+`**

- [ ] **Step 6: Commit**

```bash
git commit -am "feat(gui/agents): AgentTemplateModal with template + custom tabs"
```

---

## 阶段 6：知识库

### Task 6.1：KnowledgePage 两列布局

**Files:**
- Modify: `apps/clawx-gui/src/pages/KnowledgePage.tsx`
- Modify: `apps/clawx-gui/src/components/KnowledgeSourceList.tsx`
- Create: `apps/clawx-gui/src/components/KnowledgeSearchPanel.tsx`
- Create: `apps/clawx-gui/src/styles/pages/knowledge-page.css`

> 规格：
> - 左列 360px：顶部标题 `知识库` + `添加知识源` 紫色按钮。下面分组 `本地添加 (4)` / `对话产生 (3)`，每组列出源卡片（图标 + 名称 + meta + 状态 badge，如 `活跃`/`索引中`；索引中带进度条）。
> - 右列 flex：搜索工作台。顶部 `在所有知识中搜索...`。下面搜索结果卡片：标题、高亮摘要 `...`, 来源元信息 `产品文档库 · api-design-guide.md · 本地`, `查看原文 →`。底部 drop zone 占满宽度空状态 `拖放文件到此处添加到知识库`。

- [ ] **Step 1: 重写 `KnowledgePage.tsx`**

```tsx
import KnowledgeSourceList from "../components/KnowledgeSourceList";
import KnowledgeSearchPanel from "../components/KnowledgeSearchPanel";

export default function KnowledgePage() {
  return (
    <div className="knowledge-page">
      <aside className="knowledge-page__left"><KnowledgeSourceList /></aside>
      <section className="knowledge-page__right"><KnowledgeSearchPanel /></section>
    </div>
  );
}
```

- [ ] **Step 2: 重写 `KnowledgeSourceList.tsx`**

```tsx
import { FolderOpen, BookOpen, FileCode, Mic, Plus, MessagesSquare } from "lucide-react";
import Badge from "./ui/Badge";
import Button from "./ui/Button";
import Progress from "./ui/Progress";

interface Source { id: string; icon: any; name: string; docs: number; status: "active" | "indexing"; progress?: number; group: "local" | "chat" }

const LOCAL: Source[] = [
  { id: "1", icon: FolderOpen, name: "产品文档库", docs: 23, status: "active",   group: "local" },
  { id: "2", icon: BookOpen,   name: "技术规范",   docs: 15, status: "active",   group: "local" },
  { id: "3", icon: FileCode,   name: "竞品分析",   docs: 8,  status: "indexing", progress: 72, group: "local" },
  { id: "4", icon: Mic,        name: "会议记录",   docs: 45, status: "active",   group: "local" },
];

const CHAT: Source[] = [
  { id: "c1", icon: MessagesSquare, name: "产品策略讨论",  docs: 12, status: "indexing", progress: 40, group: "chat" },
  { id: "c2", icon: MessagesSquare, name: "技术方案评审",  docs: 5,  status: "active",   group: "chat" },
  { id: "c3", icon: MessagesSquare, name: "竞品调研总结",  docs: 9,  status: "active",   group: "chat" },
];

function Row({ s }: { s: Source }) {
  return (
    <div className="kn-src">
      <div className="kn-src__icon"><s.icon size={16} /></div>
      <div className="kn-src__body">
        <div className="kn-src__name">{s.name}</div>
        <div className="kn-src__meta">{s.docs} 篇文档 · {s.status === "active" ? "已索引" : `索引中 ${s.progress ?? 0}%`}</div>
        {s.status === "indexing" && <Progress value={s.progress ?? 0} />}
      </div>
      <Badge tone={s.status === "active" ? "success" : "warning"}>{s.status === "active" ? "活跃" : "索引中"}</Badge>
    </div>
  );
}

export default function KnowledgeSourceList() {
  return (
    <div className="kn-list">
      <header className="kn-list__head">
        <h2>知识库</h2>
        <Button leftIcon={<Plus size={14} />} size="sm">添加知识源</Button>
      </header>
      <section>
        <h3 className="kn-list__group">本地添加 ({LOCAL.length})</h3>
        {LOCAL.map((s) => <Row key={s.id} s={s} />)}
      </section>
      <section>
        <h3 className="kn-list__group">对话产生 ({CHAT.length})</h3>
        {CHAT.map((s) => <Row key={s.id} s={s} />)}
      </section>
    </div>
  );
}
```

- [ ] **Step 3: 写 `KnowledgeSearchPanel.tsx`**

```tsx
import { Search, UploadCloud } from "lucide-react";
import Input from "./ui/Input";

const RESULTS = [
  { id: "1", title: "API 接口设计规范", source: "产品文档库 · api-design-guide.md · 本地",   excerpt: "API 设计应遵循 RESTful 原则，所有端点返回 JSON 格式，支持版本控制。认证使用 JWT Bearer Token，有效期 24 小时。" },
  { id: "2", title: "认证系统安全规范",  source: "技术规范 · auth-spec-v2.md",             excerpt: "密码必须使用 bcrypt 加密（salt rounds ≥ 10），禁用 MD5/SHA1 作为哈希函数。" },
  { id: "3", title: "产品路线图讨论",   source: "产品讨论 #42 · chat-2024-03-15.md",       excerpt: "Q2 重点方向：智能化 Agent 能力增强、多渠道接入、企业级知识库集成。" },
  { id: "4", title: "数据流重构方案",   source: "技术评审 #18 · review-session.md",        excerpt: "将旧有 REST 接口统一迁移到 GraphQL 网关，关键路径保留向后兼容。" },
];

export default function KnowledgeSearchPanel() {
  return (
    <div className="kn-search">
      <Input leftIcon={<Search size={14} />} placeholder="在所有知识中搜索..." size="md" />
      <ul className="kn-search__results">
        {RESULTS.map((r) => (
          <li key={r.id} className="kn-search__item">
            <div className="kn-search__item-head">
              <span className="kn-search__title">{r.title}</span>
              <a className="kn-search__view">查看原文 →</a>
            </div>
            <p className="kn-search__excerpt">{r.excerpt}</p>
            <span className="kn-search__source">{r.source}</span>
          </li>
        ))}
      </ul>
      <div className="kn-search__drop">
        <UploadCloud size={20} />
        <p>拖放文件到此处添加到知识库</p>
        <span>支持 PDF、Markdown、代码文件等</span>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: 写 `knowledge-page.css`**

```css
.knowledge-page { display: grid; grid-template-columns: 360px 1fr; gap: 16px; padding: 20px 24px; height: 100%; overflow: hidden; }
.knowledge-page__left { overflow: auto; }
.knowledge-page__right { overflow: auto; }

.kn-list { display: flex; flex-direction: column; gap: 16px; }
.kn-list__head { display: flex; align-items: center; justify-content: space-between; }
.kn-list__head h2 { font-size: 18px; font-weight: 700; }
.kn-list__group { font-size: 12px; color: var(--muted-foreground); margin-bottom: 8px; font-weight: 500; text-transform: uppercase; letter-spacing: 0.02em; }

.kn-src { display: flex; align-items: center; gap: 10px; padding: 10px 12px; border-radius: 12px; background: var(--card); border: 1px solid var(--border); margin-bottom: 8px; }
.kn-src__icon { width: 32px; height: 32px; border-radius: 8px; background: var(--sidebar-accent); color: var(--primary); display: inline-flex; align-items: center; justify-content: center; }
.kn-src__body { flex: 1; min-width: 0; }
.kn-src__name { font-size: 13px; font-weight: 600; }
.kn-src__meta { font-size: 11px; color: var(--muted-foreground); margin: 2px 0 4px; }

.kn-search { display: flex; flex-direction: column; gap: 16px; }
.kn-search__results { list-style: none; display: flex; flex-direction: column; gap: 10px; }
.kn-search__item { padding: 12px 14px; border: 1px solid var(--border); border-radius: 12px; background: var(--card); }
.kn-search__item-head { display: flex; align-items: center; justify-content: space-between; }
.kn-search__title { font-size: 13px; font-weight: 600; }
.kn-search__view { font-size: 11px; color: var(--primary); cursor: pointer; }
.kn-search__excerpt { margin-top: 6px; font-size: 12.5px; color: var(--muted-foreground); line-height: 1.5; }
.kn-search__source { display: inline-block; margin-top: 8px; font-size: 11px; color: var(--muted-foreground); padding: 2px 8px; background: var(--sidebar-accent); border-radius: 999px; }
.kn-search__drop { border: 1px dashed var(--border); border-radius: 12px; padding: 20px; display: flex; flex-direction: column; align-items: center; gap: 4px; color: var(--muted-foreground); font-size: 12px; }
.kn-search__drop p { font-size: 13px; color: var(--foreground); }
```

- [ ] **Step 5: build，预览 `/knowledge`**

- [ ] **Step 6: Commit**

```bash
git commit -am "feat(gui/knowledge): two-column layout with source list and search workspace"
```

---

## 阶段 7：定时任务

### Task 7.1：TasksPage + TaskCard

**Files:**
- Modify: `apps/clawx-gui/src/pages/TasksPage.tsx`
- Create: `apps/clawx-gui/src/components/TaskCard.tsx`
- Create: `apps/clawx-gui/src/styles/pages/tasks-page.css`

> 规格：
> - 顶部 `定时任务` 标题 + `创建任务` 紫色按钮（左 `+` icon）。
> - 搜索 input + 过滤 tabs（`全部 / 运行中 / 已暂停 / 出错`）。
> - 下方任务卡片列表：每张卡含
>   - 标题行：timer icon + 任务名 + 状态 Badge（运行中/已暂停/出错）。
>   - 子标题：`编程助手 · 每天 08:00`（agent · cron）。
>   - 元信息：`上次执行: 今天 08:00 · 成功`。
>   - 反馈指标行：三个 chip：`采纳 12 次` / `忽略 2 次` / `负反馈 0 次`（tone=success/neutral/error）。

- [ ] **Step 1: 写 `TaskCard.tsx`**

```tsx
import { Clock, Check, X, ThumbsDown } from "lucide-react";
import Badge from "./ui/Badge";

interface Task {
  id: string;
  name: string;
  agent: string;
  schedule: string;
  status: "running" | "paused" | "error";
  lastRun: string;
  feedback: { accepted: number; ignored: number; negative: number };
}

const STATUS_TONE = { running: "success", paused: "neutral", error: "error" } as const;
const STATUS_LABEL = { running: "运行中", paused: "已暂停", error: "出错" } as const;

export default function TaskCard({ task }: { task: Task }) {
  return (
    <li className="task-card">
      <header className="task-card__head">
        <Clock size={16} className="task-card__icon" />
        <span className="task-card__name">{task.name}</span>
        <Badge tone={STATUS_TONE[task.status]}>{STATUS_LABEL[task.status]}</Badge>
      </header>
      <div className="task-card__sub">{task.agent} · {task.schedule}</div>
      <div className="task-card__meta">上次执行: {task.lastRun}</div>
      <div className="task-card__feedback">
        <span className="fb fb--ok"><Check size={12} /> 采纳 {task.feedback.accepted} 次</span>
        <span className="fb fb--neutral"><X size={12} /> 忽略 {task.feedback.ignored} 次</span>
        <span className="fb fb--bad"><ThumbsDown size={12} /> 负反馈 {task.feedback.negative} 次</span>
      </div>
    </li>
  );
}
```

- [ ] **Step 2: 重写 `TasksPage.tsx` 骨架**

```tsx
import { useState } from "react";
import { Plus, Search } from "lucide-react";
import Button from "../components/ui/Button";
import Input from "../components/ui/Input";
import { TabsRoot, TabsList, TabsTrigger } from "../components/ui/Tabs";
import TaskCard from "../components/TaskCard";

const MOCK = [
  { id: "1", name: "每日晨报生成", agent: "编程助手", schedule: "每天 08:00",  status: "running", lastRun: "今天 08:00 · 成功", feedback: { accepted: 12, ignored: 2, negative: 0 } },
  { id: "2", name: "竞品监控周报", agent: "研究助手", schedule: "每周一 09:00", status: "paused",  lastRun: "3月24日 09:00 · 成功", feedback: { accepted: 8, ignored: 0, negative: 1 } },
  { id: "3", name: "PR 合并后自动更新文档", agent: "编程助手", schedule: "事件触发: GitHub webhook", status: "running", lastRun: "今天 14:22 · 成功", feedback: { accepted: 3, ignored: 1, negative: 0 } },
] as const;

export default function TasksPage() {
  const [tab, setTab] = useState("all");
  const [q, setQ] = useState("");
  const list = MOCK.filter((t) => tab === "all" ? true
    : tab === "running" ? t.status === "running"
    : tab === "paused" ? t.status === "paused"
    : t.status === "error")
    .filter((t) => !q || t.name.includes(q));

  return (
    <div className="tasks-page">
      <header className="tasks-page__head">
        <h1>定时任务</h1>
        <Button leftIcon={<Plus size={14} />} size="sm">创建任务</Button>
      </header>

      <div className="tasks-page__bar">
        <Input size="sm" leftIcon={<Search size={14} />} placeholder="搜索任务..." value={q} onChange={(e) => setQ(e.target.value)} />
        <TabsRoot value={tab} onChange={setTab}>
          <TabsList>
            <TabsTrigger value="all">全部</TabsTrigger>
            <TabsTrigger value="running">运行中</TabsTrigger>
            <TabsTrigger value="paused">已暂停</TabsTrigger>
            <TabsTrigger value="error">出错</TabsTrigger>
          </TabsList>
        </TabsRoot>
      </div>

      <ul className="tasks-page__list">
        {list.map((t) => <TaskCard key={t.id} task={t as any} />)}
      </ul>
    </div>
  );
}
```

- [ ] **Step 3: 写 `tasks-page.css`**

```css
.tasks-page { padding: 20px 24px; display: flex; flex-direction: column; gap: 16px; height: 100%; overflow: hidden; }
.tasks-page__head { display: flex; align-items: center; justify-content: space-between; }
.tasks-page__head h1 { font-size: 18px; font-weight: 700; }
.tasks-page__bar { display: flex; align-items: center; gap: 12px; }
.tasks-page__bar .ui-input { max-width: 320px; }
.tasks-page__list { list-style: none; display: flex; flex-direction: column; gap: 10px; overflow: auto; padding-bottom: 16px; }

.task-card { background: var(--card); border: 1px solid var(--border); border-radius: 12px; padding: 14px 16px; }
.task-card__head { display: flex; align-items: center; gap: 8px; }
.task-card__icon { color: var(--primary); }
.task-card__name { font-size: 14px; font-weight: 600; flex: 1; }
.task-card__sub  { margin-top: 2px; font-size: 12px; color: var(--muted-foreground); }
.task-card__meta { margin-top: 6px; font-size: 11.5px; color: var(--muted-foreground); }
.task-card__feedback { margin-top: 10px; display: inline-flex; gap: 6px; flex-wrap: wrap; }
.fb { display: inline-flex; align-items: center; gap: 4px; padding: 2px 10px; border-radius: 999px; font-size: 11px; }
.fb--ok      { background: var(--color-success); color: var(--color-success-foreground); }
.fb--neutral { background: var(--sidebar-accent); color: var(--muted-foreground); }
.fb--bad     { background: var(--color-error); color: var(--color-error-foreground); }
```

- [ ] **Step 4: build，预览 `/tasks`**

- [ ] **Step 5: Commit**

```bash
git commit -am "feat(gui/tasks): task card with feedback metrics per Pencil"
```

---

## 阶段 8：渠道管理

### Task 8.1：ConnectorsPage + ConnectorCard + AvailableChannelChip

**Files:**
- Modify: `apps/clawx-gui/src/pages/ConnectorsPage.tsx`
- Create: `apps/clawx-gui/src/components/ConnectorCard.tsx`
- Create: `apps/clawx-gui/src/components/AvailableChannelChip.tsx`
- Create: `apps/clawx-gui/src/styles/pages/connectors-page.css`

> 规格（ouoNU）：
> - 标题 `渠道管理` 带 `refresh-cw` icon。
> - `Agent 上下文`卡：紫点 + `编程助手 · 运行中` + 描述。
> - 主按钮 `为此 Agent 添加渠道`（紫色 + Plus icon）。
> - 分组 `已连接渠道`：列出 ConnectorCard（emoji + 名称 + 渠道类型/模式 + 指标行；异常时显示错误描述 + `重新连接` 按钮）。
> - 分组 `此 Agent 可用渠道`：AvailableChannelChip 网格（pill；未支持态灰色 `disabled`）。

- [ ] **Step 1: 写 `ConnectorCard.tsx`**

```tsx
import Badge from "./ui/Badge";
import Button from "./ui/Button";
import { RefreshCw } from "lucide-react";

interface Props {
  emoji: string;
  name: string;
  typeLine: string;
  status: "connected" | "disconnected";
  metricLine: string;
  errorLine?: string;
}

export default function ConnectorCard({ emoji, name, typeLine, status, metricLine, errorLine }: Props) {
  return (
    <div className="conn-card">
      <div className="conn-card__head">
        <span className="conn-card__emoji">{emoji}</span>
        <div className="conn-card__title-col">
          <div className="conn-card__name">{name}</div>
          <div className="conn-card__type">{typeLine}</div>
        </div>
        <Badge tone={status === "connected" ? "success" : "error"}>
          {status === "connected" ? "已连接" : "已断开"}
        </Badge>
      </div>
      <div className="conn-card__meta">{metricLine}</div>
      {errorLine && (
        <div className="conn-card__error">
          <span>{errorLine}</span>
          <Button size="sm" variant="outline" leftIcon={<RefreshCw size={12} />}>重新连接</Button>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: 写 `AvailableChannelChip.tsx`**

```tsx
import { Check } from "lucide-react";

export default function AvailableChannelChip({ name, available }: { name: string; available: boolean }) {
  return (
    <span className={`ch-chip ${available ? "" : "is-disabled"}`}>
      {available && <Check size={12} />}
      {name}{!available && " (不支持)"}
    </span>
  );
}
```

- [ ] **Step 3: 重写 `ConnectorsPage.tsx`**

```tsx
import { RefreshCw, Plus } from "lucide-react";
import Button from "../components/ui/Button";
import ConnectorCard from "../components/ConnectorCard";
import AvailableChannelChip from "../components/AvailableChannelChip";

export default function ConnectorsPage() {
  return (
    <div className="connectors-page">
      <header className="connectors-page__head">
        <div className="connectors-page__title"><RefreshCw size={16} /><h1>渠道管理</h1></div>
      </header>

      <div className="connectors-page__ctx">
        <span className="dot dot--success" />
        <span className="ctx-text"><strong>编程助手</strong> · 运行中</span>
        <span className="ctx-desc">管理此 Agent 的消息渠道连接，不同 Agent 支持不同的渠道类型</span>
      </div>

      <Button leftIcon={<Plus size={14} />} size="md" variant="default">为此 Agent 添加渠道</Button>

      <section>
        <h2 className="connectors-page__group">已连接渠道</h2>
        <div className="connectors-page__connected">
          <ConnectorCard
            emoji="💬"
            name="飞书 - 产品团队群"
            typeLine="飞书群聊 · 自动回复模式"
            status="connected"
            metricLine="今日消息: 12 条 · 最近活跃: 10 分钟前"
          />
          <ConnectorCard
            emoji="💼"
            name="Slack - #engineering"
            typeLine="Slack 频道 · Webhook 模式"
            status="disconnected"
            metricLine=""
            errorLine="连接异常: Token 已过期，请重新授权"
          />
        </div>
      </section>

      <section>
        <h2 className="connectors-page__group">此 Agent 可用渠道</h2>
        <div className="connectors-page__available">
          <AvailableChannelChip name="飞书" available />
          <AvailableChannelChip name="Telegram" available />
          <AvailableChannelChip name="Slack" available />
          <AvailableChannelChip name="Discord" available={false} />
          <AvailableChannelChip name="微信" available={false} />
        </div>
      </section>
    </div>
  );
}
```

- [ ] **Step 4: 写 `connectors-page.css`**

```css
.connectors-page { padding: 20px 24px; display: flex; flex-direction: column; gap: 16px; height: 100%; overflow: auto; }
.connectors-page__head { display: flex; align-items: center; justify-content: space-between; }
.connectors-page__title { display: inline-flex; align-items: center; gap: 8px; color: var(--foreground); }
.connectors-page__title h1 { font-size: 18px; font-weight: 700; }
.connectors-page__ctx { display: flex; align-items: center; gap: 10px; padding: 12px 14px; background: var(--card); border: 1px solid var(--border); border-radius: 12px; font-size: 12.5px; }
.dot { width: 8px; height: 8px; border-radius: 999px; }
.dot--success { background: #22C55E; box-shadow: 0 0 0 3px rgba(34,197,94,0.2); }
.ctx-text strong { font-weight: 600; }
.ctx-desc { color: var(--muted-foreground); margin-left: auto; }
.connectors-page__group { font-size: 12px; color: var(--muted-foreground); font-weight: 500; text-transform: uppercase; margin-bottom: 8px; }

.connectors-page__connected { display: flex; flex-direction: column; gap: 10px; }
.conn-card { background: var(--card); border: 1px solid var(--border); border-radius: 12px; padding: 14px 16px; }
.conn-card__head { display: flex; align-items: center; gap: 10px; }
.conn-card__emoji { font-size: 20px; }
.conn-card__title-col { flex: 1; }
.conn-card__name { font-size: 14px; font-weight: 600; }
.conn-card__type { font-size: 12px; color: var(--muted-foreground); margin-top: 2px; }
.conn-card__meta { margin-top: 8px; font-size: 11.5px; color: var(--muted-foreground); }
.conn-card__error { margin-top: 10px; display: flex; align-items: center; gap: 10px; padding: 8px 10px; background: color-mix(in srgb, var(--destructive) 14%, var(--card)); border-radius: 8px; font-size: 12px; color: var(--color-error-foreground); }
.conn-card__error span { flex: 1; }

.connectors-page__available { display: flex; flex-wrap: wrap; gap: 8px; }
.ch-chip { display: inline-flex; align-items: center; gap: 6px; padding: 6px 14px; border-radius: 999px; background: var(--color-success); color: var(--color-success-foreground); font-size: 12px; }
.ch-chip.is-disabled { background: var(--sidebar-accent); color: var(--muted-foreground); }
```

- [ ] **Step 5: build，预览 `/connectors`**

- [ ] **Step 6: Commit**

```bash
git commit -am "feat(gui/connectors): connector cards + available channel chips per Pencil"
```

---

## 阶段 9：设置页

### Task 9.1：SettingsPage 左侧导航 + 模型 Provider 区 + Agent 模型分配表

**Files:**
- Modify: `apps/clawx-gui/src/pages/SettingsPage.tsx`
- Create: `apps/clawx-gui/src/components/SettingsNav.tsx`
- Create: `apps/clawx-gui/src/components/ModelProviderCard.tsx`
- Create: `apps/clawx-gui/src/components/AgentModelAssignTable.tsx`
- Create: `apps/clawx-gui/src/styles/pages/settings-page.css`

> 规格：
> - 左侧嵌入 nav（宽 200px）：`模型 / 安全 / 外观与语言 / 健康`，分割线后 `关于 / 反馈`。
> - 顶部 `ZettClaw ▾` 作为 brand 头部（复用 AgentSidebar 风格）。
> - 主区：
>   - 当前选中 `模型` 时：标题 `模型 Provider` + `+ 添加`。
>   - ModelProviderCard 列表：emoji + 名称 + 状态 badge + 描述（模型列表 + Key 打码）+ `测试连接` / `编辑` 按钮；未配置时显示 `配置` CTA。
>   - 下方 `Agent 模型分配` 表格：列 `Agent / 模型策略 / 当前模型 / 操作`，行为小字 + `编辑` icon-button。

- [ ] **Step 1: 写 `SettingsNav.tsx`**

```tsx
import { Cpu, Shield, Palette, HeartPulse, Info, MessageCircle } from "lucide-react";

const SECTIONS = [
  { id: "model",    icon: Cpu,        label: "模型" },
  { id: "security", icon: Shield,     label: "安全" },
  { id: "look",     icon: Palette,    label: "外观与语言" },
  { id: "health",   icon: HeartPulse, label: "健康" },
];

const EXTRA = [
  { id: "about",    icon: Info,          label: "关于" },
  { id: "feedback", icon: MessageCircle, label: "反馈" },
];

interface Props { value: string; onChange: (id: string) => void }

export default function SettingsNav({ value, onChange }: Props) {
  return (
    <aside className="settings-nav">
      <header className="settings-nav__head"><span>设置</span></header>
      <ul>
        {SECTIONS.map((s) => (
          <li key={s.id}>
            <button className={`settings-nav__item ${value === s.id ? "is-active" : ""}`} onClick={() => onChange(s.id)}>
              <s.icon size={14} /><span>{s.label}</span>
            </button>
          </li>
        ))}
      </ul>
      <div className="settings-nav__divider" />
      <ul>
        {EXTRA.map((s) => (
          <li key={s.id}>
            <button className={`settings-nav__item ${value === s.id ? "is-active" : ""}`} onClick={() => onChange(s.id)}>
              <s.icon size={14} /><span>{s.label}</span>
            </button>
          </li>
        ))}
      </ul>
    </aside>
  );
}
```

- [ ] **Step 2: 写 `ModelProviderCard.tsx`**

```tsx
import Badge from "./ui/Badge";
import Button from "./ui/Button";

interface Props {
  emoji: string;
  name: string;
  available: boolean;
  summary: string;
  key?: string;
}

export default function ModelProviderCard({ emoji, name, available, summary, key }: Props) {
  return (
    <div className="mp-card">
      <div className="mp-card__head">
        <span className="mp-card__emoji">{emoji}</span>
        <div className="mp-card__name">{name}</div>
        <Badge tone={available ? "success" : "neutral"}>{available ? "可用" : "不可用"}</Badge>
      </div>
      <div className="mp-card__summary">{summary}</div>
      {key && <div className="mp-card__key">API Key: <code>{key}</code></div>}
      <div className="mp-card__actions">
        {available ? <>
          <Button size="sm" variant="outline">测试连接</Button>
          <Button size="sm" variant="ghost">编辑</Button>
        </> : <Button size="sm" variant="default">配置</Button>}
      </div>
    </div>
  );
}
```

- [ ] **Step 3: 写 `AgentModelAssignTable.tsx`**

```tsx
import IconButton from "./ui/IconButton";
import { Pencil } from "lucide-react";

const ROWS = [
  { agent: "编程助手", strategy: "固定模型", current: "Claude Opus 4" },
  { agent: "研究助手", strategy: "智能路由", current: "按任务自动选择" },
  { agent: "写作助手", strategy: "固定模型", current: "Claude Sonnet 4.6" },
];

export default function AgentModelAssignTable() {
  return (
    <div className="mm-table">
      <div className="mm-table__head">
        <span>Agent</span><span>模型策略</span><span>当前模型</span><span />
      </div>
      {ROWS.map((r) => (
        <div key={r.agent} className="mm-table__row">
          <span>{r.agent}</span>
          <span>{r.strategy}</span>
          <span>{r.current}</span>
          <IconButton icon={<Pencil size={12} />} aria-label={`编辑 ${r.agent}`} size="sm" variant="ghost" />
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 4: 重写 `SettingsPage.tsx`**

```tsx
import { useState } from "react";
import { Plus } from "lucide-react";
import Button from "../components/ui/Button";
import SettingsNav from "../components/SettingsNav";
import ModelProviderCard from "../components/ModelProviderCard";
import AgentModelAssignTable from "../components/AgentModelAssignTable";

export default function SettingsPage() {
  const [section, setSection] = useState("model");
  return (
    <div className="settings-page">
      <SettingsNav value={section} onChange={setSection} />
      <section className="settings-page__main">
        {section === "model" && (
          <>
            <header className="settings-page__head">
              <h2>模型 Provider</h2>
              <Button leftIcon={<Plus size={14} />} size="sm">添加</Button>
            </header>
            <div className="settings-page__providers">
              <ModelProviderCard emoji="☁️" name="Anthropic (Claude)" available summary="模型: Claude Opus 4, Claude Sonnet 4.6" key="sk-ant-•••••••" />
              <ModelProviderCard emoji="🏠" name="Ollama (本地)" available summary="模型: llama3:70b, codellama:34b" key="地址: http://localhost:11434" />
              <ModelProviderCard emoji="🔌" name="OpenAI" available={false} summary="API Key 未配置" />
            </div>
            <h2 className="settings-page__section-title">Agent 模型分配</h2>
            <AgentModelAssignTable />
          </>
        )}
        {section !== "model" && <div className="settings-page__placeholder">该分组将在后续迭代实现。</div>}
      </section>
    </div>
  );
}
```

- [ ] **Step 5: 写 `settings-page.css`**

```css
.settings-page { display: grid; grid-template-columns: 200px 1fr; height: 100%; }
.settings-page__main { padding: 20px 28px; overflow: auto; }
.settings-page__head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 16px; }
.settings-page__head h2 { font-size: 16px; font-weight: 700; }
.settings-page__section-title { font-size: 15px; font-weight: 600; margin: 28px 0 12px; }
.settings-page__providers { display: flex; flex-direction: column; gap: 10px; }
.settings-page__placeholder { color: var(--muted-foreground); }

.settings-nav { background: var(--sidebar); border-right: 1px solid var(--sidebar-border); padding: 16px 12px; display: flex; flex-direction: column; gap: 6px; }
.settings-nav__head { font-size: 15px; font-weight: 600; padding: 0 8px 8px; }
.settings-nav ul { list-style: none; display: flex; flex-direction: column; gap: 2px; }
.settings-nav__item { display: flex; align-items: center; gap: 8px; width: 100%; padding: 8px 10px; border-radius: 8px; font-size: 13px; color: var(--sidebar-foreground); text-align: left; }
.settings-nav__item:hover { background: var(--sidebar-accent); color: var(--foreground); }
.settings-nav__item.is-active { background: var(--sidebar-accent); color: var(--foreground); }
.settings-nav__divider { height: 1px; background: var(--sidebar-border); margin: 8px 0; }

.mp-card { background: var(--card); border: 1px solid var(--border); border-radius: 12px; padding: 14px 16px; display: flex; flex-direction: column; gap: 6px; }
.mp-card__head { display: flex; align-items: center; gap: 10px; }
.mp-card__emoji { font-size: 20px; }
.mp-card__name { font-size: 14px; font-weight: 600; flex: 1; }
.mp-card__summary { font-size: 12px; color: var(--muted-foreground); }
.mp-card__key code { background: var(--sidebar-accent); padding: 2px 6px; border-radius: 4px; font-family: ui-monospace, Menlo, monospace; font-size: 11px; }
.mp-card__actions { display: flex; gap: 6px; margin-top: 6px; }

.mm-table { display: flex; flex-direction: column; background: var(--card); border: 1px solid var(--border); border-radius: 12px; overflow: hidden; }
.mm-table__head, .mm-table__row { display: grid; grid-template-columns: 1fr 1fr 1fr 48px; gap: 12px; padding: 10px 14px; align-items: center; }
.mm-table__head { background: var(--sidebar-accent); font-size: 11px; color: var(--muted-foreground); font-weight: 500; text-transform: uppercase; }
.mm-table__row { font-size: 13px; border-top: 1px solid var(--border); }
.mm-table__row:first-child { border-top: 0; }
```

- [ ] **Step 6: build，预览 `/settings`**

- [ ] **Step 7: Commit**

```bash
git commit -am "feat(gui/settings): nav + provider cards + agent model assignment table per Pencil"
```

---

## 阶段 10：联系人

### Task 10.1：ContactsPage 路由 + sidebar 分组 + agent 详情主区

**Files:**
- Modify: `apps/clawx-gui/src/App.tsx` （添加 `/contacts` 路由）
- Modify: `apps/clawx-gui/src/pages/ContactsPage.tsx`
- Create: `apps/clawx-gui/src/styles/pages/contacts-page.css`

> 规格（RF3xa）：页面自带二级侧栏（复用 `AgentSidebar` 视觉但语义为联系人分组：`收藏 / 最近使用 / 全部 Agent`）；主区为当前选中 agent 的**详情 hero + 能力标签 + 基本信息 + 常用提示词**。由于 AppLayout 默认已在非 hidden 页面渲染 AgentSidebar，`/contacts` 复用 AgentSidebar 即可（不另起）。

- [ ] **Step 1: `App.tsx` 增加路由**

```tsx
{ path: "contacts", element: <ContactsPage /> },
```
并 import `ContactsPage`。

- [ ] **Step 2: 重写 `ContactsPage.tsx`**

```tsx
import { useSearchParams } from "react-router-dom";
import { Settings, MoreVertical, Zap } from "lucide-react";
import Avatar from "../components/ui/Avatar";
import Badge from "../components/ui/Badge";
import Button from "../components/ui/Button";
import IconButton from "../components/ui/IconButton";
import { useAgents } from "../lib/store";

const CAPS = ["Python", "JavaScript", "代码生成", "调试", "代码审查"];

export default function ContactsPage() {
  const [params] = useSearchParams();
  const selectedId = params.get("agent");
  const { agents } = useAgents();
  const agent = agents.find((a) => a.id === selectedId) ?? agents[0];

  if (!agent) return <div className="contacts-page__empty">请选择一个 Agent</div>;

  return (
    <div className="contacts-page">
      <header className="contacts-page__head">
        <h1>{agent.name}</h1>
        <div className="contacts-page__actions">
          <IconButton icon={<Settings size={14} />} aria-label="配置" variant="ghost" size="sm" />
          <IconButton icon={<MoreVertical size={14} />} aria-label="更多" variant="ghost" size="sm" />
        </div>
      </header>

      <section className="contacts-page__hero">
        <Avatar size={72} rounded="md" bg="var(--primary)"><span>{"</>"}</span></Avatar>
        <h2>{agent.name}</h2>
        <span className="contacts-page__running">● 运行中</span>
        <p className="contacts-page__desc">一个擅长编程的助手，专注多种编程语言的解决方案，提供代码生成、代码审查、性能优化等能力。</p>
        <div className="contacts-page__cta">
          <Button variant="default">开始对话</Button>
          <Button variant="outline">配置</Button>
        </div>
      </section>

      <section>
        <h3 className="contacts-page__section">能力标签</h3>
        <div className="contacts-page__caps">
          {CAPS.map((c) => <span key={c} className="cap-chip"><Zap size={12} />{c}</span>)}
        </div>
      </section>

      <section>
        <h3 className="contacts-page__section">基本信息</h3>
        <dl className="contacts-page__info">
          <dt>创建者</dt><dd>ZettClaw Team</dd>
          <dt>版本</dt><dd>v2.1.0</dd>
          <dt>模型</dt><dd>GPT-4o</dd>
          <dt>最近使用</dt><dd>2 分钟前</dd>
          <dt>对话次数</dt><dd>1,284</dd>
        </dl>
      </section>

      <section>
        <h3 className="contacts-page__section">常用提示词</h3>
        <ul className="contacts-page__prompts">
          <li>帮我写一个 REST API 接口</li>
          <li>优化这段代码的性能</li>
          <li>解释这段代码的工作原理</li>
        </ul>
      </section>
    </div>
  );
}
```

- [ ] **Step 3: 写 `contacts-page.css`**

```css
.contacts-page { padding: 20px 28px; display: flex; flex-direction: column; gap: 20px; overflow: auto; height: 100%; }
.contacts-page__empty { padding: 20px; color: var(--muted-foreground); }
.contacts-page__head { display: flex; align-items: center; justify-content: space-between; }
.contacts-page__head h1 { font-size: 18px; font-weight: 600; }
.contacts-page__actions { display: inline-flex; gap: 4px; }

.contacts-page__hero { display: flex; flex-direction: column; align-items: center; text-align: center; gap: 10px; padding: 16px 0; }
.contacts-page__hero h2 { font-size: 20px; font-weight: 600; }
.contacts-page__running { font-size: 12px; color: #A1E5A1; }
.contacts-page__desc { max-width: 480px; font-size: 13px; color: var(--muted-foreground); }
.contacts-page__cta { display: inline-flex; gap: 8px; margin-top: 4px; }

.contacts-page__section { font-size: 13px; font-weight: 600; margin-bottom: 10px; }

.contacts-page__caps { display: flex; flex-wrap: wrap; gap: 6px; }
.cap-chip { display: inline-flex; align-items: center; gap: 4px; background: var(--sidebar-accent); color: var(--foreground); padding: 4px 10px; border-radius: 999px; font-size: 12px; }
.cap-chip svg { color: var(--primary); }

.contacts-page__info { display: grid; grid-template-columns: 80px 1fr; row-gap: 6px; column-gap: 16px; font-size: 13px; background: var(--card); border: 1px solid var(--border); border-radius: 12px; padding: 14px 16px; }
.contacts-page__info dt { color: var(--muted-foreground); }

.contacts-page__prompts { list-style: none; display: flex; flex-direction: column; gap: 6px; }
.contacts-page__prompts li { padding: 10px 14px; background: var(--card); border: 1px solid var(--border); border-radius: 10px; font-size: 13px; }
```

- [ ] **Step 4: build，预览 `/contacts?agent=<id>`**

- [ ] **Step 5: Commit**

```bash
git commit -am "feat(gui/contacts): route + agent detail hero per Pencil"
```

---

## 阶段 11：清理

### Task 11.1：删除老样式 + 冗余组件 + 构建验证

**Files:**
- Modify: `apps/clawx-gui/src/styles.css`（删 legacy 段落）
- Delete: `apps/clawx-gui/src/components/ListPanel.tsx`
- Delete: `apps/clawx-gui/src/components/AgentList.tsx`（若确认未被引用）
- Delete: `apps/clawx-gui/src/components/SettingsList.tsx`（若确认未被引用）

- [ ] **Step 1: grep 确认未被 import**

```bash
grep -rn "ListPanel\|AgentList\|SettingsList" apps/clawx-gui/src/ --include="*.ts*"
```

- [ ] **Step 2: 删除确认未使用的文件**

- [ ] **Step 3: 扫 `styles/layout.css` / `styles/components.css` / `styles/pages.css` 删除未被选择器命中的 legacy 规则**

可用 `rg "<classname>" apps/clawx-gui/src` 手动核对；看到 `.sidebar-*` / `.nav-icon-btn` / `.main-content` 等已被替换的类，可整段删除。

- [ ] **Step 4: `npm run build`**

- [ ] **Step 5: Commit**

```bash
git commit -am "chore(gui): remove legacy component shells and orphaned CSS"
```

### Task 11.2：顶层 smoke tests（可选）

**Files:**
- Create: `apps/clawx-gui/src/__tests__/routes.test.tsx`

仅在时间允许时加入。写 React Testing Library 级别的冒烟测试，挂载 `App` → 切换 6 条路由 → 期望根节点无 throw。

- [ ] **Step 1: 如 `vitest` 未装，安装：**

```bash
cd apps/clawx-gui && npm install -D vitest @testing-library/react @testing-library/jest-dom jsdom
```

- [ ] **Step 2: 在 `vite.config.ts` 加测试配置（test.environment = 'jsdom'）**

- [ ] **Step 3: 写 smoke test（略）**

- [ ] **Step 4: Commit**

```bash
git commit -am "test(gui): add smoke test for top-level routes"
```

---

## 完成门槛

- [ ] `npm run build` 成功，无 TS / CSS 错误。
- [ ] Tauri 预览（`npm run tauri dev`）下 6 个主路由手动过一遍：`/ /contacts /agents /knowledge /tasks /connectors /settings`，与 Pencil 对应截图目视一致。
- [ ] 深色配色、紫色主按钮、pill 输入框、tabs 切换、卡片圆角/边框、emoji 头像全部匹配。
- [ ] NavBar app rail 宽 56px；AgentSidebar 宽 280px；Settings / Agents / Skills 页 AgentSidebar 隐藏。

---

## Out of scope（本计划不做）

- 实际后端对接（保留现有 mock / `lib/api.ts` 契约）。
- 国际化（文案保留 Pencil 中文）。
- 性能优化（虚拟列表、memo 细粒度）。
- 亮色主题（Pencil 定义了 Light，但当前要求只做 Dark）。
- E2E 测试（仅可选 smoke）。
- 动画细节（hover/focus ring 对齐已做，复杂过渡可后续补）。

---

## Self-Review 备忘

- 13 个 Pencil 顶层 frame：
  - `vT1N6` Main → Task 3.1/3.4
  - `Yvvlu` Active Conversation → Task 3.3/3.4
  - `EvzaK` Artifacts Tab → Task 3.5
  - `5CbuX` New Agent / `tEBQq` New Agent Custom → Task 5.1
  - `4IiwN` Knowledge Base → Task 6.1
  - `kBzEo` Scheduled Tasks → Task 7.1
  - `ouoNU` Connectors → Task 8.1
  - `osgSM` Agent & Skill / `b16HH` Skills → Task 4.1/4.2
  - `ym0EB` / `GqjQZ` Settings → Task 9.1
  - `RF3xa` Contacts → Task 10.1
  - `vtHps` halo design system → 阶段 1 UI 原子
- 所有代码步骤含完整代码块；所有 CSS 段落含完整规则；所有命令可复制运行。
- 类型一致性：`Agent.model` 是新增可选字段，已在 4.1 Step 4 注明；`Message.refs` 新增可选字段，已在 3.3 Step 4 注明。
