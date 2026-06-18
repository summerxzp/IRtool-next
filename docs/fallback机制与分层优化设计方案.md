# IRtool Fallback 机制与分层优化设计方案

> 基于 `docs/引入fallback结构优化设计方案.md` 的思路，结合项目实际现状优化而成。
> 项目已实现 `irtool-service` 服务层、Tauri 薄代理、egui 骨架，以及 `egui-fallback` feature flag 的雏形。
> 本方案聚焦于：完善 fallback 启动机制、修复分层违规、明确实施规范。

---

## 1. 现状评估

### 1.1 已实现

| 模块 | 状态 | 说明 |
|------|------|------|
| `irtool-service` | ✅ 完成 | AppContext + EventBus + 各 Service（network/autoruns/monitor/...） |
| Tauri 薄代理 | ✅ 完成 | 所有 `#[tauri::command]` 已改为调用 Service |
| EventBus → Tauri 桥接 | ✅ 完成 | `events.rs` 统一转发 |
| `irtool-egui` 骨架 | ✅ 完成 | app/layout/nav/theme + network 页 + widgets |
| `egui-fallback` feature | ⚠️ 雏形 | main.rs 有检测逻辑，但检测不严格、无降级标识 |

### 1.2 现有 fallback 实现

[crates/irtool-tauri/src/main.rs](file:///e:/Code/IRtool-next/crates/irtool-tauri/src/main.rs)：

```rust
fn main() {
    #[cfg(feature = "egui-fallback")]
    if !is_webview2_available() {
        irtool_egui::run();   // 同进程调用
        return;
    }
    irtool_lib::run();
}
```

**模式**：单二进制 + feature flag + 同进程 fallback。
**优点**：单 exe 分发简单、无子进程管理、切换无缝。
**问题**：
1. WebView2 检测只查注册表 key 是否存在，未校验 `pv` 值（key 残留但实际卸载会误判）
2. egui 不知道自己是 fallback 启动，无法在 UI 上标识"降级模式"
3. `egui-fallback` 不在默认 feature，发布构建需显式启用
4. egui 直接依赖 `irtool-net-monitor`，破坏分层

### 1.3 未采用参考方案的哪些部分

参考方案提出 `models / config / platform / services` 四层分离。本项目**不采用**：
- 项目已有 `irtool-core` + 各业务 crate 自带 `types.rs`，再拆 `models/config/platform` 是过度工程
- `irtool-service` 已承担 services 层职责，业务 crate 自带平台能力封装（如 `irtool-sysmon` 封装 ETW/WMI）
- CLI 不在本次范围

**结论**：在现有 `irtool-service` 基础上完善，不做七层重构。

---

## 2. Fallback 机制优化

### 2.1 模式选择：保留单二进制 feature flag

对比两种模式：

| 维度 | 单二进制 feature flag（现有） | 子进程模式 |
|------|------------------------------|-----------|
| 分发 | 1 个 exe | 2 个 exe，需版本同步 |
| 体积 | Tauri+egui 混合，+5~8MB | 总和类似 |
| 隔离 | 同进程，状态共享 | 独立进程 |
| 复杂度 | 低 | 中 |
| 降级体验 | 无缝 | spawn 新进程可能闪烁 |

**决策：保留单二进制 feature flag 模式**。理由：
- 用户核心需求是"缺 WebView2 能 fallback"，单二进制最简单可靠
- 体积增加可接受（release LTO 后约 +5MB）
- 同进程调用在 `irtool_lib::run()` 之前执行，Tauri 全局状态尚未初始化，无冲突

### 2.2 WebView2 检测优化

**问题**：当前只检查注册表 key 是否存在，key 残留但 WebView2 实际卸载时会误判为可用。

**当前实现**（已实施）：打开 key 后读取 `pv` 值，校验非空且非 `0.0.0.0`。

```rust
#[cfg(feature = "egui-fallback")]
fn is_webview2_available() -> bool {
    use windows::Win32::System::Registry::{
        RegCloseKey, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_SZ,
    };
    use windows::core::{w, PCWSTR};

    let paths = [
        w!(r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"),
        w!(r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"),
        w!(r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{2CD8A007-E189-409D-A2C8-9AF4EF3C72AA}"),
    ];

    for path in &paths {
        let mut key = HKEY::default();
        let ok = unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, *path, None, KEY_READ, &mut key) };
        if ok.is_err() { continue; }
        let mut buf = [0u16; 64];
        let mut len: u32 = (buf.len() * 2) as u32;
        let pv_ok = unsafe {
            RegGetValueW(key, PCWSTR::null(), w!("pv"), RRF_RT_REG_SZ, None,
                Some(buf.as_mut_ptr() as *mut _), Some(&mut len))
        };
        unsafe { let _ = RegCloseKey(key); }
        if pv_ok.is_ok() && len > 0 {
            let pv = String::from_utf16_lossy(&buf[..(len as usize / 2).saturating_sub(1)]);
            if !pv.is_empty() && pv != "0.0.0.0" { return true; }
        }
    }
    false
}
```

**后续优化方向**：注册表检测仍非最稳，微软官方推荐 `GetAvailableCoreWebView2BrowserVersionString()` API 真正检测运行时可用性。未来可引入 `webview2-com` crate 或动态加载 `WebView2Loader.dll` 调用此 API，注册表检测作为 fallback。当前注册表 + pv 校验已覆盖绝大多数场景，此项列为后续优化。

### 2.3 Fallback 标识传递：StartupMode enum

**问题**：egui 不知道自己是 fallback 启动还是独立运行，无法在 UI 上标识。

**方案**：用类型安全的 `StartupMode` enum 替代环境变量，通过 `irtool_egui::run(mode)` 传递。

```rust
// crates/irtool-egui/src/lib.rs
pub enum StartupMode {
    Normal,    // 独立运行（irtool-egui 二进制直接启动）
    Fallback,  // 作为 WebView2 缺失的 fallback 启动
}

pub fn run(mode: StartupMode) { ... }
```

```rust
// crates/irtool-tauri/src/main.rs
fn main() {
    #[cfg(feature = "egui-fallback")]
    if !is_webview2_available() {
        tracing::warn!("WebView2 not available, falling back to egui frontend");
        irtool_egui::run(irtool_egui::StartupMode::Fallback);
        return;
    }
    irtool_lib::run();
}
```

```rust
// crates/irtool-egui/src/app.rs
pub struct IrtoolApp {
    pub is_fallback: bool,
}

impl IrtoolApp {
    pub fn new(..., mode: StartupMode) -> Self {
        Self { is_fallback: mode == StartupMode::Fallback, ... }
    }
}
```

TopBar 渲染时，若 `is_fallback` 为 true，显示黄色 "降级模式" 徽章。

**优于环境变量的理由**：类型安全、编译期检查、无需字符串解析、未来扩展（如 `StartupMode::Cli`）方便。

### 2.4 Feature flag 启用策略

**问题**：`egui-fallback` 不在默认 feature，CI/发布构建可能遗漏。

**策略**：
- **开发构建**（`cargo tauri dev`）：不启用 `egui-fallback`，避免 egui 依赖拖慢编译
- **发布构建**（`cargo tauri build`）：必须启用 `egui-fallback`

实现方式：在发布脚本/CI 中显式传参：
```bash
cargo tauri build --features irtool-tauri/egui-fallback
```

或在 `.cargo/config.toml` 中为 release profile 配置（不推荐，会影响 dev）。

**注意**：`tauri.conf.json` 的 bundle 配置无需改动，egui 代码编译进主二进制，不是外部资源。

### 2.5 Tauri 启动失败兜底（不覆盖）

**场景**：注册表检测通过（pv 值有效），但 WebView2 实际损坏，Tauri 启动时 panic。

**不覆盖原因**：
- `Cargo.toml` release profile 设置 `panic = "abort"`，无法用 `catch_unwind` 捕获
- 此场景极少见（注册表有效但运行时损坏）
- 覆盖成本高（需要子进程监控或修改 panic 策略，影响性能）

**说明**：当前方案已覆盖"无 WebView2"的主场景（注册表检测 → fallback egui）。此处"不覆盖"仅指"注册表残留但运行时损坏"的极端场景。IR 工具场景下不应依赖用户重装运行时，但该极端场景无低成本覆盖方式，接受此风险。

### 2.6 AppContext 生命周期

**约定**：`AppContext` 内部所有字段均为 `Arc<...>`，已实现 `Clone`。`ctx.clone()` 是廉价的引用计数递增，共享同一份底层状态。

**规则**：
- Tauri 端：`AppContext::new()` 创建一次，通过 `.manage(ctx.clone())` 注册到 Tauri 状态，各 command 通过 `State<'_, AppContext>` 获取
- egui 端：`AppContext::new()` 创建一次，`ctx.clone()` 传入 `IrtoolApp`
- **不要**在 Tauri/egui 各自重新创建 `AppContext`，否则配置、EventBus、数据库连接会分裂
- EventBus、collector、store 等共享资源通过 `AppContext` 的 `Arc` 字段天然共享，无需额外同步

```rust
// 正确：创建一次，clone 共享
let ctx = AppContext::new(app_dirs);
tauri::Builder::default().manage(ctx.clone());  // Tauri 持有 clone
let app = IrtoolApp::new(ctx, ...);              // egui 持有 clone

// 错误：各自创建，状态分裂
let tauri_ctx = AppContext::new(app_dirs);  // ❌
let egui_ctx = AppContext::new(app_dirs);   // ❌ 不同实例
```

---

## 3. 分层规范优化

### 3.1 问题：egui 直接依赖业务 crate

[crates/irtool-egui/Cargo.toml](file:///e:/Code/IRtool-next/crates/irtool-egui/Cargo.toml)：
```toml
irtool-net-monitor = { path = "../irtool-net-monitor" }  # ❌ 违规
```

[crates/irtool-egui/src/pages/network.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/network.rs#L5)：
```rust
use irtool_net_monitor::{ConnState, NetConn, Proto};  // ❌ 直接依赖业务 crate
```

**后果**：后续每个 egui 页面都会直接依赖对应业务 crate，`irtool-service` 形同虚设，分层失去意义。

### 3.2 方案：irtool-service re-export 业务类型

在 `irtool-service` 中新增 `types` 模块，聚合 re-export 前端需要的业务类型：

```rust
// crates/irtool-service/src/types.rs
//! Re-exported business types for frontend consumption.
//!
//! Frontend crates (irtool-egui, future irtool-cli) should depend ONLY on
//! irtool-service and import types from here, NOT directly from business crates.
//! This ensures the service layer remains the single dependency boundary.

// ── Network ──
pub use irtool_net_monitor::{
    NetConn, ConnState, Proto, AddressFamily,
    CmdlineStatus, CmdlineResult, RetentionPolicy,
};

// ── Autoruns ──
pub use irtool_autoruns::{
    AutorunItem, ScanOptions, ScanProgress, ScanPhase,
    SignatureProgress, SignatureStatus, DeleteResult,
};

// ── Sysmon ──
pub use irtool_sysmon::{SysmonEvent, SysmonConfig};

// ── Monitor ──
pub use irtool_monitor::{
    Alert, MonitorEvent, MonitorConfig, EventQuery, EventPage,
    EventSource, RuntimeTelemetry,
};

// ── Pcap ──
pub use irtool_pcap::{PcapEvent, PcapCounters};
```

```rust
// crates/irtool-service/src/lib.rs
pub mod context;
pub mod dto;
pub mod event_bus;
pub mod services;
pub mod types;  // 新增

pub use context::AppContext;
pub use event_bus::{AppEvent, EventBus};
```

**egui 侧改造**：

```toml
# crates/irtool-egui/Cargo.toml
[dependencies]
irtool-service = { path = "../irtool-service" }
# 删除: irtool-net-monitor = { path = "../irtool-net-monitor" }
```

```rust
// crates/irtool-egui/src/pages/network.rs
// 改前: use irtool_net_monitor::{ConnState, NetConn, Proto};
// 改后:
use irtool_service::types::{ConnState, NetConn, Proto};
```

### 3.3 DTO 策略

当前 `dto/` 只有 `network.rs` 和 `app.rs`，其他 service 直接返回业务 crate 类型。统一策略：

| 类型类别 | 存放位置 | 示例 |
|----------|----------|------|
| **核心数据类型** | 业务 crate，经 `types.rs` re-export | `NetConn`, `AutorunItem`, `Alert` |
| **组合输入参数** | `irtool-service/dto/` | `NetworkPollingControl`（组合 interval+paused+retention） |
| **组合输出 payload** | `irtool-service/dto/` | `NetworkSnapshotPayload`（items+timestamp） |
| **App 级元信息** | `irtool-service/dto/` | `AppInfo` |

**原则**：`dto/` 只放"跨 crate 组合的、业务 crate 中不存在的类型"。核心数据类型走 re-export，不重复定义。

---

## 4. 产物与构建

### 4.1 单二进制构建

```bash
# 开发（不含 fallback，编译快）
cargo tauri dev

# 发布（含 fallback）
cargo tauri build --features irtool-tauri/egui-fallback
```

产物：单个 `IRtool.exe`，内含 Tauri + egui 代码。
- 有 WebView2：走 Tauri 前端
- 无 WebView2：走 egui 前端

### 4.2 体积估算

| 组件 | 体积 |
|------|------|
| Tauri 版（不含 egui-fallback） | ~10-15 MB |
| + egui-fallback feature | +5-8 MB |
| **发布版总计** | **~15-23 MB** |

可接受。egui 不需要 WebView2 运行时，部署体积仍优于嵌入 Fixed Runtime（+150MB）。

---

## 5. 实施计划

### Phase 1: 分层修复（P0，立即）

| 任务 | 文件 | 说明 |
|------|------|------|
| 创建 `types.rs` | `crates/irtool-service/src/types.rs` | re-export 业务类型 |
| 注册 `types` 模块 | `crates/irtool-service/src/lib.rs` | 加 `pub mod types;` |
| 移除 egui 业务 crate 依赖 | `crates/irtool-egui/Cargo.toml` | 删 `irtool-net-monitor` |
| 改 egui import | `crates/irtool-egui/src/pages/network.rs` | 改用 `irtool_service::types::` |
| 验证编译 | - | `cargo build -p irtool-egui` 通过 |

### Phase 2: Fallback 机制优化（P0，立即）

| 任务 | 文件 | 说明 |
|------|------|------|
| 优化 WebView2 检测 | `crates/irtool-tauri/src/main.rs` | 读 `pv` 值校验 |
| 传递 fallback 标识 | `crates/irtool-tauri/src/main.rs` | 设 `IRTOOL_FALLBACK=1` |
| egui 读取标识 | `crates/irtool-egui/src/app.rs` | 加 `is_fallback` 字段 |
| TopBar 显示降级模式 | `crates/irtool-egui/src/layout.rs` | 黄色徽章 |
| 验证 fallback | - | 禁用 WebView2 注册表项测试 |

### Phase 3: 其他优化（P1-P2，后续）

见第 6 节待办清单。

---

## 6. 待办清单

### P1: Bug 修复与性能

| ID | 任务 | 文件 | 说明 |
|----|------|------|------|
| B1 | 修复行选中 bug | `irtool-egui/src/pages/network.rs` | `is_selected` 未检查 `selected_remote`，同 PID 多连接会全部高亮 |
| B2 | 缓存 filtered+sorted 结果 | `irtool-egui/src/pages/network.rs` | 加 `cache_dirty` 标记，避免每帧 clone+sort |
| B3 | 缓存选中行 NetConn | `irtool-egui/src/pages/network.rs` | `find_selected_conn()` 每帧多次 O(n) 扫描+clone |
| B4 | 事件驱动重绘 | `irtool-egui/src/event_bridge.rs` + `app.rs` | EventBridge 持有 `egui::Context`，事件到达时 `request_repaint()`，移除固定 100ms 重绘 |
| B5 | 简化 `start_default_polling` 签名 | `irtool-service/src/services/network.rs` | 用 `tokio::runtime::Handle` 替代闭包参数 |

### P2: 规范与一致性

| ID | 任务 | 说明 |
|----|------|------|
| C1 | egui 单实例检测 | 用命名互斥锁 `Global\IRtool-SingleInstance`，与 Tauri 的 `tauri-plugin-single-instance` 互斥 |
| C2 | EventBus 容量提升 | `broadcast::channel(256)` → `1024`，对 `NetworkSnapshot` 高频事件考虑 lagged 后主动拉取 |
| C3 | DTO 补全 | 其他 service 的组合参数补到 `dto/`（如 autoruns 的 ScanOptions 若需跨 crate 组合） |
| C4 | 功能对等矩阵 | 在 DESIGN.md 加表格跟踪 7 个页面在 Tauri/egui 下的实现状态 |
| C5 | 错误恢复策略 | DESIGN.md 补充：连续失败 N 次的降级、banner 自动消失规则 |

### P3: 长期（暂不动）

| ID | 任务 | 说明 |
|----|------|------|
| D1 | AppEvent 拆分 | 若 variant 继续增长，按域拆分 EventBus。当前 15+ variant 可接受 |
| D2 | CLI 模式 | 参考方案建议的 `irtool-cli`，不在本次范围 |
