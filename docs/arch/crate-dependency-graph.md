# ClawX Crate 依赖关系图

**版本:** 3.0
**日期:** 2026年3月17日

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
                          │  │         │                           │
Layer 3 (Services)   clawx-memory  clawx-kb  clawx-skills  clawx-gateway  clawx-ota
                          │          │            │              │
Layer 4 (Runtime)         └──────────┴────────────┘              │
                              clawx-runtime                      │
                                   │                             │
Layer 5 (API/Apps)          clawx-api   clawx-ffi   clawx-daemon
                              │           │              │
                        clawx-service   clawx-cli
```

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
        clawx-scheduler, clawx-channel, clawx-artifact, clawx-daemon, clawx-service
```

### clawx-hal (Layer 1)
```
依赖: clawx-types
外部: tokio, async-trait, tracing
被依赖: clawx-ota
```

### clawx-llm (Layer 2)
```
依赖: clawx-types, clawx-config
外部: tokio, async-trait, reqwest, serde, serde_json, tracing, futures
被依赖: clawx-memory, clawx-kb, clawx-runtime
```

### clawx-security (Layer 2)
```
依赖: clawx-types, clawx-config, clawx-eventbus
外部: tokio, async-trait, regex, tracing, chrono
被依赖: clawx-skills, clawx-runtime
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
```

### clawx-channel (Layer 2)
```
依赖: clawx-types, clawx-eventbus
外部: tokio, async-trait, tracing
被依赖: clawx-gateway
```

### clawx-artifact (Layer 2)
```
依赖: clawx-types, clawx-eventbus
外部: tokio, tracing
被依赖: (通过 eventbus 间接与 runtime 交互)
```

### clawx-memory (Layer 3)
```
依赖: clawx-types, clawx-llm, clawx-eventbus
外部: tokio, async-trait, sqlx, serde, serde_json, tracing, chrono, uuid
被依赖: clawx-runtime
```

### clawx-kb (Layer 3)
```
依赖: clawx-types, clawx-llm, clawx-eventbus
外部: tokio, async-trait, tracing
被依赖: clawx-runtime (计划中)
```

### clawx-skills (Layer 3)
```
依赖: clawx-types, clawx-security, clawx-eventbus
外部: tokio, async-trait, tracing
被依赖: clawx-runtime (计划中, v0.2)
```

### clawx-gateway (Layer 3)
```
依赖: clawx-types, clawx-channel
外部: tokio, tracing
被依赖: clawx-api
```

### clawx-ota (Layer 3)
```
依赖: clawx-types, clawx-hal
外部: tokio, tracing
被依赖: (独立模块)
```

### clawx-runtime (Layer 4)
```
依赖: clawx-types, clawx-llm, clawx-memory, clawx-security, clawx-eventbus, clawx-vault
外部: tokio, async-trait, serde, serde_json, tracing, uuid
被依赖: clawx-api, clawx-ffi, clawx-daemon, clawx-service, clawx-cli
```

### clawx-api (Layer 5)
```
依赖: clawx-types, clawx-runtime, clawx-gateway
外部: tokio, axum, tracing
被依赖: (顶层模块)
```

### clawx-ffi (Layer 5)
```
依赖: clawx-types, clawx-runtime
外部: tokio, tracing
被依赖: SwiftUI GUI (编译时链接)
```

### clawx-daemon (Layer 5)
```
依赖: clawx-types, clawx-eventbus, clawx-runtime
外部: tokio, tracing
被依赖: clawx-service
```

### clawx-service (App)
```
依赖: clawx-types, clawx-runtime, clawx-config, clawx-daemon, clawx-eventbus
外部: tokio, tracing, tracing-subscriber, anyhow
```

### clawx-cli (App)
```
依赖: clawx-types, clawx-runtime, clawx-config
外部: tokio, tracing, tracing-subscriber, anyhow
```

---

## 3. 依赖规则

### 3.1 禁止的依赖方向

| 规则 | 说明 |
|------|------|
| 下层不依赖上层 | types 不得依赖任何其他 crate |
| Domain 不依赖 API | 领域层不感知接口层 |
| Infrastructure 不依赖 Domain | vault/hal/daemon 不直接依赖 memory/kb/skills |
| 模块间通过 EventBus 解耦 | 避免 scheduler → runtime 直接依赖 |

### 3.2 共享类型规则

- 所有跨 crate 的类型定义在 `clawx-types` 中
- Crate 内部类型不暴露给其他 crate
- Trait 接口定义在 `clawx-types`，实现在各 crate

### 3.3 外部依赖管理

- 所有外部依赖版本在 workspace `Cargo.toml` 中统一管理
- 避免重复引入功能相同的外部 crate
- 安全审计：定期运行 `cargo audit`
