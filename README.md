# IRtool

[![Version](https://img.shields.io/badge/v2.0.2-blue.svg)](https://github.com/summerxzp/IRtool-next/releases)
[![License](https://img.shields.io/badge/MIT-green.svg)](LICENSE)
[![Windows](https://img.shields.io/badge/Windows%20x64-lightgrey.svg)]()

Windows 应急响应工具，面向一线安全人员，提供主机取证、威胁发现与快速处置能力。

## 功能

**持久化检测** — 扫描启动项、计划任务、服务、驱动等持久化条目，自动签名验证与风险评级

**网络监控** — 实时 TCP/UDP 连接监控，关联进程路径与命令行，连接历史回溯

**日志采集** — Sysmon 事件实时订阅（DNS/网络/远程线程/文件创建等），DNS 抓包与 TLS SNI 提取

**数据库检索** — 全量事件查询，按来源/类型/进程/域名过滤，CSV 导出

**处置工作台** — 跨数据源搜索、IOC 规则扫描、关联发现、一键处置（终止进程/删除条目/跳转注册表）

**外部工具** — Bootstrap 模式自动下载 Sysinternals 工具，Authenticode 签名验证

## 下载

[Releases](https://github.com/summerxzp/IRtool-next/releases) 页面下载便携版，解压即用。

> 需要管理员权限 + WebView2 Runtime（Win10/11 通常已预装）

## 开发

```bash
cd ui && pnpm install && cd ..
cargo tauri dev
```

## 许可证

[MIT](LICENSE)
