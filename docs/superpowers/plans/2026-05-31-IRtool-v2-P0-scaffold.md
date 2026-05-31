# IRtool v2 — P0 脚手架阶段 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在新仓库 `IRtool-next` 中搭建 Tauri 2 + Rust + React + TypeScript + shadcn/ui 工程骨架，能 `cargo tauri dev` 启动一个带侧栏 + TopBar + StatusBar 的空壳应用，深浅色主题、UAC 提权、单实例、日志、i18n、CI 全部就位。

**Architecture:** Cargo workspace 拆 8 个 crate（v2.0 实际编译 7 个：core/net-monitor/autoruns/sysmon/rules/threat-intel/process/tauri；threat-intel 仅 trait + NoopProvider）。前端 Vite + React 18 工程位于 `ui/`，通过 Tauri `frontendDist` 集成。所有跨平台样板用脚本一次性生成；P0 阶段不写业务功能，只把"启动 → 看到空壳"打通并验证可发布。

**Tech Stack:** Rust 1.85 stable, Tauri 2.x, tokio 1.40, windows-rs 0.59, tracing 0.1, React 18.3, Vite 6.x, TypeScript 5.6, Tailwind CSS 4, shadcn/ui, TanStack Router 1.x, Zustand 5, react-i18next 15.

**Spec 引用:** `docs/superpowers/specs/2026-05-31-IRtool-refactor-design.md`

---

## 前置条件

实施者本地需具备：

- Windows 10/11（开发与运行均需 Windows，因为后续 crate 调用 windows-rs）
- Rust 1.85+ stable + cargo（`rustup default stable`）
- Node.js 20+ 与 pnpm 9+（`corepack enable && corepack prepare pnpm@latest --activate`）
- Git 2.40+
- Visual Studio 2022 Build Tools（C++ 桌面开发负载，Tauri 编译需 MSVC）
- WebView2 Runtime（Win11 自带；Win10 需手动安装）
- Tauri CLI: `cargo install tauri-cli --version "^2.0"` 或 `pnpm add -g @tauri-apps/cli@latest`

实施者预读：

- spec §1.2 / §3 / §5.2 / §7（重构目标、架构、设计令牌、UI 原则）
- Tauri 2 文档 https://v2.tauri.app/start/

---

## 仓库与文件结构

P0 完成后的目标目录树（仅 P0 涉及文件）：

```
IRtool-next/
├── .github/workflows/
│   ├── ci.yml
│   ├── audit.yml
│   └── bench.yml
├── .gitignore
├── .gitattributes
├── README.md
├── LICENSE
├── rustfmt.toml
├── clippy.toml
├── Cargo.toml                      # workspace 根
├── Cargo.lock
├── crates/
│   ├── irtool-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── error.rs
│   │       └── config.rs
│   ├── irtool-net-monitor/         # P1 实装；P0 仅占位 lib.rs
│   ├── irtool-autoruns/            # P2 实装；P0 仅占位
│   ├── irtool-sysmon/              # P4 实装；P0 仅占位
│   ├── irtool-rules/               # P3 实装；P0 仅占位
│   ├── irtool-threat-intel/        # P3 trait + Noop；P0 仅占位
│   ├── irtool-process/             # P4 实装；P0 仅占位
│   └── irtool-tauri/
│       ├── Cargo.toml
│       ├── build.rs
│       ├── tauri.conf.json
│       ├── icons/                  # icon.png + Square*.png + StoreLogo.png
│       ├── irtool.manifest         # UAC 提权 manifest
│       └── src/
│           ├── main.rs
│           ├── logger.rs
│           ├── single_instance.rs
│           └── menu.rs
├── ui/
│   ├── package.json
│   ├── pnpm-lock.yaml
│   ├── vite.config.ts
│   ├── tsconfig.json
│   ├── tsconfig.node.json
│   ├── tailwind.config.ts
│   ├── postcss.config.js
│   ├── components.json             # shadcn 配置
│   ├── index.html
│   └── src/
│       ├── main.tsx
│       ├── App.tsx
│       ├── routes/
│       │   ├── __root.tsx
│       │   ├── network.tsx         # 占位
│       │   ├── log-collector.tsx   # 占位
│       │   ├── autoruns.tsx        # 占位
│       │   └── workspace.tsx       # 占位
│       ├── components/
│       │   ├── ui/                 # shadcn 原子（button、tooltip、separator 由 P0 拉入）
│       │   ├── layout/
│       │   │   ├── AppShell.tsx
│       │   │   ├── Sidebar.tsx
│       │   │   ├── TopBar.tsx
│       │   │   └── StatusBar.tsx
│       │   └── theme/
│       │       └── ThemeProvider.tsx
│       ├── stores/
│       │   └── theme-store.ts
│       ├── lib/
│       │   ├── ipc.ts              # P0 仅有 call/on 包装
│       │   └── i18n.ts
│       ├── styles/
│       │   ├── globals.css         # Tailwind + 设计令牌
│       │   └── tokens.css
│       └── locales/
│           ├── zh-CN.json
│           └── en-US.json
└── tools/                          # P2/P4 阶段加入二进制；P0 仅 .gitkeep
```

**文件职责说明：**

- `Cargo.toml` (workspace 根)：定义 workspace 与共享依赖版本，确保所有 crate 编译一致
- `crates/irtool-core`：`IrError`、`AppConfig`、`TaskRegistry`、`DataStore` 共享类型；P0 仅实现 `IrError` 与配置 stub
- `crates/irtool-tauri/src/main.rs`：注册命令、装配窗口、单实例、UAC、日志；P0 仅一个 `cmd_app_info` 命令验证 IPC
- `crates/irtool-tauri/src/logger.rs`：tracing 初始化（rolling file + console）
- `crates/irtool-tauri/src/single_instance.rs`：单实例插件回调
- `ui/src/components/layout/AppShell.tsx`：3 区布局容器（侧栏 56px + 主区 + 状态栏 24px）
- `ui/src/components/layout/Sidebar.tsx`：4 个 Tab + 设置 5 个 NavLink 占位
- `ui/src/components/layout/TopBar.tsx`：Logo + 搜索框（P0 不工作）+ 设置/关于按钮
- `ui/src/components/layout/StatusBar.tsx`：管理员状态、Sysmon 状态、时钟（P0 仅显示静态文本占位）
- `ui/src/components/theme/ThemeProvider.tsx`：把 zustand 主题写到 `<html data-theme>` 与 `class`
- `ui/src/stores/theme-store.ts`：主题持久化到 localStorage
- `ui/src/lib/ipc.ts`：`call<T>(cmd, args)` 与 `on<T>(event, cb)` 类型化包装
- `ui/src/lib/i18n.ts`：i18next 初始化 + 资源加载

---

## 任务概览

P0 共 14 个任务，按依赖顺序执行。预计 0.5 周（3-4 个工作日，单人）。

| # | 任务 | 输出验证 |
|---|---|---|
| 1 | 初始化新仓库与 Cargo workspace | `cargo build` 通过空 workspace |
| 2 | 创建 irtool-core crate 基础类型 | `cargo test -p irtool-core` 通过 |
| 3 | 创建其余 6 个占位 crate | `cargo build --workspace` 通过 |
| 4 | 初始化 Tauri 2 主进程 | `cargo tauri dev` 弹空白窗口 |
| 5 | 初始化前端工程 Vite + React + TS | `pnpm dev` 启动 + Tauri 显示 React 默认页 |
| 6 | 安装 Tailwind 4 + 设计令牌 | 切换 `data-theme` 颜色变化 |
| 7 | 安装 shadcn/ui 与基础组件 | Button/Separator 渲染正常 |
| 8 | 实现 UAC manifest + 单实例 | 启动弹 UAC，第二次启动激活已有窗口 |
| 9 | 实现 tracing 日志系统 | logs 目录写入 `irtool-YYYYMMDD.log` |
| 10 | 实现深浅主题 + 持久化 | 重启保留主题选择 |
| 11 | 实现路由与 i18next | 4 个占位路由可切换；中英文切换 |
| 12 | 实现顶层布局 (Sidebar/TopBar/StatusBar) | UI 与 spec §5.1 mockup 一致 |
| 13 | 实现 specta 类型自动生成 | 修改 Rust 类型后 `pnpm generate-types` 更新 ts |
| 14 | 配置 GitHub Actions CI | 推到远端三个 workflow 通过 |

---

## Task 1: 初始化新仓库与 Cargo workspace

**Files:**
- Create: `D:\project\IRtool-next\.gitignore`
- Create: `D:\project\IRtool-next\.gitattributes`
- Create: `D:\project\IRtool-next\rustfmt.toml`
- Create: `D:\project\IRtool-next\clippy.toml`
- Create: `D:\project\IRtool-next\Cargo.toml`
- Create: `D:\project\IRtool-next\README.md`
- Create: `D:\project\IRtool-next\LICENSE` (复制 v1 LICENSE)

- [ ] **Step 1.1: 创建仓库目录并 git init**

```bash
mkdir -p "D:/project/IRtool-next"
cd "D:/project/IRtool-next"
git init
git branch -M main
```

- [ ] **Step 1.2: 写 `.gitignore`**

文件内容：

```gitignore
# Rust
/target/
**/*.rs.bk
Cargo.lock.bak

# Node
ui/node_modules/
ui/dist/
*.tsbuildinfo

# Tauri
crates/irtool-tauri/gen/
crates/irtool-tauri/target/

# IDE
.idea/
.vscode/
*.swp
.DS_Store
Thumbs.db

# Logs / runtime
logs/
*.log
crash.log

# Local env
.env
.env.local
*.local

# Build artifacts
dist/
build/
out/
release/
```

- [ ] **Step 1.3: 写 `.gitattributes`（统一行尾）**

```gitattributes
* text=auto eol=lf
*.bat text eol=crlf
*.cmd text eol=crlf
*.ps1 text eol=crlf
*.exe binary
*.dll binary
*.png binary
*.jpg binary
*.ico binary
```

- [ ] **Step 1.4: 写 `rustfmt.toml`**

```toml
edition = "2021"
max_width = 120
tab_spaces = 4
newline_style = "Unix"
use_field_init_shorthand = true
imports_granularity = "Module"
group_imports = "StdExternalCrate"
```

- [ ] **Step 1.5: 写 `clippy.toml`**

```toml
msrv = "1.85"
cognitive-complexity-threshold = 30
```

- [ ] **Step 1.6: 写根 `Cargo.toml` workspace 配置**

```toml
[workspace]
members = [
    "crates/irtool-core",
    "crates/irtool-net-monitor",
    "crates/irtool-autoruns",
    "crates/irtool-sysmon",
    "crates/irtool-rules",
    "crates/irtool-threat-intel",
    "crates/irtool-process",
    "crates/irtool-tauri",
]
resolver = "2"

[workspace.package]
version = "2.0.0-alpha.1"
edition = "2021"
rust-version = "1.85"
authors = ["summerxzp"]
license = "MIT"
repository = "https://github.com/summerxzp/IRtool-next"

[workspace.dependencies]
# 异步运行时
tokio = { version = "1.40", features = ["full"] }
tokio-util = { version = "0.7", features = ["rt"] }

# 序列化与错误
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
anyhow = "1"

# 日志
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tracing-appender = "0.2"

# Win32 绑定（具体 features 由调用 crate 在自身 Cargo.toml 内启用）
windows = { version = "0.59" }

# 工具
encoding_rs = "0.8"
csv = "1.3"
quick-xml = "0.36"
regex = "1.10"
aho-corasick = "1.1"
dashmap = "6"

# 网络（v2.0 仅 threat-intel crate 引入；运行时不发请求）
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }

# Tauri
tauri = { version = "2", features = ["protocol-asset"] }
tauri-build = { version = "2" }
tauri-plugin-single-instance = "2"
tauri-plugin-fs = "2"
tauri-plugin-shell = "2"
tauri-plugin-process = "2"
tauri-plugin-store = "2"
tauri-plugin-dialog = "2"

# 类型生成
specta = "2.0.0-rc"
specta-typescript = "0.0.7"
tauri-specta = { version = "2.0.0-rc", features = ["derive", "typescript"] }

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

- [ ] **Step 1.7: 写 `README.md`（最小占位）**

```markdown
# IRtool-next (v2.0)

Windows 应急响应桌面工具的 v2 重构版本。基于 Tauri + Rust + React。

设计文档：见 v1 仓库 `docs/superpowers/specs/2026-05-31-IRtool-refactor-design.md`

## 状态

P0 脚手架阶段（开发中）

## 开发

```bash
# 安装前端依赖
cd ui && pnpm install && cd ..

# 启动开发模式
cargo tauri dev
```
```

- [ ] **Step 1.8: 复制 LICENSE**

从 `D:\project\IRtool\LICENSE` 复制到 `D:\project\IRtool-next\LICENSE`。

- [ ] **Step 1.9: 创建 crates 目录占位**

```bash
mkdir -p crates/irtool-core/src
mkdir -p crates/irtool-net-monitor/src
mkdir -p crates/irtool-autoruns/src
mkdir -p crates/irtool-sysmon/src
mkdir -p crates/irtool-rules/src
mkdir -p crates/irtool-threat-intel/src
mkdir -p crates/irtool-process/src
mkdir -p crates/irtool-tauri/src
mkdir -p tools
touch tools/.gitkeep
```

- [ ] **Step 1.10: 验证 workspace 解析**

由于此时 crates/* 还没有 Cargo.toml，先暂时只列空 members。修改 `Cargo.toml` 把 `members` 改成 `members = []`，运行：

```bash
cargo metadata --format-version=1 > /dev/null
```

预期：成功输出（不报错）。然后改回完整 members 列表，等 Task 2-3 后再次验证。

- [ ] **Step 1.11: 提交**

```bash
git add .gitignore .gitattributes rustfmt.toml clippy.toml Cargo.toml README.md LICENSE crates/ tools/
git commit -m "chore: init Cargo workspace skeleton with 8 crate slots"
```

---

## Task 2: irtool-core crate 基础类型

**Files:**
- Create: `crates/irtool-core/Cargo.toml`
- Create: `crates/irtool-core/src/lib.rs`
- Create: `crates/irtool-core/src/error.rs`
- Create: `crates/irtool-core/src/config.rs`
- Test: `crates/irtool-core/src/error.rs` (内置 #[cfg(test)])

- [ ] **Step 2.1: 写 `crates/irtool-core/Cargo.toml`**

```toml
[package]
name = "irtool-core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio-util = { workspace = true }
tracing = { workspace = true }
specta = { workspace = true, features = ["serde"] }
```

- [ ] **Step 2.2: 写失败的错误类型测试**

文件 `crates/irtool-core/src/error.rs` 加测试模块（与下面 Step 2.4 实现一同写）：

```rust
use serde::Serialize;
use specta::Type;

#[derive(thiserror::Error, Debug, Serialize, Type)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
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

    #[error("feature disabled: {0}")]
    FeatureDisabled(String),

    #[error("internal: {0}")]
    Internal(String),
}

impl From<std::io::Error> for IrError {
    fn from(value: std::io::Error) -> Self {
        IrError::Io(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_serializes_with_kind_tag() {
        let err = IrError::ExternalTool {
            tool: "autorunsc".into(),
            code: 1,
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"kind\":\"external_tool\""));
        assert!(json.contains("\"code\":1"));
    }

    #[test]
    fn permission_denied_renders() {
        let err = IrError::PermissionDenied;
        assert_eq!(err.to_string(), "permission denied: requires administrator");
    }

    #[test]
    fn io_error_converts() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let ir: IrError = io_err.into();
        match ir {
            IrError::Io(msg) => assert!(msg.contains("missing")),
            _ => panic!("expected Io variant"),
        }
    }
}
```

- [ ] **Step 2.3: 写 `crates/irtool-core/src/config.rs` (P0 stub)**

```rust
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct AppConfig {
    pub theme: Theme,
    pub language: Language,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
    System,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Language {
    #[default]
    ZhCn,
    EnUs,
}
```

- [ ] **Step 2.4: 写 `crates/irtool-core/src/lib.rs`**

```rust
pub mod config;
pub mod error;

pub use config::{AppConfig, Language, Theme};
pub use error::IrError;
```

- [ ] **Step 2.5: 把 `members` 加回根 `Cargo.toml`（如 Step 1.10 已暂时清空）**

确认根 `Cargo.toml` 的 `members` 为完整 8 个 crate 列表。

- [ ] **Step 2.6: 运行测试验证**

```bash
cargo test -p irtool-core
```

预期：`3 passed; 0 failed`。

- [ ] **Step 2.7: 运行 fmt + clippy**

```bash
cargo fmt --all
cargo clippy -p irtool-core -- -D warnings
```

预期：无 warning。

- [ ] **Step 2.8: 提交**

```bash
git add crates/irtool-core/
git commit -m "feat(core): IrError + AppConfig stub with specta types and unit tests"
```

---

## Task 3: 创建其余 6 个占位 crate

每个占位 crate 仅含 `Cargo.toml` 与一个最小 `lib.rs`，让 `cargo build --workspace` 通过；实际实现留给 P1-P4。

**Files:**
- Create: `crates/irtool-net-monitor/Cargo.toml` 与 `src/lib.rs`
- Create: `crates/irtool-autoruns/Cargo.toml` 与 `src/lib.rs`
- Create: `crates/irtool-sysmon/Cargo.toml` 与 `src/lib.rs`
- Create: `crates/irtool-rules/Cargo.toml` 与 `src/lib.rs`
- Create: `crates/irtool-threat-intel/Cargo.toml` 与 `src/lib.rs`
- Create: `crates/irtool-process/Cargo.toml` 与 `src/lib.rs`

- [ ] **Step 3.1: 创建 `irtool-net-monitor` 占位**

`crates/irtool-net-monitor/Cargo.toml`:

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
tokio = { workspace = true }
specta = { workspace = true, features = ["serde"] }
```

`crates/irtool-net-monitor/src/lib.rs`:

```rust
//! P1 阶段实装：网络连接采集
//! 当前为占位 crate，仅保证 workspace 编译通过。

#![allow(dead_code)]

pub fn placeholder() -> &'static str {
    "irtool-net-monitor: pending P1"
}
```

- [ ] **Step 3.2: 创建 `irtool-autoruns` 占位**

`crates/irtool-autoruns/Cargo.toml`:

```toml
[package]
name = "irtool-autoruns"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
irtool-core = { path = "../irtool-core" }
serde = { workspace = true }
tokio = { workspace = true }
specta = { workspace = true, features = ["serde"] }
```

`crates/irtool-autoruns/src/lib.rs`:

```rust
//! P2 阶段实装：autorunsc 调用 + WinTrust 签名验证
#![allow(dead_code)]

pub fn placeholder() -> &'static str {
    "irtool-autoruns: pending P2"
}
```

- [ ] **Step 3.3: 创建 `irtool-sysmon` 占位**

`crates/irtool-sysmon/Cargo.toml`:

```toml
[package]
name = "irtool-sysmon"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
irtool-core = { path = "../irtool-core" }
serde = { workspace = true }
tokio = { workspace = true }
specta = { workspace = true, features = ["serde"] }
```

`crates/irtool-sysmon/src/lib.rs`:

```rust
//! P4 阶段实装：Sysmon 安装/订阅
#![allow(dead_code)]

pub fn placeholder() -> &'static str {
    "irtool-sysmon: pending P4"
}
```

- [ ] **Step 3.4: 创建 `irtool-rules` 占位**

`crates/irtool-rules/Cargo.toml`:

```toml
[package]
name = "irtool-rules"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
irtool-core = { path = "../irtool-core" }
serde = { workspace = true }
specta = { workspace = true, features = ["serde"] }
```

`crates/irtool-rules/src/lib.rs`:

```rust
//! P3 阶段实装：规则引擎
#![allow(dead_code)]

pub fn placeholder() -> &'static str {
    "irtool-rules: pending P3"
}
```

- [ ] **Step 3.5: 创建 `irtool-threat-intel` 占位（含 trait 骨架）**

`crates/irtool-threat-intel/Cargo.toml`:

```toml
[package]
name = "irtool-threat-intel"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[features]
default = []
intel-weibu = []
intel-virustotal = []

[dependencies]
irtool-core = { path = "../irtool-core" }
serde = { workspace = true }
async-trait = "0.1"
tokio = { workspace = true }
specta = { workspace = true, features = ["serde"] }
```

`crates/irtool-threat-intel/src/lib.rs`:

```rust
//! v2.0 仅 trait + NoopProvider；v2.1+ 接入 Weibu/VirusTotal Provider
//! 见设计文档 §4.6 与 §14 路线图

use async_trait::async_trait;
use irtool_core::IrError;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum IntelResult {
    Disabled { reason: String },
    Clean,
    Suspicious { score: u32, sources: Vec<String> },
    Malicious { score: u32, sources: Vec<String> },
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum IocQuery {
    Hash(String),
    Ip(String),
    Domain(String),
}

#[async_trait]
pub trait ThreatIntelProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn query(&self, query: &IocQuery) -> Result<IntelResult, IrError>;
}

pub struct NoopProvider;

#[async_trait]
impl ThreatIntelProvider for NoopProvider {
    fn name(&self) -> &str {
        "noop"
    }

    async fn query(&self, _query: &IocQuery) -> Result<IntelResult, IrError> {
        Ok(IntelResult::Disabled {
            reason: "v2.0 not implemented; planned for v2.1+".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn noop_provider_returns_disabled() {
        let provider = NoopProvider;
        let result = provider.query(&IocQuery::Hash("abc".into())).await.unwrap();
        match result {
            IntelResult::Disabled { reason } => assert!(reason.contains("v2.0")),
            _ => panic!("expected Disabled"),
        }
    }
}
```

- [ ] **Step 3.6: 创建 `irtool-process` 占位**

`crates/irtool-process/Cargo.toml`:

```toml
[package]
name = "irtool-process"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
irtool-core = { path = "../irtool-core" }
serde = { workspace = true }
specta = { workspace = true, features = ["serde"] }
```

`crates/irtool-process/src/lib.rs`:

```rust
//! P4 阶段实装：进程树枚举
#![allow(dead_code)]

pub fn placeholder() -> &'static str {
    "irtool-process: pending P4"
}
```

- [ ] **Step 3.7: 验证全 workspace 编译**

```bash
cargo build --workspace
```

预期：所有 crate 编译通过，无 error，无 warning（除 dead_code allow 部分）。

- [ ] **Step 3.8: 跑 threat-intel 异步测试**

```bash
cargo test -p irtool-threat-intel
```

预期：`1 passed; 0 failed`。

- [ ] **Step 3.9: 提交**

```bash
git add crates/
git commit -m "feat: scaffold 6 placeholder crates with stubs and threat-intel trait"
```

---

## Task 4: 初始化 Tauri 2 主进程

**Files:**
- Create: `crates/irtool-tauri/Cargo.toml`
- Create: `crates/irtool-tauri/build.rs`
- Create: `crates/irtool-tauri/tauri.conf.json`
- Create: `crates/irtool-tauri/src/main.rs`
- Create: `crates/irtool-tauri/icons/` (复制 v1 icons 或用 Tauri 默认)

- [ ] **Step 4.1: 写 `crates/irtool-tauri/Cargo.toml`**

```toml
[package]
name = "irtool-tauri"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[lib]
name = "irtool_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { workspace = true }

[dependencies]
# workspace crates
irtool-core = { path = "../irtool-core" }
irtool-net-monitor = { path = "../irtool-net-monitor" }
irtool-autoruns = { path = "../irtool-autoruns" }
irtool-sysmon = { path = "../irtool-sysmon" }
irtool-rules = { path = "../irtool-rules" }
irtool-threat-intel = { path = "../irtool-threat-intel" }
irtool-process = { path = "../irtool-process" }

# tauri
tauri = { workspace = true }
tauri-plugin-single-instance = { workspace = true }
tauri-plugin-fs = { workspace = true }
tauri-plugin-shell = { workspace = true }
tauri-plugin-process = { workspace = true }
tauri-plugin-store = { workspace = true }
tauri-plugin-dialog = { workspace = true }

# 日志
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
tracing-appender = { workspace = true }

# 异步与序列化
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }

# specta (类型生成)
specta = { workspace = true, features = ["serde"] }
specta-typescript = { workspace = true }
tauri-specta = { workspace = true }

[features]
default = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]
```

- [ ] **Step 4.2: 写 `crates/irtool-tauri/build.rs`**

```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 4.3: 写 `crates/irtool-tauri/tauri.conf.json`**

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "IRtool",
  "version": "2.0.0-alpha.1",
  "identifier": "com.summerxzp.IRtool.v2",
  "build": {
    "frontendDist": "../../ui/dist",
    "devUrl": "http://localhost:5173",
    "beforeDevCommand": "pnpm --filter ui dev",
    "beforeBuildCommand": "pnpm --filter ui build"
  },
  "app": {
    "windows": [
      {
        "title": "IRtool",
        "width": 1280,
        "height": 800,
        "minWidth": 960,
        "minHeight": 600,
        "decorations": true,
        "transparent": false,
        "resizable": true,
        "center": true
      }
    ],
    "security": {
      "csp": "default-src 'self'; img-src 'self' asset: data:; style-src 'self' 'unsafe-inline'; script-src 'self'"
    }
  },
  "bundle": {
    "active": true,
    "targets": ["msi", "nsis"],
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "windows": {
      "wix": {
        "language": ["zh-CN", "en-US"]
      },
      "nsis": {
        "installerIcon": "icons/icon.ico",
        "languages": ["SimpChinese", "English"]
      }
    },
    "shortDescription": "Windows 应急响应桌面工具",
    "longDescription": "IRtool 是一款面向 Windows 终端应急响应场景的本地桌面工具"
  }
}
```

- [ ] **Step 4.4: 复制图标文件**

从 v1 仓库 `D:\project\IRtool\` 找 .ico/.png 图标资源（如果有），或运行：

```bash
cargo install tauri-cli@^2 --locked
cd "D:/project/IRtool-next/crates/irtool-tauri"
mkdir -p icons
# 用 tauri 默认 icon
curl -L -o icons/icon.png https://tauri.app/meta/favicon-32x32.png
cargo tauri icon icons/icon.png
```

预期：`icons/` 目录下生成 `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.icns`, `icon.ico`, `Square*.png`, `StoreLogo.png`。

- [ ] **Step 4.5: 写最小 `crates/irtool-tauri/src/main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

#[tauri::command]
fn cmd_app_info() -> serde_json::Value {
    serde_json::json!({
        "name": "IRtool",
        "version": env!("CARGO_PKG_VERSION"),
        "build": "alpha",
    })
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![cmd_app_info])
        .setup(|app| {
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

- [ ] **Step 4.6: 验证构建**

由于此时没有前端，先创建临时 dist 占位：

```bash
mkdir -p "D:/project/IRtool-next/ui/dist"
echo '<!DOCTYPE html><html><body><h1>IRtool boot test</h1></body></html>' > "D:/project/IRtool-next/ui/dist/index.html"
```

修改 `tauri.conf.json` 暂时注释掉 `beforeDevCommand` 与 `devUrl`（Task 5 会恢复），改为：

```json
"build": {
  "frontendDist": "../../ui/dist"
}
```

然后：

```bash
cd "D:/project/IRtool-next"
cargo build -p irtool-tauri
```

预期：编译成功（首次很慢，~5 分钟）。

- [ ] **Step 4.7: 启动验证**

```bash
cargo run -p irtool-tauri
```

预期：弹出 Tauri 窗口显示 "IRtool boot test"。手动关闭窗口结束。

- [ ] **Step 4.8: 还原 `tauri.conf.json` 完整 build 配置**

把 Step 4.3 的 `build` 节恢复（含 `devUrl` 与 `beforeDevCommand`），删除临时 `ui/dist/index.html`：

```bash
rm "D:/project/IRtool-next/ui/dist/index.html"
rmdir "D:/project/IRtool-next/ui/dist"
```

- [ ] **Step 4.9: 提交**

```bash
git add crates/irtool-tauri/ Cargo.toml
git commit -m "feat(tauri): bootstrap Tauri 2 main process with cmd_app_info smoke command"
```

---

## Task 5: 初始化前端工程 Vite + React + TypeScript

**Files:**
- Create: `ui/package.json`
- Create: `ui/vite.config.ts`
- Create: `ui/tsconfig.json`
- Create: `ui/tsconfig.node.json`
- Create: `ui/index.html`
- Create: `ui/src/main.tsx`
- Create: `ui/src/App.tsx`

- [ ] **Step 5.1: 创建 `ui/package.json`**

```json
{
  "name": "irtool-ui",
  "private": true,
  "version": "2.0.0-alpha.1",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview",
    "lint": "tsc --noEmit",
    "generate-types": "echo 'TS types regenerated by Rust build via specta'"
  },
  "dependencies": {
    "@tauri-apps/api": "^2",
    "@tauri-apps/plugin-shell": "^2",
    "@tauri-apps/plugin-process": "^2",
    "@tauri-apps/plugin-store": "^2",
    "@tauri-apps/plugin-dialog": "^2",
    "react": "^18.3.1",
    "react-dom": "^18.3.1"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "@types/react": "^18.3.12",
    "@types/react-dom": "^18.3.1",
    "@vitejs/plugin-react": "^4.3.3",
    "typescript": "^5.6.3",
    "vite": "^6.0.0"
  },
  "packageManager": "pnpm@9.12.0"
}
```

- [ ] **Step 5.2: 创建 `ui/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "useDefineForClassFields": true,
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "moduleDetection": "force",
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "baseUrl": ".",
    "paths": {
      "@/*": ["src/*"]
    }
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

- [ ] **Step 5.3: 创建 `ui/tsconfig.node.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2023"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowSyntheticDefaultImports": true,
    "strict": true,
    "noEmit": true
  },
  "include": ["vite.config.ts"]
}
```

- [ ] **Step 5.4: 创建 `ui/vite.config.ts`**

```typescript
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 5174,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**", "**/crates/**", "**/target/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "es2022",
    minify: "esbuild",
    sourcemap: true,
    chunkSizeWarningLimit: 1000,
  },
});
```

- [ ] **Step 5.5: 创建 `ui/index.html`**

```html
<!doctype html>
<html lang="zh-CN" data-theme="dark">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>IRtool</title>
    <link rel="icon" href="/favicon.ico" />
  </head>
  <body class="bg-bg-base text-fg-primary">
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 5.6: 创建 `ui/src/main.tsx`**

```typescript
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

- [ ] **Step 5.7: 创建 `ui/src/App.tsx` (P0 minimal)**

```typescript
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

interface AppInfo {
  name: string;
  version: string;
  build: string;
}

function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<AppInfo>("cmd_app_info")
      .then(setInfo)
      .catch((e) => setError(String(e)));
  }, []);

  return (
    <div style={{ padding: 24, fontFamily: "system-ui, sans-serif" }}>
      <h1>IRtool v2 boot</h1>
      {info && (
        <pre>{JSON.stringify(info, null, 2)}</pre>
      )}
      {error && <p style={{ color: "red" }}>error: {error}</p>}
    </div>
  );
}

export default App;
```

- [ ] **Step 5.8: 安装依赖**

```bash
cd "D:/project/IRtool-next/ui"
pnpm install
```

预期：`pnpm-lock.yaml` 生成，依赖装完。

- [ ] **Step 5.9: 测试前端独立启动**

```bash
pnpm dev
```

预期：Vite 在 5173 端口启动。手动浏览器访问 http://localhost:5173 看到 React 默认页（虽然 invoke 在浏览器会失败但页面应渲染）。Ctrl+C 关闭。

- [ ] **Step 5.10: 测试 Tauri 全流程**

```bash
cd "D:/project/IRtool-next"
cargo tauri dev
```

预期：
1. 自动启动 Vite 开发服务器
2. Cargo 编译完成后弹出 Tauri 窗口
3. 窗口显示 "IRtool v2 boot" + JSON `{name: "IRtool", version: "2.0.0-alpha.1", build: "alpha"}`
4. F12 打开 DevTools 无错误

手动关闭。

- [ ] **Step 5.11: 添加 ui 到 .gitignore（部分文件已在根 .gitignore）**

确认 `D:\project\IRtool-next\.gitignore` 已含 `ui/node_modules/` 与 `ui/dist/`。

- [ ] **Step 5.12: 提交**

```bash
git add ui/ Cargo.toml
git commit -m "feat(ui): bootstrap Vite + React + TS frontend with cmd_app_info smoke"
```

---

## Task 6: 安装 Tailwind CSS 4 + 设计令牌

**Files:**
- Create: `ui/postcss.config.js`
- Create: `ui/tailwind.config.ts`
- Create: `ui/src/styles/tokens.css`
- Create: `ui/src/styles/globals.css`
- Modify: `ui/src/main.tsx` (导入 globals.css)

- [ ] **Step 6.1: 安装 Tailwind 4 与 PostCSS**

```bash
cd "D:/project/IRtool-next/ui"
pnpm add -D tailwindcss@^4 @tailwindcss/postcss@^4 postcss autoprefixer
```

- [ ] **Step 6.2: 创建 `ui/postcss.config.js`**

```javascript
export default {
  plugins: {
    "@tailwindcss/postcss": {},
    autoprefixer: {},
  },
};
```

- [ ] **Step 6.3: 创建 `ui/tailwind.config.ts`**

```typescript
import type { Config } from "tailwindcss";

const config: Config = {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  darkMode: ["class", '[data-theme="dark"]'],
  theme: {
    extend: {
      colors: {
        "bg-base": "var(--bg-base)",
        "bg-elev-1": "var(--bg-elev-1)",
        "bg-elev-2": "var(--bg-elev-2)",
        border: "var(--border)",
        "fg-primary": "var(--fg-primary)",
        "fg-secondary": "var(--fg-secondary)",
        "fg-tertiary": "var(--fg-tertiary)",
        accent: "var(--accent)",
        success: "var(--success)",
        warning: "var(--warning)",
        danger: "var(--danger)",
        critical: "var(--critical)",
      },
      fontFamily: {
        sans: ["Inter", "Microsoft YaHei", "sans-serif"],
        mono: ["JetBrains Mono", "Cascadia Mono", "monospace"],
      },
      fontSize: {
        xs: ["11px", "16px"],
        sm: ["12px", "18px"],
        base: ["13px", "20px"],
        md: ["14px", "22px"],
        lg: ["16px", "24px"],
      },
    },
  },
  plugins: [],
};

export default config;
```

- [ ] **Step 6.4: 创建 `ui/src/styles/tokens.css`**

```css
:root,
:root[data-theme="dark"] {
  --bg-base: #0b0d10;
  --bg-elev-1: #14171c;
  --bg-elev-2: #1c2127;
  --border: #262c34;
  --fg-primary: #e6e8eb;
  --fg-secondary: #9aa3ad;
  --fg-tertiary: #6b7480;
  --accent: #4c8dff;
  --success: #2ecc71;
  --warning: #f0b429;
  --danger: #ef4444;
  --critical: #b91c1c;
}

:root[data-theme="light"] {
  --bg-base: #f7f8fa;
  --bg-elev-1: #ffffff;
  --bg-elev-2: #eef1f5;
  --border: #d8dde5;
  --fg-primary: #1a1d23;
  --fg-secondary: #4a5260;
  --fg-tertiary: #788090;
  --accent: #3a7af0;
  --success: #20a04f;
  --warning: #c98a07;
  --danger: #d63838;
  --critical: #911818;
}
```

- [ ] **Step 6.5: 创建 `ui/src/styles/globals.css`**

```css
@import "tailwindcss";
@import "./tokens.css";

@layer base {
  html,
  body,
  #root {
    height: 100%;
    margin: 0;
    padding: 0;
  }

  body {
    background: var(--bg-base);
    color: var(--fg-primary);
    font-family: "Inter", "Microsoft YaHei", sans-serif;
    font-size: 13px;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
  }

  /* 滚动条主题化 */
  ::-webkit-scrollbar {
    width: 10px;
    height: 10px;
  }
  ::-webkit-scrollbar-track {
    background: var(--bg-base);
  }
  ::-webkit-scrollbar-thumb {
    background: var(--border);
    border-radius: 5px;
  }
  ::-webkit-scrollbar-thumb:hover {
    background: var(--fg-tertiary);
  }
}
```

- [ ] **Step 6.6: 修改 `ui/src/main.tsx` 导入 globals.css**

```typescript
import React from "react";
import ReactDOM from "react-dom/client";
import "./styles/globals.css";
import App from "./App";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

- [ ] **Step 6.7: 修改 `ui/src/App.tsx` 用 Tailwind 类**

```typescript
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

interface AppInfo {
  name: string;
  version: string;
  build: string;
}

function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<AppInfo>("cmd_app_info")
      .then(setInfo)
      .catch((e) => setError(String(e)));
  }, []);

  return (
    <div className="min-h-screen p-6">
      <h1 className="text-lg font-semibold text-fg-primary mb-4">
        IRtool v2 boot
      </h1>
      {info && (
        <pre className="bg-bg-elev-1 border border-border p-3 rounded font-mono text-sm text-fg-secondary">
          {JSON.stringify(info, null, 2)}
        </pre>
      )}
      {error && <p className="text-danger mt-2">error: {error}</p>}
    </div>
  );
}

export default App;
```

- [ ] **Step 6.8: 验证主题切换**

```bash
cargo tauri dev
```

预期：窗口显示深色背景，蓝色 accent 文字。在 DevTools Console 运行：

```javascript
document.documentElement.setAttribute('data-theme', 'light')
```

预期：背景立即切到浅色（`--bg-base: #f7f8fa`），文字切深色。再切回 dark 验证。

- [ ] **Step 6.9: 提交**

```bash
git add ui/postcss.config.js ui/tailwind.config.ts ui/src/styles/ ui/src/main.tsx ui/src/App.tsx ui/package.json ui/pnpm-lock.yaml
git commit -m "feat(ui): tailwind 4 + design tokens with dark/light theme via data-theme"
```

---

## Task 7: 安装 shadcn/ui 与基础组件

**Files:**
- Create: `ui/components.json`
- Create: `ui/src/lib/utils.ts`
- Create: `ui/src/components/ui/button.tsx`
- Create: `ui/src/components/ui/separator.tsx`
- Create: `ui/src/components/ui/tooltip.tsx`

- [ ] **Step 7.1: 安装 shadcn 依赖**

```bash
cd "D:/project/IRtool-next/ui"
pnpm add class-variance-authority clsx tailwind-merge
pnpm add @radix-ui/react-separator @radix-ui/react-tooltip @radix-ui/react-slot
pnpm add lucide-react
pnpm add tw-animate-css
```

- [ ] **Step 7.2: 创建 `ui/src/lib/utils.ts`**

```typescript
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
```

- [ ] **Step 7.3: 创建 `ui/components.json` (shadcn 配置)**

```json
{
  "$schema": "https://ui.shadcn.com/schema.json",
  "style": "new-york",
  "rsc": false,
  "tsx": true,
  "tailwind": {
    "config": "tailwind.config.ts",
    "css": "src/styles/globals.css",
    "baseColor": "neutral",
    "cssVariables": true,
    "prefix": ""
  },
  "aliases": {
    "components": "@/components",
    "utils": "@/lib/utils",
    "ui": "@/components/ui",
    "lib": "@/lib",
    "hooks": "@/hooks"
  },
  "iconLibrary": "lucide"
}
```

- [ ] **Step 7.4: 创建 `ui/src/components/ui/button.tsx`**

```typescript
import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      variant: {
        default:
          "bg-accent text-white hover:bg-accent/90",
        secondary:
          "bg-bg-elev-2 text-fg-primary hover:bg-bg-elev-2/80 border border-border",
        ghost:
          "hover:bg-bg-elev-2 text-fg-secondary hover:text-fg-primary",
        destructive:
          "bg-danger text-white hover:bg-danger/90",
        link:
          "text-accent underline-offset-4 hover:underline",
      },
      size: {
        default: "h-8 px-3 py-1",
        sm: "h-7 px-2 text-xs",
        lg: "h-10 px-6 text-md",
        icon: "h-8 w-8",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, asChild = false, ...props }, ref) => {
    const Comp = asChild ? Slot : "button";
    return (
      <Comp
        className={cn(buttonVariants({ variant, size, className }))}
        ref={ref}
        {...props}
      />
    );
  },
);
Button.displayName = "Button";

export { Button, buttonVariants };
```

- [ ] **Step 7.5: 创建 `ui/src/components/ui/separator.tsx`**

```typescript
import * as React from "react";
import * as SeparatorPrimitive from "@radix-ui/react-separator";
import { cn } from "@/lib/utils";

const Separator = React.forwardRef<
  React.ElementRef<typeof SeparatorPrimitive.Root>,
  React.ComponentPropsWithoutRef<typeof SeparatorPrimitive.Root>
>(({ className, orientation = "horizontal", decorative = true, ...props }, ref) => (
  <SeparatorPrimitive.Root
    ref={ref}
    decorative={decorative}
    orientation={orientation}
    className={cn(
      "shrink-0 bg-border",
      orientation === "horizontal" ? "h-px w-full" : "h-full w-px",
      className,
    )}
    {...props}
  />
));
Separator.displayName = SeparatorPrimitive.Root.displayName;

export { Separator };
```

- [ ] **Step 7.6: 创建 `ui/src/components/ui/tooltip.tsx`**

```typescript
import * as React from "react";
import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import { cn } from "@/lib/utils";

const TooltipProvider = TooltipPrimitive.Provider;
const Tooltip = TooltipPrimitive.Root;
const TooltipTrigger = TooltipPrimitive.Trigger;

const TooltipContent = React.forwardRef<
  React.ElementRef<typeof TooltipPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof TooltipPrimitive.Content>
>(({ className, sideOffset = 4, ...props }, ref) => (
  <TooltipPrimitive.Content
    ref={ref}
    sideOffset={sideOffset}
    className={cn(
      "z-50 overflow-hidden rounded-md border border-border bg-bg-elev-2 px-2 py-1 text-xs text-fg-primary shadow-md",
      className,
    )}
    {...props}
  />
));
TooltipContent.displayName = TooltipPrimitive.Content.displayName;

export { Tooltip, TooltipTrigger, TooltipContent, TooltipProvider };
```

- [ ] **Step 7.7: 用 Button 验证渲染**

修改 `ui/src/App.tsx`：

```typescript
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";

interface AppInfo {
  name: string;
  version: string;
  build: string;
}

function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<AppInfo>("cmd_app_info")
      .then(setInfo)
      .catch((e) => setError(String(e)));
  }, []);

  return (
    <div className="min-h-screen p-6 space-y-4">
      <h1 className="text-lg font-semibold">IRtool v2 boot</h1>
      <Separator />
      {info && (
        <pre className="bg-bg-elev-1 border border-border p-3 rounded font-mono text-sm text-fg-secondary">
          {JSON.stringify(info, null, 2)}
        </pre>
      )}
      {error && <p className="text-danger">error: {error}</p>}
      <div className="flex gap-2">
        <Button variant="default">Primary</Button>
        <Button variant="secondary">Secondary</Button>
        <Button variant="ghost">Ghost</Button>
        <Button variant="destructive">Destructive</Button>
      </div>
    </div>
  );
}

export default App;
```

- [ ] **Step 7.8: 启动验证**

```bash
cd "D:/project/IRtool-next"
cargo tauri dev
```

预期：窗口显示 4 种按钮样式正确，hover 有过渡，焦点环正常。

- [ ] **Step 7.9: 提交**

```bash
git add ui/components.json ui/src/lib/ ui/src/components/ ui/src/App.tsx ui/package.json ui/pnpm-lock.yaml
git commit -m "feat(ui): shadcn config + Button/Separator/Tooltip primitives"
```

---

## Task 8: UAC manifest + 单实例

**Files:**
- Create: `crates/irtool-tauri/irtool.manifest`
- Create: `crates/irtool-tauri/src/single_instance.rs`
- Modify: `crates/irtool-tauri/Cargo.toml` (build.rs 链接 manifest)
- Modify: `crates/irtool-tauri/build.rs`
- Modify: `crates/irtool-tauri/src/main.rs`

- [ ] **Step 8.1: 写 `crates/irtool-tauri/irtool.manifest`**

```xml
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity
      version="2.0.0.0"
      name="com.summerxzp.IRtool.v2"
      type="win32"/>
  <description>IRtool v2 - Windows Incident Response Tool</description>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
      <supportedOS Id="{1f676c76-80e1-4239-95bb-83d0f6d0da78}"/>
    </application>
  </compatibility>
</assembly>
```

- [ ] **Step 8.2: 修改 `crates/irtool-tauri/build.rs` 链接 manifest**

```rust
fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_manifest_file("irtool.manifest");
        res.compile().expect("failed to compile manifest");
    }
    tauri_build::build()
}
```

- [ ] **Step 8.3: 修改 `crates/irtool-tauri/Cargo.toml` 添加 winres**

在 `[build-dependencies]` 节加：

```toml
[build-dependencies]
tauri-build = { workspace = true }
winres = "0.1"
```

- [ ] **Step 8.4: 写 `crates/irtool-tauri/src/single_instance.rs`**

```rust
use tauri::{AppHandle, Manager, Runtime};
use tracing::info;

pub fn handle_second_instance<R: Runtime>(
    app: &AppHandle<R>,
    args: Vec<String>,
    cwd: String,
) {
    info!(?args, ?cwd, "second instance attempted; bringing existing window to front");

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.set_focus();
        let _ = window.show();
    }
}
```

- [ ] **Step 8.5: 修改 `crates/irtool-tauri/src/main.rs` 集成单实例**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod single_instance;

use tauri::Manager;

#[tauri::command]
fn cmd_app_info() -> serde_json::Value {
    serde_json::json!({
        "name": "IRtool",
        "version": env!("CARGO_PKG_VERSION"),
        "build": "alpha",
        "is_admin": is_running_as_admin(),
    })
}

#[cfg(windows)]
fn is_running_as_admin() -> bool {
    use std::os::windows::ffi::OsStrExt;
    use std::ffi::OsStr;

    unsafe {
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
        use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

        let _ = OsStr::new("dummy").encode_wide();

        let mut token: HANDLE = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut size = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        )
        .is_ok();

        ok && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(windows))]
fn is_running_as_admin() -> bool {
    false
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            single_instance::handle_second_instance(app, args, cwd);
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![cmd_app_info])
        .setup(|app| {
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

- [ ] **Step 8.6: 修改 `crates/irtool-tauri/Cargo.toml` 添加 windows 依赖**

```toml
[dependencies]
# ... 已有
windows = { workspace = true, features = [
  "Win32_Foundation",
  "Win32_Security",
  "Win32_System_Threading",
] }
```

- [ ] **Step 8.6.5: 同步更新前端 `ui/src/App.tsx` AppInfo interface**

避免前后端类型不一致。修改 `ui/src/App.tsx` 顶部 interface：

```typescript
interface AppInfo {
  name: string;
  version: string;
  build: string;
  is_admin: boolean;
}
```

其他保持 Task 7.7 的内容不变。Task 13 会用 specta 自动生成代替手写 interface。

- [ ] **Step 8.7: 验证 UAC 提权**

```bash
cargo tauri dev
```

预期：
1. 启动时弹出 UAC 提示框（首次或非提权进程启动时）
2. 接受后窗口正常显示
3. JSON 中 `is_admin: true`

- [ ] **Step 8.8: 验证单实例**

应用打开后,在另一个终端再次运行：

```bash
cargo run -p irtool-tauri --release
```

预期：第二次启动不弹新窗口，第一个窗口被激活到前台（如最小化则恢复）。日志中出现 "second instance attempted"。

- [ ] **Step 8.9: 提交**

```bash
git add crates/irtool-tauri/
git commit -m "feat(tauri): UAC manifest + single instance with focus restoration"
```

---

## Task 9: tracing 日志系统

**Files:**
- Create: `crates/irtool-tauri/src/logger.rs`
- Modify: `crates/irtool-tauri/src/main.rs`

- [ ] **Step 9.1: 写 `crates/irtool-tauri/src/logger.rs`**

```rust
use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter, Layer};

pub struct LoggerGuard {
    _file_guard: WorkerGuard,
}

pub fn init_logger(log_dir: PathBuf) -> LoggerGuard {
    if !log_dir.exists() {
        let _ = std::fs::create_dir_all(&log_dir);
    }

    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("irtool")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&log_dir)
        .expect("failed to create rolling file appender");

    let (non_blocking, file_guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,irtool=debug,tauri=info"));

    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .with_filter(env_filter);

    let console_layer = if cfg!(debug_assertions) {
        Some(
            fmt::layer()
                .with_target(true)
                .with_ansi(true)
                .compact()
                .with_filter(
                    EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| EnvFilter::new("debug,tauri=info")),
                ),
        )
    } else {
        None
    };

    tracing_subscriber::registry()
        .with(file_layer)
        .with(console_layer)
        .init();

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        log_dir = %log_dir.display(),
        "logger initialized"
    );

    LoggerGuard {
        _file_guard: file_guard,
    }
}
```

- [ ] **Step 9.2: 修改 `crates/irtool-tauri/src/main.rs` 调用 logger**

在 `main()` 顶部加：

```rust
mod logger;
mod single_instance;

use tauri::Manager;
use tracing::info;

// ...保持 cmd_app_info 和 is_running_as_admin

fn main() {
    let log_dir = if cfg!(debug_assertions) {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("logs")
    } else {
        // release: %LOCALAPPDATA%\IRtool\logs
        let local = std::env::var_os("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        local.join("IRtool").join("logs")
    };

    let _logger_guard = logger::init_logger(log_dir.clone());

    info!("============================================");
    info!("IRtool v{} starting", env!("CARGO_PKG_VERSION"));
    info!("Log dir: {}", log_dir.display());
    info!("Admin: {}", is_running_as_admin());
    info!("============================================");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            single_instance::handle_second_instance(app, args, cwd);
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![cmd_app_info])
        .setup(|app| {
            info!("main window setup");
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

- [ ] **Step 9.3: 验证日志写入**

```bash
cd "D:/project/IRtool-next"
cargo tauri dev
```

预期：
1. 控制台输出彩色日志
2. `D:\project\IRtool-next\logs\irtool.YYYY-MM-DD.log` 文件创建
3. 文件内含 "logger initialized"、"IRtool v... starting"、"main window setup"

关闭后查看：

```bash
cat "D:/project/IRtool-next/logs/irtool."*.log | head -20
```

- [ ] **Step 9.4: 提交**

```bash
git add crates/irtool-tauri/
git commit -m "feat(tauri): tracing logger with daily rotation and console layer in dev"
```

---

## Task 10: 深浅主题切换 + 持久化

**Files:**
- Create: `ui/src/stores/theme-store.ts`
- Create: `ui/src/components/theme/ThemeProvider.tsx`
- Modify: `ui/src/main.tsx`

- [ ] **Step 10.1: 安装 zustand**

```bash
cd "D:/project/IRtool-next/ui"
pnpm add zustand
```

- [ ] **Step 10.2: 创建 `ui/src/stores/theme-store.ts`**

```typescript
import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";

export type Theme = "dark" | "light" | "system";

interface ThemeState {
  theme: Theme;
  resolvedTheme: "dark" | "light";
  setTheme: (theme: Theme) => void;
  applyResolvedTheme: () => void;
}

function resolveSystem(): "dark" | "light" {
  if (typeof window === "undefined") return "dark";
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

export const useThemeStore = create<ThemeState>()(
  persist(
    (set, get) => ({
      theme: "dark",
      resolvedTheme: "dark",
      setTheme: (theme) => {
        const resolved = theme === "system" ? resolveSystem() : theme;
        set({ theme, resolvedTheme: resolved });
        document.documentElement.setAttribute("data-theme", resolved);
        document.documentElement.classList.toggle("dark", resolved === "dark");
      },
      applyResolvedTheme: () => {
        const { theme } = get();
        const resolved = theme === "system" ? resolveSystem() : theme;
        set({ resolvedTheme: resolved });
        document.documentElement.setAttribute("data-theme", resolved);
        document.documentElement.classList.toggle("dark", resolved === "dark");
      },
    }),
    {
      name: "irtool-theme",
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({ theme: state.theme }),
    },
  ),
);
```

- [ ] **Step 10.3: 创建 `ui/src/components/theme/ThemeProvider.tsx`**

```typescript
import { useEffect } from "react";
import { useThemeStore } from "@/stores/theme-store";

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const applyResolvedTheme = useThemeStore((s) => s.applyResolvedTheme);
  const theme = useThemeStore((s) => s.theme);

  useEffect(() => {
    applyResolvedTheme();
  }, [theme, applyResolvedTheme]);

  useEffect(() => {
    if (theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => applyResolvedTheme();
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [theme, applyResolvedTheme]);

  return <>{children}</>;
}
```

- [ ] **Step 10.4: 修改 `ui/src/main.tsx` 包裹 ThemeProvider**

```typescript
import React from "react";
import ReactDOM from "react-dom/client";
import "./styles/globals.css";
import App from "./App";
import { ThemeProvider } from "@/components/theme/ThemeProvider";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ThemeProvider>
      <App />
    </ThemeProvider>
  </React.StrictMode>,
);
```

- [ ] **Step 10.5: 添加主题切换按钮到 App**

修改 `ui/src/App.tsx`：

```typescript
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Sun, Moon, Monitor } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { useThemeStore } from "@/stores/theme-store";

interface AppInfo {
  name: string;
  version: string;
  build: string;
  is_admin: boolean;
}

function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { theme, setTheme } = useThemeStore();

  useEffect(() => {
    invoke<AppInfo>("cmd_app_info")
      .then(setInfo)
      .catch((e) => setError(String(e)));
  }, []);

  return (
    <div className="min-h-screen p-6 space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-semibold">IRtool v2 boot</h1>
        <div className="flex gap-1">
          <Button
            variant={theme === "dark" ? "default" : "ghost"}
            size="icon"
            onClick={() => setTheme("dark")}
            title="Dark"
          >
            <Moon className="h-4 w-4" />
          </Button>
          <Button
            variant={theme === "light" ? "default" : "ghost"}
            size="icon"
            onClick={() => setTheme("light")}
            title="Light"
          >
            <Sun className="h-4 w-4" />
          </Button>
          <Button
            variant={theme === "system" ? "default" : "ghost"}
            size="icon"
            onClick={() => setTheme("system")}
            title="System"
          >
            <Monitor className="h-4 w-4" />
          </Button>
        </div>
      </div>
      <Separator />
      {info && (
        <pre className="bg-bg-elev-1 border border-border p-3 rounded font-mono text-sm text-fg-secondary">
          {JSON.stringify(info, null, 2)}
        </pre>
      )}
      {error && <p className="text-danger">error: {error}</p>}
    </div>
  );
}

export default App;
```

- [ ] **Step 10.6: 验证主题持久化**

```bash
cargo tauri dev
```

预期：
1. 启动深色主题（默认）
2. 点 Sun 切换到浅色，背景立即变白
3. 关闭窗口
4. 再次 `cargo tauri dev` 启动 → 应自动浅色（持久化生效）
5. 切回 dark 验证再次启动深色

- [ ] **Step 10.7: 提交**

```bash
git add ui/src/stores/ ui/src/components/theme/ ui/src/main.tsx ui/src/App.tsx ui/package.json ui/pnpm-lock.yaml
git commit -m "feat(ui): theme store with localStorage persistence + dark/light/system toggle"
```

---

## Task 11: 路由 + i18next

**Files:**
- Create: `ui/src/lib/i18n.ts`
- Create: `ui/src/locales/zh-CN.json`
- Create: `ui/src/locales/en-US.json`
- Create: `ui/src/routes/__root.tsx`
- Create: `ui/src/routes/network.tsx`
- Create: `ui/src/routes/log-collector.tsx`
- Create: `ui/src/routes/autoruns.tsx`
- Create: `ui/src/routes/workspace.tsx`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/main.tsx`

- [ ] **Step 11.1: 安装路由 + i18n 依赖**

```bash
cd "D:/project/IRtool-next/ui"
pnpm add @tanstack/react-router @tanstack/react-router-devtools
pnpm add -D @tanstack/router-plugin
pnpm add i18next react-i18next i18next-browser-languagedetector
```

- [ ] **Step 11.2: 修改 `ui/vite.config.ts` 加 router 插件**

```typescript
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { TanStackRouterVite } from "@tanstack/router-plugin/vite";
import path from "node:path";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [
    TanStackRouterVite({ target: "react", autoCodeSplitting: true }),
    react(),
  ],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 5174 }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**", "**/crates/**", "**/target/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "es2022",
    minify: "esbuild",
    sourcemap: true,
    chunkSizeWarningLimit: 1000,
  },
});
```

- [ ] **Step 11.3: 创建 `ui/src/locales/zh-CN.json`**

```json
{
  "app": {
    "name": "IRtool",
    "tagline": "Windows 应急响应工具"
  },
  "nav": {
    "network": "网络监控",
    "log-collector": "日志采集",
    "autoruns": "持久化检测",
    "workspace": "工作台",
    "settings": "设置"
  },
  "status": {
    "admin": "管理员",
    "non-admin": "非管理员",
    "sysmon-installed": "Sysmon 已安装",
    "sysmon-running": "Sysmon 运行中",
    "sysmon-not-installed": "Sysmon 未安装"
  },
  "common": {
    "loading": "加载中…",
    "empty": "暂无数据",
    "retry": "重试",
    "cancel": "取消",
    "confirm": "确认"
  }
}
```

- [ ] **Step 11.4: 创建 `ui/src/locales/en-US.json`**

```json
{
  "app": {
    "name": "IRtool",
    "tagline": "Windows Incident Response Tool"
  },
  "nav": {
    "network": "Network",
    "log-collector": "Log Collector",
    "autoruns": "Autoruns",
    "workspace": "Workspace",
    "settings": "Settings"
  },
  "status": {
    "admin": "Admin",
    "non-admin": "Non-admin",
    "sysmon-installed": "Sysmon installed",
    "sysmon-running": "Sysmon running",
    "sysmon-not-installed": "Sysmon not installed"
  },
  "common": {
    "loading": "Loading…",
    "empty": "No data",
    "retry": "Retry",
    "cancel": "Cancel",
    "confirm": "Confirm"
  }
}
```

- [ ] **Step 11.5: 创建 `ui/src/lib/i18n.ts`**

```typescript
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import LanguageDetector from "i18next-browser-languagedetector";

import zhCN from "@/locales/zh-CN.json";
import enUS from "@/locales/en-US.json";

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources: {
      "zh-CN": { translation: zhCN },
      "en-US": { translation: enUS },
    },
    fallbackLng: "zh-CN",
    supportedLngs: ["zh-CN", "en-US"],
    interpolation: { escapeValue: false },
    detection: {
      order: ["localStorage", "navigator"],
      lookupLocalStorage: "irtool-lang",
      caches: ["localStorage"],
    },
  });

export default i18n;
```

- [ ] **Step 11.6: 创建路由文件**

`ui/src/routes/__root.tsx`:

```typescript
import { createRootRoute, Outlet } from "@tanstack/react-router";
import { TanStackRouterDevtools } from "@tanstack/react-router-devtools";

export const Route = createRootRoute({
  component: () => (
    <>
      <Outlet />
      {import.meta.env.DEV && <TanStackRouterDevtools position="bottom-right" />}
    </>
  ),
});
```

`ui/src/routes/network.tsx`:

```typescript
import { createFileRoute } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

export const Route = createFileRoute("/network")({
  component: NetworkPage,
});

function NetworkPage() {
  const { t } = useTranslation();
  return (
    <div className="p-6">
      <h2 className="text-lg font-semibold">{t("nav.network")}</h2>
      <p className="text-fg-secondary mt-2">P1 阶段实装</p>
    </div>
  );
}
```

`ui/src/routes/log-collector.tsx`:

```typescript
import { createFileRoute } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

export const Route = createFileRoute("/log-collector")({
  component: LogCollectorPage,
});

function LogCollectorPage() {
  const { t } = useTranslation();
  return (
    <div className="p-6">
      <h2 className="text-lg font-semibold">{t("nav.log-collector")}</h2>
      <p className="text-fg-secondary mt-2">P4 阶段实装</p>
    </div>
  );
}
```

`ui/src/routes/autoruns.tsx`:

```typescript
import { createFileRoute } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

export const Route = createFileRoute("/autoruns")({
  component: AutorunsPage,
});

function AutorunsPage() {
  const { t } = useTranslation();
  return (
    <div className="p-6">
      <h2 className="text-lg font-semibold">{t("nav.autoruns")}</h2>
      <p className="text-fg-secondary mt-2">P2 阶段实装</p>
    </div>
  );
}
```

`ui/src/routes/workspace.tsx`:

```typescript
import { createFileRoute } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

export const Route = createFileRoute("/workspace")({
  component: WorkspacePage,
});

function WorkspacePage() {
  const { t } = useTranslation();
  return (
    <div className="p-6">
      <h2 className="text-lg font-semibold">{t("nav.workspace")}</h2>
      <p className="text-fg-secondary mt-2">P3 阶段实装</p>
    </div>
  );
}
```

`ui/src/routes/index.tsx`（默认重定向到 network）:

```typescript
import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/")({
  beforeLoad: () => {
    throw redirect({ to: "/network" });
  },
});
```

- [ ] **Step 11.7: 修改 `ui/src/main.tsx` 集成 router 与 i18n**

```typescript
import React from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider, createRouter } from "@tanstack/react-router";
import "./styles/globals.css";
import "./lib/i18n";
import { ThemeProvider } from "@/components/theme/ThemeProvider";
import { routeTree } from "./routeTree.gen";

const router = createRouter({ routeTree });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ThemeProvider>
      <RouterProvider router={router} />
    </ThemeProvider>
  </React.StrictMode>,
);
```

注：`routeTree.gen.ts` 由 router 插件自动生成，首次启动 `pnpm dev` 时会创建。

- [ ] **Step 11.8: 删除原 `ui/src/App.tsx`（被路由取代）**

```bash
rm "D:/project/IRtool-next/ui/src/App.tsx"
```

App 内容 Task 12 会以布局形式重新组织。

- [ ] **Step 11.9: 验证路由与 i18n**

```bash
cd "D:/project/IRtool-next"
cargo tauri dev
```

预期：
1. Vite 启动时 router 插件生成 `routeTree.gen.ts`
2. Tauri 窗口默认显示 "网络监控"（重定向自 /）
3. DevTools 切换 URL（在 Console 跑 `window.history.pushState({}, '', '/autoruns')` + reload）应渲染对应页面
4. localStorage 设 `irtool-lang=en-US` reload → 标题切换为 "Network"

- [ ] **Step 11.10: 提交**

```bash
git add ui/src/locales/ ui/src/lib/i18n.ts ui/src/routes/ ui/src/main.tsx ui/vite.config.ts ui/package.json ui/pnpm-lock.yaml
git rm ui/src/App.tsx
git commit -m "feat(ui): TanStack Router file-based routing + i18next zh-CN/en-US"
```

---

## Task 12: 顶层布局 (Sidebar / TopBar / StatusBar)

**Files:**
- Create: `ui/src/components/layout/AppShell.tsx`
- Create: `ui/src/components/layout/Sidebar.tsx`
- Create: `ui/src/components/layout/TopBar.tsx`
- Create: `ui/src/components/layout/StatusBar.tsx`
- Modify: `ui/src/routes/__root.tsx`

- [ ] **Step 12.1: 创建 `ui/src/components/layout/Sidebar.tsx`**

```typescript
import { Link, useRouterState } from "@tanstack/react-router";
import { Activity, ScrollText, Repeat, Briefcase, Settings } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";

interface NavItem {
  to: string;
  icon: React.ComponentType<{ className?: string }>;
  i18nKey: string;
}

const NAV_ITEMS: NavItem[] = [
  { to: "/network", icon: Activity, i18nKey: "nav.network" },
  { to: "/log-collector", icon: ScrollText, i18nKey: "nav.log-collector" },
  { to: "/autoruns", icon: Repeat, i18nKey: "nav.autoruns" },
  { to: "/workspace", icon: Briefcase, i18nKey: "nav.workspace" },
];

export function Sidebar() {
  const { t } = useTranslation();
  const path = useRouterState({ select: (s) => s.location.pathname });

  return (
    <TooltipProvider delayDuration={300}>
      <aside className="w-14 bg-bg-elev-1 border-r border-border flex flex-col">
        <div className="flex-1 flex flex-col items-center pt-3 gap-1">
          {NAV_ITEMS.map((item) => {
            const Icon = item.icon;
            const isActive = path.startsWith(item.to);
            return (
              <Tooltip key={item.to}>
                <TooltipTrigger asChild>
                  <Link
                    to={item.to}
                    className={cn(
                      "h-10 w-10 rounded-md flex items-center justify-center transition-colors relative",
                      isActive
                        ? "text-accent"
                        : "text-fg-tertiary hover:text-fg-primary hover:bg-bg-elev-2",
                    )}
                  >
                    {isActive && (
                      <span className="absolute left-0 top-1.5 bottom-1.5 w-0.5 bg-accent rounded-r" />
                    )}
                    <Icon className="h-5 w-5" />
                  </Link>
                </TooltipTrigger>
                <TooltipContent side="right">
                  {t(item.i18nKey)}
                </TooltipContent>
              </Tooltip>
            );
          })}
        </div>
        <div className="pb-3 flex flex-col items-center gap-1">
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                className={cn(
                  "h-10 w-10 rounded-md flex items-center justify-center transition-colors",
                  "text-fg-tertiary hover:text-fg-primary hover:bg-bg-elev-2",
                )}
              >
                <Settings className="h-5 w-5" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="right">{t("nav.settings")}</TooltipContent>
          </Tooltip>
        </div>
      </aside>
    </TooltipProvider>
  );
}
```

- [ ] **Step 12.2: 创建 `ui/src/components/layout/TopBar.tsx`**

```typescript
import { Search, Sun, Moon } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { useThemeStore } from "@/stores/theme-store";

export function TopBar() {
  const { t } = useTranslation();
  const { resolvedTheme, setTheme } = useThemeStore();

  return (
    <header className="h-10 bg-bg-elev-1 border-b border-border flex items-center px-3 gap-3">
      <div className="flex items-center gap-2">
        <span className="text-sm font-semibold text-fg-primary">
          {t("app.name")}
        </span>
        <span className="text-xs text-fg-tertiary">v2.0.0-alpha.1</span>
      </div>
      <div className="flex-1 max-w-xl mx-auto">
        <div className="relative">
          <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-fg-tertiary" />
          <input
            type="text"
            placeholder="Ctrl+P"
            className="w-full h-7 bg-bg-base border border-border rounded pl-7 pr-3 text-xs placeholder:text-fg-tertiary focus:outline-none focus:border-accent"
            disabled
          />
        </div>
      </div>
      <div className="flex items-center gap-1">
        <Button
          variant="ghost"
          size="icon"
          onClick={() =>
            setTheme(resolvedTheme === "dark" ? "light" : "dark")
          }
          title={resolvedTheme === "dark" ? "Light" : "Dark"}
        >
          {resolvedTheme === "dark" ? (
            <Sun className="h-4 w-4" />
          ) : (
            <Moon className="h-4 w-4" />
          )}
        </Button>
      </div>
    </header>
  );
}
```

- [ ] **Step 12.3: 创建 `ui/src/components/layout/StatusBar.tsx`**

```typescript
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { Shield, ShieldOff, Clock } from "lucide-react";

interface AppInfo {
  name: string;
  version: string;
  build: string;
  is_admin: boolean;
}

export function StatusBar() {
  const { t } = useTranslation();
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [now, setNow] = useState(new Date());

  useEffect(() => {
    invoke<AppInfo>("cmd_app_info").then(setInfo).catch(() => null);
  }, []);

  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(id);
  }, []);

  return (
    <footer className="h-6 bg-bg-elev-1 border-t border-border flex items-center px-3 gap-3 text-xs text-fg-secondary">
      <div className="flex items-center gap-1">
        {info?.is_admin ? (
          <>
            <Shield className="h-3 w-3 text-success" />
            <span>{t("status.admin")}</span>
          </>
        ) : (
          <>
            <ShieldOff className="h-3 w-3 text-warning" />
            <span>{t("status.non-admin")}</span>
          </>
        )}
      </div>
      <div className="h-3 w-px bg-border" />
      <div>{t("status.sysmon-not-installed")}</div>
      <div className="flex-1" />
      <div className="flex items-center gap-1">
        <Clock className="h-3 w-3" />
        <span className="font-mono">
          {now.toLocaleTimeString("en-GB", { hour12: false })}
        </span>
      </div>
    </footer>
  );
}
```

- [ ] **Step 12.4: 创建 `ui/src/components/layout/AppShell.tsx`**

```typescript
import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";
import { StatusBar } from "./StatusBar";

export function AppShell({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex h-screen bg-bg-base text-fg-primary">
      <Sidebar />
      <div className="flex-1 flex flex-col min-w-0">
        <TopBar />
        <main className="flex-1 overflow-auto bg-bg-base">{children}</main>
        <StatusBar />
      </div>
    </div>
  );
}
```

- [ ] **Step 12.5: 修改 `ui/src/routes/__root.tsx` 用 AppShell 包裹**

```typescript
import { createRootRoute, Outlet } from "@tanstack/react-router";
import { TanStackRouterDevtools } from "@tanstack/react-router-devtools";
import { AppShell } from "@/components/layout/AppShell";

export const Route = createRootRoute({
  component: () => (
    <AppShell>
      <Outlet />
      {import.meta.env.DEV && <TanStackRouterDevtools position="bottom-right" />}
    </AppShell>
  ),
});
```

- [ ] **Step 12.6: 启动验证**

```bash
cd "D:/project/IRtool-next"
cargo tauri dev
```

预期：
1. 窗口左侧 56px 侧栏显示 4 个图标 + 1 个设置图标
2. 顶部 40px TopBar 显示 IRtool + 搜索框 + 主题切换
3. 底部 24px StatusBar 显示 "管理员" + Sysmon 状态 + 实时时钟
4. 中间渲染当前路由内容（默认网络监控页）
5. 点击侧栏切换 4 个 Tab，路由切换正常
6. 主题切换按钮可工作

- [ ] **Step 12.7: 提交**

```bash
git add ui/src/components/layout/ ui/src/routes/__root.tsx
git commit -m "feat(ui): top-level AppShell with sidebar/topbar/statusbar matching design tokens"
```

---

## Task 13: specta 类型自动生成

**Files:**
- Modify: `crates/irtool-tauri/src/main.rs` (使用 tauri-specta 收集器)
- Create: `crates/irtool-tauri/src/types.rs` (聚合所有 Type 注册)
- Create: `ui/src/lib/bindings.ts` (生成产物，加 .gitignore 例外)

- [ ] **Step 13.1: 创建 `crates/irtool-tauri/src/types.rs`**

```rust
//! 聚合所有需暴露给前端的类型，由 specta 生成 TS 定义。

pub use irtool_core::{AppConfig, IrError, Language, Theme};
pub use irtool_threat_intel::{IntelResult, IocQuery};
```

- [ ] **Step 13.2: 改造 `crates/irtool-tauri/src/main.rs` 用 tauri-specta**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod logger;
mod single_instance;
mod types;

use serde::Serialize;
use specta::Type;
use specta_typescript::Typescript;
use tauri::Manager;
use tauri_specta::{collect_commands, Builder};
use tracing::info;

#[derive(Serialize, Type)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub build: String,
    pub is_admin: bool,
}

#[tauri::command]
#[specta::specta]
fn cmd_app_info() -> AppInfo {
    AppInfo {
        name: "IRtool".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        build: "alpha".into(),
        is_admin: is_running_as_admin(),
    }
}

#[cfg(windows)]
fn is_running_as_admin() -> bool {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token: HANDLE = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut size = 0u32;
        GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        )
        .is_ok()
            && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(windows))]
fn is_running_as_admin() -> bool {
    false
}

fn main() {
    let log_dir = if cfg!(debug_assertions) {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("logs")
    } else {
        let local = std::env::var_os("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        local.join("IRtool").join("logs")
    };

    let _logger_guard = logger::init_logger(log_dir.clone());

    info!("============================================");
    info!("IRtool v{} starting", env!("CARGO_PKG_VERSION"));
    info!("Log dir: {}", log_dir.display());
    info!("Admin: {}", is_running_as_admin());
    info!("============================================");

    let builder = Builder::<tauri::Wry>::new()
        .commands(collect_commands![cmd_app_info]);

    #[cfg(debug_assertions)]
    {
        builder
            .export(
                Typescript::default()
                    .header("// @ts-nocheck\n// auto-generated by tauri-specta — DO NOT EDIT\n"),
                "../../ui/src/lib/bindings.ts",
            )
            .expect("failed to export bindings.ts");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            single_instance::handle_second_instance(app, args, cwd);
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            info!("main window setup");
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

- [ ] **Step 13.3: 第一次构建生成 bindings**

```bash
cd "D:/project/IRtool-next"
cargo tauri dev
```

启动后立即检查 `D:/project/IRtool-next/ui/src/lib/bindings.ts` 是否生成。预期内容包含：

```typescript
// @ts-nocheck
// auto-generated by tauri-specta — DO NOT EDIT

export const commands = {
  async cmdAppInfo(): Promise<AppInfo> { /* ... */ },
};

export type AppInfo = {
  name: string;
  version: string;
  build: string;
  is_admin: boolean;
};
```

- [ ] **Step 13.4: 修改 StatusBar 与 main 用生成的类型**

修改 `ui/src/components/layout/StatusBar.tsx` 顶部：

```typescript
import { commands, type AppInfo } from "@/lib/bindings";
// 替换原来的 invoke 调用：
useEffect(() => {
  commands.cmdAppInfo().then(setInfo).catch(() => null);
}, []);
```

- [ ] **Step 13.5: 验证类型同步**

故意修改 `crates/irtool-tauri/src/main.rs` 的 `AppInfo` 结构（如加一个 `pub debug: bool` 字段），重启 `cargo tauri dev`，检查 `bindings.ts` 是否同步更新该字段。

验证后还原（删除新加字段）。

- [ ] **Step 13.6: 提交**

```bash
git add crates/irtool-tauri/src/ ui/src/lib/bindings.ts ui/src/components/layout/StatusBar.tsx
git commit -m "feat(tauri): tauri-specta auto-generates TS bindings on dev build"
```

---

## Task 14: GitHub Actions CI

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `.github/workflows/audit.yml`
- Create: `.github/workflows/bench.yml`

- [ ] **Step 14.1: 创建 `.github/workflows/ci.yml`**

```yaml
name: CI

on:
  pull_request:
  push:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: "-D warnings"

jobs:
  rust:
    name: Rust check
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - name: cargo fmt
        run: cargo fmt --all -- --check
      - name: cargo clippy
        run: cargo clippy --workspace --all-targets -- -D warnings
      - name: cargo test
        run: cargo test --workspace --no-fail-fast

  ui:
    name: UI check
    runs-on: windows-latest
    defaults:
      run:
        working-directory: ui
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v4
        with:
          version: 9
      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: pnpm
          cache-dependency-path: ui/pnpm-lock.yaml
      - name: Install
        run: pnpm install --frozen-lockfile
      - name: Type check
        run: pnpm lint
      - name: Build
        run: pnpm build

  tauri-build:
    name: Tauri build smoke
    runs-on: windows-latest
    needs: [rust, ui]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - uses: pnpm/action-setup@v4
        with:
          version: 9
      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: pnpm
          cache-dependency-path: ui/pnpm-lock.yaml
      - name: Install UI
        working-directory: ui
        run: pnpm install --frozen-lockfile
      - name: Build UI
        working-directory: ui
        run: pnpm build
      - name: Cargo build release
        run: cargo build --release -p irtool-tauri
```

- [ ] **Step 14.2: 创建 `.github/workflows/audit.yml`**

```yaml
name: Security audit

on:
  schedule:
    - cron: "0 9 * * 1"  # 每周一 9:00 UTC
  workflow_dispatch:

jobs:
  cargo-audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Install cargo-audit
        run: cargo install cargo-audit --locked
      - name: Run cargo audit
        run: cargo audit --deny warnings

  npm-audit:
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: ui
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v4
        with:
          version: 9
      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: pnpm
          cache-dependency-path: ui/pnpm-lock.yaml
      - name: Install
        run: pnpm install --frozen-lockfile
      - name: Audit
        run: pnpm audit --audit-level=high
```

- [ ] **Step 14.3: 创建 `.github/workflows/bench.yml` (P0 占位)**

```yaml
name: Benchmark baseline

on:
  schedule:
    - cron: "0 10 * * 1"
  workflow_dispatch:

jobs:
  bench:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Placeholder
        run: |
          echo "Benchmarks will be added in P5"
          exit 0
```

- [ ] **Step 14.4: 本地验证 workflows 语法**

```bash
cd "D:/project/IRtool-next"
# 用 act 或 yamllint 检查 (可选,无 act 则跳过)
# 主要确认 YAML 语法正确
```

人工 review 三份 yml 即可。

- [ ] **Step 14.5: 提交**

```bash
git add .github/
git commit -m "ci: add ci/audit/bench workflows for rust + ui pipelines"
```

---

## P0 验收清单

完成所有 14 个任务后，对照下表逐项验收：

| 验收项 | 验证方式 | 预期结果 |
|---|---|---|
| Workspace 编译通过 | `cargo build --workspace` | 无 error/warning |
| 全部测试通过 | `cargo test --workspace` | 4+ tests passed |
| Clippy 无 warning | `cargo clippy --workspace --all-targets -- -D warnings` | 通过 |
| 前端 build 通过 | `cd ui && pnpm build` | dist/ 生成 |
| Tauri dev 启动 | `cargo tauri dev` | 弹出窗口，UAC 提权 |
| 主题切换 | UI 顶栏切换 | 立即变换且重启保留 |
| 路由切换 | 点击 4 个 Tab | URL 与内容同步变化 |
| i18n 中英文 | localStorage 改 `irtool-lang` 后重启 | 文案切换 |
| 单实例 | 重复启动 | 第二次激活第一个窗口 |
| 日志写入 | 检查 `logs/irtool.YYYY-MM-DD.log` | 含启动日志 |
| TS bindings 自动生成 | 修改 Rust 类型后重启 dev | bindings.ts 同步更新 |
| StatusBar 显示管理员 | UAC 通过启动 | "管理员" + 绿色盾牌 |
| GitHub Actions CI | 推到远端 | 三个 workflow 触发并通过 |

完成后打 tag：

```bash
git tag v2.0.0-alpha.P0
```

---

## P1-P5 后续

P0 完成后再分别产出以下 plan（基于 P0 实际工程经验调整粒度）：

- `2026-XX-XX-IRtool-v2-P1-network.md` - 网络监控
- `2026-XX-XX-IRtool-v2-P2-autoruns.md` - 持久化检测
- `2026-XX-XX-IRtool-v2-P3-workspace.md` - 工作台 + 规则引擎
- `2026-XX-XX-IRtool-v2-P4-log-collector.md` - 日志采集 + 进程树 + Timeline
- `2026-XX-XX-IRtool-v2-P5-release.md` - 性能基线 + E2E + 打包发布

每份 plan 单独成文,实施完毕后自然进入下一个。
