# Backend Decoupling + HTTP API Layer Plan (Optimized v2)

> **For agentic workers:** 优先按阶段执行，不要大爆炸改造。每个阶段必须满足“可编译 + 可回滚 + 有验收”后再进入下一阶段。

**Goal**
- 将后端核心能力从 Tauri 壳层解耦，支持 `yeek-tauri` 与 `yeek-server` 共享同一套 core。
- 新增 HTTP API + SSE 事件通道，供 Electron/浏览器模式复用。
- 保持现有 Tauri 功能和行为不回退。

**Non-Goals (当前阶段不做)**
- 不重写存储层为完全 async ORM。
- 不在本阶段引入 OpenAPI 代码生成（先做合同一致性测试）。

---

## 0) 关键约束与设计原则

- **禁止 `unsafe` 下转型**：SSE 订阅不能通过 trait object 指针强转。
- **单一核心**：`core` 不依赖 `tauri` / `axum`，仅包含业务与存储编排。
- **桥接层最薄**：`tauri_bridge` 和 `http` 只做参数映射、错误映射、序列化。
- **可编译矩阵**：必须通过以下三档：
  - `cargo check`（默认 tauri）
  - `cargo check --no-default-features --features http-server`
  - `cargo check --all-features`
- **增量迁移**：每阶段只改一类耦合，避免整文件替换。

---

## 1) 目标结构（先定结构再迁移）

```text
src-tauri/
  Cargo.toml
  build.rs
  src/
    lib.rs
    main.rs                      # tauri entry
    bin/server.rs                # http entry

    core/
      mod.rs
      commands.rs                # 纯函数入口（无 tauri/axum）
      state.rs
      errors.rs
      events.rs                  # EventEmitter trait + payload

    tauri_bridge/
      mod.rs
      emitter.rs                 # TauriEventEmitter
      commands.rs                # #[tauri::command] thin wrappers

    http/
      mod.rs
      app_state.rs               # HttpRuntimeState { app_state, sse }
      emitter.rs                 # SseEventEmitter（可订阅）
      dto.rs                     # 强类型请求/响应 DTO
      error.rs                   # AppError -> HTTP status/json
      routes.rs                  # axum handlers + router

    adapter/
    domain/
    service/
    store/
    sync/
```

---

## 2) Phase A - Core 解耦（不引入 HTTP）

**目标**：先把 Tauri 依赖从核心逻辑中剥离，确保现有桌面能力不变。

### A1. 引入 `EventEmitter` trait（core/events.rs）
- [ ] 定义 `EventEmitter: Send + Sync`，保留当前 4 个事件方法。
- [ ] payload 类型迁移到 `core/events.rs`，避免跨模块循环依赖。

### A2. `AppState` 去 Tauri 依赖（core/state.rs）
- [ ] `app_handle` 替换为 `Arc<dyn EventEmitter>`。
- [ ] 增加 `emitter()` accessor。
- [ ] 仅调整构造参数，不改业务语义。

### A3. `sync/background.rs` 与 `sync/watcher.rs` 改用 trait
- [ ] 所有 `AppHandle.emit(...)` 改为 `emitter.emit_*`。
- [ ] 保持 scan guard 语义一致。

### A4. 将命令主体迁移到 `core/commands.rs`
- [ ] 每个命令拆成 `core` 纯函数。
- [ ] 原 `app/commands.rs` 暂保留为兼容层（后续迁移到 `tauri_bridge/commands.rs`）。

**验收**
- [ ] `cargo check` 通过。
- [ ] `cargo tauri dev` 功能回归通过（手测：列表、搜索、删除、rescan、插件页）。

---

## 3) Phase B - Tauri 桥接层瘦身

**目标**：Tauri 只做桥接，不再承载业务逻辑。

### B1. 新建 `tauri_bridge/commands.rs`
- [ ] 放置全部 `#[tauri::command]`，内部仅调用 `core::commands::*`。
- [ ] 参数映射仅做字段重命名，不做业务判断。

### B2. 新建 `tauri_bridge/emitter.rs`
- [ ] `TauriEventEmitter` 实现 `EventEmitter`。

### B3. `lib.rs` 只负责启动装配
- [ ] 初始化 DB、watcher、state。
- [ ] `invoke_handler` 仅引入 `tauri_bridge::commands::*`。

**验收**
- [ ] `cargo check` 通过。
- [ ] Tauri 行为与 Phase A 一致。

---

## 4) Phase C - HTTP 服务（类型安全 + 无 unsafe）

**目标**：上线 `yeek-server`，并确保 SSE/REST 安全可维护。

### C1. Feature 与 build 闭环
- [ ] `Cargo.toml`：
  - `default = ["tauri-shell"]`
  - `tauri-shell` / `http-server` 分离。
- [ ] `build.rs` 增加 feature gating：
  - 仅在 `tauri-shell` 下执行 tauri build 逻辑。
- [ ] `lib.rs` 模块与 `run()` 全部 `#[cfg(feature = "tauri-shell")]`。

### C2. `http::dto` 强类型请求
- [ ] 禁止 `Json<Value>` + `unwrap_or("")`。
- [ ] 所有写接口使用 `#[derive(Deserialize)]` DTO。
- [ ] query 参数使用 typed structs。

### C3. `http::error` 统一错误映射
- [ ] `AppError::Validation` -> 400。
- [ ] `AppError::NotFound` -> 404。
- [ ] 并发冲突（scan already running）-> 409。
- [ ] 统一 JSON 结构：`{ code, message, details? }`。

### C4. `SseEventEmitter` + `HttpRuntimeState`（关键）
- [ ] `SseEventEmitter` 持有 `broadcast::Sender<SseEnvelope>`。
- [ ] `HttpRuntimeState`：
  - `app_state: Arc<AppState>`
  - `sse: Arc<SseEventEmitter>`
- [ ] SSE handler 直接 `state.sse.subscribe()`，**不做任何 downcast/unsafe**。

### C5. server entry（`src/bin/server.rs`）
- [ ] 启动 DB、watcher、startup scan。
- [ ] 绑定 `127.0.0.1:17321`（默认本地环回）。
- [ ] CORS 默认非 permissive（见 Phase E）。

**验收**
- [ ] `cargo check --no-default-features --features http-server`。
- [ ] `cargo build --no-default-features --features http-server --bin yeek-server`。
- [ ] `curl /api/system/status`、`/api/sessions`、`/api/events` 可用。

---

## 5) Phase D - 前端传输抽象（合同一致性优先）

**目标**：前端同一套 API 调用代码可跑在 Tauri invoke 与 HTTP fetch。

### D1. `src/lib/transport.ts`
- [ ] 定义 `Transport` 接口：`command<T>(name, args)`。
- [ ] `TauriTransport` + `HttpTransport`。

### D2. `src/lib/api.ts` 全量迁移到 transport
- [ ] 不允许直接 `invoke()`。
- [ ] 每个方法保持原签名，内部换 transport。

### D3. `src/lib/events.ts`
- [ ] `TauriEventTransport` + `SseEventTransport`。
- [ ] 页面监听统一通过 `getEventTransport().on()`。

### D4. 合同一致性测试（先于代码生成）
- [ ] 增加“命令名 <-> HTTP 路由” parity tests：
  - 缺失映射即测试失败。
  - 重名或路径参数不一致即失败。

**验收**
- [ ] `npm run build`。
- [ ] Tauri 模式 UI 全流程可用。
- [ ] HTTP 模式 UI 全流程可用（至少核心页面）。

---

## 6) Phase E - 性能与安全加固

### E1. DB 访问并发策略（HTTP 模式）
- [ ] 将阻塞 DB 操作放入 `tokio::task::spawn_blocking` 或独立 DB worker。
- [ ] 对高频读接口（sessions/search）记录 P95。

### E2. CORS 最小权限
- [ ] 默认仅允许 `http://localhost:*` 与 `tauri://localhost`（按实际需要配置）。
- [ ] 提供环境变量扩展白名单。

### E3. SSE 健壮性
- [ ] `keep-alive`、backpressure（channel 容量与丢弃策略）明确。
- [ ] 事件 envelope 固定：`{ event, payload, ts }`。

### E4. 可观测性
- [ ] 增加 request id、接口耗时日志。
- [ ] 明确错误码枚举，方便前端诊断。

**验收**
- [ ] 压测 smoke（并发 20/50）无明显阻塞雪崩。
- [ ] 安全自检（CORS、监听地址、错误暴露）通过。

---

## 7) CI/CD Gate（必须落地）

- [ ] Rust matrix:
  - `cargo check`
  - `cargo check --no-default-features --features http-server`
  - `cargo check --all-features`
- [ ] Frontend:
  - `npm run build`
- [ ] Integration smoke（可脚本化）：
  - 启动 `yeek-server`
  - `curl` 关键端点
  - SSE 事件连通测试

---

## 8) 回滚与发布策略

- [ ] 每个 Phase 单独 PR，禁止跨阶段混改。
- [ ] 每个 Phase 完成后打 tag（`decouple-phase-a` ...）。
- [ ] 发现回归时仅回滚当前 Phase PR。
- [ ] 版本策略：
  - Phase C 完成后可发布 `2.0.0-alpha.1`
  - Phase E 完成后再考虑 `beta`

---

## 9) 实施顺序（建议）

1. Phase A（core 解耦）
2. Phase B（tauri bridge 瘦身）
3. Phase C（http server）
4. Phase D（frontend dual transport）
5. Phase E（hardening）

> 这套顺序的核心价值：每一步都“可运行、可验证、可回退”，避免一次性大改导致调试面失控。
