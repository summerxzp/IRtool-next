# IRtool v2 重构设计方案

- **状态**: Draft
- **日期**: 2026-05-31
- **作者**: summerxzp_hp（与 Claude 协作）
- **基线版本**: v1.1.7（PyQt6 + Python 实现）
- **目标版本**: v2.0.0（Tauri + Rust + React 实现）

---

## 1. 背景与目标

### 1.1 现状回顾

v1.1.7 是一个面向 Windows 终端应急响应场景的桌面工具，纯 Python + PyQt6 实现。功能完整、稳定可用，但存在以下问题：

- **UI 视觉质感不足**：QtWidgets 风格陈旧，自定义主题代价高，与现代桌面软件（Linear、PowerToys、Files 等）有明显差距
- **大列表渲染压力**：Autoruns 扫描结果常 800+ 项，搜索/过滤/重排时主线程偶有掉帧；网络监控刷新时同样存在抖动
- **单文件臃肿**：`autoruns_tab.py` 1671 行、`log_collector_tab.py` 1680 行、`workspace_tab.py` 968 行，UI 层耦合度高，维护成本上升
- **包体积偏大**：PyInstaller onedir + 7z SFX 后仍有较大体积（Python 解释器 + Qt 二进制是主要负担）
- **跟手性**：Sysmon 事件订阅、autorunsc 调用、HTTP 情报查询都通过 QThread 包裹，但部分阻塞操作仍会传递到 UI

### 1.2 重构目标

1. **更美观的 UI**：现代控制台 + 安全工具混合风，深浅色双主题，风格统一
2. **更流畅的操作**：5000 行表格 60fps，启动 ≤ 1.5s，扫描类任务相比 v1 提升 ≥ 50%
3. **更小的包体积**：总安装包 ≤ 25MB（不含 Sysmon 驱动安装期资源）
4. **更可维护的代码**：单文件 ≤ 600 行，Rust crate 工作区清晰拆分，前端按 feature 分包
5. **补齐高价值能力**：进程树视图、Timeline 事件回放（应急响应常用，v1 缺失）

### 1.3 非目标（明确不做）

- 跨平台（Linux/macOS）：v2 仍只面向 Windows 10/11
- 服务化/B-S 架构：保持本地桌面单机工具定位
- 团队协作/多人审查：仅个人/单机使用
- v1 → v2 数据双向兼容：v1 的 `data/rules.json` 等可读取，但 v2 的存储格式不向后兼容
- 重写 autorunsc/Sysmon 内核能力：保留官方 Sysinternals 二进制
- **v2.0 首发不实装威胁情报 Provider**（Weibu / VirusTotal）与 **skill_scan** 模块：v1 中两者均未进入正式使用，v2.0 仅保留架构扩展点（trait + stub 实现 + 设置预留位），实装放到 v2.1+
- **v2.0 不申请数字签名证书**：用户首启需在 SmartScreen "More info → Run anyway"；发布时附 SHA256 校验
- **v2.0 不引入匿名遥测**：用户问题排查通过手动提取本地日志文件
- **WebView2 离线 fixed-version 包不打入主分发**：主安装包仅含 evergreen bootstrapper（约 120KB）；如有企业内网无网络场景，单独提供 fixed-version 包（约 80-120MB）下载链接

---

## 2. 技术栈决策

### 2.1 总体技术栈

| 层 | 选型 | 版本下限 |
|---|---|---|
| Shell | Tauri | 2.x |
| 后端语言 | Rust | 1.85 stable |
| 异步运行时 | tokio | 1.40+ |
| Win32 绑定 | windows-rs | 0.59+ |
| HTTP 客户端 | reqwest | 0.12+ |
| 序列化 | serde / serde_json | 1.x |
| 错误 | thiserror / anyhow | latest |
| 编码 | encoding_rs | 0.8+ |
| 前端框架 | React | 18.3+ |
| 构建 | Vite | 6.x |
| 语言 | TypeScript | 5.6+ |
| UI 组件 | shadcn/ui + Radix UI | latest |
| 样式 | Tailwind CSS | 4.x |
| 大表格 | TanStack Table + react-virtual | v8 / v3 |
| 数据获取/缓存 | TanStack Query | v5 |
| 状态 | Zustand | v5 |
| 图标 | lucide-react | latest |
| 日期 | date-fns | v4 |
| 图表 | Recharts | v2 |
| 路由 | TanStack Router 或 react-router | v1.x / v6 |
| 国际化 | i18next + react-i18next | v23 / v15 |

### 2.2 关键决策与理由

#### 2.2.1 为什么 Tauri 而非 Electron

- 包体积：Tauri 复用系统 WebView2，最终二进制可控制在 6-10MB；Electron 内嵌 Chromium 导致 80-120MB 起步
- 内存：Tauri 空载约 30-60MB，Electron 起步 120MB+
- 安全：Tauri 后端为 Rust，不暴露 Node.js 整套 stdlib

#### 2.2.2 为什么 Rust 全部重写而非 Python sidecar

用户场景对启动速度、签名验证速度、Sysmon 事件吞吐都敏感。Python sidecar 启动慢、IPC 开销大、解释器还要打进包内增加 25MB。一次性 Rust 重写虽然工期更长，但消除了所有运行时负担。

#### 2.2.3 为什么 React + shadcn/ui 而非 Svelte

- 生态成熟度：TanStack Table 在 React 上是事实标准，5000 行虚拟化场景已被验证
- AI 协作产出质量：React + shadcn 的代码模式最稳定
- shadcn 的 "复制源码" 策略让我们能精确控制每个组件，没有"框架黑盒"

#### 2.2.4 为什么保留 autorunsc64.exe 与 Sysmon64.exe

- autorunsc 覆盖 100+ 自启位置（注册表、服务、计划任务、驱动、Codecs、AppInit、IFEO、WMI 订阅、Office 加载项等），Rust 重写工程量极大且容易遗漏
- Sysmon 是内核驱动，必须官方安装；调用 Sysmon64.exe install/uninstall 是标准做法

#### 2.2.5 为什么签名验证用 Rust 原生 WinTrust

- sigcheck64.exe 1.8MB，每条记录调用一次进程开销大（v1 已使用批量调用 + 后台 worker 缓解）
- WinTrust API 是 Windows 官方签名验证 API，sigcheck 内部就是调它；Rust 通过 windows-rs 直接调用，单条 ≤ 10ms
- 收益：去掉 1.8MB 二进制；性能提升 5-10 倍；不再依赖外部进程

---

## 3. 总体架构

### 3.1 分层

```
┌────────────────────────────────────────────────────────────────┐
│  Front-end (React + Vite + TS, 渲染进程 / WebView2)              │
│  ┌──────────┬──────────┬──────────┬──────────┬─────────────┐    │
│  │  网络监控 │ 持久化检测│  工作台  │ 日志采集 │  全局: 设置  │    │
│  └────┬─────┴────┬─────┴────┬─────┴────┬─────┴──────┬──────┘    │
│       │ TanStack Query / Zustand / shadcn/ui / Tailwind         │
└───────┼──────────┼──────────┼──────────┼────────────┼──────────┘
        │ invoke()           │ listen() / Channel<T>
┌───────▼──────────▼──────────▼──────────▼────────────▼──────────┐
│  Tauri IPC (commands + events)                                  │
└───────┬──────────┬──────────┬──────────┬────────────┬──────────┘
        │          │          │          │            │
┌───────▼──────────▼──────────▼──────────▼────────────▼──────────┐
│  irtool-tauri (Rust 主进程: 命令注册、菜单、单实例、UAC、日志)     │
├─────────────────────────────────────────────────────────────────┤
│  Rust 业务 crates:                                               │
│   ├─ irtool-core         共享类型/错误/配置/内存仓库               │
│   ├─ irtool-net-monitor  IPHELPER + Tokio 流                     │
│   ├─ irtool-autoruns     autorunsc 调度 + WinTrust 签名验证        │
│   ├─ irtool-sysmon       EVT 订阅 + 安装/状态                     │
│   ├─ irtool-rules        规则引擎 (contains/regex/equals) + IOC   │
│   ├─ irtool-threat-intel Provider trait + Noop（v2.1+ 接入实装）    │
│   └─ irtool-process      Toolhelp32 进程树                        │
├─────────────────────────────────────────────────────────────────┤
│  系统层 / 外部二进制                                              │
│   ├─ windows-rs (Win32 / WMI / EVT / WinTrust / Toolhelp32)       │
│   ├─ tools/autorunsc64.exe                                        │
│   └─ tools/Sysmon64.exe + sysmon_config.xml                       │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 进程模型

只有一个进程：Tauri 主进程 + WebView2 渲染进程（由 Tauri 框架管理）。所有业务逻辑都在主进程的 Rust 代码中执行；前端只承担渲染、交互、状态。

### 3.3 仓库与代码组织

**推荐**：新仓库 `IRtool-next`，原仓库锁 v1.x 维护模式（仅接受关键修复）。

理由：
- v1 的 PyInstaller 工具链、Python 依赖、PyQt6 与 v2 完全不重叠
- CI 流程（构建 / 签名 / 发布）需要彻底重做，新仓库避免历史包袱
- v1 仍可与 v2 并存使用一段时间，便于回滚

新仓库目录结构：

```
IRtool-next/
├── Cargo.toml                  # workspace
├── crates/
│   ├── irtool-core/
│   ├── irtool-net-monitor/
│   ├── irtool-autoruns/
│   ├── irtool-sysmon/
│   ├── irtool-rules/
│   ├── irtool-threat-intel/
│   ├── irtool-process/
│   └── irtool-tauri/           # 二进制 crate
│       ├── src/
│       │   ├── main.rs
│       │   ├── commands/
│       │   │   ├── mod.rs
│       │   │   ├── network.rs
│       │   │   ├── autoruns.rs
│       │   │   ├── workspace.rs
│       │   │   ├── log_collector.rs
│       │   │   └── settings.rs
│       │   ├── events.rs
│       │   ├── menu.rs
│       │   ├── single_instance.rs
│       │   └── logger.rs
│       ├── icons/
│       ├── tauri.conf.json
│       └── build.rs
├── ui/                         # 前端
│   ├── package.json
│   ├── vite.config.ts
│   ├── tsconfig.json
│   ├── tailwind.config.ts
│   ├── components.json         # shadcn 配置
│   ├── index.html
│   └── src/
│       ├── main.tsx
│       ├── App.tsx
│       ├── routes/             # 路由
│       ├── features/
│       │   ├── network/
│       │   │   ├── api.ts        # invoke 包装
│       │   │   ├── store.ts      # Zustand
│       │   │   ├── hooks.ts      # useQuery / useMutation
│       │   │   ├── types.ts
│       │   │   ├── pages/
│       │   │   └── components/
│       │   ├── autoruns/
│       │   ├── workspace/
│       │   ├── log-collector/
│       │   └── settings/
│       ├── components/
│       │   ├── ui/             # shadcn 原子组件
│       │   ├── layout/         # Sidebar / TopBar / StatusBar
│       │   ├── data-table/     # 通用虚拟表格
│       │   └── shared/
│       ├── lib/
│       │   ├── ipc.ts          # Tauri invoke/event 封装
│       │   ├── format.ts
│       │   └── utils.ts
│       ├── stores/
│       └── styles/
├── data/                       # 内置规则、Sysmon 配置等只读资源
│   ├── rules.json
│   └── sysmon_config.xml
├── tools/                      # 外部二进制（runtime resource）
│   ├── autorunsc64.exe
│   └── Sysmon64.exe
├── tests/                      # 集成测试
├── scripts/                    # 打包脚本
├── docs/
└── .github/workflows/
```

---

## 4. 后端模块设计（Rust）

### 4.1 irtool-core

**职责**：跨 crate 共享的最小基础设施。

```rust
// 错误
#[derive(thiserror::Error, Debug, serde::Serialize)]
pub enum IrError {
    #[error("io: {0}")]
    Io(String),
    #[error("permission denied: requires administrator")]
    PermissionDenied,
    #[error("external tool failed: {tool} exit={code}")]
    ExternalTool { tool: String, code: i32 },
    #[error("parse error: {0}")]
    Parse(String),
    #[error("network: {0}")]
    Network(String),
    #[error("cancelled")]
    Cancelled,
    #[error("internal: {0}")]
    Internal(String),
}

// 任务句柄
pub type TaskId = u64;
pub struct TaskRegistry { /* RwLock<HashMap<TaskId, CancellationToken>> */ }

// 配置
#[derive(serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    pub theme: Theme,
    pub language: Lang,
    pub network: NetworkConfig,
    pub autoruns: AutorunsConfig,
    pub sysmon: SysmonConfig,
    pub threat_intel: ThreatIntelConfig,
}

// 数据仓库（线程安全的内存缓存）
pub struct DataStore { /* DashMap-based */ }
impl DataStore {
    pub fn put_autoruns(&self, items: Vec<AutorunItem>);
    pub fn get_autoruns(&self) -> Vec<AutorunItem>;
    pub fn put_network(&self, items: Vec<NetConn>);
    /* ... */
}
```

注意点：
- `IrError` 实现 `serde::Serialize`，能直接作为 Tauri command 的 `Result<T, IrError>` 错误返回
- `TaskRegistry` 用 `tokio_util::sync::CancellationToken`，与 `tokio::select!` 协同实现可取消任务

### 4.2 irtool-net-monitor

**职责**：枚举 TCP/UDP 连接 + IPv4/IPv6 + 进程关联。

```rust
pub struct NetConn {
    pub proto: Proto,            // TCP/UDP
    pub family: Family,          // v4/v6
    pub local: SocketAddr,
    pub remote: Option<SocketAddr>,
    pub state: ConnState,
    pub pid: u32,
    pub process_name: Option<String>,
    pub process_path: Option<PathBuf>,
    pub timestamp: SystemTime,
}

pub trait NetCollector: Send + Sync {
    fn snapshot(&self) -> Result<Vec<NetConn>, IrError>;
}

pub struct WindowsNetCollector;  // 用 GetExtendedTcpTable / GetExtendedUdpTable

pub fn start_polling(
    interval: Duration,
    on_snapshot: impl Fn(Vec<NetConn>) + Send + 'static,
    cancel: CancellationToken,
) -> tokio::task::JoinHandle<()>;
```

注意点：
- 默认 1s 轮询；进程信息（路径、命令行）首次解析后缓存到下次进程退出
- 终止进程使用 `OpenProcess(PROCESS_TERMINATE)` + `TerminateProcess`
- 导出 CSV 在前端做（拿到完整数据后），避免后端持有大对象

### 4.3 irtool-autoruns

**职责**：调度 autorunsc64.exe + 解析 CSV + 风险评估 + 签名验证。

```rust
pub struct AutorunItem {
    pub category: String,        // Run, RunOnce, Services, Tasks, ...
    pub name: String,
    pub publisher: Option<String>,
    pub image_path: Option<PathBuf>,
    pub launch_string: Option<String>,
    pub registry_path: Option<String>,
    pub enabled: bool,
    pub risk: RiskLevel,         // Low/Medium/High/Critical
    pub signature: Option<SignatureStatus>,
    pub raw: HashMap<String, String>,
}

pub struct AutorunsScanner {
    exe_path: PathBuf,
    /* ... */
}

impl AutorunsScanner {
    pub async fn scan(
        &self,
        progress: impl Fn(ScanProgress) + Send + 'static,
        cancel: CancellationToken,
    ) -> Result<Vec<AutorunItem>, IrError>;

    pub async fn delete_entry(&self, item: &AutorunItem) -> Result<(), IrError>;
}

pub mod sigcheck {
    pub enum SignatureStatus {
        Valid { signer: String, timestamp: Option<SystemTime> },
        Invalid(String),
        Unsigned,
        Unknown,
    }
    pub fn verify(path: &Path) -> Result<SignatureStatus, IrError>;
    pub fn verify_batch(paths: &[PathBuf]) -> Vec<(PathBuf, SignatureStatus)>;
}
```

注意点：
- autorunsc CSV 输出可能是 ANSI 或 UTF-16 LE BOM；用 `encoding_rs` 嗅探
- 解析使用 `csv` crate，第一列是注册表路径而非数据，需处理 quoted comma
- `verify_batch` 内部用 rayon 并行调用 WinTrust，每项 ≤ 10ms，整体 800 项 ≤ 1s
- 删除项分类型处理：注册表项删 key；任务计划用 COM ITaskService；服务用 SCM Delete API

### 4.4 irtool-sysmon

**职责**：Sysmon 状态检测、安装/卸载、事件订阅与流式推送。

```rust
pub enum SysmonState {
    NotInstalled,
    Installed { version: String, running: bool },
    Error(String),
}

pub fn detect() -> SysmonState;
pub async fn install(config_path: &Path) -> Result<(), IrError>;
pub async fn uninstall() -> Result<(), IrError>;
pub async fn update_config(config_path: &Path) -> Result<(), IrError>;

pub struct SysmonEvent {
    pub event_id: u32,           // 1=ProcessCreate, 3=NetworkConnect, 7=ImageLoad, ...
    pub time: SystemTime,
    pub computer: String,
    pub fields: HashMap<String, String>,
}

pub trait SysmonSubscriber {
    fn start(
        &mut self,
        on_event: impl Fn(SysmonEvent) + Send + 'static,
        cancel: CancellationToken,
    ) -> Result<(), IrError>;
}

pub struct EvtPullSubscriber { /* uses windows-rs EVT API */ }
```

注意点：
- 安装走 `Sysmon64.exe -accepteula -i config.xml`；用 `tokio::process::Command`，捕获 stdout/stderr 写日志
- 订阅用 EVT pull mode（`EvtSubscribe` + `EvtNext`），1s 轮询；不用 push 回调避免线程亲和性问题
- 事件 XML 用 `quick-xml` 解析；EventData 字段保留原始键名
- 频率控制：单次 fetch 上限 1000 条，避免短时间洪水

### 4.5 irtool-rules

**职责**：规则匹配引擎。

```rust
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub category: RuleCategory,    // Persistence/Network/Skill/Mixed
    pub field: String,             // 目标字段
    pub op: MatchOp,               // contains/regex/equals
    pub value: String,
    pub severity: Severity,
    pub explain: String,
    pub action: Option<ActionTemplate>,
}

pub enum MatchOp { Contains, Regex, Equals }

pub struct RuleEngine {
    rules: Vec<Rule>,
    regex_cache: DashMap<String, Regex>,
}

impl RuleEngine {
    pub fn load(path: &Path) -> Result<Self, IrError>;
    pub fn scan<T: Searchable>(&self, items: &[T]) -> Vec<RuleMatch>;
    pub fn add(&mut self, rule: Rule);
    pub fn save(&self, path: &Path) -> Result<(), IrError>;
}

pub trait Searchable {
    fn field(&self, key: &str) -> Option<&str>;
    fn entity_kind(&self) -> &str;
}
```

注意点：
- regex 使用 Rust `regex` crate；编译后缓存
- `Searchable` trait 让规则引擎可同时扫 AutorunItem / NetConn / SysmonEvent
- IOC 列表（hash/IP/domain）作为特殊规则类型，使用 `aho-corasick` 加速大规模匹配

### 4.6 irtool-threat-intel（v2.0 仅预留接口）

**职责**：哈希/IOC 查询，多 Provider 抽象。**v2.0 不实装真实 Provider**，仅保留 trait 与 NoopProvider，确保未来打开此能力时无须改动调用方。

```rust
#[async_trait::async_trait]
pub trait ThreatIntelProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn query_hash(&self, hash: &str) -> Result<IntelResult, IrError>;
    async fn query_ip(&self, ip: &str) -> Result<IntelResult, IrError>;
    async fn query_domain(&self, domain: &str) -> Result<IntelResult, IrError>;
}

// v2.0 仅此一个 Provider（始终返回 IntelResult::Disabled）
pub struct NoopProvider;

// v2.1+ 接入：保留代码骨架，feature flag 守护
#[cfg(feature = "intel-weibu")]
pub struct WeibuProvider { /* ... */ }
#[cfg(feature = "intel-virustotal")]
pub struct VirusTotalProvider { /* ... */ }

pub struct ThreatIntelService {
    providers: Vec<Box<dyn ThreatIntelProvider>>,
    // v2.0 缓存与限流可不实现，留 stub
}
```

注意点：
- crate 必须存在并被 `irtool-tauri` 依赖，让 commands 能编译
- `cmd_threat_query*` 系列在 v2.0 一律返回 `IntelResult::Disabled` 或错误码 `feature_disabled`
- 前端在工作台和详情面板**不展示**威胁情报相关 UI（不留空入口）；v2.1+ 接入时同步打开 UI
- 未来打开 Provider 时再考虑：`governor` QPS 控制、`moka` 缓存 + TTL、API key 通过 Windows DPAPI 加密持久化

### 4.7 irtool-process

**职责**：进程树枚举（v2 新增能力）。

```rust
pub struct ProcessNode {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    pub path: Option<PathBuf>,
    pub cmdline: Option<String>,
    pub user: Option<String>,
    pub start_time: SystemTime,
    pub children: Vec<ProcessNode>,
}

pub fn snapshot_tree() -> Result<Vec<ProcessNode>, IrError>;
pub fn snapshot_flat() -> Result<Vec<ProcessNode>, IrError>;
pub fn kill(pid: u32) -> Result<(), IrError>;
```

注意点：
- 用 `CreateToolhelp32Snapshot` + `Process32First/Next` 枚举
- 父子关系用 PPID 重建树；处理 PID 复用问题（参考创建时间）

### 4.8 irtool-tauri

**职责**：Tauri 主进程入口。注册所有 commands 与 events，装配菜单、单实例、UAC、日志。

#### 4.8.1 单实例与 UAC

```toml
# tauri.conf.json
{
  "bundle": {
    "windows": {
      "wix": {
        "language": ["zh-CN"]
      }
    }
  },
  "app": {
    "windows": [
      { "title": "IRtool", "width": 1280, "height": 800, "minWidth": 960, "minHeight": 600 }
    ]
  }
}
```

UAC 通过 manifest 文件配置 `requireAdministrator`；启动时若未提权则 `ShellExecuteW("runas", ...)` 重启自己（与 v1 一致）。

单实例使用 `tauri-plugin-single-instance`，第二个实例启动时把现有窗口拉到前台。

#### 4.8.2 commands（前后端契约）

详见 §6 数据流。

#### 4.8.3 日志

- 用 `tracing` + `tracing-appender`，每日轮转，单文件 ≤ 10MB，保留 5 个
- 日志写入 `%LOCALAPPDATA%\IRtool\logs\` 或便携模式下应用同目录
- 启动时打印 AppID/Version/Build/AppDir 头（沿用 v1 习惯）

---

## 5. 前端模块设计（React）

### 5.1 顶层布局

```
┌─────────────────────────────────────────────────────────┐
│ [Logo IRtool v2.0]            [搜索全局]  [设置] [关于]  │  TopBar (40px)
├─────┬───────────────────────────────────────────────────┤
│  📡 │                                                    │
│  网络│                                                    │
│  📜 │              当前 Feature 主区域                    │
│  日志│                                                    │
│  🔄 │                                                    │
│  持久│                                                    │
│  🛠️ │                                                    │
│  工作│                                                    │
│  ──  │                                                    │
│  ⚙️ │                                                    │
│ Side │                                                    │
│ 56px │                                                    │
├─────┴───────────────────────────────────────────────────┤
│ ✓ 管理员 │ Sysmon 已运行 │ 12 项异常 │ 09:42:15           │  StatusBar (24px)
└─────────────────────────────────────────────────────────┘
```

理由：
- 侧边栏比顶部 Tab 更适合长期 Tab 增长（v2 可能再加进程树、文件取证）
- 顶部留搜索栏便于 Cmd-K 风格全局调用
- 状态栏永远显示关键运行信息

### 5.2 设计令牌

```css
/* tailwind.config.ts 派生 + 全局 CSS variables */
:root[data-theme="dark"] {
  --bg-base: #0b0d10;          /* 整体深色背景 */
  --bg-elev-1: #14171c;        /* 卡片/面板 */
  --bg-elev-2: #1c2127;        /* hover/选中 */
  --border: #262c34;
  --fg-primary: #e6e8eb;
  --fg-secondary: #9aa3ad;
  --fg-tertiary: #6b7480;
  --accent: #4c8dff;           /* 主品牌色 */
  --success: #2ecc71;
  --warning: #f0b429;
  --danger: #ef4444;
  --critical: #b91c1c;
  --mono: 'JetBrains Mono', 'Cascadia Mono', monospace;
  --sans: 'Inter', 'Microsoft YaHei', sans-serif;
}
:root[data-theme="light"] { /* 镜像版 */ }
```

shadcn 的 `tailwind.config.ts` 直接接入这些 token，所有组件都用语义色而非具体值。

### 5.3 通用组件

#### 5.3.1 DataTable（虚拟化大表格）

```typescript
// components/data-table/DataTable.tsx
export interface DataTableProps<T> {
  columns: ColumnDef<T>[];
  data: T[];
  estimatedRowHeight?: number;
  onRowClick?: (row: T) => void;
  rowClassName?: (row: T) => string;
  toolbar?: React.ReactNode;
  emptyMessage?: string;
  density?: 'compact' | 'normal';
  // 列可见性、排序、过滤、分组、选择等
}
```

底层：`@tanstack/react-table` v8 + `@tanstack/react-virtual` v3。所有 Tab 的列表都通过这个组件渲染，确保一致的交互（右键菜单、列拖拽、列宽记忆、键盘导航）。

#### 5.3.2 SplitPane（多栏布局）

用 `react-resizable-panels`。持久化分栏比例到 localStorage。

#### 5.3.3 CodeBlock（命令模板/原始 XML 展示）

包装 `shiki` 做语法高亮，复制按钮，"在终端打开"按钮。

#### 5.3.4 RiskBadge / StatusBadge

shadcn `Badge` 派生，颜色映射到 `--success/--warning/--danger/--critical`。

### 5.4 features 拆分

每个 feature 是一个内聚单元：

```
features/network/
├── api.ts          # invoke 包装
├── types.ts        # NetConn、ConnState 等
├── hooks.ts        # useNetwork(), useKillProcess()
├── store.ts        # Zustand: 过滤条件、列可见性
├── pages/
│   └── NetworkPage.tsx
└── components/
    ├── NetworkTable.tsx     # 用通用 DataTable
    ├── NetworkToolbar.tsx
    ├── NetworkDetail.tsx
    └── KillProcessDialog.tsx
```

#### 5.4.1 features/network

- 主区：左 70% 表格，右 30% 详情面板（resize panel）
- 工具栏：协议过滤、状态过滤、搜索框、刷新间隔、暂停、导出
- 表格列：协议、本地地址、远程地址、状态、PID、进程名、路径、首次出现、最后出现
- 详情：进程树（向上 3 层 + 子进程）、命令行、签名状态、规则命中
- 操作：终止进程（确认）、复制行、在 Workspace 搜索（v2.0 不展示"查询情报"入口）

#### 5.4.2 features/autoruns

- 主区：左 60% 树形 + 表格混合视图，右 40% 详情面板
- 工具栏：分类过滤、签名状态过滤、搜索、扫描/取消、深度选项
- 树：默认按 category 分组（Run/Services/Tasks…），可切换扁平视图
- 详情面板分 Tab：基本信息 / 签名 / 命令行 / 规则命中 / 风险解释
- 操作：禁用 / 删除 / 跳转注册表 / 跳转任务计划 / 在 Workspace 搜索 / 加密导出

#### 5.4.3 features/workspace

- 三栏布局：左侧规则管理 / 中间结果列表 / 右侧详情与处置
- 规则编辑器对话框：表单驱动 + 实时预览匹配
- 处置命令模板：选规则匹配项 → 选模板 → 填参数 → 复制或在 cmd/PowerShell 打开
- 结果可导出 JSON/CSV/MD 报告
- **v2.0 不展示威胁情报相关 UI**（IOC 批量查询、查询情报右键项均不出现）；v2.1+ 接入 Provider 时同步打开

#### 5.4.4 features/log-collector

- 顶部状态卡片：Sysmon 状态（未安装/运行中/已停止）+ 配置版本 + 操作（安装/卸载/更新配置）
- 主区：实时事件流（虚拟表格） + Timeline 视图切换
- 事件详情：解析 EventData 字段，关联进程树
- 过滤：事件 ID 多选、时间范围、PID/进程名、关键字
- 导出：原始 EVTX 段 / CSV / JSON

#### 5.4.5 features/settings

- 主题（深 / 浅 / 跟随系统）
- 语言（zh-CN / en-US）
- 默认刷新间隔 / 默认导出格式
- 工具路径覆盖（autorunsc / Sysmon）
- 日志级别 / 日志位置打开
- **威胁情报 API key 管理**：v2.0 显示 "未启用（v2.1+ 计划接入）" 占位说明，无配置入口

### 5.5 前端 IPC 封装

```typescript
// lib/ipc.ts
import { invoke } from '@tauri-apps/api/core';
import { listen, type Event } from '@tauri-apps/api/event';

export async function call<T>(cmd: string, args?: object): Promise<T> {
  return invoke<T>(cmd, args);
}

export function on<T>(event: string, cb: (e: Event<T>) => void) {
  return listen<T>(event, cb);
}

// 任务取消
export async function cancelTask(id: number) {
  return invoke('cmd_cancel_task', { id });
}
```

业务侧用 TanStack Query 管理（首次拉取走 invoke，后续走 event 推送更新缓存）：

```typescript
// features/network/hooks.ts
export function useNetwork() {
  const qc = useQueryClient();

  // 首次或手动刷新走 invoke
  const query = useQuery({
    queryKey: ['network', 'list'],
    queryFn: () => call<NetConn[]>('cmd_network_snapshot'),
  });

  // 后续后端定时推送（默认 1s）走 listen 更新 cache
  useEffect(() => {
    const unlisten = on<NetConn[]>('evt_network_snapshot', e => {
      qc.setQueryData(['network', 'list'], e.payload);
    });
    return () => { unlisten.then(fn => fn()); };
  }, [qc]);

  return query;
}
```

实时事件（Sysmon、扫描进度）类似:

```typescript
useEffect(() => {
  const unlisten = on<SysmonEvent>('evt_sysmon_event', e => {
    addEvent(e.payload);
  });
  return () => { unlisten.then(fn => fn()); };
}, []);
```

---

## 6. 数据流（IPC 契约）

### 6.1 Commands（前端 → 后端）

| Command | 入参 | 出参 | 取消 | 说明 |
|---|---|---|---|---|
| `cmd_app_info` | - | `AppInfo` | - | 启动信息、是否管理员 |
| `cmd_get_config` | - | `AppConfig` | - | 全部设置 |
| `cmd_update_config` | `AppConfig` | `()` | - | 写盘 |
| `cmd_cancel_task` | `task_id` | `()` | - | 通用取消 |
| `cmd_network_snapshot` | - | `NetConn[]` | - | 一次性快照 |
| `cmd_network_kill_process` | `pid` | `()` | - | 终止进程 |
| `cmd_autoruns_scan` | `ScanOptions` | `task_id` | ✓ | 异步扫描，进度通过事件 |
| `cmd_autoruns_get_result` | `task_id` | `AutorunItem[]` | - | 取最终结果 |
| `cmd_autoruns_verify_signatures` | `paths[]` | `task_id` | ✓ | 批量签名验证 |
| `cmd_autoruns_delete_entry` | `entry_id` | `()` | - | 删除项 |
| `cmd_sysmon_state` | - | `SysmonState` | - | 当前状态 |
| `cmd_sysmon_install` | `config_path?` | `()` | - | 安装 |
| `cmd_sysmon_uninstall` | - | `()` | - | 卸载 |
| `cmd_sysmon_subscribe` | `filters` | `task_id` | ✓ | 启动事件订阅 |
| `cmd_rules_list` | - | `Rule[]` | - | 全部规则 |
| `cmd_rules_save` | `Rule[]` | `()` | - | 保存 |
| `cmd_rules_scan` | `entity_kind` | `RuleMatch[]` | - | 扫描内存里的实体 |
| `cmd_threat_query` | `IocQuery` | `IntelResult[]` | - | **v2.0 stub**，返回 `feature_disabled` |
| `cmd_threat_query_batch` | `IocQuery[]` | `task_id` | ✓ | **v2.0 stub**，返回 `feature_disabled` |
| `cmd_process_tree` | - | `ProcessNode[]` | - | 进程树 |
| `cmd_process_kill` | `pid` | `()` | - | 杀进程 |
| `cmd_export` | `ExportRequest` | `path` | - | 通用导出 |

### 6.2 Events（后端 → 前端）

| Event | 载荷 | 触发时机 |
|---|---|---|
| `evt_network_snapshot` | `NetConn[]` | 网络轮询每秒推送 |
| `evt_autoruns_progress` | `{ task_id, current, total, message }` | autorunsc 进度 |
| `evt_autoruns_signature_progress` | `{ task_id, current, total }` | 批量签名进度 |
| `evt_sysmon_event` | `SysmonEvent` | 单条 Sysmon 事件 |
| `evt_threat_query_result` | `{ task_id, query, results }` | **v2.0 不发出**（schema 保留） |
| `evt_task_cancelled` | `task_id` | 任务被取消 |
| `evt_task_failed` | `{ task_id, error }` | 任务失败 |

### 6.3 错误处理

- 所有 command 返回 `Result<T, IrError>`，前端用 try/catch 捕获
- 业务错误（权限、超时、外部工具失败）走 `IrError` 路径
- 致命错误（命令未注册、序列化失败）走 Tauri 默认 panic 流程，前端统一拦截显示崩溃对话框

### 6.4 取消机制

```rust
#[tauri::command]
async fn cmd_autoruns_scan(
    state: State<'_, AppState>,
    options: ScanOptions,
) -> Result<TaskId, IrError> {
    let id = state.tasks.next_id();
    let token = state.tasks.register(id);
    let scanner = state.autoruns_scanner.clone();
    let app = state.app_handle.clone();

    tokio::spawn(async move {
        let progress_emit = move |p| { let _ = app.emit("evt_autoruns_progress", p); };
        match scanner.scan(progress_emit, token).await {
            Ok(items) => state.data_store.put_autoruns(items),
            Err(e) => { let _ = app.emit("evt_task_failed", (id, e)); }
        }
    });
    Ok(id)
}
```

---

## 7. UI 设计原则

### 7.1 信息密度

- 默认 `density: compact`：行高 28px，字号 12-13px
- 大表格使用等宽字体显示路径、命令行、哈希
- 重要数值（PID、风险值、时间）右对齐，文本左对齐
- 状态指示用图标 + 文字双通道（满足色盲可访问性）

### 7.2 颜色用法（语义不审美）

- 蓝色：主操作 / 选中 / 链接
- 绿：正常 / 已签名 / 任务成功
- 黄：警告 / 中危 / 进行中
- 红：异常 / 高危 / 失败
- 深红：紧急 / 关键
- 灰：禁用 / 未知 / 次要文本

### 7.3 交互一致性

- 所有可点击行 hover 出 `bg-elev-2`，选中出左侧 2px accent 条
- 右键菜单全局统一（Radix DropdownMenu）：复制 / 在新标签打开 / 在 Workspace 搜索 / ...（v2.0 不含"查询情报"）
- 双击表格行打开详情；Esc 关闭详情
- 长任务有进度条 + 取消按钮 + 估算剩余时间
- 危险操作（终止进程、删除自启项）二次确认，确认按钮置灰 1s 防误点

### 7.4 键盘可达

- **Ctrl+P**：全局命令面板/全局搜索（跨 Tab 找进程/连接/自启项），仿 VS Code 习惯
- **Ctrl+F**：当前 Tab 内搜索（聚焦当前页工具栏的搜索框）
- Ctrl+1..4：切换 Tab（网络 / 持久化 / 工作台 / 日志）
- Ctrl+R：刷新当前 Tab
- Ctrl+E：导出当前结果
- Esc：关闭详情/对话框
- Tab/Shift+Tab：在工具栏-表格-详情面板间切换

### 7.5 空状态、加载、错误

- 空状态：图标 + 一句解释 + 一个主操作按钮
- 加载：骨架屏（不是 spinner），保持布局稳定
- 错误：错误卡片显示 `IrError.message`，带"重试"和"复制错误"按钮，重要错误带"打开日志"

---

## 8. 阶段规划

### 8.1 默认 6 周排期

| 阶段 | 周数 | 核心交付 | 关键里程碑 |
|---|---|---|---|
| **P0 脚手架** | 0.5w | Tauri+Vite 工程；Cargo workspace；shadcn/ui 安装；基础布局；UAC manifest；单实例；日志系统；深浅主题切换；CI 雏形 | 能 `cargo tauri dev` 跑出空壳并显示侧栏+TopBar+StatusBar |
| **P1 网络监控** | 1w | irtool-net-monitor crate；windows-rs 绑定；NetCollector；终止进程；通用 DataTable；前端 features/network 完整版；导出 CSV | 5000 条连接 60fps 滚动；功能等价 v1 |
| **P2 持久化检测** | 1.5w | irtool-autoruns crate；autorunsc 调度；CSV 解析；WinTrust 签名；风险评估；features/autoruns；详情面板；树形+扁平双视图；删除/禁用 | autorunsc 启动→展示首条 ≤ 3s；签名验证 800 项 ≤ 1s |
| **P3 工作台** | 1w | irtool-rules（实装）+ irtool-threat-intel（仅 trait + NoopProvider）；features/workspace 三栏；规则编辑器；命令模板；导出报告 | 跨 Tab 联调（点 Autoruns "在工作台搜索" 跳转并自动过滤） |
| **P4 日志采集 + 进程树** | 1w | irtool-sysmon crate（EVT pull subscription）；安装/卸载；features/log-collector；Timeline 视图；irtool-process + 进程树面板（嵌入 Network/Autoruns 详情） | 1000 events/分钟流式刷新无掉帧 |
| **P5 打磨与发布** | 1w | 性能基线测试；E2E（Playwright + Tauri）；崩溃捕获；i18n 中英文；MSI/EXE 打包；CI workflows（含 cargo-audit/npm-audit 周扫）；用户文档；SHA256 校验文件 | 安装包 ≤ 25MB；启动 ≤ 1.5s；首启 SmartScreen 流程文档化 |

### 8.2 紧凑 4 周排期（如需）

合并 P0 进 P1；P3 砍掉命令模板（保留规则引擎核心）；P4 砍掉 Timeline；P5 压到 0.5 周。

| 阶段 | 周数 |
|---|---|
| 网络监控（含脚手架） | 1.0w |
| 持久化检测 | 1.5w |
| 工作台（核心） | 0.7w |
| 日志采集（核心） | 0.5w |
| 打磨发布 | 0.3w |

注意：4 周排期会让 v2 首发能力略弱于 v1，不推荐。

### 8.3 提交节奏

- 每个阶段在新分支 `feat/p1-network`、`feat/p2-autoruns` …
- 每个 crate 单独 PR，crate 内部子模块单独 commit
- 前后端契约（Tauri commands）变更要在 commit message 里显式声明
- 阶段结束打 tag `v2.0.0-alpha.{P1,P2,...}`，可作为里程碑回滚点

---

## 9. 性能与体积目标

### 9.1 硬指标

| 项 | v1 现状 | v2 目标 | 测量方式 |
|---|---|---|---|
| 冷启动到首屏 | 3-5s | ≤ 1.5s | tracing 埋点 |
| 空载内存 | ~200MB | ≤ 100MB | Task Manager |
| 安装包体积 | ~70MB（SFX 内） | ≤ 25MB | 构建脚本 |
| Autoruns 全量扫描+签名 | ~30s | ≤ 12s | 阶段事件时间戳 |
| Sysmon 1000 events/min | 偶有掉帧 | 60fps 不掉帧 | Performance Profiler |
| 5000 行表格滚动 | 30-45fps | 60fps | Chrome DevTools |
| 搜索过滤响应 | 100-300ms | ≤ 50ms | 埋点 |

### 9.2 性能基线测试

新增 `tests/perf/` 目录：
- `bench_autoruns.rs`：cargo bench，autorunsc 端到端
- `bench_sigverify.rs`：1000 个文件批量签名
- `e2e/network-perf.spec.ts`：Playwright 测 5000 行表格 fps
- `e2e/sysmon-perf.spec.ts`：模拟 1000 events/min

每次发布前跑一遍，回归则阻断。

---

## 10. 测试策略

### 10.1 测试金字塔

| 层 | 工具 | 覆盖目标 |
|---|---|---|
| 单元 | `cargo test` | 解析器、规则引擎、签名验证、IOC 匹配 |
| 集成 | `cargo test --test ...` | 调用 autorunsc 模拟 fixture；EVT 订阅 mock |
| 前端组件 | Vitest + React Testing Library | DataTable、Toolbar、Dialog 行为 |
| E2E | Playwright + tauri-driver | 启动 → 切 Tab → 触发扫描 → 看结果 |

### 10.2 不再用 mock 的场景

- WinTrust 验证：用真实测试证书签名的小 exe（`testdata/signed.exe`、`testdata/unsigned.exe`）
- autorunsc：对官方 exe 加超时（30s）和最小有效输出（≥ 10 项）断言
- Sysmon：CI 跑不动驱动安装，用导出的真实 EVTX 文件回放

### 10.3 CI

GitHub Actions 工作流：
- `ci.yml`：PR 触发，跑 cargo fmt/clippy/test + 前端 lint/typecheck/test
- `release.yml`：tag 触发，构建 + SHA256 校验 + GitHub Release 上传（v2.0 不签名）
- `bench.yml`：每周一次性能基线，掉超过 10% 自动发 issue
- `audit.yml`：每周一跑 `cargo audit` + `npm audit --audit-level=high`，发现高危依赖自动开 issue

---

## 11. 打包与分发

### 11.1 构建流程

```
1. Cargo build --release（生成 irtool.exe ~6-10MB）
2. Vite build（前端 dist 嵌入 Tauri，~1MB）
3. tauri build（合成最终 exe，包含 webview2-loader）
4. 复制 tools/autorunsc64.exe + Sysmon64.exe + data/sysmon_config.xml + data/rules.json
5. 生成 SHA256 校验文件（`IRtool-v2.0.0-x64.exe.sha256`）
6. 选择性生成：
   - MSI 安装包（tauri 内置 wix）
   - 7z SFX 自解压（沿用 v1 经验）
   - ZIP（最简单分发）
```

v2.0 **不签名**：发布物附 SHA256，README 引导用户校验。

### 11.2 WebView2 兼容

- Tauri 2 默认要求 WebView2 Runtime
- 主分发包采用 `embed-bootstrapper` 模式：用户机器无 WebView2 时自动联网下载安装（bootstrapper ~120KB，对主包体积影响可忽略）
- **fixed-version 离线包不打入主分发**：估算约 80-120MB（Edge WebView2 整套），仅在企业内网无网络场景下需要时单独提供下载链接（GitHub Release attach）

### 11.3 数字签名

- v2.0 **不申请代码签名证书**
- 用户首启需在 Windows SmartScreen 选择 "More info → Run anyway"，README 提供截图说明
- 发布时同步附 SHA256 校验文件供用户验证完整性

### 11.4 自动更新

v2.0 不实现自动更新（保持 v1 简洁定位）；规划 v2.1+ 引入 `tauri-plugin-updater`。

---

## 12. 风险与注意点

### 12.1 技术风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| Rust + windows-rs 学习曲线 | 工期延长 30% | P0 阶段先打通 1 个最小 windows-rs 调用样例（如 `GetCurrentProcessId`），验证工具链 |
| WebView2 在某些企业内网无网络 | 用户无法启动 | 单独提供 fixed-version 离线包（~80-120MB）下载链接，主分发不打包 |
| autorunsc 输出格式偶尔变化 | 解析失败 | 单元测试覆盖多个版本的 fixture；解析器宽容缺失列 |
| WinTrust API 在不同 Windows 版本行为不一致 | 签名验证误判 | 测试矩阵覆盖 Win10 21H2 / Win11 23H2 / Win11 24H2 |
| Sysmon 安装失败（v1 已遇到 wevtutil 失败） | 日志采集功能不可用 | 沿用 v1 修复经验：`Sysmon64.exe -i` 直接调用，避免 wevtutil 中间层 |
| 大量事件流推送阻塞前端 | UI 卡死 | 后端聚合（每 200ms 一批）+ 前端 throttle 渲染 |

### 12.2 工程风险

| 风险 | 缓解 |
|---|---|
| Rust 重写工期超预期 | 阶段分明，P1 网络监控先打通端到端，验证架构再展开 |
| 前后端契约频繁变动 | 用 ts-rs / specta 自动从 Rust 类型生成 TS 类型定义 |
| 包体积超标 | 每个 crate 加 `--release --strip` 与 LTO；Tauri build 启用 `tauri-build` 体积优化；前端 `vite-bundle-analyzer` 持续监控 |

### 12.3 用户体验风险

| 风险 | 缓解 |
|---|---|
| v1 用户习惯顶部 Tab 突然换侧栏 | 设置里提供"经典模式"开关（顶部 Tab） |
| 深色主题在外接显示器对比度不足 | 提供 contrast 调节滑块 |
| 安全软件误报 Tauri 二进制 | v2.0 不签名，发布物附 SHA256 校验；README 提供 SmartScreen 首启指引；规划 v2.1+ 申请代码签名证书 |

### 12.4 注意点（实施时容易踩坑）

1. **windows-rs unsafe 边界**：每个 unsafe 块前注释 SAFETY，专人 review
2. **CSV 编码嗅探**：v1 已踩过 ANSI/UTF-16 BOM 坑，v2 必须从第一天就用 `encoding_rs::Encoding::for_bom`
3. **Tauri 路径解析**：开发模式 vs 打包模式的 resource 目录不同，统一通过 `app_handle.path().resource_dir()` 获取
4. **进程提权**：UAC 重启后命令行参数会丢失，需要用环境变量或临时文件传递
5. **TanStack Table 列宽持久化**：默认不持久化，需手动写 localStorage
6. **Tauri 2 Channel API**：流式数据建议用 `tauri::ipc::Channel<T>` 而非 emit，性能更好
7. **shadcn 组件不要 npm 安装**：CLI `npx shadcn@latest add` 复制源码到本地，可定制
8. **Toolhelp32Snapshot 需要权限**：跨 session 进程枚举需要 `SeDebugPrivilege`，启动时申请

---

## 13. 迁移路径

### 13.1 v1 → v2 的数据兼容

- `data/rules.json`：保持向后兼容，v2 启动时自动迁移到新格式
- 用户配置：v1 没有持久化用户配置，v2 是首次引入，不存在迁移
- 日志：v1 与 v2 日志位置不冲突，可并存

### 13.2 v1 仓库去向

- v1 锁版本 `v1.x` 进入维护模式：仅接受关键 bug 修复
- README 顶部置顶链接到 v2 仓库
- v2 README 同样链接 v1，便于过渡期回滚

### 13.3 命名

- v2 安装包文件名：`IRtool-v2.0.0-x64.msi` / `IRtool-v2.0.0-x64.exe`
- 安装目录：`%PROGRAMFILES%\IRtool\v2\`（与 v1 不冲突）
- 注册表项：`HKLM\Software\IRtool\v2`
- AppID：`com.summerxzp.IRtool.v2`（与 v1 单实例互斥锁不冲突，可同时安装两版）

---

## 14. v2.1+ 路线图

v2.0 聚焦核心重构与稳定可用，以下能力规划在 v2.1+ 迭代逐步引入。所有架构在 v2.0 都已预留接口，新增时只需补充实现，无须改动调用链。

| 能力 | 触发条件/形式 | 涉及改动点 |
|---|---|---|
| **威胁情报 Provider 激活**（Weibu / VirusTotal） | 实际使用诉求出现 | 实装 `WeibuProvider` / `VirusTotalProvider`（feature flag 守护）；打开工作台 IOC 批量查询入口；启用网络/持久化详情面板的"查询情报"右键项；激活 `cmd_threat_query*` |
| **API key 安全存储** | 与 Provider 激活同步 | 用 Windows DPAPI 加密 API key 持久化到 `%LOCALAPPDATA%\IRtool\secrets`；设置页打开 API key 配置入口 |
| **Skill Scan 模块** | v1 实际投入使用后 | 新增 `crates/irtool-skill-scan`；新增 `features/skill-scan` Tab；规则引擎引入 `RuleCategory::Skill` 处理 |
| **自动更新** | 用户体量上升后 | 引入 `tauri-plugin-updater`；构建 GitHub Release 风格的更新通道 |
| **数字代码签名** | 用户投诉 SmartScreen 较多时 | 申请代码签名证书；CI release.yml 引入签名步骤 |
| **经典 Tab 顶部布局** | v1 用户反馈强烈时 | 设置项加"经典模式"开关；保留侧栏布局为默认 |
| **Linux/macOS 跨平台** | 仅在跨场景需求出现时 | 抽象 windows-rs 调用为 trait；为非 Windows 平台提供降级实现（多数 Win32 能力无法跨平台，需重新设计） |
| **匿名遥测** | 仅在排障困难且用户认可时 | 引入第三方分析（如 PostHog 自托管），默认关闭，opt-in |

---

## 15. 附录

### 15.1 v1 → v2 模块对照

| v1 模块 | 行数 | v2 归属 | 备注 |
|---|---|---|---|
| `main.py` | 397 | `crates/irtool-tauri/src/main.rs` | UAC、单实例、日志逻辑保留 |
| `core/network_monitor.py` | 146 | `crates/irtool-net-monitor` | psutil → windows-rs |
| `core/autoruns_parser.py` | 848 | `crates/irtool-autoruns` | 解析逻辑迁移，行数预计降至 500 |
| `core/signature_parser.py` | 123 | `crates/irtool-autoruns/sigcheck` | 替换为 WinTrust 原生 |
| `core/rule_engine.py` | 458 | `crates/irtool-rules` | 行数预计 300 |
| `core/risk_hint.py` | 308 | `crates/irtool-autoruns/risk` | 内联到 autoruns crate |
| `core/threat_intel/*` | ~400 | `crates/irtool-threat-intel` | **v2.0 仅 trait + NoopProvider**；Weibu/VirusTotal Provider 推到 v2.1+ |
| `core/sysmon/*` | ~300 | `crates/irtool-sysmon` | EVT API 重写 |
| `core/skill_scan/*` + `core/skill_audit/*` | ~600 | `crates/irtool-skill-scan`（v2.1+） | **v2.0 不迁移**，v1 中未进入正式使用 |
| `core/data_store.py` | 81 | `crates/irtool-core/store` | DashMap |
| `core/icon_provider.py` | 197 | `ui/components/icons/` | 前端用 lucide-react，删除 |
| `ui/autoruns_tab.py` | 1671 | `ui/src/features/autoruns/` | 拆为多个组件文件，每个 ≤ 300 行 |
| `ui/network_tab.py` | 710 | `ui/src/features/network/` | 同上 |
| `ui/log_collector_tab.py` | 1680 | `ui/src/features/log-collector/` | 同上 |
| `ui/workspace_tab.py` | 968 | `ui/src/features/workspace/` | 同上 |
| `ui/ui_style.py` | 555 | `ui/src/styles/` + `tailwind.config.ts` | 设计令牌迁移 |
| `utils/safe_executor.py` | 212 | `crates/irtool-core/exec` | tokio::process 包装 |
| `utils/path_resolver.py` | 112 | Tauri 内置 `path()` | 删除 |
| `utils/exporter.py` | 35 | 前端 `lib/export.ts` | 前端实现 |

总行数预估：v2 Rust ~3000 行，TypeScript ~5000 行，合计 ~8000 行（v1 ~12500 行）。

### 15.2 关键依赖版本锁定示例

`Cargo.toml`（workspace 根）:

```toml
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.package]
version = "2.0.0"
edition = "2021"
authors = ["summerxzp"]

[workspace.dependencies]
tokio = { version = "1.40", features = ["full"] }
tokio-util = "0.7"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
tracing-appender = "0.2"
windows = { version = "0.59", features = [
  "Win32_Foundation",
  "Win32_System_ProcessStatus",
  "Win32_NetworkManagement_IpHelper",
  "Win32_System_EventLog",
  "Win32_Security_WinTrust",
  "Win32_Security_Cryptography_Catalog",
  "Win32_System_Diagnostics_ToolHelp",
] }
encoding_rs = "0.8"
csv = "1.3"
quick-xml = "0.36"
regex = "1.10"
aho-corasick = "1.1"
# v2.0 仅 threat-intel crate 编译时引入，运行时不发起网络请求；v2.1+ 实装 Provider 时再扩展
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
dashmap = "6"
# moka / governor 仅在 v2.1+ Provider 实装时加入 workspace dependencies
```

`ui/package.json` 关键依赖:

```json
{
  "dependencies": {
    "@tauri-apps/api": "^2",
    "@tanstack/react-query": "^5",
    "@tanstack/react-router": "^1",
    "@tanstack/react-table": "^8",
    "@tanstack/react-virtual": "^3",
    "react": "^18.3",
    "react-dom": "^18.3",
    "zustand": "^5",
    "tailwindcss": "^4",
    "lucide-react": "^0.469",
    "date-fns": "^4",
    "recharts": "^2",
    "react-resizable-panels": "^2",
    "shiki": "^1",
    "i18next": "^23",
    "react-i18next": "^15"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "@types/react": "^18",
    "typescript": "^5.6",
    "vite": "^6",
    "vitest": "^2",
    "@playwright/test": "^1",
    "tauri-driver": "*"
  }
}
```

### 15.3 Tauri commands 参数与返回类型

完整 TypeScript 类型定义将通过 `specta` 从 Rust 自动生成到 `ui/src/lib/bindings.ts`。

---

## 16. 总结

v2.0 通过 **Tauri + Rust + React + shadcn/ui** 的组合，在保留 v1 已正式使用的核心能力（网络监控 / 持久化检测 / 工作台规则引擎 / 日志采集）基础上，把 UI 现代化、性能基线、包体积同时拉升一个档次，并补齐进程树与 Timeline 这类应急响应高频能力。所选技术每一项都是当前桌面应用工程化的主流务实选择，不追新但跟得上业界节奏。

威胁情报 Provider、Skill Scan、自动更新、数字签名等能力在架构上预留接口，按 §14 路线图于 v2.1+ 按需引入，避免首发承担非必要复杂度。

实施次序遵循"端到端先打通最简单 Tab，验证整套链路再扩展"的原则。预计 6 周完成首发可用版本，4 周可压缩但首发能力会略弱于 v1。

下一步进入 writing-plans 阶段，把此设计拆解为可执行的实施计划（按阶段分解到 commit 粒度的任务列表）。
