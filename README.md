# IRtool

[![Version](https://img.shields.io/badge/version-v2.0.0-blue.svg)](https://github.com/summerxzp/IRtool-next)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20x64-lightgrey.svg)]()
[![Tauri](https://img.shields.io/badge/Tauri-v2-orange.svg)](https://v2.tauri.app)
[![Rust](https://img.shields.io/badge/Rust-1.80%2B-dea584.svg)](https://www.rust-lang.org)

Windows 应急响应桌面工具，面向一线安全人员提供主机取证、威胁发现与快速处置能力。

## 功能概览

| 模块 | 功能 |
|------|------|
| **持久化检测** | 基于 Sysinternals Autoruns 扫描启动项、计划任务、服务等，自动签名验证与风险评估 |
| **网络监控** | 实时 TCP/UDP 连接监控，进程命令行关联（WMI），连接历史与统计 |
| **日志采集** | Sysmon 事件订阅（DNS/网络连接/远程线程/文件创建等），DNS 抓包与 TLS SNI 提取 |
| **数据库检索** | 全量事件数据库查询，按来源/类型/进程/IP/域名过滤，CSV 导出 |
| **处置工作台** | 跨数据源搜索、IOC 规则扫描、关联发现、一键处置（终止进程/删除条目等） |
| **外部工具管理** | Bootstrap Installer 模式，自动下载 Sysinternals 工具，Authenticode 签名验证 |

## 技术栈

- **后端**: Rust + Tauri v2 + Win32 API
- **前端**: React + TypeScript + Tailwind CSS + shadcn/ui
- **数据库**: SQLite（事件存储）
- **监控**: etw-reader (Sysmon ETW) + WMI (进程命令行)

## 下载

从 [Releases](https://github.com/summerxzp/IRtool-next/releases) 页面下载便携版。

> 便携版解压即用，所有数据存放在 exe 同目录。也可将 exe 放到任意目录运行（安装模式数据存 `%APPDATA%/IRtool/`）。

### 运行要求

- Windows 10/11 x64
- 管理员权限（核心功能需要）
- [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)（Win10/11 通常已预装）

## 开发

```bash
# 安装前端依赖
cd ui && pnpm install && cd ..

# 启动开发模式
cargo tauri dev
```

### 构建便携版

```powershell
powershell -ExecutionPolicy Bypass -File scripts/build-portable.ps1
```

## 项目结构

```
├── crates/
│   ├── irtool-tauri/       # Tauri 应用入口
│   ├── irtool-core/        # 核心库（路径管理、错误类型）
│   ├── irtool-autoruns/    # 持久化检测
│   ├── irtool-net-monitor/ # 网络监控
│   ├── irtool-sysmon/      # Sysmon 管理
│   ├── irtool-monitor/     # 事件监测引擎
│   ├── irtool-tools/       # 外部工具管理
│   ├── irtool-rules/       # 规则引擎
│   └── irtool-pcap/        # 网络抓包
├── ui/                     # React 前端
├── scripts/                # 构建脚本
└── CONTEXT.md              # 领域术语与架构决策
```

## 许可证

[MIT License](LICENSE)
