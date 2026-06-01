# IRtool v2 — P1 网络监控 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 P0 脚手架基础上实装网络监控模块。后端用 Rust + windows-rs 直接调用 IPHELPER API（替换 v1 的 psutil），前端用通用虚拟化 DataTable + 三栏布局（工具栏 / 表格+详情 / 统计栏）。功能等价 v1，性能目标：5000 条连接 60fps 滚动、200ms debounce 搜索、首次响应 ≤ 100ms。

**Architecture:** 4 crate 协作：`irtool-core` 提供 `IrError` 与 `TaskRegistry`；`irtool-net-monitor` 提供 `NetCollector` trait + `WindowsNetCollector`（TCP/UDP × IPv4/IPv6 4 张表）+ 历史合并 + 进程信息缓存；`irtool-tauri` 注册 commands/events + 后台轮询；前端 `features/network` 用 TanStack Query + listen 双向更新缓存，TanStack Table virtualizer 渲染。

**Tech Stack:** Rust 1.85, windows-rs 0.59 (Win32_NetworkManagement_IpHelper / Win32_System_Threading / Win32_System_ProcessStatus / Win32_Security), tokio 1.40, dashmap 6, React 18.3, @tanstack/react-table v8, @tanstack/react-virtual v3, react-resizable-panels, papaparse (CSV).

**Spec 引用:** `D:\project\IRtool\docs\superpowers\specs\2026-05-31-IRtool-refactor-design.md` §4.2 / §5.4.1 / §6.1 / §6.2 / §7

**前置依赖:** P0 已完成（v2.0.0-alpha.P0 tag 存在）

---

## P0 阶段反馈与 P1 锁定

实施 P0 时出现的偏差，全部纳入 P1 plan 基线：

| 项 | P0 实际 | P1 锁定 |
|---|---|---|
| 工具链 | stable-x86_64-pc-windows-gnu (MinGW) | **保持 GNU 推进 P1**；遇到 windows-rs 编译失败立即切 MSVC（Visual Studio Build Tools 2022 + C++ 桌面开发工作负载）。**P5 打包前必须切 MSVC** |
| 资源嵌入 | embed-resource 2.x（替代 winres） | 保持 |
| irtool-tauri crate-type | `["rlib"]` | 保持 |
| specta features | `["serde", "derive"]` | **所有 specta 依赖统一含 `derive`** |
| bindings.ts 路径 | `concat!(env!("CARGO_MANIFEST_DIR"), "/../../ui/src/lib/bindings.ts")` | 保持 |

**P1 风险信号**（如果遇到立即切 MSVC）：
- windows-rs 链接错误 `LNK1318: too many sections` 或 `relocation truncated to fit`
- `tauri build` 报 `link.exe failed` 或 NSIS bundle 失败
- WinTrust API（P2）无法解析符号
- Sysmon EVT API（P4）未导出

---

## 仓库与文件结构

P1 完成后新增/修改文件：

```
IRtool-next/
├── crates/
│   ├── irtool-core/
│   │   └── src/
│   │       ├── task.rs                # 新增：TaskRegistry + CancellationToken
│   │       └── lib.rs                 # 修改：导出 task
│   └── irtool-net-monitor/
│       ├── Cargo.toml                 # 修改：加 windows + 进程查询 features
│       └── src/
│           ├── lib.rs                 # 重写
│           ├── types.rs               # NetConn / Proto / Family / ConnState
│           ├── collector.rs           # NetCollector trait + WindowsNetCollector
│           ├── tcp.rs                 # TCP IPv4/IPv6 枚举
│           ├── udp.rs                 # UDP IPv4/IPv6 枚举
│           ├── process_info.rs        # PID → name/path/cmdline + 缓存
│           ├── history.rs             # first_seen/last_seen/is_current 合并
│           └── kill.rs                # TerminateProcess 包装
├── crates/irtool-tauri/
│   ├── Cargo.toml                     # 修改：加 irtool-net-monitor 显式依赖（已有）+ tokio rt feature
│   └── src/
│       ├── main.rs                    # 修改：注册 cmd + state + spawn 轮询
│       ├── commands/
│       │   ├── mod.rs                 # 新增
│       │   └── network.rs             # 新增 4 个 cmd_network_*
│       ├── events.rs                  # 新增：事件名常量
│       └── state.rs                   # 新增：AppState 全局
└── ui/
    ├── package.json                   # 修改：加 papaparse, react-resizable-panels
    └── src/
        ├── components/
        │   └── data-table/
        │       ├── DataTable.tsx      # 新增：通用虚拟表格
        │       ├── columns-utils.ts
        │       └── filters.ts
        ├── lib/
        │   ├── ipc.ts                 # 修改：补全 listen helper
        │   └── csv.ts                 # 新增：通用 CSV 导出
        ├── features/
        │   └── network/
        │       ├── api.ts             # commands 包装
        │       ├── types.ts           # 从 bindings 重导
        │       ├── store.ts           # Zustand: 过滤 / 列可见 / 历史保留 / 暂停
        │       ├── hooks.ts           # useNetwork (useQuery + listen)
        │       ├── columns.tsx        # ColumnDef<NetConn>[]
        │       ├── pages/
        │       │   └── NetworkPage.tsx
        │       └── components/
        │           ├── NetworkToolbar.tsx
        │           ├── NetworkTable.tsx
        │           ├── NetworkDetail.tsx
        │           ├── NetworkStatsBar.tsx
        │           └── KillProcessDialog.tsx
        ├── components/ui/
        │   ├── input.tsx              # 新增：shadcn input
        │   ├── select.tsx             # 新增：shadcn select
        │   ├── dialog.tsx             # 新增：shadcn dialog
        │   ├── badge.tsx              # 新增：shadcn badge
        │   ├── dropdown-menu.tsx      # 新增：shadcn dropdown-menu (右键菜单)
        │   └── alert-dialog.tsx       # 新增：shadcn alert-dialog (终止确认)
        ├── routes/
        │   └── network.tsx            # 修改：调用 NetworkPage
        └── locales/
            ├── zh-CN.json             # 新增 network 命名空间
            └── en-US.json             # 新增 network 命名空间
```

---

## 任务概览

P1 共 20 个任务，预计 5 工作日。

| # | 任务 | 关键产出 |
|---|---|---|
| 1 | irtool-core 加 TaskRegistry | 通用任务取消 token |
| 2 | net-monitor 数据模型与错误 | NetConn/Proto/Family/ConnState 类型 |
| 3 | TCP IPv4 枚举 | GetExtendedTcpTable 拿 PID + addr |
| 4 | TCP IPv6 枚举 | GetExtendedTcpTable v6 |
| 5 | UDP IPv4/IPv6 枚举 | GetExtendedUdpTable |
| 6 | 进程信息缓存 | QueryFullProcessImageNameW + create_time 校验 |
| 7 | NetCollector 组装 + kill_process | 4 张表合并 + TerminateProcess |
| 8 | 历史合并与去重 | first_seen/last_seen/is_current |
| 9 | 后端轮询任务 + 事件 emit | 1s tick + evt_network_snapshot |
| 10 | Tauri commands 注册 + bindings 同步 | snapshot/kill/control 4 个 cmd |
| 11 | 通用 DataTable 组件 | 虚拟化 + 列管理 + 排序 + 选中 |
| 12 | shadcn 原子组件补齐 | input/select/dialog/badge/dropdown-menu/alert-dialog |
| 13 | features/network 骨架 | api/store/hooks/types/i18n |
| 14 | NetworkToolbar | 6 控件 + debounce 搜索 |
| 15 | NetworkTable + columns | 虚拟化集成 + 状态着色 |
| 16 | NetworkDetail 详情面板 | resize panel + 基本信息 |
| 17 | NetworkStatsBar | 6 项统计 |
| 18 | 终止确认 + 上下文菜单 + 复制行 | AlertDialog + DropdownMenu |
| 19 | CSV 导出 | papaparse + dialog 保存 |
| 20 | 性能基线 + smoke 验证 | 5000 行 60fps |

---

## Task 1: irtool-core 加 TaskRegistry

**Files:**
- Create: `D:\project\IRtool-next\crates\irtool-core\src\task.rs`
- Modify: `D:\project\IRtool-next\crates\irtool-core\src\lib.rs`
- Modify: `D:\project\IRtool-next\crates\irtool-core\Cargo.toml`

- [ ] **Step 1.1: 修改 `crates/irtool-core/Cargo.toml` 加 dashmap**

```toml
[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio-util = { workspace = true }
tracing = { workspace = true }
specta = { workspace = true, features = ["serde", "derive"] }
dashmap = { workspace = true }
```

- [ ] **Step 1.2: 写 `crates/irtool-core/src/task.rs`**

```rust
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio_util::sync::CancellationToken;

pub type TaskId = u64;

#[derive(Default)]
pub struct TaskRegistry {
    next_id: AtomicU64,
    tokens: DashMap<TaskId, CancellationToken>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self) -> (TaskId, CancellationToken) {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let token = CancellationToken::new();
        self.tokens.insert(id, token.clone());
        (id, token)
    }

    pub fn cancel(&self, id: TaskId) -> bool {
        if let Some((_, token)) = self.tokens.remove(&id) {
            token.cancel();
            true
        } else {
            false
        }
    }

    pub fn finish(&self, id: TaskId) {
        self.tokens.remove(&id);
    }

    pub fn is_active(&self, id: TaskId) -> bool {
        self.tokens.contains_key(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancel_propagates_to_token() {
        let reg = TaskRegistry::new();
        let (id, token) = reg.register();
        assert!(!token.is_cancelled());
        let cancelled = reg.cancel(id);
        assert!(cancelled);
        assert!(token.is_cancelled());
    }

    #[test]
    fn finish_removes_without_cancel() {
        let reg = TaskRegistry::new();
        let (id, token) = reg.register();
        reg.finish(id);
        assert!(!reg.is_active(id));
        assert!(!token.is_cancelled());
    }

    #[test]
    fn cancel_unknown_returns_false() {
        let reg = TaskRegistry::new();
        assert!(!reg.cancel(999));
    }
}
```

- [ ] **Step 1.3: 修改 `crates/irtool-core/src/lib.rs`**

```rust
pub mod config;
pub mod error;
pub mod task;

pub use config::{AppConfig, Language, Theme};
pub use error::IrError;
pub use task::{TaskId, TaskRegistry};
```

- [ ] **Step 1.4: 跑测试**

```bash
cd "D:/project/IRtool-next"
cargo test -p irtool-core
```

预期：`6 passed; 0 failed`（含 P0 的 3 个 + P1 新增 3 个）。

- [ ] **Step 1.5: clippy 检查**

```bash
cargo clippy -p irtool-core --all-targets -- -D warnings
```

- [ ] **Step 1.6: 提交**

```bash
git add crates/irtool-core/
git commit -m "feat(core): TaskRegistry with CancellationToken-based task lifecycle"
```

---

## Task 2: net-monitor 数据模型与错误

**Files:**
- Modify: `D:\project\IRtool-next\crates\irtool-net-monitor\Cargo.toml`
- Create: `D:\project\IRtool-next\crates\irtool-net-monitor\src\types.rs`
- Modify: `D:\project\IRtool-next\crates\irtool-net-monitor\src\lib.rs`

- [ ] **Step 2.1: 修改 `crates/irtool-net-monitor/Cargo.toml`**

```toml
[package]
name = "irtool-net-monitor"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
irtool-core = { path = "../irtool-core" }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
tokio-util = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }
dashmap = { workspace = true }
specta = { workspace = true, features = ["serde", "derive"] }

[target.'cfg(windows)'.dependencies]
windows = { workspace = true, features = [
    "Win32_Foundation",
    "Win32_NetworkManagement_IpHelper",
    "Win32_Networking_WinSock",
    "Win32_System_Threading",
    "Win32_System_ProcessStatus",
    "Win32_Security",
] }
```

- [ ] **Step 2.2: 写 `crates/irtool-net-monitor/src/types.rs`**

```rust
use serde::{Deserialize, Serialize};
use specta::Type;
use std::net::IpAddr;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum Proto {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum Family {
    V4,
    V6,
}

/// 与 Windows MIB_TCP_STATE 对齐 + UDP 用 None
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConnState {
    Closed,
    Listen,
    SynSent,
    SynRcvd,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
    DeleteTcb,
    /// UDP 无状态 / 未知
    None,
}

impl ConnState {
    pub fn from_mib_tcp_state(state: u32) -> Self {
        // 对应 windows::Win32::NetworkManagement::IpHelper::MIB_TCP_STATE
        // 1=CLOSED, 2=LISTEN, 3=SYN_SENT, 4=SYN_RCVD, 5=ESTABLISHED,
        // 6=FIN_WAIT1, 7=FIN_WAIT2, 8=CLOSE_WAIT, 9=CLOSING, 10=LAST_ACK,
        // 11=TIME_WAIT, 12=DELETE_TCB
        match state {
            1 => Self::Closed,
            2 => Self::Listen,
            3 => Self::SynSent,
            4 => Self::SynRcvd,
            5 => Self::Established,
            6 => Self::FinWait1,
            7 => Self::FinWait2,
            8 => Self::CloseWait,
            9 => Self::Closing,
            10 => Self::LastAck,
            11 => Self::TimeWait,
            12 => Self::DeleteTcb,
            _ => Self::None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Closed => "CLOSED",
            Self::Listen => "LISTEN",
            Self::SynSent => "SYN_SENT",
            Self::SynRcvd => "SYN_RCVD",
            Self::Established => "ESTABLISHED",
            Self::FinWait1 => "FIN_WAIT1",
            Self::FinWait2 => "FIN_WAIT2",
            Self::CloseWait => "CLOSE_WAIT",
            Self::Closing => "CLOSING",
            Self::LastAck => "LAST_ACK",
            Self::TimeWait => "TIME_WAIT",
            Self::DeleteTcb => "DELETE_TCB",
            Self::None => "",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
pub struct NetEndpoint {
    /// IP 字符串（"::" / "0.0.0.0" / "192.168.1.1"）
    pub addr: String,
    pub port: u16,
}

impl NetEndpoint {
    pub fn from_v4(addr: u32, port: u16) -> Self {
        let ip = std::net::Ipv4Addr::from(u32::from_be(addr));
        Self {
            addr: ip.to_string(),
            port: u16::from_be(port),
        }
    }

    pub fn from_v6(addr: [u8; 16], port: u16) -> Self {
        let ip = std::net::Ipv6Addr::from(addr);
        Self {
            addr: ip.to_string(),
            port: u16::from_be(port),
        }
    }
}

/// 唯一标识一条连接（用于历史合并去重）
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NetConnKey {
    pub proto: Proto,
    pub family: Family,
    pub local_addr: String,
    pub local_port: u16,
    pub remote_addr: String,
    pub remote_port: u16,
    pub pid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NetConn {
    pub proto: Proto,
    pub family: Family,
    pub local: NetEndpoint,
    pub remote: NetEndpoint,
    pub state: ConnState,
    pub pid: u32,
    pub process_name: Option<String>,
    pub process_path: Option<String>,
    pub process_cmdline: Option<String>,

    /// epoch 秒；首次出现时间
    pub first_seen: u64,
    /// epoch 秒；最近一次仍存在时间
    pub last_seen: u64,
    /// 是否在最近一次快照中存在
    pub is_current: bool,
}

impl NetConn {
    pub fn key(&self) -> NetConnKey {
        NetConnKey {
            proto: self.proto,
            family: self.family,
            local_addr: self.local.addr.clone(),
            local_port: self.local.port,
            remote_addr: self.remote.addr.clone(),
            remote_port: self.remote.port,
            pid: self.pid,
        }
    }
}

pub fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn ip_addr_string(ip: IpAddr) -> String {
    ip.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_v4_converts_byte_order() {
        // localhost = 127.0.0.1 = 0x7F000001
        // 网络字节序 (big-endian) 表示后再 u32::from_be 还原
        let raw_be: u32 = u32::from_be_bytes([127, 0, 0, 1]);
        let ep = NetEndpoint::from_v4(raw_be.to_be(), 8080u16.to_be());
        assert_eq!(ep.addr, "127.0.0.1");
        assert_eq!(ep.port, 8080);
    }

    #[test]
    fn endpoint_v6_loopback() {
        let mut bytes = [0u8; 16];
        bytes[15] = 1; // ::1
        let ep = NetEndpoint::from_v6(bytes, 80u16.to_be());
        assert_eq!(ep.addr, "::1");
        assert_eq!(ep.port, 80);
    }

    #[test]
    fn conn_state_maps_mib() {
        assert_eq!(ConnState::from_mib_tcp_state(5), ConnState::Established);
        assert_eq!(ConnState::from_mib_tcp_state(2), ConnState::Listen);
        assert_eq!(ConnState::from_mib_tcp_state(99), ConnState::None);
    }

    #[test]
    fn key_differs_by_pid() {
        let mut conn = NetConn {
            proto: Proto::Tcp,
            family: Family::V4,
            local: NetEndpoint { addr: "127.0.0.1".into(), port: 80 },
            remote: NetEndpoint { addr: "1.1.1.1".into(), port: 443 },
            state: ConnState::Established,
            pid: 100,
            process_name: None,
            process_path: None,
            process_cmdline: None,
            first_seen: 0,
            last_seen: 0,
            is_current: true,
        };
        let k1 = conn.key();
        conn.pid = 200;
        let k2 = conn.key();
        assert_ne!(k1, k2);
    }
}
```

- [ ] **Step 2.3: 修改 `crates/irtool-net-monitor/src/lib.rs`**

```rust
//! 网络连接监控（v2 实装）

pub mod types;

pub use types::{ConnState, Family, NetConn, NetConnKey, NetEndpoint, Proto};
```

- [ ] **Step 2.4: 跑测试**

```bash
cargo test -p irtool-net-monitor
```

预期：`4 passed`。

- [ ] **Step 2.5: 提交**

```bash
git add crates/irtool-net-monitor/
git commit -m "feat(net): NetConn types with proto/family/state and key for history dedup"
```

---

## Task 3: TCP IPv4 枚举

**Files:**
- Create: `D:\project\IRtool-next\crates\irtool-net-monitor\src\tcp.rs`
- Modify: `D:\project\IRtool-next\crates\irtool-net-monitor\src\lib.rs`

- [ ] **Step 3.1: 写 `crates/irtool-net-monitor/src/tcp.rs`**

```rust
use crate::types::{ConnState, Family, NetConn, NetEndpoint, Proto, now_epoch_secs};
use irtool_core::IrError;
use std::mem::size_of;

#[cfg(windows)]
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCP6TABLE_OWNER_PID,
    MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_ALL,
};
#[cfg(windows)]
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};

#[cfg(windows)]
pub fn enumerate_tcp_v4() -> Result<Vec<NetConn>, IrError> {
    unsafe {
        // 第一次调用获取 buffer 大小
        let mut buf_len: u32 = 0;
        let _ = GetExtendedTcpTable(
            None,
            &mut buf_len,
            false,
            AF_INET.0 as u32,
            TCP_TABLE_OWNER_PID_ALL,
            0,
        );

        if buf_len == 0 {
            return Ok(Vec::new());
        }

        let mut buf = vec![0u8; buf_len as usize];
        let rc = GetExtendedTcpTable(
            Some(buf.as_mut_ptr() as *mut _),
            &mut buf_len,
            false,
            AF_INET.0 as u32,
            TCP_TABLE_OWNER_PID_ALL,
            0,
        );
        if rc != 0 {
            return Err(IrError::Internal(format!(
                "GetExtendedTcpTable v4 failed: {}",
                rc
            )));
        }

        let table = &*(buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
        let count = table.dwNumEntries as usize;

        // 安全访问 table 中的可变长度数组
        let row_size = size_of::<MIB_TCPROW_OWNER_PID>();
        let header_size = size_of::<u32>();
        if buf.len() < header_size + count * row_size {
            return Err(IrError::Internal(
                "tcp v4 table size insufficient".into(),
            ));
        }

        let rows_ptr = (buf.as_ptr() as *const u8).add(header_size)
            as *const MIB_TCPROW_OWNER_PID;

        let now = now_epoch_secs();
        let mut conns = Vec::with_capacity(count);
        for i in 0..count {
            let row = &*rows_ptr.add(i);
            let local = NetEndpoint::from_v4(row.dwLocalAddr, row.dwLocalPort as u16);
            let remote = NetEndpoint::from_v4(row.dwRemoteAddr, row.dwRemotePort as u16);
            let state = ConnState::from_mib_tcp_state(row.dwState);
            conns.push(NetConn {
                proto: Proto::Tcp,
                family: Family::V4,
                local,
                remote,
                state,
                pid: row.dwOwningPid,
                process_name: None,
                process_path: None,
                process_cmdline: None,
                first_seen: now,
                last_seen: now,
                is_current: true,
            });
        }
        Ok(conns)
    }
}

#[cfg(windows)]
pub fn enumerate_tcp_v6() -> Result<Vec<NetConn>, IrError> {
    unsafe {
        let mut buf_len: u32 = 0;
        let _ = GetExtendedTcpTable(
            None,
            &mut buf_len,
            false,
            AF_INET6.0 as u32,
            TCP_TABLE_OWNER_PID_ALL,
            0,
        );

        if buf_len == 0 {
            return Ok(Vec::new());
        }

        let mut buf = vec![0u8; buf_len as usize];
        let rc = GetExtendedTcpTable(
            Some(buf.as_mut_ptr() as *mut _),
            &mut buf_len,
            false,
            AF_INET6.0 as u32,
            TCP_TABLE_OWNER_PID_ALL,
            0,
        );
        if rc != 0 {
            return Err(IrError::Internal(format!(
                "GetExtendedTcpTable v6 failed: {}",
                rc
            )));
        }

        let table = &*(buf.as_ptr() as *const MIB_TCP6TABLE_OWNER_PID);
        let count = table.dwNumEntries as usize;

        let row_size = size_of::<MIB_TCP6ROW_OWNER_PID>();
        let header_size = size_of::<u32>();
        if buf.len() < header_size + count * row_size {
            return Err(IrError::Internal(
                "tcp v6 table size insufficient".into(),
            ));
        }

        let rows_ptr = (buf.as_ptr() as *const u8).add(header_size)
            as *const MIB_TCP6ROW_OWNER_PID;

        let now = now_epoch_secs();
        let mut conns = Vec::with_capacity(count);
        for i in 0..count {
            let row = &*rows_ptr.add(i);
            let local =
                NetEndpoint::from_v6(row.ucLocalAddr, row.dwLocalPort as u16);
            let remote =
                NetEndpoint::from_v6(row.ucRemoteAddr, row.dwRemotePort as u16);
            let state = ConnState::from_mib_tcp_state(row.dwState);
            conns.push(NetConn {
                proto: Proto::Tcp,
                family: Family::V6,
                local,
                remote,
                state,
                pid: row.dwOwningPid,
                process_name: None,
                process_path: None,
                process_cmdline: None,
                first_seen: now,
                last_seen: now,
                is_current: true,
            });
        }
        Ok(conns)
    }
}

#[cfg(not(windows))]
pub fn enumerate_tcp_v4() -> Result<Vec<NetConn>, IrError> {
    Err(IrError::Internal("tcp v4 only supported on Windows".into()))
}

#[cfg(not(windows))]
pub fn enumerate_tcp_v6() -> Result<Vec<NetConn>, IrError> {
    Err(IrError::Internal("tcp v6 only supported on Windows".into()))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn tcp_v4_returns_some_listening() {
        let conns = enumerate_tcp_v4().expect("query failed");
        // Windows 总会有 LISTENING 项（svchost / system services）
        assert!(!conns.is_empty(), "expected at least one TCP v4 connection");
        for c in &conns {
            assert_eq!(c.proto, Proto::Tcp);
            assert_eq!(c.family, Family::V4);
        }
    }

    #[test]
    fn tcp_v6_query_does_not_panic() {
        let _ = enumerate_tcp_v6().expect("query failed");
    }
}
```

- [ ] **Step 3.2: 修改 `crates/irtool-net-monitor/src/lib.rs`**

```rust
//! 网络连接监控（v2 实装）

pub mod tcp;
pub mod types;

pub use types::{ConnState, Family, NetConn, NetConnKey, NetEndpoint, Proto};
```

- [ ] **Step 3.3: 跑测试**

```bash
cargo test -p irtool-net-monitor
```

预期：`6 passed`（含 TCP 2 个）。

- [ ] **Step 3.4: 验证编译**

```bash
cargo build -p irtool-net-monitor
```

如果遇到 `LNK` 链接错误（GNU 工具链典型问题），按 P0 反馈风险信号切 MSVC。

- [ ] **Step 3.5: 提交**

```bash
git add crates/irtool-net-monitor/
git commit -m "feat(net): TCP v4/v6 enumeration via GetExtendedTcpTable"
```

---

## Task 4: UDP IPv4/IPv6 枚举

**Files:**
- Create: `D:\project\IRtool-next\crates\irtool-net-monitor\src\udp.rs`
- Modify: `D:\project\IRtool-next\crates\irtool-net-monitor\src\lib.rs`

- [ ] **Step 4.1: 写 `crates/irtool-net-monitor/src/udp.rs`**

```rust
use crate::types::{ConnState, Family, NetConn, NetEndpoint, Proto, now_epoch_secs};
use irtool_core::IrError;
use std::mem::size_of;

#[cfg(windows)]
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedUdpTable, MIB_UDP6ROW_OWNER_PID, MIB_UDP6TABLE_OWNER_PID,
    MIB_UDPROW_OWNER_PID, MIB_UDPTABLE_OWNER_PID, UDP_TABLE_OWNER_PID,
};
#[cfg(windows)]
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};

#[cfg(windows)]
pub fn enumerate_udp_v4() -> Result<Vec<NetConn>, IrError> {
    unsafe {
        let mut buf_len: u32 = 0;
        let _ = GetExtendedUdpTable(
            None,
            &mut buf_len,
            false,
            AF_INET.0 as u32,
            UDP_TABLE_OWNER_PID,
            0,
        );

        if buf_len == 0 {
            return Ok(Vec::new());
        }

        let mut buf = vec![0u8; buf_len as usize];
        let rc = GetExtendedUdpTable(
            Some(buf.as_mut_ptr() as *mut _),
            &mut buf_len,
            false,
            AF_INET.0 as u32,
            UDP_TABLE_OWNER_PID,
            0,
        );
        if rc != 0 {
            return Err(IrError::Internal(format!(
                "GetExtendedUdpTable v4 failed: {}",
                rc
            )));
        }

        let table = &*(buf.as_ptr() as *const MIB_UDPTABLE_OWNER_PID);
        let count = table.dwNumEntries as usize;
        let row_size = size_of::<MIB_UDPROW_OWNER_PID>();
        let header_size = size_of::<u32>();
        if buf.len() < header_size + count * row_size {
            return Err(IrError::Internal("udp v4 table size insufficient".into()));
        }

        let rows_ptr = (buf.as_ptr() as *const u8).add(header_size)
            as *const MIB_UDPROW_OWNER_PID;

        let now = now_epoch_secs();
        let mut conns = Vec::with_capacity(count);
        for i in 0..count {
            let row = &*rows_ptr.add(i);
            let local = NetEndpoint::from_v4(row.dwLocalAddr, row.dwLocalPort as u16);
            // UDP 无远端连接概念，远端置空
            let remote = NetEndpoint {
                addr: String::new(),
                port: 0,
            };
            conns.push(NetConn {
                proto: Proto::Udp,
                family: Family::V4,
                local,
                remote,
                state: ConnState::None,
                pid: row.dwOwningPid,
                process_name: None,
                process_path: None,
                process_cmdline: None,
                first_seen: now,
                last_seen: now,
                is_current: true,
            });
        }
        Ok(conns)
    }
}

#[cfg(windows)]
pub fn enumerate_udp_v6() -> Result<Vec<NetConn>, IrError> {
    unsafe {
        let mut buf_len: u32 = 0;
        let _ = GetExtendedUdpTable(
            None,
            &mut buf_len,
            false,
            AF_INET6.0 as u32,
            UDP_TABLE_OWNER_PID,
            0,
        );

        if buf_len == 0 {
            return Ok(Vec::new());
        }

        let mut buf = vec![0u8; buf_len as usize];
        let rc = GetExtendedUdpTable(
            Some(buf.as_mut_ptr() as *mut _),
            &mut buf_len,
            false,
            AF_INET6.0 as u32,
            UDP_TABLE_OWNER_PID,
            0,
        );
        if rc != 0 {
            return Err(IrError::Internal(format!(
                "GetExtendedUdpTable v6 failed: {}",
                rc
            )));
        }

        let table = &*(buf.as_ptr() as *const MIB_UDP6TABLE_OWNER_PID);
        let count = table.dwNumEntries as usize;
        let row_size = size_of::<MIB_UDP6ROW_OWNER_PID>();
        let header_size = size_of::<u32>();
        if buf.len() < header_size + count * row_size {
            return Err(IrError::Internal("udp v6 table size insufficient".into()));
        }

        let rows_ptr = (buf.as_ptr() as *const u8).add(header_size)
            as *const MIB_UDP6ROW_OWNER_PID;

        let now = now_epoch_secs();
        let mut conns = Vec::with_capacity(count);
        for i in 0..count {
            let row = &*rows_ptr.add(i);
            let local =
                NetEndpoint::from_v6(row.ucLocalAddr, row.dwLocalPort as u16);
            let remote = NetEndpoint {
                addr: String::new(),
                port: 0,
            };
            conns.push(NetConn {
                proto: Proto::Udp,
                family: Family::V6,
                local,
                remote,
                state: ConnState::None,
                pid: row.dwOwningPid,
                process_name: None,
                process_path: None,
                process_cmdline: None,
                first_seen: now,
                last_seen: now,
                is_current: true,
            });
        }
        Ok(conns)
    }
}

#[cfg(not(windows))]
pub fn enumerate_udp_v4() -> Result<Vec<NetConn>, IrError> {
    Err(IrError::Internal("udp v4 only supported on Windows".into()))
}

#[cfg(not(windows))]
pub fn enumerate_udp_v6() -> Result<Vec<NetConn>, IrError> {
    Err(IrError::Internal("udp v6 only supported on Windows".into()))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn udp_v4_returns_results() {
        let conns = enumerate_udp_v4().expect("query failed");
        for c in &conns {
            assert_eq!(c.proto, Proto::Udp);
            assert_eq!(c.family, Family::V4);
            assert_eq!(c.state, ConnState::None);
        }
    }
}
```

- [ ] **Step 4.2: 修改 `crates/irtool-net-monitor/src/lib.rs`**

```rust
//! 网络连接监控（v2 实装）

pub mod tcp;
pub mod types;
pub mod udp;

pub use types::{ConnState, Family, NetConn, NetConnKey, NetEndpoint, Proto};
```

- [ ] **Step 4.3: 跑测试**

```bash
cargo test -p irtool-net-monitor
```

预期：`7 passed`。

- [ ] **Step 4.4: 提交**

```bash
git add crates/irtool-net-monitor/
git commit -m "feat(net): UDP v4/v6 enumeration via GetExtendedUdpTable"
```

---

## Task 5: 进程信息缓存

**Files:**
- Create: `D:\project\IRtool-next\crates\irtool-net-monitor\src\process_info.rs`
- Modify: `D:\project\IRtool-next\crates\irtool-net-monitor\src\lib.rs`

- [ ] **Step 5.1: 写 `crates/irtool-net-monitor/src/process_info.rs`**

```rust
use dashmap::DashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(windows)]
use windows::core::PWSTR;
#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(windows)]
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
    PROCESS_QUERY_LIMITED_INFORMATION,
};

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub name: String,
    pub path: Option<PathBuf>,
    pub cached_at: Instant,
}

const CACHE_TTL: Duration = Duration::from_secs(5);
const TOMBSTONE_DEAD: &str = "[已结束]";
const TOMBSTONE_DENIED: &str = "[权限不足]";

#[derive(Debug, Default, Clone)]
pub struct ProcessInfoCache {
    inner: Arc<DashMap<u32, ProcessInfo>>,
}

impl ProcessInfoCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
        }
    }

    pub fn get(&self, pid: u32) -> ProcessInfo {
        if let Some(entry) = self.inner.get(&pid) {
            if entry.cached_at.elapsed() < CACHE_TTL {
                return entry.clone();
            }
        }
        let info = lookup_process(pid);
        self.inner.insert(pid, info.clone());
        info
    }

    pub fn invalidate(&self, pid: u32) {
        self.inner.remove(&pid);
    }

    pub fn clear(&self) {
        self.inner.clear();
    }

    pub fn cleanup_expired(&self) {
        let now = Instant::now();
        self.inner
            .retain(|_, info| now.duration_since(info.cached_at) < CACHE_TTL * 4);
    }
}

#[cfg(windows)]
fn lookup_process(pid: u32) -> ProcessInfo {
    if pid == 0 {
        return ProcessInfo {
            name: "System Idle".into(),
            path: None,
            cached_at: Instant::now(),
        };
    }
    if pid == 4 {
        return ProcessInfo {
            name: "System".into(),
            path: None,
            cached_at: Instant::now(),
        };
    }

    unsafe {
        let handle: HANDLE = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(e) => {
                let name = if e.code().0 as u32 == 0x80070005 {
                    TOMBSTONE_DENIED.to_string()
                } else {
                    TOMBSTONE_DEAD.to_string()
                };
                return ProcessInfo {
                    name,
                    path: None,
                    cached_at: Instant::now(),
                };
            }
        };

        let mut buf = [0u16; 1024];
        let mut size = buf.len() as u32;
        let path_str = if QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buf.as_mut_ptr()),
            &mut size,
        )
        .is_ok()
        {
            String::from_utf16_lossy(&buf[..size as usize])
        } else {
            String::new()
        };

        let _ = CloseHandle(handle);

        let path = if !path_str.is_empty() {
            Some(PathBuf::from(&path_str))
        } else {
            None
        };

        let name = path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("PID {}", pid));

        ProcessInfo {
            name,
            path,
            cached_at: Instant::now(),
        }
    }
}

#[cfg(not(windows))]
fn lookup_process(_pid: u32) -> ProcessInfo {
    ProcessInfo {
        name: "[unsupported]".into(),
        path: None,
        cached_at: Instant::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_returns_same_within_ttl() {
        let cache = ProcessInfoCache::new();
        let info1 = cache.get(std::process::id());
        let info2 = cache.get(std::process::id());
        assert_eq!(info1.name, info2.name);
        assert_eq!(info1.cached_at, info2.cached_at);
    }

    #[test]
    fn invalidate_removes_entry() {
        let cache = ProcessInfoCache::new();
        let pid = std::process::id();
        let _ = cache.get(pid);
        cache.invalidate(pid);
        assert!(!cache.inner.contains_key(&pid));
    }

    #[cfg(windows)]
    #[test]
    fn current_process_lookup_has_name() {
        let info = lookup_process(std::process::id());
        assert!(!info.name.is_empty());
        assert!(!info.name.starts_with('['));
    }
}
```

- [ ] **Step 5.2: 修改 `crates/irtool-net-monitor/src/lib.rs`**

```rust
//! 网络连接监控（v2 实装）

pub mod process_info;
pub mod tcp;
pub mod types;
pub mod udp;

pub use process_info::{ProcessInfo, ProcessInfoCache};
pub use types::{ConnState, Family, NetConn, NetConnKey, NetEndpoint, Proto};
```

- [ ] **Step 5.3: 跑测试**

```bash
cargo test -p irtool-net-monitor
```

预期：`10 passed`。

- [ ] **Step 5.4: 提交**

```bash
git add crates/irtool-net-monitor/
git commit -m "feat(net): ProcessInfoCache with QueryFullProcessImageNameW + 5s TTL"
```

---

## Task 6: kill_process 实现

**Files:**
- Create: `D:\project\IRtool-next\crates\irtool-net-monitor\src\kill.rs`
- Modify: `D:\project\IRtool-next\crates\irtool-net-monitor\src\lib.rs`

- [ ] **Step 6.1: 写 `crates/irtool-net-monitor/src/kill.rs`**

```rust
use irtool_core::IrError;

#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, GetLastError};
#[cfg(windows)]
use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

#[cfg(windows)]
pub fn kill_process(pid: u32) -> Result<(), IrError> {
    if pid == 0 || pid == 4 {
        return Err(IrError::Internal(format!(
            "refuse to kill system pid {}",
            pid
        )));
    }
    unsafe {
        match OpenProcess(PROCESS_TERMINATE, false, pid) {
            Ok(handle) => {
                let result = TerminateProcess(handle, 1);
                let _ = CloseHandle(handle);
                if result.is_err() {
                    return Err(IrError::Internal(format!(
                        "TerminateProcess failed for pid {}",
                        pid
                    )));
                }
                Ok(())
            }
            Err(e) => {
                let last = GetLastError().0;
                if e.code().0 as u32 == 0x80070005 || last == ERROR_ACCESS_DENIED.0 {
                    Err(IrError::PermissionDenied)
                } else {
                    Err(IrError::Internal(format!(
                        "OpenProcess failed for pid {}: {}",
                        pid, e
                    )))
                }
            }
        }
    }
}

#[cfg(not(windows))]
pub fn kill_process(_pid: u32) -> Result<(), IrError> {
    Err(IrError::Internal("kill only supported on Windows".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kill_system_idle_refused() {
        let r = kill_process(0);
        assert!(r.is_err());
    }

    #[test]
    fn kill_system_refused() {
        let r = kill_process(4);
        assert!(r.is_err());
    }
}
```

- [ ] **Step 6.2: 修改 `crates/irtool-net-monitor/src/lib.rs`**

```rust
pub mod kill;
pub mod process_info;
pub mod tcp;
pub mod types;
pub mod udp;

pub use kill::kill_process;
pub use process_info::{ProcessInfo, ProcessInfoCache};
pub use types::{ConnState, Family, NetConn, NetConnKey, NetEndpoint, Proto};
```

- [ ] **Step 6.3: 跑测试 + 提交**

```bash
cargo test -p irtool-net-monitor
git add crates/irtool-net-monitor/
git commit -m "feat(net): kill_process via OpenProcess(PROCESS_TERMINATE) + TerminateProcess"
```

---

## Task 7: NetCollector 组装

**Files:**
- Create: `D:\project\IRtool-next\crates\irtool-net-monitor\src\collector.rs`
- Modify: `D:\project\IRtool-next\crates\irtool-net-monitor\src\lib.rs`

- [ ] **Step 7.1: 写 `crates/irtool-net-monitor/src/collector.rs`**

```rust
use crate::process_info::ProcessInfoCache;
use crate::tcp::{enumerate_tcp_v4, enumerate_tcp_v6};
use crate::types::NetConn;
use crate::udp::{enumerate_udp_v4, enumerate_udp_v6};
use irtool_core::IrError;
use tracing::warn;

pub trait NetCollector: Send + Sync {
    fn snapshot(&self) -> Result<Vec<NetConn>, IrError>;
}

pub struct WindowsNetCollector {
    process_cache: ProcessInfoCache,
}

impl WindowsNetCollector {
    pub fn new() -> Self {
        Self {
            process_cache: ProcessInfoCache::new(),
        }
    }

    pub fn process_cache(&self) -> &ProcessInfoCache {
        &self.process_cache
    }

    fn enrich(&self, mut conns: Vec<NetConn>) -> Vec<NetConn> {
        for c in &mut conns {
            let info = self.process_cache.get(c.pid);
            c.process_name = Some(info.name);
            c.process_path = info.path.map(|p| p.to_string_lossy().into_owned());
        }
        self.process_cache.cleanup_expired();
        conns
    }
}

impl Default for WindowsNetCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl NetCollector for WindowsNetCollector {
    fn snapshot(&self) -> Result<Vec<NetConn>, IrError> {
        let mut all = Vec::with_capacity(2048);

        match enumerate_tcp_v4() {
            Ok(v) => all.extend(v),
            Err(e) => warn!("tcp v4 failed: {}", e),
        }
        match enumerate_tcp_v6() {
            Ok(v) => all.extend(v),
            Err(e) => warn!("tcp v6 failed: {}", e),
        }
        match enumerate_udp_v4() {
            Ok(v) => all.extend(v),
            Err(e) => warn!("udp v4 failed: {}", e),
        }
        match enumerate_udp_v6() {
            Ok(v) => all.extend(v),
            Err(e) => warn!("udp v6 failed: {}", e),
        }

        Ok(self.enrich(all))
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn snapshot_returns_enriched_conns() {
        let c = WindowsNetCollector::new();
        let conns = c.snapshot().expect("snapshot failed");
        assert!(!conns.is_empty());
        for conn in &conns {
            assert!(conn.process_name.is_some(), "process name should be enriched");
        }
    }
}
```

- [ ] **Step 7.2: 修改 `crates/irtool-net-monitor/src/lib.rs`**

```rust
pub mod collector;
pub mod kill;
pub mod process_info;
pub mod tcp;
pub mod types;
pub mod udp;

pub use collector::{NetCollector, WindowsNetCollector};
pub use kill::kill_process;
pub use process_info::{ProcessInfo, ProcessInfoCache};
pub use types::{ConnState, Family, NetConn, NetConnKey, NetEndpoint, Proto};
```

- [ ] **Step 7.3: 跑测试 + 提交**

```bash
cargo test -p irtool-net-monitor
git add crates/irtool-net-monitor/
git commit -m "feat(net): NetCollector trait + WindowsNetCollector merging 4 tables"
```

---

## Task 8: 历史合并与去重

**Files:**
- Create: `D:\project\IRtool-next\crates\irtool-net-monitor\src\history.rs`
- Modify: `D:\project\IRtool-next\crates\irtool-net-monitor\src\lib.rs`

- [ ] **Step 8.1: 写 `crates/irtool-net-monitor/src/history.rs`**

```rust
use crate::types::{NetConn, NetConnKey, now_epoch_secs};
use dashmap::DashMap;
use std::sync::Arc;

/// 历史保留策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetentionPolicy {
    /// 不保留历史，仅当前快照
    None,
    /// 保留 N 秒内已断开的连接
    Seconds(u64),
    /// 永久保留
    Forever,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self::Seconds(600) // 10 分钟，与 v1 默认一致
    }
}

#[derive(Debug, Default, Clone)]
pub struct HistoryStore {
    inner: Arc<DashMap<NetConnKey, NetConn>>,
}

impl HistoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// 用最新快照合并到历史。返回合并后的所有条目（含已断开但仍保留的）。
    pub fn merge(
        &self,
        current_snapshot: Vec<NetConn>,
        retention: RetentionPolicy,
    ) -> Vec<NetConn> {
        let now = now_epoch_secs();

        // 1. 合并当前快照
        let mut current_keys = Vec::with_capacity(current_snapshot.len());
        for mut conn in current_snapshot {
            let key = conn.key();
            current_keys.push(key.clone());
            if let Some(mut existing) = self.inner.get_mut(&key) {
                // 已存在：保留 first_seen，更新 last_seen + state + is_current
                existing.last_seen = now;
                existing.state = conn.state;
                existing.is_current = true;
                existing.process_name = conn.process_name.clone();
                existing.process_path = conn.process_path.clone();
            } else {
                conn.first_seen = now;
                conn.last_seen = now;
                conn.is_current = true;
                self.inner.insert(key, conn);
            }
        }

        // 2. 标记本次快照中已不存在的为 historical
        let current_set: std::collections::HashSet<_> = current_keys.into_iter().collect();
        self.inner.iter_mut().for_each(|mut e| {
            let key = e.key().clone();
            if !current_set.contains(&key) && e.is_current {
                e.is_current = false;
            }
        });

        // 3. 按 retention 清理
        match retention {
            RetentionPolicy::None => {
                self.inner.retain(|_, v| v.is_current);
            }
            RetentionPolicy::Seconds(secs) => {
                self.inner
                    .retain(|_, v| v.is_current || now.saturating_sub(v.last_seen) <= secs);
            }
            RetentionPolicy::Forever => {}
        }

        self.inner.iter().map(|e| e.value().clone()).collect()
    }

    pub fn clear_history(&self) {
        self.inner.retain(|_, v| v.is_current);
    }

    pub fn clear_all(&self) {
        self.inner.clear();
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ConnState, Family, NetEndpoint, Proto};

    fn mk_conn(pid: u32, port: u16, state: ConnState) -> NetConn {
        NetConn {
            proto: Proto::Tcp,
            family: Family::V4,
            local: NetEndpoint {
                addr: "127.0.0.1".into(),
                port,
            },
            remote: NetEndpoint {
                addr: "1.1.1.1".into(),
                port: 443,
            },
            state,
            pid,
            process_name: Some("test.exe".into()),
            process_path: None,
            process_cmdline: None,
            first_seen: 0,
            last_seen: 0,
            is_current: true,
        }
    }

    #[test]
    fn first_snapshot_marks_all_current() {
        let store = HistoryStore::new();
        let snap = vec![
            mk_conn(100, 8080, ConnState::Established),
            mk_conn(101, 9090, ConnState::Listen),
        ];
        let merged = store.merge(snap, RetentionPolicy::Forever);
        assert_eq!(merged.len(), 2);
        for c in &merged {
            assert!(c.is_current);
        }
    }

    #[test]
    fn second_snapshot_drops_missing_marks_historical() {
        let store = HistoryStore::new();
        store.merge(
            vec![mk_conn(100, 8080, ConnState::Established)],
            RetentionPolicy::Forever,
        );
        std::thread::sleep(std::time::Duration::from_secs(1));
        let merged = store.merge(vec![], RetentionPolicy::Forever);
        assert_eq!(merged.len(), 1);
        assert!(!merged[0].is_current);
    }

    #[test]
    fn retention_none_drops_historical() {
        let store = HistoryStore::new();
        store.merge(
            vec![mk_conn(100, 8080, ConnState::Established)],
            RetentionPolicy::Forever,
        );
        let merged = store.merge(vec![], RetentionPolicy::None);
        assert_eq!(merged.len(), 0);
    }

    #[test]
    fn first_seen_preserved_across_merges() {
        let store = HistoryStore::new();
        let merged1 = store.merge(
            vec![mk_conn(100, 8080, ConnState::Established)],
            RetentionPolicy::Forever,
        );
        let first_seen = merged1[0].first_seen;
        std::thread::sleep(std::time::Duration::from_secs(1));
        let merged2 = store.merge(
            vec![mk_conn(100, 8080, ConnState::Established)],
            RetentionPolicy::Forever,
        );
        assert_eq!(merged2[0].first_seen, first_seen);
        assert!(merged2[0].last_seen >= first_seen);
    }
}
```

- [ ] **Step 8.2: 修改 `crates/irtool-net-monitor/src/lib.rs`**

```rust
pub mod collector;
pub mod history;
pub mod kill;
pub mod process_info;
pub mod tcp;
pub mod types;
pub mod udp;

pub use collector::{NetCollector, WindowsNetCollector};
pub use history::{HistoryStore, RetentionPolicy};
pub use kill::kill_process;
pub use process_info::{ProcessInfo, ProcessInfoCache};
pub use types::{ConnState, Family, NetConn, NetConnKey, NetEndpoint, Proto};
```

- [ ] **Step 8.3: 跑测试 + 提交**

```bash
cargo test -p irtool-net-monitor
git add crates/irtool-net-monitor/
git commit -m "feat(net): HistoryStore with RetentionPolicy preserves first_seen on re-emerge"
```

---

## Task 9: 后端轮询任务 + 事件 emit

**Files:**
- Create: `D:\project\IRtool-next\crates\irtool-tauri\src\state.rs`
- Create: `D:\project\IRtool-next\crates\irtool-tauri\src\events.rs`
- Modify: `D:\project\IRtool-next\crates\irtool-tauri\Cargo.toml`

- [ ] **Step 9.1: 修改 `crates/irtool-tauri/Cargo.toml` 加 net-monitor & 显式 tokio rt**

`[dependencies]` 增加（已有的不变）:

```toml
parking_lot = "0.12"
```

确认 `irtool-net-monitor` 与 `irtool-core` 已在依赖。

- [ ] **Step 9.2: 写 `crates/irtool-tauri/src/state.rs`**

```rust
use irtool_core::TaskRegistry;
use irtool_net_monitor::{HistoryStore, RetentionPolicy, WindowsNetCollector};
use parking_lot::Mutex;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct AppState {
    pub tasks: Arc<TaskRegistry>,
    pub net_collector: Arc<WindowsNetCollector>,
    pub net_history: Arc<HistoryStore>,
    pub net_polling: Arc<Mutex<NetworkPollingState>>,
}

pub struct NetworkPollingState {
    pub interval_ms: u64,
    pub paused: bool,
    pub retention: RetentionPolicy,
    pub cancel: Option<CancellationToken>,
}

impl Default for NetworkPollingState {
    fn default() -> Self {
        Self {
            interval_ms: 1000,
            paused: false,
            retention: RetentionPolicy::default(),
            cancel: None,
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(TaskRegistry::new()),
            net_collector: Arc::new(WindowsNetCollector::new()),
            net_history: Arc::new(HistoryStore::new()),
            net_polling: Arc::new(Mutex::new(NetworkPollingState::default())),
        }
    }
}
```

- [ ] **Step 9.3: 写 `crates/irtool-tauri/src/events.rs`**

```rust
//! 后端推前端的事件名常量。

pub const EVT_NETWORK_SNAPSHOT: &str = "evt_network_snapshot";
pub const EVT_NETWORK_ERROR: &str = "evt_network_error";
pub const EVT_TASK_CANCELLED: &str = "evt_task_cancelled";
pub const EVT_TASK_FAILED: &str = "evt_task_failed";
```

- [ ] **Step 9.4: 提交**

```bash
git add crates/irtool-tauri/
git commit -m "feat(tauri): AppState with NetworkPollingState + event name constants"
```

---

## Task 10: Tauri commands + bindings 同步

**Files:**
- Create: `D:\project\IRtool-next\crates\irtool-tauri\src\commands\mod.rs`
- Create: `D:\project\IRtool-next\crates\irtool-tauri\src\commands\network.rs`
- Modify: `D:\project\IRtool-next\crates\irtool-tauri\src\main.rs`

- [ ] **Step 10.1: 写 `crates/irtool-tauri/src/commands/mod.rs`**

```rust
pub mod network;
```

- [ ] **Step 10.2: 写 `crates/irtool-tauri/src/commands/network.rs`**

```rust
use crate::events::{EVT_NETWORK_ERROR, EVT_NETWORK_SNAPSHOT};
use crate::state::AppState;
use irtool_core::IrError;
use irtool_net_monitor::{kill_process, NetCollector, NetConn, RetentionPolicy};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::time::Duration;
use tauri::{Emitter, State};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NetworkSnapshotPayload {
    pub items: Vec<NetConn>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RetentionPolicyDto {
    None,
    Seconds(u64),
    Forever,
}

impl From<RetentionPolicyDto> for RetentionPolicy {
    fn from(value: RetentionPolicyDto) -> Self {
        match value {
            RetentionPolicyDto::None => RetentionPolicy::None,
            RetentionPolicyDto::Seconds(s) => RetentionPolicy::Seconds(s),
            RetentionPolicyDto::Forever => RetentionPolicy::Forever,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NetworkPollingControl {
    pub interval_ms: Option<u64>,
    pub paused: Option<bool>,
    pub retention: Option<RetentionPolicyDto>,
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_network_snapshot(
    state: State<'_, AppState>,
) -> Result<NetworkSnapshotPayload, IrError> {
    let collector = state.net_collector.clone();
    let history = state.net_history.clone();
    let retention = state.net_polling.lock().retention;
    let snap = tokio::task::spawn_blocking(move || collector.snapshot())
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))??;
    let merged = history.merge(snap, retention);
    Ok(NetworkSnapshotPayload {
        items: merged,
        timestamp: irtool_net_monitor::types::now_epoch_secs(),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_network_kill_process(pid: u32) -> Result<(), IrError> {
    tokio::task::spawn_blocking(move || kill_process(pid))
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_network_set_polling(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    control: NetworkPollingControl,
) -> Result<(), IrError> {
    let mut polling = state.net_polling.lock();
    if let Some(interval) = control.interval_ms {
        polling.interval_ms = interval.clamp(500, 60_000);
    }
    if let Some(paused) = control.paused {
        polling.paused = paused;
    }
    if let Some(retention) = control.retention {
        polling.retention = retention.into();
    }
    let new_interval = polling.interval_ms;
    let paused = polling.paused;

    // 取消旧 token、重启 task
    if let Some(token) = polling.cancel.take() {
        token.cancel();
    }
    if !paused {
        let token = CancellationToken::new();
        polling.cancel = Some(token.clone());
        drop(polling);

        let collector = state.net_collector.clone();
        let history = state.net_history.clone();
        let retention_now = state.net_polling.lock().retention;
        let app_clone = app.clone();
        tokio::spawn(async move {
            run_polling_loop(collector, history, retention_now, app_clone, new_interval, token)
                .await;
        });
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_network_clear_history(state: State<'_, AppState>) -> Result<(), IrError> {
    state.net_history.clear_history();
    Ok(())
}

async fn run_polling_loop(
    collector: std::sync::Arc<irtool_net_monitor::WindowsNetCollector>,
    history: std::sync::Arc<irtool_net_monitor::HistoryStore>,
    retention: RetentionPolicy,
    app: tauri::AppHandle,
    interval_ms: u64,
    cancel: CancellationToken,
) {
    info!(interval_ms, "network polling loop starting");
    let mut ticker = tokio::time::interval(Duration::from_millis(interval_ms));
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("network polling loop cancelled");
                break;
            }
            _ = ticker.tick() => {
                let collector_clone = collector.clone();
                let snap = tokio::task::spawn_blocking(move || collector_clone.snapshot()).await;
                match snap {
                    Ok(Ok(items)) => {
                        let merged = history.merge(items, retention);
                        let payload = NetworkSnapshotPayload {
                            items: merged,
                            timestamp: irtool_net_monitor::types::now_epoch_secs(),
                        };
                        if let Err(e) = app.emit(EVT_NETWORK_SNAPSHOT, &payload) {
                            error!("emit snapshot failed: {}", e);
                        }
                    }
                    Ok(Err(e)) => {
                        let _ = app.emit(EVT_NETWORK_ERROR, e.to_string());
                    }
                    Err(e) => {
                        let _ = app.emit(EVT_NETWORK_ERROR, format!("join error: {}", e));
                    }
                }
            }
        }
    }
}

pub fn start_default_polling(state: &AppState, app: &tauri::AppHandle) {
    let token = CancellationToken::new();
    state.net_polling.lock().cancel = Some(token.clone());
    let collector = state.net_collector.clone();
    let history = state.net_history.clone();
    let retention = state.net_polling.lock().retention;
    let interval = state.net_polling.lock().interval_ms;
    let app_clone = app.clone();
    tokio::spawn(async move {
        run_polling_loop(collector, history, retention, app_clone, interval, token).await;
    });
}
```

- [ ] **Step 10.3: 修改 `crates/irtool-tauri/src/main.rs` 注册 commands + state**

把 specta builder 部分扩成：

```rust
mod commands;
mod events;
mod logger;
mod single_instance;
mod state;
mod types;

use crate::commands::network::*;
use crate::state::AppState;
use specta_typescript::Typescript;
use tauri::Manager;
use tauri_specta::{collect_commands, Builder};
use tracing::info;

// （cmd_app_info / is_running_as_admin 保持 P0 实现，删除原 specta::Type 的 AppInfo struct 在文件顶部即可）

fn main() {
    let log_dir = /* P0 已写 */;
    let _logger_guard = logger::init_logger(log_dir.clone());

    info!("============================================");
    info!("IRtool v{} starting", env!("CARGO_PKG_VERSION"));
    info!("Admin: {}", is_running_as_admin());
    info!("============================================");

    let builder = Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            cmd_app_info,
            cmd_network_snapshot,
            cmd_network_kill_process,
            cmd_network_set_polling,
            cmd_network_clear_history,
        ]);

    #[cfg(debug_assertions)]
    {
        builder
            .export(
                Typescript::default()
                    .header("// @ts-nocheck\n// auto-generated by tauri-specta — DO NOT EDIT\n"),
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../ui/src/lib/bindings.ts"
                ),
            )
            .expect("failed to export bindings.ts");
    }

    let app_state = AppState::new();

    tauri::Builder::default()
        .manage(app_state.clone())
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            single_instance::handle_second_instance(app, args, cwd);
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            commands::network::start_default_polling(&app_state, &app.handle());
            info!("main window setup; default polling started");
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

注：`AppState` 需要 `Clone`，已在 Step 9.2 满足。`start_default_polling` 入参用 `&app.handle()`。

- [ ] **Step 10.4: 启动验证**

```bash
cd "D:/project/IRtool-next"
cargo tauri dev
```

预期：
1. bindings.ts 自动更新含 `cmdNetworkSnapshot/cmdNetworkKillProcess/cmdNetworkSetPolling/cmdNetworkClearHistory`、`NetConn`、`NetworkSnapshotPayload`、`RetentionPolicyDto` 等类型
2. 后端日志中出现 "network polling loop starting"
3. DevTools Console 跑：

```javascript
import('@tauri-apps/api/event').then(({ listen }) =>
  listen('evt_network_snapshot', e => console.log('snap', e.payload.items.length))
);
```

每秒输出一条快照（示例：`snap 312`）。

- [ ] **Step 10.5: 提交**

```bash
git add crates/irtool-tauri/ ui/src/lib/bindings.ts
git commit -m "feat(tauri): network commands + 1s polling loop emitting evt_network_snapshot"
```

---

## Task 11: 通用 DataTable 组件

**Files:**
- Create: `D:\project\IRtool-next\ui\src\components\data-table\DataTable.tsx`
- Create: `D:\project\IRtool-next\ui\src\components\data-table\columns-utils.ts`
- Modify: `D:\project\IRtool-next\ui\package.json`

- [ ] **Step 11.1: 安装依赖**

```bash
cd "D:/project/IRtool-next/ui"
pnpm add @tanstack/react-table @tanstack/react-virtual react-resizable-panels papaparse
pnpm add -D @types/papaparse
```

- [ ] **Step 11.2: 写 `ui/src/components/data-table/columns-utils.ts`**

```typescript
import type { ColumnDef } from "@tanstack/react-table";

export function lookupColumnSize<T>(
  columns: ColumnDef<T, unknown>[],
  id: string,
  fallback = 100,
): number {
  const col = columns.find((c) => (c as any).id === id || (c as any).accessorKey === id);
  return (col as any)?.size ?? fallback;
}

export function persistColumnSizes(key: string, sizes: Record<string, number>) {
  try {
    localStorage.setItem(`irtool-cols-${key}`, JSON.stringify(sizes));
  } catch {
    /* ignore */
  }
}

export function loadColumnSizes(key: string): Record<string, number> {
  try {
    const raw = localStorage.getItem(`irtool-cols-${key}`);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}
```

- [ ] **Step 11.3: 写 `ui/src/components/data-table/DataTable.tsx`**

```typescript
import * as React from "react";
import {
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type ColumnDef,
  type SortingState,
  type RowSelectionState,
} from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import { cn } from "@/lib/utils";

export interface DataTableProps<T> {
  columns: ColumnDef<T, unknown>[];
  data: T[];
  /** 每行像素高度。compact 28，normal 32 */
  rowHeight?: number;
  /** 选中行回调（单选） */
  onRowSelect?: (row: T | null) => void;
  /** 唯一行 key，用于稳定虚拟化与选中跨刷新保持 */
  getRowId: (row: T) => string;
  /** 行背景类（基于 row 状态） */
  rowClassName?: (row: T) => string | undefined;
  /** 右键菜单 */
  onRowContextMenu?: (row: T, event: React.MouseEvent) => void;
  /** 持久化标识，用于列宽 / 排序记忆 */
  persistKey?: string;
  /** 空状态 */
  empty?: React.ReactNode;
  /** 紧凑模式 */
  density?: "compact" | "normal";
}

export function DataTable<T>({
  columns,
  data,
  rowHeight,
  onRowSelect,
  getRowId,
  rowClassName,
  onRowContextMenu,
  persistKey: _persistKey,
  empty,
  density = "compact",
}: DataTableProps<T>) {
  const [sorting, setSorting] = React.useState<SortingState>([]);
  const [rowSelection, setRowSelection] = React.useState<RowSelectionState>({});
  const tableContainerRef = React.useRef<HTMLDivElement>(null);

  const computedRowHeight = rowHeight ?? (density === "compact" ? 28 : 34);

  const table = useReactTable({
    data,
    columns,
    state: { sorting, rowSelection },
    onSortingChange: setSorting,
    onRowSelectionChange: setRowSelection,
    enableMultiRowSelection: false,
    getRowId,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  React.useEffect(() => {
    if (!onRowSelect) return;
    const ids = Object.keys(rowSelection);
    if (ids.length === 0) {
      onRowSelect(null);
    } else {
      const row = data.find((d) => getRowId(d) === ids[0]);
      onRowSelect(row ?? null);
    }
  }, [rowSelection, data, onRowSelect, getRowId]);

  const rows = table.getRowModel().rows;

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => tableContainerRef.current,
    estimateSize: () => computedRowHeight,
    overscan: 12,
  });

  const virtualItems = virtualizer.getVirtualItems();
  const totalSize = virtualizer.getTotalSize();
  const paddingTop = virtualItems[0]?.start ?? 0;
  const paddingBottom =
    virtualItems.length > 0
      ? totalSize - (virtualItems[virtualItems.length - 1].end ?? 0)
      : 0;

  return (
    <div
      ref={tableContainerRef}
      className="h-full w-full overflow-auto bg-bg-base"
    >
      <table className="w-full text-sm font-sans border-collapse">
        <thead className="sticky top-0 z-10 bg-bg-elev-1">
          {table.getHeaderGroups().map((hg) => (
            <tr key={hg.id} className="border-b border-border">
              {hg.headers.map((header) => (
                <th
                  key={header.id}
                  className={cn(
                    "h-7 px-2 text-left font-medium text-fg-secondary text-xs select-none",
                    header.column.getCanSort() && "cursor-pointer hover:text-fg-primary",
                  )}
                  style={{ width: header.column.columnDef.size, minWidth: 60 }}
                  onClick={header.column.getToggleSortingHandler()}
                >
                  <div className="flex items-center gap-1">
                    {flexRender(header.column.columnDef.header, header.getContext())}
                    {{
                      asc: <span className="text-accent">▲</span>,
                      desc: <span className="text-accent">▼</span>,
                    }[header.column.getIsSorted() as string] ?? null}
                  </div>
                </th>
              ))}
            </tr>
          ))}
        </thead>
        <tbody>
          {rows.length === 0 ? (
            <tr>
              <td
                colSpan={columns.length}
                className="text-center text-fg-tertiary py-8"
              >
                {empty ?? "暂无数据"}
              </td>
            </tr>
          ) : (
            <>
              {paddingTop > 0 && (
                <tr style={{ height: paddingTop }}>
                  <td colSpan={columns.length} />
                </tr>
              )}
              {virtualItems.map((vRow) => {
                const row = rows[vRow.index];
                const original = row.original as T;
                const isSelected = row.getIsSelected();
                return (
                  <tr
                    key={row.id}
                    className={cn(
                      "border-b border-border/50 cursor-pointer transition-colors",
                      isSelected
                        ? "bg-bg-elev-2"
                        : "hover:bg-bg-elev-2/40",
                      rowClassName?.(original),
                    )}
                    style={{ height: computedRowHeight }}
                    onClick={() => row.toggleSelected()}
                    onContextMenu={(e) => {
                      if (!isSelected) row.toggleSelected();
                      onRowContextMenu?.(original, e);
                    }}
                  >
                    {row.getVisibleCells().map((cell) => (
                      <td
                        key={cell.id}
                        className="px-2 truncate text-fg-primary text-sm"
                        style={{ width: cell.column.columnDef.size }}
                      >
                        {flexRender(cell.column.columnDef.cell, cell.getContext())}
                      </td>
                    ))}
                  </tr>
                );
              })}
              {paddingBottom > 0 && (
                <tr style={{ height: paddingBottom }}>
                  <td colSpan={columns.length} />
                </tr>
              )}
            </>
          )}
        </tbody>
      </table>
    </div>
  );
}
```

- [ ] **Step 11.4: 提交**

```bash
git add ui/src/components/data-table/ ui/package.json ui/pnpm-lock.yaml
git commit -m "feat(ui): generic DataTable with TanStack Table + react-virtual + sort/select"
```

---

## Task 12: shadcn 原子组件补齐

**Files:**
- Create: `ui/src/components/ui/input.tsx`
- Create: `ui/src/components/ui/select.tsx`
- Create: `ui/src/components/ui/badge.tsx`
- Create: `ui/src/components/ui/dialog.tsx`
- Create: `ui/src/components/ui/alert-dialog.tsx`
- Create: `ui/src/components/ui/dropdown-menu.tsx`

- [ ] **Step 12.1: 安装 shadcn 依赖**

```bash
cd "D:/project/IRtool-next/ui"
pnpm add @radix-ui/react-select @radix-ui/react-dialog @radix-ui/react-alert-dialog @radix-ui/react-dropdown-menu
```

- [ ] **Step 12.2: 用 shadcn CLI 拉取（推荐）或手写**

如能联网，直接：

```bash
pnpm dlx shadcn@latest add input select badge dialog alert-dialog dropdown-menu
```

如离线，写以下内容：

`ui/src/components/ui/input.tsx`:

```typescript
import * as React from "react";
import { cn } from "@/lib/utils";

const Input = React.forwardRef<HTMLInputElement, React.InputHTMLAttributes<HTMLInputElement>>(
  ({ className, type, ...props }, ref) => (
    <input
      type={type}
      ref={ref}
      className={cn(
        "flex h-7 w-full rounded-md border border-border bg-bg-base px-2 py-1 text-sm transition-colors placeholder:text-fg-tertiary focus:outline-none focus:border-accent disabled:cursor-not-allowed disabled:opacity-50",
        className,
      )}
      {...props}
    />
  ),
);
Input.displayName = "Input";

export { Input };
```

`ui/src/components/ui/badge.tsx`:

```typescript
import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const badgeVariants = cva(
  "inline-flex items-center rounded-md border px-1.5 py-0.5 text-xs font-medium transition-colors",
  {
    variants: {
      variant: {
        default: "border-transparent bg-bg-elev-2 text-fg-primary",
        success: "border-transparent bg-success/15 text-success",
        warning: "border-transparent bg-warning/15 text-warning",
        danger: "border-transparent bg-danger/15 text-danger",
        info: "border-transparent bg-accent/15 text-accent",
        outline: "border-border text-fg-secondary",
      },
    },
    defaultVariants: { variant: "default" },
  },
);

export interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement>, VariantProps<typeof badgeVariants> {}

function Badge({ className, variant, ...props }: BadgeProps) {
  return <span className={cn(badgeVariants({ variant }), className)} {...props} />;
}

export { Badge, badgeVariants };
```

**剩余 4 个组件（select / dialog / alert-dialog / dropdown-menu）的源码内容**

shadcn 组件源码会随版本演进而变化，**强制要求用 CLI 拉取**（不要手抄过时代码）。如 Step 12.2 的 `pnpm dlx shadcn@latest add` 因网络受限失败：

1. 临时切换到本机或代理可上网，跑 `pnpm dlx shadcn@latest add select dialog alert-dialog dropdown-menu` 让它写入 `ui/src/components/ui/`
2. 或者从 https://ui.shadcn.com/docs/components/{组件名} 的 "Manual Installation" 节复制最新源码到对应文件

每个组件都是独立的 .tsx 文件；不要内联到其他组件。所有导入路径用 `@/` 别名（components.json 已配）。

- [ ] **Step 12.3: 验证 import**

```bash
cd "D:/project/IRtool-next/ui"
pnpm lint
```

- [ ] **Step 12.4: 提交**

```bash
git add ui/src/components/ui/ ui/package.json ui/pnpm-lock.yaml
git commit -m "feat(ui): shadcn input/select/badge/dialog/alert-dialog/dropdown-menu"
```

---

## Task 13: features/network 骨架

**Files:**
- Create: `ui/src/features/network/types.ts`
- Create: `ui/src/features/network/api.ts`
- Create: `ui/src/features/network/store.ts`
- Create: `ui/src/features/network/hooks.ts`
- Modify: `ui/src/locales/zh-CN.json`
- Modify: `ui/src/locales/en-US.json`

- [ ] **Step 13.1: 写 `ui/src/features/network/types.ts`**

```typescript
export type {
  NetConn,
  ConnState,
  Family,
  Proto,
  NetEndpoint,
  NetworkSnapshotPayload,
  RetentionPolicyDto,
  NetworkPollingControl,
} from "@/lib/bindings";
```

- [ ] **Step 13.2: 写 `ui/src/features/network/api.ts`**

```typescript
import { commands } from "@/lib/bindings";
import type {
  NetConn,
  NetworkPollingControl,
  NetworkSnapshotPayload,
} from "./types";

export async function snapshot(): Promise<NetworkSnapshotPayload> {
  return commands.cmdNetworkSnapshot();
}

export async function killProcess(pid: number): Promise<void> {
  return commands.cmdNetworkKillProcess(pid);
}

export async function setPolling(control: NetworkPollingControl): Promise<void> {
  return commands.cmdNetworkSetPolling(control);
}

export async function clearHistory(): Promise<void> {
  return commands.cmdNetworkClearHistory();
}

export type { NetConn };
```

- [ ] **Step 13.3: 安装 TanStack Query**

```bash
cd "D:/project/IRtool-next/ui"
pnpm add @tanstack/react-query
```

并在 `ui/src/main.tsx` 包裹 QueryClientProvider：

```typescript
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { staleTime: 0, refetchOnWindowFocus: false, retry: 1 },
  },
});

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <ThemeProvider>
        <RouterProvider router={router} />
      </ThemeProvider>
    </QueryClientProvider>
  </React.StrictMode>,
);
```

- [ ] **Step 13.4: 写 `ui/src/features/network/store.ts`**

```typescript
import { create } from "zustand";
import type { ConnState, Proto, RetentionPolicyDto } from "./types";

export interface NetworkFilters {
  search: string;
  proto: Proto | "all";
  state: ConnState | "all";
  showHistory: boolean;
}

interface NetworkState {
  filters: NetworkFilters;
  setFilter: <K extends keyof NetworkFilters>(key: K, value: NetworkFilters[K]) => void;
  resetFilters: () => void;

  paused: boolean;
  setPaused: (paused: boolean) => void;

  intervalMs: number;
  setIntervalMs: (ms: number) => void;

  retention: RetentionPolicyDto;
  setRetention: (r: RetentionPolicyDto) => void;
}

const DEFAULT_FILTERS: NetworkFilters = {
  search: "",
  proto: "all",
  state: "all",
  showHistory: true,
};

export const useNetworkStore = create<NetworkState>((set) => ({
  filters: DEFAULT_FILTERS,
  setFilter: (key, value) =>
    set((s) => ({ filters: { ...s.filters, [key]: value } })),
  resetFilters: () => set({ filters: DEFAULT_FILTERS }),

  paused: false,
  setPaused: (paused) => set({ paused }),

  intervalMs: 1000,
  setIntervalMs: (ms) => set({ intervalMs: ms }),

  retention: { Seconds: 600 } as RetentionPolicyDto,
  setRetention: (retention) => set({ retention }),
}));
```

- [ ] **Step 13.5: 写 `ui/src/features/network/hooks.ts`**

```typescript
import { useQuery, useQueryClient, useMutation } from "@tanstack/react-query";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect } from "react";
import * as api from "./api";
import { useNetworkStore } from "./store";
import type { NetworkSnapshotPayload } from "./types";

const QK_NETWORK = ["network", "snapshot"] as const;

export function useNetwork() {
  const qc = useQueryClient();
  const { paused, intervalMs, retention } = useNetworkStore();

  const query = useQuery({
    queryKey: QK_NETWORK,
    queryFn: api.snapshot,
  });

  // 监听后端推送
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    listen<NetworkSnapshotPayload>("evt_network_snapshot", (e) => {
      qc.setQueryData(QK_NETWORK, e.payload);
    }).then((u) => {
      unlisten = u;
    });
    return () => {
      unlisten?.();
    };
  }, [qc]);

  // 同步暂停/间隔/保留策略到后端
  // 注意：后端 NetworkPollingControl 字段是 snake_case（specta 默认不转 case），
  // 前端 store 用 camelCase 习惯，所以这里需要显式映射。
  useEffect(() => {
    api
      .setPolling({
        interval_ms: intervalMs,
        paused,
        retention,
      })
      .catch(console.error);
  }, [paused, intervalMs, retention]);

  return query;
}

export function useKillProcess() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: api.killProcess,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: QK_NETWORK });
    },
  });
}

export function useClearHistory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: api.clearHistory,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: QK_NETWORK });
    },
  });
}
```

- [ ] **Step 13.6: 扩展 i18n 资源**

`ui/src/locales/zh-CN.json` 加 `network` 节：

```json
{
  "network": {
    "title": "网络监控",
    "toolbar": {
      "refresh": "刷新",
      "pause": "暂停",
      "resume": "恢复",
      "interval": "间隔",
      "interval-1s": "1 秒",
      "interval-2s": "2 秒",
      "interval-5s": "5 秒",
      "proto-all": "全部协议",
      "state-all": "全部状态",
      "search-placeholder": "PID / IP / 端口 / 进程名",
      "show-history": "显示历史",
      "clear-history": "清空历史",
      "export-csv": "导出 CSV",
      "kill-process": "终止进程",
      "retention": "历史保留",
      "retention-1m": "1 分钟",
      "retention-5m": "5 分钟",
      "retention-10m": "10 分钟",
      "retention-forever": "永久"
    },
    "columns": {
      "proto": "协议",
      "family": "族",
      "local": "本地地址",
      "remote": "远程地址",
      "state": "状态",
      "pid": "PID",
      "process": "进程名",
      "path": "路径",
      "first-seen": "首次出现",
      "last-seen": "最近出现"
    },
    "stats": {
      "endpoints": "端点",
      "established": "已建立",
      "listening": "监听",
      "time-wait": "TimeWait",
      "close-wait": "CloseWait",
      "history": "历史"
    },
    "detail": {
      "title": "连接详情",
      "process": "进程信息",
      "command-line": "命令行",
      "command-line-pending": "命令行暂未实装（P4 进程能力）",
      "first-seen": "首次出现",
      "last-seen": "最近出现",
      "select-row": "请选择一条连接查看详情"
    },
    "kill-confirm": {
      "title": "确认终止进程",
      "message": "将终止 PID {{pid}} ({{name}})。此操作不可撤销。",
      "confirm": "终止",
      "cancel": "取消"
    },
    "context-menu": {
      "copy-row": "复制行",
      "open-explorer": "在资源管理器中打开",
      "kill": "终止进程",
      "search-workspace": "在工作台搜索"
    }
  }
}
```

`en-US.json` 同结构英文翻译（略，按字面翻）。

- [ ] **Step 13.7: 提交**

```bash
git add ui/src/features/network/ ui/src/locales/ ui/src/main.tsx ui/package.json ui/pnpm-lock.yaml
git commit -m "feat(ui/network): types + api + store + hooks + i18n + QueryClient"
```

---

## Task 14: NetworkToolbar

**Files:**
- Create: `ui/src/features/network/components/NetworkToolbar.tsx`

- [ ] **Step 14.1: 写 NetworkToolbar**

```typescript
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Pause, Play, RefreshCcw, Trash2, Download, X } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useNetworkStore } from "../store";
import type { ConnState, Proto, RetentionPolicyDto } from "../types";

interface Props {
  onExport: () => void;
  onClearHistory: () => void;
  onKillSelected: () => void;
  hasSelection: boolean;
  loading: boolean;
}

const PROTO_OPTIONS: Array<Proto | "all"> = ["all", "tcp", "udp"];
const STATE_OPTIONS: Array<ConnState | "all"> = [
  "all",
  "ESTABLISHED",
  "LISTEN",
  "TIME_WAIT",
  "CLOSE_WAIT",
  "SYN_SENT",
  "SYN_RCVD",
];

export function NetworkToolbar({
  onExport,
  onClearHistory,
  onKillSelected,
  hasSelection,
  loading,
}: Props) {
  const { t } = useTranslation();
  const {
    filters,
    setFilter,
    paused,
    setPaused,
    intervalMs,
    setIntervalMs,
    retention,
    setRetention,
  } = useNetworkStore();

  const [searchInput, setSearchInput] = useState(filters.search);
  useEffect(() => {
    const id = setTimeout(() => setFilter("search", searchInput), 200);
    return () => clearTimeout(id);
  }, [searchInput, setFilter]);

  const retentionValue =
    retention === "Forever"
      ? "forever"
      : retention === "None"
        ? "none"
        : `s${(retention as { Seconds: number }).Seconds}`;

  const handleRetentionChange = (v: string) => {
    let next: RetentionPolicyDto;
    if (v === "forever") next = "Forever";
    else if (v === "none") next = "None";
    else next = { Seconds: parseInt(v.replace("s", ""), 10) };
    setRetention(next);
  };

  return (
    <div className="flex items-center gap-2 p-2 bg-bg-elev-1 border-b border-border">
      <Button
        variant={paused ? "secondary" : "default"}
        size="sm"
        onClick={() => setPaused(!paused)}
      >
        {paused ? <Play className="h-3.5 w-3.5" /> : <Pause className="h-3.5 w-3.5" />}
        <span className="ml-1">
          {paused ? t("network.toolbar.resume") : t("network.toolbar.pause")}
        </span>
      </Button>

      <Select value={String(intervalMs)} onValueChange={(v) => setIntervalMs(parseInt(v, 10))}>
        <SelectTrigger className="h-7 w-24 text-xs">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="1000">{t("network.toolbar.interval-1s")}</SelectItem>
          <SelectItem value="2000">{t("network.toolbar.interval-2s")}</SelectItem>
          <SelectItem value="5000">{t("network.toolbar.interval-5s")}</SelectItem>
        </SelectContent>
      </Select>

      <Select
        value={filters.proto}
        onValueChange={(v) => setFilter("proto", v as Proto | "all")}
      >
        <SelectTrigger className="h-7 w-24 text-xs">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {PROTO_OPTIONS.map((p) => (
            <SelectItem key={p} value={p}>
              {p === "all" ? t("network.toolbar.proto-all") : p.toUpperCase()}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      <Select
        value={filters.state}
        onValueChange={(v) => setFilter("state", v as ConnState | "all")}
      >
        <SelectTrigger className="h-7 w-32 text-xs">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {STATE_OPTIONS.map((s) => (
            <SelectItem key={s} value={s}>
              {s === "all" ? t("network.toolbar.state-all") : s}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      <Input
        type="text"
        placeholder={t("network.toolbar.search-placeholder")}
        value={searchInput}
        onChange={(e) => setSearchInput(e.target.value)}
        className="flex-1 max-w-xs"
      />
      {searchInput && (
        <Button
          variant="ghost"
          size="icon"
          onClick={() => setSearchInput("")}
          title="clear"
        >
          <X className="h-3.5 w-3.5" />
        </Button>
      )}

      <Select value={retentionValue} onValueChange={handleRetentionChange}>
        <SelectTrigger className="h-7 w-32 text-xs">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="s60">{t("network.toolbar.retention-1m")}</SelectItem>
          <SelectItem value="s300">{t("network.toolbar.retention-5m")}</SelectItem>
          <SelectItem value="s600">{t("network.toolbar.retention-10m")}</SelectItem>
          <SelectItem value="forever">{t("network.toolbar.retention-forever")}</SelectItem>
        </SelectContent>
      </Select>

      <div className="flex-1" />

      <Button
        variant="destructive"
        size="sm"
        onClick={onKillSelected}
        disabled={!hasSelection}
      >
        <X className="h-3.5 w-3.5 mr-1" />
        {t("network.toolbar.kill-process")}
      </Button>
      <Button variant="secondary" size="sm" onClick={onExport}>
        <Download className="h-3.5 w-3.5 mr-1" />
        {t("network.toolbar.export-csv")}
      </Button>
      <Button variant="secondary" size="sm" onClick={onClearHistory}>
        <Trash2 className="h-3.5 w-3.5 mr-1" />
        {t("network.toolbar.clear-history")}
      </Button>
      <Button variant="ghost" size="icon" disabled={loading}>
        <RefreshCcw
          className={`h-3.5 w-3.5 ${loading ? "animate-spin" : ""}`}
        />
      </Button>
    </div>
  );
}
```

- [ ] **Step 14.2: 提交**

```bash
git add ui/src/features/network/components/NetworkToolbar.tsx
git commit -m "feat(ui/network): toolbar with filters/pause/interval/retention/actions"
```

---

## Task 15: NetworkTable + columns

**Files:**
- Create: `ui/src/features/network/columns.tsx`
- Create: `ui/src/features/network/components/NetworkTable.tsx`

- [ ] **Step 15.1: 写 `ui/src/features/network/columns.tsx`**

```typescript
import type { ColumnDef } from "@tanstack/react-table";
import { Badge } from "@/components/ui/badge";
import type { NetConn, ConnState } from "./types";

const STATE_VARIANT: Partial<Record<ConnState, "default" | "success" | "warning" | "danger" | "info">> = {
  ESTABLISHED: "success",
  LISTEN: "info",
  TIME_WAIT: "warning",
  CLOSE_WAIT: "danger",
};

function fmtAddr(addr: string) {
  if (!addr || addr === "0.0.0.0" || addr === "::") return "*";
  return addr;
}

function fmtPort(port: number) {
  return port === 0 ? "*" : String(port);
}

function fmtTime(epoch: number) {
  if (!epoch) return "-";
  const d = new Date(epoch * 1000);
  return d.toLocaleString("en-GB", { hour12: false });
}

export const networkColumns: ColumnDef<NetConn>[] = [
  {
    id: "proto",
    accessorFn: (r) => r.proto,
    header: "Proto",
    size: 60,
    cell: ({ row }) => row.original.proto.toUpperCase(),
  },
  {
    id: "family",
    accessorFn: (r) => r.family,
    header: "Fam",
    size: 50,
    cell: ({ row }) => row.original.family.toUpperCase(),
  },
  {
    id: "local",
    accessorFn: (r) => `${r.local.addr}:${r.local.port}`,
    header: "Local",
    size: 200,
    cell: ({ row }) =>
      `${fmtAddr(row.original.local.addr)}:${fmtPort(row.original.local.port)}`,
  },
  {
    id: "remote",
    accessorFn: (r) => `${r.remote.addr}:${r.remote.port}`,
    header: "Remote",
    size: 200,
    cell: ({ row }) =>
      `${fmtAddr(row.original.remote.addr)}:${fmtPort(row.original.remote.port)}`,
  },
  {
    id: "state",
    accessorFn: (r) => r.state,
    header: "State",
    size: 110,
    cell: ({ row }) => {
      const s = row.original.state;
      if (!s || s === "None") return <span className="text-fg-tertiary">-</span>;
      const variant = STATE_VARIANT[s] ?? "default";
      return <Badge variant={variant}>{s}</Badge>;
    },
  },
  {
    id: "pid",
    accessorFn: (r) => r.pid,
    header: "PID",
    size: 70,
    cell: ({ row }) => (
      <span className="font-mono text-xs">{row.original.pid}</span>
    ),
  },
  {
    id: "process",
    accessorFn: (r) => r.process_name ?? "",
    header: "Process",
    size: 160,
    cell: ({ row }) => row.original.process_name ?? "",
  },
  {
    id: "path",
    accessorFn: (r) => r.process_path ?? "",
    header: "Path",
    size: 280,
    cell: ({ row }) => (
      <span className="font-mono text-xs text-fg-secondary">
        {row.original.process_path ?? ""}
      </span>
    ),
  },
  {
    id: "first_seen",
    accessorFn: (r) => r.first_seen,
    header: "First Seen",
    size: 160,
    cell: ({ row }) => (
      <span className="font-mono text-xs">{fmtTime(row.original.first_seen)}</span>
    ),
  },
  {
    id: "last_seen",
    accessorFn: (r) => r.last_seen,
    header: "Last Seen",
    size: 160,
    cell: ({ row }) => (
      <span className="font-mono text-xs">{fmtTime(row.original.last_seen)}</span>
    ),
  },
];
```

- [ ] **Step 15.2: 写 `ui/src/features/network/components/NetworkTable.tsx`**

```typescript
import { useMemo } from "react";
import { DataTable } from "@/components/data-table/DataTable";
import { networkColumns } from "../columns";
import { useNetworkStore } from "../store";
import type { NetConn } from "../types";

interface Props {
  data: NetConn[];
  onRowSelect: (row: NetConn | null) => void;
  onRowContextMenu?: (row: NetConn, event: React.MouseEvent) => void;
}

function rowKey(row: NetConn) {
  return `${row.proto}|${row.family}|${row.local.addr}:${row.local.port}|${row.remote.addr}:${row.remote.port}|${row.pid}`;
}

function rowClassName(row: NetConn) {
  if (!row.is_current) return "opacity-60";
  return undefined;
}

export function NetworkTable({ data, onRowSelect, onRowContextMenu }: Props) {
  const { filters } = useNetworkStore();

  const filtered = useMemo(() => {
    let result = data;
    if (!filters.showHistory) {
      result = result.filter((r) => r.is_current);
    }
    if (filters.proto !== "all") {
      result = result.filter((r) => r.proto === filters.proto);
    }
    if (filters.state !== "all") {
      result = result.filter((r) => r.state === filters.state);
    }
    if (filters.search.trim()) {
      const q = filters.search.toLowerCase();
      result = result.filter((r) => {
        const blob =
          `${r.pid} ${r.process_name ?? ""} ${r.process_path ?? ""} ${r.local.addr}:${r.local.port} ${r.remote.addr}:${r.remote.port}`.toLowerCase();
        return blob.includes(q);
      });
    }
    return result;
  }, [data, filters]);

  return (
    <DataTable
      columns={networkColumns}
      data={filtered}
      getRowId={rowKey}
      rowClassName={rowClassName}
      onRowSelect={onRowSelect}
      onRowContextMenu={onRowContextMenu}
      persistKey="network"
      density="compact"
    />
  );
}
```

- [ ] **Step 15.3: 提交**

```bash
git add ui/src/features/network/
git commit -m "feat(ui/network): table with columns + filtering + state badge + history fade"
```

---

## Task 16: NetworkDetail 详情面板

**Files:**
- Create: `ui/src/features/network/components/NetworkDetail.tsx`

- [ ] **Step 16.1: 写 NetworkDetail**

```typescript
import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import type { NetConn } from "../types";

interface Props {
  conn: NetConn | null;
}

function fmtTime(epoch: number) {
  if (!epoch) return "-";
  return new Date(epoch * 1000).toLocaleString("en-GB", { hour12: false });
}

export function NetworkDetail({ conn }: Props) {
  const { t } = useTranslation();

  if (!conn) {
    return (
      <div className="h-full flex items-center justify-center text-fg-tertiary text-sm p-6 text-center">
        {t("network.detail.select-row")}
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto p-4 space-y-4">
      <div>
        <div className="flex items-center gap-2 mb-2">
          <Badge variant="info">{conn.proto.toUpperCase()}</Badge>
          <Badge variant="outline">{conn.family.toUpperCase()}</Badge>
          {conn.state && conn.state !== "None" && (
            <Badge>{conn.state}</Badge>
          )}
          {!conn.is_current && <Badge variant="warning">history</Badge>}
        </div>
        <div className="text-sm font-mono text-fg-primary">
          {conn.local.addr}:{conn.local.port} → {conn.remote.addr || "*"}:
          {conn.remote.port || "*"}
        </div>
      </div>

      <Separator />

      <div>
        <div className="text-xs text-fg-tertiary mb-1">{t("network.detail.process")}</div>
        <div className="text-sm">
          <span className="text-fg-primary">{conn.process_name ?? "-"}</span>
          <span className="text-fg-tertiary ml-2 font-mono text-xs">PID {conn.pid}</span>
        </div>
        {conn.process_path && (
          <div className="text-xs font-mono text-fg-secondary mt-1 break-all">
            {conn.process_path}
          </div>
        )}
      </div>

      <Separator />

      <div>
        <div className="text-xs text-fg-tertiary mb-1">{t("network.detail.command-line")}</div>
        <div className="text-xs font-mono text-fg-tertiary italic">
          {t("network.detail.command-line-pending")}
        </div>
      </div>

      <Separator />

      <div className="grid grid-cols-2 gap-x-4 gap-y-2 text-xs">
        <div>
          <div className="text-fg-tertiary">{t("network.detail.first-seen")}</div>
          <div className="font-mono text-fg-secondary">{fmtTime(conn.first_seen)}</div>
        </div>
        <div>
          <div className="text-fg-tertiary">{t("network.detail.last-seen")}</div>
          <div className="font-mono text-fg-secondary">{fmtTime(conn.last_seen)}</div>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 16.2: 提交**

```bash
git add ui/src/features/network/components/NetworkDetail.tsx
git commit -m "feat(ui/network): detail panel with proto/state badges + process info"
```

---

## Task 17: NetworkStatsBar

**Files:**
- Create: `ui/src/features/network/components/NetworkStatsBar.tsx`

- [ ] **Step 17.1: 写 NetworkStatsBar**

```typescript
import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import type { NetConn } from "../types";

interface Props {
  data: NetConn[];
}

export function NetworkStatsBar({ data }: Props) {
  const { t } = useTranslation();
  const stats = useMemo(() => {
    let endpoints = 0;
    let established = 0;
    let listening = 0;
    let timeWait = 0;
    let closeWait = 0;
    let history = 0;
    for (const c of data) {
      endpoints++;
      if (!c.is_current) {
        history++;
        continue;
      }
      switch (c.state) {
        case "ESTABLISHED": established++; break;
        case "LISTEN": listening++; break;
        case "TIME_WAIT": timeWait++; break;
        case "CLOSE_WAIT": closeWait++; break;
      }
    }
    return { endpoints, established, listening, timeWait, closeWait, history };
  }, [data]);

  return (
    <div className="h-7 px-3 flex items-center gap-4 bg-bg-elev-1 border-t border-border text-xs text-fg-secondary">
      <span>
        {t("network.stats.endpoints")}: <span className="text-fg-primary font-medium">{stats.endpoints}</span>
      </span>
      <span className="text-success">
        {t("network.stats.established")}: <span className="font-medium">{stats.established}</span>
      </span>
      <span className="text-accent">
        {t("network.stats.listening")}: <span className="font-medium">{stats.listening}</span>
      </span>
      <span className="text-warning">
        {t("network.stats.time-wait")}: <span className="font-medium">{stats.timeWait}</span>
      </span>
      <span className="text-danger">
        {t("network.stats.close-wait")}: <span className="font-medium">{stats.closeWait}</span>
      </span>
      <div className="flex-1" />
      <span className="text-fg-tertiary">
        {t("network.stats.history")}: <span className="font-medium">{stats.history}</span>
      </span>
    </div>
  );
}
```

- [ ] **Step 17.2: 提交**

```bash
git add ui/src/features/network/components/NetworkStatsBar.tsx
git commit -m "feat(ui/network): stats bar with 6 counters"
```

---

## Task 18: 终止确认对话框 + 上下文菜单

**Files:**
- Create: `ui/src/features/network/components/KillProcessDialog.tsx`
- Modify: `ui/src/features/network/pages/NetworkPage.tsx` (Task 20 创建,这里仅约定接口)

- [ ] **Step 18.1: 写 KillProcessDialog**

```typescript
import { useTranslation } from "react-i18next";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import type { NetConn } from "../types";

interface Props {
  conn: NetConn | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: (pid: number) => void;
}

export function KillProcessDialog({ conn, open, onOpenChange, onConfirm }: Props) {
  const { t } = useTranslation();

  if (!conn) return null;

  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t("network.kill-confirm.title")}</AlertDialogTitle>
          <AlertDialogDescription>
            {t("network.kill-confirm.message", {
              pid: conn.pid,
              name: conn.process_name ?? "?",
            })}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>{t("network.kill-confirm.cancel")}</AlertDialogCancel>
          <AlertDialogAction
            className="bg-danger text-white hover:bg-danger/90"
            onClick={() => {
              onConfirm(conn.pid);
              onOpenChange(false);
            }}
          >
            {t("network.kill-confirm.confirm")}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
```

- [ ] **Step 18.2: 提交**

```bash
git add ui/src/features/network/components/KillProcessDialog.tsx
git commit -m "feat(ui/network): KillProcessDialog using shadcn AlertDialog"
```

---

## Task 19: CSV 导出

**Files:**
- Create: `ui/src/lib/csv.ts`
- Modify: `ui/src/features/network/pages/NetworkPage.tsx` (Task 20)

- [ ] **Step 19.1: 写 `ui/src/lib/csv.ts`**

```typescript
import Papa from "papaparse";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";

export async function exportCsv<T extends Record<string, unknown>>(
  rows: T[],
  fields: Array<keyof T>,
  defaultFilename: string,
): Promise<{ saved: boolean; path?: string }> {
  if (rows.length === 0) {
    return { saved: false };
  }

  const path = await save({
    title: "Export CSV",
    defaultPath: defaultFilename,
    filters: [{ name: "CSV", extensions: ["csv"] }],
  });

  if (!path) return { saved: false };

  const projected = rows.map((r) => {
    const out: Record<string, unknown> = {};
    for (const f of fields) {
      out[String(f)] = r[f] ?? "";
    }
    return out;
  });

  const csv = Papa.unparse(projected, { quotes: true });
  await writeTextFile(path, csv);
  return { saved: true, path };
}
```

注意:`@tauri-apps/plugin-fs` 在 P0 已加入依赖,如未加请补 `pnpm add @tauri-apps/plugin-fs`。Tauri 端在 `tauri.conf.json` 中需开启 `fs` permission（见下一步）。

- [ ] **Step 19.2: 修改 `crates/irtool-tauri/tauri.conf.json` capabilities**

确认 `app.security` 节内含 fs 写权限。Tauri 2 通过 `capabilities` 文件管理。在 `crates/irtool-tauri/capabilities/default.json`（如不存在则创建）：

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "default capability",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "shell:default",
    "process:default",
    "dialog:default",
    "fs:default",
    "fs:allow-write-text-file"
  ]
}
```

- [ ] **Step 19.3: 提交**

```bash
git add ui/src/lib/csv.ts crates/irtool-tauri/capabilities/
git commit -m "feat(ui): csv export via papaparse + tauri fs/dialog plugins"
```

---

## Task 20: NetworkPage 集成 + 路由 + 验证

**Files:**
- Create: `ui/src/features/network/pages/NetworkPage.tsx`
- Modify: `ui/src/routes/network.tsx`

- [ ] **Step 20.1: 写 NetworkPage**

```typescript
import { useMemo, useState } from "react";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { useTranslation } from "react-i18next";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useClearHistory, useKillProcess, useNetwork } from "../hooks";
import { NetworkToolbar } from "../components/NetworkToolbar";
import { NetworkTable } from "../components/NetworkTable";
import { NetworkDetail } from "../components/NetworkDetail";
import { NetworkStatsBar } from "../components/NetworkStatsBar";
import { KillProcessDialog } from "../components/KillProcessDialog";
import { exportCsv } from "@/lib/csv";
import type { NetConn } from "../types";

export function NetworkPage() {
  const { t } = useTranslation();
  const query = useNetwork();
  const killMutation = useKillProcess();
  const clearMutation = useClearHistory();
  const [selected, setSelected] = useState<NetConn | null>(null);
  const [killDialogOpen, setKillDialogOpen] = useState(false);
  const [contextRow, setContextRow] = useState<NetConn | null>(null);
  const [contextPos, setContextPos] = useState<{ x: number; y: number } | null>(null);

  const data = useMemo(() => query.data?.items ?? [], [query.data]);

  const handleExport = async () => {
    await exportCsv(
      data.map((c) => ({
        proto: c.proto,
        family: c.family,
        local_addr: c.local.addr,
        local_port: c.local.port,
        remote_addr: c.remote.addr,
        remote_port: c.remote.port,
        state: c.state,
        pid: c.pid,
        process_name: c.process_name,
        process_path: c.process_path,
        first_seen: new Date(c.first_seen * 1000).toISOString(),
        last_seen: new Date(c.last_seen * 1000).toISOString(),
        is_current: c.is_current,
      })),
      [
        "proto", "family", "local_addr", "local_port", "remote_addr",
        "remote_port", "state", "pid", "process_name", "process_path",
        "first_seen", "last_seen", "is_current",
      ],
      `irtool-network-${Date.now()}.csv`,
    );
  };

  const handleKill = (pid: number) => {
    killMutation.mutate(pid);
  };

  const handleContextMenu = (row: NetConn, event: React.MouseEvent) => {
    event.preventDefault();
    setContextRow(row);
    setContextPos({ x: event.clientX, y: event.clientY });
  };

  return (
    <div className="h-full flex flex-col">
      <NetworkToolbar
        onExport={handleExport}
        onClearHistory={() => clearMutation.mutate()}
        onKillSelected={() => {
          if (selected) setKillDialogOpen(true);
        }}
        hasSelection={selected != null}
        loading={query.isFetching}
      />

      <div className="flex-1 min-h-0">
        <PanelGroup direction="horizontal">
          <Panel defaultSize={70} minSize={40}>
            <NetworkTable
              data={data}
              onRowSelect={setSelected}
              onRowContextMenu={handleContextMenu}
            />
          </Panel>
          <PanelResizeHandle className="w-px bg-border hover:bg-accent transition-colors" />
          <Panel defaultSize={30} minSize={20}>
            <NetworkDetail conn={selected} />
          </Panel>
        </PanelGroup>
      </div>

      <NetworkStatsBar data={data} />

      <KillProcessDialog
        conn={selected}
        open={killDialogOpen}
        onOpenChange={setKillDialogOpen}
        onConfirm={handleKill}
      />

      {contextRow && contextPos && (
        <DropdownMenu open={true} onOpenChange={() => setContextRow(null)}>
          <DropdownMenuTrigger asChild>
            <span
              className="fixed"
              style={{ top: contextPos.y, left: contextPos.x, width: 0, height: 0 }}
            />
          </DropdownMenuTrigger>
          <DropdownMenuContent>
            <DropdownMenuItem
              onClick={() => {
                navigator.clipboard.writeText(
                  `${contextRow.proto.toUpperCase()} ${contextRow.local.addr}:${contextRow.local.port} -> ${contextRow.remote.addr}:${contextRow.remote.port} pid=${contextRow.pid} ${contextRow.process_name ?? ""}`,
                );
                setContextRow(null);
              }}
            >
              {t("network.context-menu.copy-row")}
            </DropdownMenuItem>
            <DropdownMenuItem
              onClick={() => {
                setSelected(contextRow);
                setKillDialogOpen(true);
                setContextRow(null);
              }}
            >
              {t("network.context-menu.kill")}
            </DropdownMenuItem>
            <DropdownMenuItem
              disabled
              onClick={() => {
                /* P3: 跳到工作台搜索 */
                setContextRow(null);
              }}
            >
              {t("network.context-menu.search-workspace")}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      )}
    </div>
  );
}
```

- [ ] **Step 20.2: 修改 `ui/src/routes/network.tsx`**

```typescript
import { createFileRoute } from "@tanstack/react-router";
import { NetworkPage } from "@/features/network/pages/NetworkPage";

export const Route = createFileRoute("/network")({
  component: NetworkPage,
});
```

- [ ] **Step 20.3: 完整启动验证**

```bash
cd "D:/project/IRtool-next"
cargo tauri dev
```

预期清单（逐项确认）：

1. ✅ 启动后默认进入 /network 路由
2. ✅ 工具栏显示 6 个控件 + 4 个按钮
3. ✅ 表格渲染（应有 100-500 条连接）
4. ✅ 表格列状态显示彩色 Badge
5. ✅ 切换协议过滤为 TCP，列表立即过滤
6. ✅ 输入搜索词（如 "svchost"），200ms 后过滤
7. ✅ 点击行选中，右侧详情面板显示
8. ✅ 拖动中间分隔条改变左右比例
9. ✅ 右键行弹菜单（复制 / 终止 / 在工作台搜索）
10. ✅ 复制行文本到剪贴板
11. ✅ 终止进程按钮 / 右键终止 → 弹确认对话框 → 实际终止
12. ✅ 暂停按钮：点击后底部统计不更新；恢复后恢复
13. ✅ 切换刷新间隔 1s / 2s / 5s 生效
14. ✅ 切换历史保留 1m / 5m / 10m / 永久 生效
15. ✅ 清空历史按钮：移除已断开行
16. ✅ 导出 CSV：弹保存对话框 → 文件含正确字段
17. ✅ 底部统计栏 6 项数字实时更新
18. ✅ 顶部 StatusBar 仍显示管理员信息（P0 功能未受影响）

- [ ] **Step 20.4: 性能基线验证**

人工测试 5000 行场景:

```bash
# DevTools Console 跑（模拟 5000 行历史保留）
const evt = await import('@tauri-apps/api/event');
await evt.emit('test_seed_5000', null);  // 暂时无后端 seed,跳过此项
```

实际验证：跑应用 30 分钟（让历史累积到 1000+ 行），滚动列表观察 DevTools Performance：
- FPS ≥ 55
- 内存增长 < 5MB/分钟
- 主线程不阻塞 > 16ms

如不达标：检查 columns memo、是否每次都重建 columns 数组（应在文件顶层而非组件内）、检查 React.memo 的 NetworkTable。

- [ ] **Step 20.5: 提交 P1 完成**

```bash
git add ui/src/features/network/ ui/src/routes/network.tsx
git commit -m "feat(ui/network): NetworkPage integrating toolbar/table/detail/stats with split panel"
```

- [ ] **Step 20.6: 打 tag**

```bash
git tag v2.0.0-alpha.P1
```

---

## P1 验收清单

| 维度 | 验证方式 | 通过条件 |
|---|---|---|
| Workspace 编译 | `cargo build --workspace` | 无 error，clippy `--all-targets -- -D warnings` 通过 |
| Rust 测试 | `cargo test --workspace` | 含 P0 与 P1 共 ~25 项 |
| UI 类型检查 | `cd ui && pnpm lint && pnpm build` | 无 ts 错误 |
| 启动 | `cargo tauri dev` | 进入 /network，1s 后表格自动填充 |
| 列表渲染 | 滚动 1000+ 行 | 60fps 不掉帧 |
| 搜索 | 输入关键词 | 200ms debounce 后过滤 |
| 终止进程 | 选行 → 终止 → 确认 | 实际进程消失，列表移除 |
| 历史保留 | 关闭外部连接（如关浏览器 Tab）| 行变灰，10 分钟内仍存在 |
| 导出 CSV | 工具栏 → 导出 | 生成文件含全部字段 |
| 暂停/间隔 | 控件操作 | 后端日志显示 polling restart |
| 跨 Tab 切换 | 切到 /autoruns 再切回 | 状态保留（zustand 持久 in-memory） |

---

## P1 已知留待 P2-P4 的项

- 详情面板"命令行" stub 显示"P4 进程能力"，P4 实装 `process_cmdline`
- 右键 "在 Workspace 搜索" 项 disabled，P3 工作台 Tab 接通后启用
- 详情面板"签名状态"未展示，P2 持久化检测加入签名验证后回填到 NetConn 详情
- 详情面板"规则命中"未展示，P3 规则引擎接入后填充

---

## P1 风险与监控

| 风险 | 触发信号 | 处置 |
|---|---|---|
| GNU 链接错误 | `cargo build` 报 LNK1318 / 段过多 | 立即切 MSVC（见 P0 反馈章节） |
| 大量历史导致内存膨胀 | DevTools Memory > 200MB | 调小默认 retention 到 5m |
| 进程信息查询慢 | 单次 snapshot > 200ms | 检查 ProcessInfoCache TTL，是否有反复缓存失效 |
| TanStack Table 重渲染过多 | DevTools Profiler 红色长帧 | columns 提到模块顶层，避免每次构建新 ColumnDef[] |
| Tauri emit 频率过高 | CPU 持续 > 30% | 降默认 polling 到 2s；后端 emit 加 throttle（每秒最多 1 次） |

---

## 下一阶段

P1 完成并打 tag `v2.0.0-alpha.P1` 后，进入 P2 持久化检测（autoruns + WinTrust 签名验证）。届时再产出 `2026-XX-XX-IRtool-v2-P2-autoruns.md`。

