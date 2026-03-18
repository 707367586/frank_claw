# ClawX 安全架构 v4.0

**日期:** 2026-03-18

---

## 1. 设计原则

本地数据主权 │ 最小权限 │ 纵深防御 (12层) │ 零信任执行 │ 宿主边界注入 │ 密钥零化 │ 可审计 │ 安全默认

---

## 2. 威胁模型

| ID | 威胁 | 严重度 | 防御层 |
|----|------|--------|--------|
| T1 | Prompt 注入 | Critical | L1 + L5 |
| T2 | 敏感数据外泄 | Critical | L3 + L5 + L8 |
| T3 | 文件系统破坏 | High | L2 + L4 + L7 |
| T4 | 恶意 Skills | High | L2 + L3 + L4 + L10 |
| T5 | SSRF 攻击 | High | L6 |
| T6 | 供应链攻击 | High | L10 |
| T7 | Agent 执行循环 | Medium | L9 |
| T8 | 路径穿越 | High | L7 |
| T9 | 密钥内存残留 | Medium | L8 |
| T10 | DDoS/滥用 | Medium | L11 |
| T11 | 审计日志篡改 | Medium | L12 |

---

## 3. 分级执行模型 (L2)

> **术语说明:** T1/T2/T3 指**执行级别**（Execution Tier），用于描述代码运行的隔离程度。L1-L12 指**防御层**（Defense Layer），用于描述纵深防御体系的安全能力。两者是不同维度的概念，不要混淆。

| 级别 | 环境 | 权限 | 用途 |
|------|------|------|------|
| **T1 Sandboxed** | Wasmtime WASM + 双计量 | 无文件/网络/密钥，每次新 Store | Skills/Tools |
| **T2 Restricted** | 受限子进程 + 工作区隔离 | 命令白名单, env_clear() | Shell/Python |
| **T3 Native** | 原生宿主 | 逐次弹窗确认 | 系统配置等 |

**T1 双计量防 DoS**：燃料计量（WASM 指令数上限）+ 纪元中断（挂钟超时）。资源限制：内存 ≤ 256MB，超时默认 30s，HTTP 响应 ≤ 10MB。仅暴露 4 个 Host Function：`http_request`, `secret_exists`, `log`, `now_millis`。

---

## 4. 12 层纵深防御

### L1: Prompt 注入防御

三层过滤：模式匹配 (regex) → 内容净化 (转义+隔离+编码攻击检测) → LLM 自检 (可选)

### L2: WASM 双计量沙箱

见 §3 分级执行模型。

### L3: 宿主边界凭证注入

密钥永不进入沙箱。WASM Tool 通过占位符 `{SECRET_NAME}` 引用密钥，宿主侧在 HTTP 调用边界替换真实值。流程：

```
沙箱: 构造请求含占位符 → host function http_request()
宿主: 域名白名单检查 → Keychain 读取 (Zeroizing) → 替换占位符 → 发送 → DLP 扫描响应 → 零化 → 返回
```

`secret_exists(name)` 仅检查是否配置，不返回值。

### L4: 声明式权限能力模型

每个 Skill/Tool 通过 `capabilities.toml` 声明权限（net_http/secrets/fs_read/fs_write/exec_shell），安装时用户确认，运行时校验。默认全部禁止。

### L5: DLP + Aho-Corasick 泄漏检测

三个扫描节点：LLM 出站、Tool 输出、WASM HTTP 响应返回前。Aho-Corasick O(n) 多模式匹配检测 SSH 私钥、AWS Key、API Key、GitHub Token、JWT、PEM、连接字符串、PII 等。

**PII 脱敏上云**：`zhang@example.com` → `[EMAIL_1]`，LLM 返回后还原。

### L6: SSRF 防护 + 网络白名单

所有出站请求检查：私有 IP (RFC 1918) → 链路本地 → 环回 → 云元数据 (169.254.169.254) → IPv6 私有 → DNS 重绑定 (二次解析比对) → 重定向链每跳重验。白名单外默认禁止。

### L7: 路径穿越防护

`canonicalize()` + `starts_with(workspace_root)` + 符号链接目标检查。

### L8: 密钥零化

`Zeroizing<String>` 包装敏感数据，离开作用域自动清零。API Key 存 macOS Keychain，不持久化在配置/日志/错误消息中。自定义 `SecretString` 防 tracing 意外序列化。

### L9: 循环守卫 + 子进程沙箱

调用链哈希检测乒乓模式 (VecDeque 最近 20 次)。T2 子进程 `env_clear()` + 选择性传入 PATH/HOME/LANG，不传任何密钥环境变量。

### L10: Skill Ed25519 签名验证

安装时：下载包 → Ed25519 公钥验签 → SHA-256 哈希校验 → 用户确认权限 → 安装。运行时加载前再次校验哈希。

### L11: GCRA 速率限制

| 对象 | 限制 |
|------|------|
| REST API | 100 req/min per IP |
| LLM 调用 | 可配置 per Agent |
| WASM Tool HTTP | 100 req/60s per Tool |
| IM 消息 | 30 msg/min per Channel |

GCRA 纯计算，无需后台线程补充令牌。

### L12: 哈希链审计日志

```
~/.clawx/audit/{category}-{date}.jsonl
每条: { timestamp, agent_id, action, target, result, prev_hash, hash }
```

SHA-256 哈希链，追加写入不可修改。定期完整性校验，失败弹窗告警。健康端点未认证仅返回 status+version。

---

## 5. 高风险操作确认

批量文件删除 (>5) │ sudo/rm -rf/chmod │ 白名单外网络 │ 核心记忆修改 │ Skills 安装 │ DLP 检测泄漏 → macOS 原生弹窗确认

---

## 6. 阶段实施

| 阶段 | 安全能力 |
|------|---------|
| **v0.1** | L4 权限弹窗, L5 基础DLP, L6 网络白名单+SSRF, L7, L8, L11 基础限速, L12 审计+哈希链 |
| **v0.2** | L1 Prompt注入三层, L2 WASM双计量, L3 凭证注入, L5 Aho-Corasick, L9, L10 |
| **v0.3+** | Skills 四层检测, 高级异常检测, DNS重绑定+重定向链 |
