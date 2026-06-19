# IRTool 打包指南

## 前置要求

- Rust toolchain (stable)
- Node.js + pnpm
- Windows SDK (用于 manifest 嵌入)

## 快速打包

```powershell
# 基础便携版（不含 WebView2 安装器）
powershell -ExecutionPolicy Bypass -File scripts/build-portable.ps1

# 包含 WebView2 引导程序
powershell -ExecutionPolicy Bypass -File scripts/build-portable.ps1 -IncludeWebView2Bootstrapper

# 包含 WebView2 离线安装器（推荐用于无网络环境）
powershell -ExecutionPolicy Bypass -File scripts/build-portable.ps1 -IncludeOfflineInstallers
```

## 打包流程

`build-portable.ps1` 自动执行以下步骤：

1. **前端构建**：`pnpm build`（在 `ui/` 目录）
2. **Rust 构建**：`cargo tauri build --no-bundle --features irtool-tauri/egui-fallback`
   - `--no-bundle`：只生成 exe，不生成 MSI/NSIS 安装包
   - `--features irtool-tauri/egui-fallback`：启用 egui 降级 UI
3. **打包目录准备**：创建 `dist/IRTool-v{version}-portable/` 目录
   - 复制 exe 为 `IRtool.exe`
   - 创建 `portable.flag`（便携模式标识）
   - 创建空目录：`config/`、`data/`、`logs/`、`tools/`
4. **ZIP 压缩**：生成 `dist/IRTool-v{version}-portable.zip`

## 版本号管理

版本号在 `Cargo.toml`（workspace）的 `[workspace.package] version` 中统一管理。

修改版本号后，运行同步脚本将版本号传播到所有配置文件：

```powershell
node scripts/sync-version.js
```

同步目标：
- `ui/package.json`
- `crates/irtool-tauri/tauri.conf.json`
- `crates/irtool-tauri/app-manifest.xml`（assemblyIdentity version）

## 管理员权限

应用通过 Windows Manifest (`app-manifest.xml`) 声明 `requireAdministrator`，双击 exe 时会自动弹出 UAC 提权提示。

**关键**：manifest 通过 Tauri 的 `WindowsAttributes::app_manifest()` 嵌入（见 `build.rs`），不能用 `embed_resource` 单独嵌入，否则会与 Tauri 生成的 manifest 冲突。

## egui 降级 UI

当系统缺少 WebView2 运行时时，应用自动降级到 egui 原生 UI。

- 检测逻辑：`crates/irtool-tauri/src/main.rs` 查询注册表 `HKLM` + `HKCU` 的 WebView2 安装信息
- 降级入口：`irtool_egui::run(StartupMode::Fallback)`
- 降级时会弹窗提示 WebView2 缺失，提供下载链接

## 开发模式

```powershell
# 启动开发服务器（不编译 egui-fallback）
cargo tauri dev

# 单独编译 egui crate（用于调试）
cargo build -p irtool-egui
```

## 产物说明

| 文件/目录 | 说明 |
|-----------|------|
| `IRtool.exe` | 主程序（含 React UI + egui 降级 UI） |
| `portable.flag` | 便携模式标识（存在时使用相对路径） |
| `config/` | 配置文件目录 |
| `data/` | 数据文件目录（数据库等） |
| `logs/` | 日志目录（按天滚动） |
| `tools/` | 外部工具目录（autorunsc64.exe 等） |
