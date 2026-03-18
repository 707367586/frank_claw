# ClawX Crate 依赖关系图

**版本:** 4.1
**日期:** 2026年3月18日

---

## 1. 依赖层级总览

Crate 按依赖深度分为 5 层，上层依赖下层，禁止反向依赖。

```
Layer 0 (Foundation)     clawx-types
                              │
Layer 1 (Config/Infra)   clawx-config    clawx-eventbus    clawx-hal
                           │    │              │              │
Layer 2 (Domain)        clawx-llm  clawx-security  clawx-vault  clawx-scheduler
                          │  │         │               │           │
                          │  │    clawx-channel   clawx-artifact  │
                          │  │         │                    │      │
Layer 3 (Services)   clawx-memory  clawx-kb  clawx-skills  │   clawx-ota
                                     │ (hal)               │      │ (hal)
                          │          │            │
Layer 4 (Runtime)         └──────────┴────────────┘
                              clawx-runtime
                                   │
Layer 5 (API/Apps)          clawx-api   clawx-controlplane-client
                              │              │          │
                        clawx-service   clawx-ffi   clawx-cli
```

**说明:**
- `clawx-daemon` 已移除，健康自检内置于 `clawx-service`（ADR-005）
- `clawx-gateway` 已移除，IM 路由内置于 `clawx-channel`（ADR-021）
- `clawx-ffi` / `clawx-cli` 通过 `clawx-controlplane-client` 访问 API，不直接依赖 runtime（ADR-004）

---

## 2. 详细依赖矩阵

### clawx-types (Layer 0)
```
依赖: (无内部依赖)
外部: serde, serde_json, chrono, uuid, thiserror, async-trait
被依赖: 所有其他 crate
```

### clawx-config (Layer 1)
```
依赖: clawx-types
外部: serde, toml, thiserror, tracing
被依赖: clawx-llm, clawx-security, clawx-service, clawx-cli
```

### clawx-eventbus (Layer 1)
```
依赖: clawx-types
外部: tokio, tracing, async-trait
被依赖: clawx-security, clawx-memory, clawx-vault, clawx-kb, clawx-skills,
        clawx-scheduler, clawx-channel, clawx-artifact, clawx-service
说明: v0.1 暂不启用，v0.2 才引入 (ADR-007)
```

### clawx-hal (Layer 1)
```
依赖: clawx-types
外部: tokio, async-trait, tracing
被依赖: clawx-security, clawx-kb, clawx-ota
说明: 封装 FSEvents/Keychain/Notification 等 macOS 宿主能力 (ADR-019)
```

### clawx-llm (Layer 2)
```
依赖: clawx-types, clawx-config
外部: tokio, async-trait, reqwest, serde, serde_json, tracing, futures
被依赖: clawx-memory, clawx-kb, clawx-runtime
```

### clawx-security (Layer 2)
```
依赖: clawx-types, clawx-config, clawx-eventbus, clawx-hal
外部: tokio, async-trait, regex, tracing, chrono
被依赖: clawx-skills, clawx-runtime
说明: 通过 clawx-hal 访问 Keychain 实现 L3 凭证注入 (ADR-016, ADR-019)
```

### clawx-vault (Layer 2)
```
依赖: clawx-types, clawx-eventbus
外部: tokio, tracing, chrono
被依赖: clawx-runtime
```

### clawx-scheduler (Layer 2)
```
依赖: clawx-types, clawx-eventbus
外部: tokio, async-trait, tracing
被依赖: (通过 eventbus 间接驱动 runtime)
说明: v0.2 扩展执行层 (ADR-021)
```

### clawx-channel (Layer 2)
```
依赖: clawx-types, clawx-eventbus
外部: tokio, async-trait, tracing
被依赖: clawx-runtime (v0.2)
说明: v0.2 扩展执行层；IM 消息路由作为内部功能，不再拆分 gateway crate (ADR-021)
```

### clawx-artifact (Layer 2)
```
依赖: clawx-types, clawx-eventbus
外部: tokio, tracing
被依赖: (通过 eventbus 间接与 runtime 交互)
说明: v0.3+ 平台服务层 (ADR-022)
```

### clawx-memory (Layer 3)
```
依赖: clawx-types, clawx-llm, clawx-eventbus
外部: tokio, async-trait, sqlx, serde, serde_json, tracing, chrono, uuid
被依赖: clawx-runtime
说明: v0.1 负责 Long-Term 持久化记忆 (Agent/User Memory)；v0.2 新增 Short-Term Memory；Working Memory 由 clawx-runtime 实现 (ADR-009, ADR-010)
```

### clawx-kb (Layer 3)
```
依赖: clawx-types, clawx-llm, clawx-eventbus, clawx-hal
外部: tokio, async-trait, tracing
被依赖: clawx-runtime
说明: 通过 clawx-hal 获取 FSEvents 文件监控能力 (ADR-019)
```

### clawx-skills (Layer 3)
```
依赖: clawx-types, clawx-security, clawx-eventbus
外部: tokio, async-trait, tracing
被依赖: clawx-runtime (v0.2)
说明: v0.2 扩展执行层 (ADR-021)
```

### clawx-ota (Layer 3)
```
依赖: clawx-types, clawx-hal
外部: tokio, tracing
被依赖: (独立模块)
说明: v0.3+ 平台服务层 (ADR-022)
```

### clawx-runtime (Layer 4)
```
依赖: clawx-types, clawx-llm, clawx-memory, clawx-security, clawx-eventbus, clawx-vault
外部: tokio, async-trait, serde, serde_json, tracing, uuid
被依赖: clawx-api, clawx-service
说明: 包含 Working Memory 管理（上下文窗口、压缩、Prompt 组装）(ADR-010)
```

### clawx-api (Layer 5)
```
依赖: clawx-types, clawx-runtime
外部: tokio, axum, tracing
被依赖: clawx-service, clawx-controlplane-client (作为 server 端)
```

### clawx-controlplane-client (Layer 5)
```
依赖: clawx-types
外部: tokio, reqwest (UDS), serde, serde_json, tracing
被依赖: clawx-ffi, clawx-cli
说明: 本地控制平面客户端共享库，通过 UDS/HTTP 连接 clawx-api (ADR-004)
```

### clawx-ffi (Layer 5)
```
依赖: clawx-types, clawx-controlplane-client
外部: tokio, tracing
被依赖: SwiftUI GUI (编译时链接)
说明: 不直接依赖 runtime，通过 controlplane-client 间接访问 (ADR-004)
```

### clawx-service (App)
```
依赖: clawx-types, clawx-runtime, clawx-api, clawx-config, clawx-eventbus
外部: tokio, tracing, tracing-subscriber, anyhow
说明: 后台主进程，由 launchd 守护，内含健康自检 (ADR-005)
```

### clawx-cli (App)
```
依赖: clawx-types, clawx-controlplane-client, clawx-config
外部: tokio, tracing, tracing-subscriber, anyhow, clap
说明: 不直接依赖 runtime，通过 controlplane-client 间接访问 (ADR-004)
```

---

## 3. 依赖规则

### 3.1 禁止的依赖方向

| 规则 | 说明 |
|------|------|
| 下层不依赖上层 | types 不得依赖任何其他 crate |
| Domain 不依赖 API | 领域层不感知接口层 |
| Config/Infra (Layer 1) 不依赖 Domain (Layer 2+) | config/eventbus/hal 不直接依赖 llm/security/vault 等上层 |
| 模块间通过 EventBus 解耦 | 避免 scheduler → runtime 直接依赖 |
| ffi/cli 不直接依赖 runtime | 统一通过 controlplane-client (ADR-004) |

### 3.2 共享类型规则

- 所有跨 crate 的类型定义在 `clawx-types` 中
- Crate 内部类型不暴露给其他 crate
- Trait 接口定义在 `clawx-types`，实现在各 crate

### 3.3 外部依赖管理

- 所有外部依赖版本在 workspace `Cargo.toml` 中统一管理
- 避免重复引入功能相同的外部 crate
- 安全审计：定期运行 `cargo audit`
