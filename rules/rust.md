# Rust Coding Rules

## Style
- Follow `rustfmt` defaults — no custom config overrides
- Run `cargo clippy --all-targets --all-features -- -D warnings` with zero warnings
- Prefer `impl Trait` over `dyn Trait` when possible for performance
- Use `#[must_use]` on functions that return important values
- `use` 导入分组：`std` / 外部 crate / 本项目 crate，组间空行分隔
- 单个 `#[derive(...)]` 合并所有派生宏，不拆成多行

## Naming (Rust API Guidelines)
- 转换方法前缀有语义区分：
  - `as_` — 零开销借用转换（如 `as_bytes() -> &[u8]`）
  - `to_` — 可能分配内存的转换（如 `to_string()`）
  - `into_` — 消费所有权（如 `into_inner()`）
- Getter 方法省略 `get_` 前缀：用 `fn len()` 而非 `fn get_len()`
- 迭代器方法：`iter()` / `iter_mut()` / `into_iter()`
- 构造函数：`new()` 主构造、`with_*()` 带参数、`from_*()` 转换
- 缩写当作单个单词：`Uuid` 而非 `UUID`，`HttpClient` 而非 `HTTPClient`

## Type Design
- **Newtype 封装原始类型**：避免原始类型混淆（如 `Miles(f64)` vs `Kilometers(f64)`）
- **语义类型优于布尔参数**：函数参数用 enum 代替 `bool`，`Widget::new(Small, Round)` 优于 `Widget::new(true, false)`
- **Struct 字段默认 private**：用 getter/setter 方法暴露，保留未来修改空间
- **Trait bounds 放在 impl 上而非 struct 上**：struct 上的 bounds 是 breaking change
- **积极实现常用 trait**：`Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `Default`, `Display`（孤儿规则阻止下游添加）
- **实现 `From` 而非 `Into`**：`Into` 有 blanket impl，同理 `TryFrom` 优于 `TryInto`
- **Sealed trait 防止外部实现**：不希望下游实现的 trait 使用 private supertrait 密封

## Ownership & Lifetimes
- **参数接受借用类型**：`&str` 而非 `&String`，`&[T]` 而非 `&Vec<T>`，`&T` 而非 `&Box<T>`
- **调用方决定所有权**：需要所有权就 move，不需要就 borrow，不要 borrow 后 clone
- **避免为满足借用检查器而 clone**：如果 clone 只是为了编译通过，说明设计有问题
- **Struct 中尽量避免生命周期**：优先存储 owned 数据，仅短生命周期视图/迭代器用引用
- **用 `Cow<'_, str>`** 处理"有时借用有时拥有"的场景

## Error Handling
- Library code 用 `thiserror`，application code 用 `anyhow`
- 错误消息小写、无尾标点（遵循 `std::error::Error` 惯例）
- 用 `#[source]` / `#[from]` 保留错误链，永远不丢失根因
- Destructor (`Drop`) 不可失败 — 需要可失败清理时提供 `close() -> Result`
- 验证优先级：类型系统静态保证 > `Result` 动态检查 > `debug_assert!` > `_unchecked` 变体

## Async & Concurrency
- Use `tokio` as the async runtime (multi-threaded)
- Prefer `tokio::spawn` over `std::thread::spawn`
- CPU-intensive work goes to `tokio::task::spawn_blocking`
- Use `tokio::select!` for concurrent operations with cancellation
- **禁止在 `.await` 跨越点持有 mutex lock** — 会导致死锁或编译失败
- 短临界区优先用 `std::sync::Mutex`，仅需跨 `.await` 持锁时才用 `tokio::sync::Mutex`
- **禁止在 async task 中执行阻塞 I/O** — 会饿死整个 runtime 线程
- 异步测试用 `#[tokio::test]`，加 `flavor = "multi_thread"` 暴露竞态

## Performance
- 优先用迭代器链而非索引循环 — 零开销抽象，编译器自动融合优化
- 已知容量时用 `Vec::with_capacity` / `String::with_capacity` 预分配
- 避免 `collect()` 成 `Vec` 仅为再次迭代 — 直接链式调用
- `Copy` 类型用 `.copied()` 而非 `.cloned()`
- 泛型参数减少假设：接受 `impl AsRef<Path>` 而非 `&PathBuf`，`impl IntoIterator` 而非 `&[T]`

## Anti-Patterns (禁止)
- 用 `Deref` 模拟继承 — 只有智能指针才应实现 `Deref`
- 库代码返回 `Box<dyn Error>` — 调用方无法 match，必须定义 error enum
- `Rc`/`Arc` 循环引用 — 用 `Weak` 打断
- `..Default::default()` 初始化 struct — 容易静默忽略新增字段，显式填写所有字段

## Clippy 配置
```rust
// 在 lib.rs / main.rs 顶部:
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::arithmetic_side_effects)]
```

## Traits & Abstractions
- All pluggable backends (LLM, Memory, Tools) defined as traits in `clawx-core`
- Traits should be object-safe when possible (for `dyn` dispatch)
- Use `async_trait` macro for async trait methods

## Dependencies
- Prefer Rust-native crates over FFI bindings
- serde 等可选依赖用 feature flag 门控（如 `"serde"` feature）
- Key dependencies:
  - `tokio` — async runtime
  - `serde` / `serde_json` — serialization
  - `sqlx` — database (SQLite/PostgreSQL)
  - `qdrant-client` — vector DB
  - `tantivy` — full-text search
  - `wasmtime` — WASM sandbox
  - `axum` — HTTP API
  - `tracing` — structured logging
  - `thiserror` / `anyhow` — error handling

## Module Organization
- One crate per major module (workspace layout)
- `lib.rs` exports public API only
- Internal types stay in private submodules
- Shared types in `clawx-types` crate
