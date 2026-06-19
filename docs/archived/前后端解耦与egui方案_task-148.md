# IRtool-next 前后端解耦 + egui Fallback 设计方案

## Context

当前 `irtool-tauri` crate 混合了 Tauri 框架绑定和业务编排逻辑：每个 `#[tauri::command]` 函数内含完整的状态访问、异步调用和事件发射代码。这导致前端与后端强耦合，无法在不依赖 Tauri/WebView2 的场景下使用替代 UI（如 egui 原生渲染）。

目标：抽取传输无关的 service 层，使 Tauri 和 egui 都能调用同一套业务逻辑。

---

## 新 Crate 结构

```
crates/
├── irtool-core/          # [不变]
├── irtool-net-monitor/   # [不变]
├── irtool-autoruns/      # [不变]
├── irtool-sysmon/        # [不变]
├── irtool-monitor/       # [不变]
├── irtool-pcap/          # [不变]
├── irtool-process/       # [不变]
├── irtool-rules/         # [不变]
├── irtool-threat-intel/  # [不变]
├── irtool-tools/         # [不变]
├── irtool-service/       # [新增] 服务层：DTO + AppContext + EventBus + Services
├── irtool-tauri/         # [改造] 薄代理 → 调用 irtool-service
└── irtool-egui/          # [后续新增] egui 原生前端
```

依赖关系：
```
irtool-tauri ──┐
               ├──► irtool-service ──► 所有业务 crates (core, net-monitor, autoruns, ...)
irtool-egui ───┘
```

`irtool-service` **不依赖** `tauri` 或 `eframe`，只依赖业务 crates + tokio + serde。

---

## Task 1: 创建 `irtool-service` crate

### 1.1 目录结构

```
crates/irtool-service/
├── Cargo.toml
└── src/
    ├── lib.rs            # pub use
    ├── context.rs        # AppContext（从 irtool-tauri/state.rs 迁移，去掉 Tauri 依赖）
    ├── event_bus.rs      # EventBus：tokio::sync::broadcast 统一事件
    ├── dto/              # DTO 类型（从 commands/*.rs 中搬出）
    │   ├── mod.rs
    │   ├── network.rs    # NetworkSnapshotPayload, NetworkPollingControl, RetentionPolicyDto, NetworkEnrichmentPayload
    │   ├── autoruns.rs
    │   ├── sysmon.rs
    │   ├── monitor.rs
    │   ├── pcap.rs
    │   ├── process.rs
    │   ├── workspace.rs
    │   └── tools.rs
    └── services/         # 每个功能域一个 service
        ├── mod.rs
        ├── app.rs        # AppService (app_info, log, force_quit)
        ├── network.rs    # NetworkService
        ├── autoruns.rs   # AutorunsService
        ├── sysmon.rs     # SysmonService
        ├── monitor.rs    # MonitorService
        ├── pcap.rs       # PcapService
        ├── process.rs    # ProcessService
        ├── workspace.rs  # WorkspaceService
        └── tools.rs      # ToolsService
```

### 1.2 AppContext（替代 AppState）

从 `crates/irtool-tauri/src/state.rs` 迁移，与当前几乎一致，增加 `EventBus` 字段，**去掉所有 Tauri 依赖**：

```rust
pub struct AppContext {
    // ... 与当前 AppState 字段完全一致 ...
    pub event_bus: EventBus,  // 新增
}
```

### 1.3 EventBus 设计

基于 `tokio::sync::broadcast`，统一所有 push 事件：

```rust
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", content = "payload")]
pub enum AppEvent {
    NetworkSnapshot(NetworkSnapshotPayload),
    NetworkError(String),
    NetworkEnrichment(NetworkEnrichmentPayload),
    AutorunsProgress(/* ... */),
    AutorunsSignatureProgress(/* ... */),
    AutorunsHashProgress(/* ... */),
    SysmonEvent(/* ... */),
    MonitorAlert(/* ... */),
    PcapEvent(/* ... */),
    CloseRequested,
}

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<AppEvent>,
}
```

- Tauri 端：subscribe → 桥接为 `app.emit()`
- egui 端：subscribe → 更新 UI 状态

### 1.4 Service 设计

**用 struct + 直接 async fn，不用 trait**。理由：
- 不需要动态派发（不存在多种实现）
- 避免 `Box<dyn>` 和 `Send` 约束复杂度
- 底层 crates 已有自己的 trait 抽象用于测试

示例：

```rust
pub struct NetworkService<'a> {
    ctx: &'a AppContext,
}

impl<'a> NetworkService<'a> {
    pub async fn snapshot(&self) -> Result<NetworkSnapshotPayload, IrError> { /* ... */ }
    pub async fn kill_process(&self, pid: u32) -> Result<(), IrError> { /* ... */ }
    pub async fn set_polling(&self, control: NetworkPollingControl) -> Result<(), IrError> { /* ... */ }
    pub fn start_default_polling(&self, handle: tokio::runtime::Handle) { /* ... */ }
}
```

---

## Task 2: 改造 `irtool-tauri` 为薄代理

### 2.1 Commands 简化

每个 command 变为一行代理：

```rust
// 改造后
#[tauri::command]
#[specta::specta]
pub async fn cmd_network_snapshot(ctx: State<'_, AppContext>) -> Result<NetworkSnapshotPayload, IrError> {
    NetworkService { ctx: &ctx }.snapshot().await
}
```

### 2.2 事件桥接

在 `lib.rs` 的 `setup` 中启动一个 task，将 `EventBus` 事件桥接到 Tauri 事件系统：

```rust
fn bridge_events_to_tauri(ctx: &AppContext, app: tauri::AppHandle) {
    let mut rx = ctx.event_bus.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            match &event {
                AppEvent::NetworkSnapshot(p) => { let _ = app.emit("evt_network_snapshot", p); }
                AppEvent::MonitorAlert(p) => { let _ = app.emit("evt_monitor_alert", p); }
                // ...
            }
        }
    });
}
```

### 2.3 specta 类型生成

保持不变。DTO 类型仍在 Rust 侧 derive `Type`，specta 自动导出到 `bindings.ts`。只是类型定义的位置从 `commands/*.rs` 移到了 `irtool-service/dto/`，specta 支持跨 crate 类型引用。

---

## Task 3（后续）: egui Fallback 前端

### 3.1 结构设计

```
crates/irtool-egui/
├── Cargo.toml       # 依赖 irtool-service + eframe + egui_extras
└── src/
    ├── main.rs       # 入口
    ├── app.rs        # eframe::App 实现
    ├── event_bridge.rs
    └── views/        # 每个 feature 一个 view
        ├── network.rs
        ├── autoruns.rs
        ├── sysmon.rs
        ├── monitor.rs
        ├── pcap.rs
        ├── workspace.rs
        └── settings.rs
```

### 3.2 通信方式

同进程直接调用 `AppContext` 方法，零序列化开销：

```rust
struct IRTApp {
    ctx: Arc<AppContext>,
    runtime: tokio::runtime::Runtime,
    event_rx: broadcast::Receiver<AppEvent>,
    // UI state ...
}
```

### 3.3 体积估算

| 组件 | 估算 |
|------|------|
| egui + eframe 基础（release + LTO + strip） | ~2-3 MB |
| 业务 crates（与 Tauri 后端共享） | ~3-5 MB |
| **egui 独立二进制总计** | **~5-8 MB** |
| Tauri 版当前（不含 WebView2 运行时） | ~10-15 MB |

egui 不需要 WebView2 运行时（由 OS 提供），所以 egui 版在**部署体积**上反而更小。两个二进制合计约 15-23 MB，相比当前单一 Tauri 版增加约 5-8 MB（因为 Rust 业务代码被编译了两次）。

### 3.4 Tauri 插件替代

egui 端需要替代 Tauri 插件提供的 OS 能力：

| Tauri 插件 | egui 替代 |
|------------|----------|
| `tauri-plugin-dialog` | `rfd` (Rusty File Dialog) |
| `tauri-plugin-notification` | `notify-rust` |
| `tauri-plugin-fs` | 直接 `std::fs` |
| `tauri-plugin-shell` | `std::process::Command` |
| `tauri-plugin-store` | `serde_json` + 文件 |

---

## 迁移路径

| 阶段 | 内容 | 预计工作量 |
|------|------|-----------|
| **Phase 1** | 创建 `irtool-service`，迁移 AppContext + EventBus + DTO 类型 | ~1天 |
| **Phase 2** | 逐模块迁移 commands → services（建议顺序：process → network → autoruns → sysmon → monitor → pcap → workspace → tools） | ~3-5天 |
| **Phase 3** | 改造 `irtool-tauri` commands 为薄代理 + 事件桥接 | ~1天（与 Phase 2 同步进行） |
| **Phase 4** | 验证：确保 Tauri 版功能完全不变，所有测试通过 | ~1天 |
| **Phase 5（后续分支）** | 创建 `irtool-egui`，实现骨架 + 网络视图，其他视图占位 | ~2-3天 |

---

## 关键设计决策

| 决策 | 选择 | 理由 |
|------|------|------|
| 服务层抽象 | struct + async fn（非 trait） | 无需动态派发，简化 Send 约束 |
| 事件流 | `tokio::sync::broadcast<AppEvent>` | 多消费者、与 Tauri emit 解耦 |
| egui 通信 | 同进程直接调用 | 零开销，无需 IPC/HTTP |
| DTO 位置 | `irtool-service/dto/` | 两个前端引用同一套类型 |
| 构建产物 | 解耦后可灵活选择独立或合并构建 | 当前先解耦，后续按需决定 |
| specta 类型生成 | 保持不变 | 跨 crate 类型引用正常工作 |

## 验证方式

- Phase 2-3 完成后：`cargo build -p irtool-tauri` 编译通过
- `cargo tauri dev` 启动后所有功能正常（网络轮询、autoruns 扫描、sysmon 事件、监控告警）
- 现有集成测试通过
- Phase 5 完成后：`cargo build -p irtool-egui` 编译通过，egui 窗口能展示网络表格数据