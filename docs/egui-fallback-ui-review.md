# IRtool egui Fallback UI — 全面代码审查报告

> 审查范围：`crates/irtool-egui/` 全部源码 + `crates/irtool-tauri/src/main.rs` fallback 入口
> 审查基准：`docs/fallback机制与分层优化设计方案.md` + `crates/irtool-egui/DESIGN.md`
> 审查视角：完整性（需求覆盖）、正确性（逻辑/安全）、影响（副作用/回归）

---

## Critical Issues (MUST FIX)

### 1. 剪贴板复制存在命令注入漏洞

**位置**: [autoruns.rs#L1249-L1261](/e:/Code/IRtool-next/crates/irtool-egui/src/pages/autoruns.rs#L1249-L1261)
**来源**: 正确性审查

**问题**: `ui_copy_to_clipboard` 通过 `Command::new("powershell")` 执行 `Set-Clipboard -Value '...'`，文本中单引号仅做了 `''` 替换。但 PowerShell 的 `-Command` 参数解析时，单引号会终止字符串字面量，攻击者可构造包含 `'` 的 autorun 条目内容注入任意 PowerShell 命令。

例如 `text = "'; Remove-Item C:\ -Recurse; '"` 替换后为 `''; Remove-Item C:\ -Recurse; ''`，最终执行的命令变为三条独立语句。

**修复**: 避免通过 shell 命令操作剪贴板。使用 `arboard` crate（egui 生态常见依赖）或直接调用 Win32 `OpenClipboard`/`SetClipboardData`：

```rust
fn ui_copy_to_clipboard(_rt: &tokio::runtime::Handle, text: String) {
    #[cfg(windows)]
    {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(text);
        }
    }
    #[cfg(not(windows))]
    { tracing::info!("clipboard copy (non-windows): {}", text); }
}
```

---

### 2. `mask_url` 字节索引在非 ASCII 字符串上会 panic

**位置**: [settings.rs#L1020-L1028](/e:/Code/IRtool-next/crates/irtool-egui/src/pages/settings.rs#L1020-L1028)
**来源**: 正确性审查

**问题**: `&url[..4]` 和 `&url[url.len()-4..]` 使用字节索引。若 URL 包含多字节 UTF-8 字符（如国际化 URL `https://测试.example.com/路径`），字节位置 4 或 `len-4` 落在多字节字符中间时会 **panic**。

**修复**: 使用字符边界索引：

```rust
fn mask_url(url: &str) -> String {
    if url.is_empty() { return String::new(); }
    let head_end = url.char_indices().nth(4).map(|(i, _)| i).unwrap_or(url.len());
    if url.chars().count() <= 12 {
        return format!("{}****", &url[..head_end]);
    }
    let tail_start = url.char_indices().rev().nth(3).map(|(i, _)| i).unwrap_or(0);
    format!("{}****{}", &url[..head_end], &url[tail_start..])
}
```

---

### 3. 网络连接选中缓存仅用 PID 匹配，同 PID 多连接时选中错误连接

**位置**: [network.rs#L136-L137](/e:/Code/IRtool-next/crates/irtool-egui/src/pages/network.rs#L136-L137),
[network.rs#L163](/e:/Code/IRtool-next/crates/irtool-egui/src/pages/network.rs#L163),
[network.rs#L614](/e:/Code/IRtool-next/crates/irtool-egui/src/pages/network.rs#L614)
**来源**: 正确性 + 完整性审查

**问题**: 行选择使用 `(pid, local, remote)` 三元组标识连接（L605-L617），但 `selected_conn` 缓存的 `.find()` 仅匹配 `c.pid == pid`。当同一进程（如浏览器）有数十个 TCP 连接时，始终返回该 PID 的第一个连接，详情面板显示错误连接信息。

**修复**: 所有 `find` 调用都应同时匹配 `selected_local` 和 `selected_remote`：

```rust
if let (Some(pid), Some(ref local), Some(ref remote)) =
    (self.selected_pid, &self.selected_local, &self.selected_remote)
{
    self.selected_conn = payload.items.iter().find(|c| {
        c.pid == pid
            && format!("{}:{}", c.local.addr, c.local.port) == *local
            && format!("{}:{}", c.remote.addr, c.remote.port) == *remote
    }).cloned();
}
```

---

### 4. `tracing::warn!` 在日志系统初始化之前调用，消息被静默丢弃

**位置**: [main.rs#L11](/e:/Code/IRtool-next/crates/irtool-tauri/src/main.rs#L11)
**来源**: 影响审查

**问题**: WebView2 不可用时调用 `tracing::warn!`，但 Tauri 路径的日志在 `irtool_lib::run()` 中初始化，egui 路径的日志在 `irtool_egui::run()` 中初始化。此时 tracing subscriber 尚未注册，关键诊断信息"fallback to egui"被完全丢弃。

**修复**: 替换为 `eprintln!`（不依赖日志框架）：

```rust
fn main() {
    #[cfg(feature = "egui-fallback")]
    if !is_webview2_available() {
        eprintln!("WebView2 not available, falling back to egui frontend");
        irtool_egui::run(irtool_egui::StartupMode::Fallback);
        return;
    }
    irtool_lib::run();
}
```

---

## Warnings (SHOULD FIX)

### 5. Autoruns 右键菜单缺少 `ctx_menu_just_opened` 保护，菜单闪烁消失

**位置**: [autoruns.rs#L569-L571](/e:/Code/IRtool-next/crates/irtool-egui/src/pages/autoruns.rs#L569-L571),
[autoruns.rs#L658-L665](/e:/Code/IRtool-next/crates/irtool-egui/src/pages/autoruns.rs#L658-L665)
**来源**: 正确性 + 完整性审查（也见于完整性 — DESIGN.md §6.4 要求）

**问题**: Network 页面有 `ctx_menu_just_opened` 标志（L79-80, L714-716），在菜单打开帧跳过"点击外部关闭"检查。Autoruns 缺少此标志。右键点击 → 同帧 `any_click()` 为 true → 指针不在菜单区域内 → 菜单立即关闭，用户只能看到一帧闪烁。

**修复**: 添加 `ctx_menu_just_opened: bool` 字段，在右键时设 true，在 `render_context_menu` 关闭逻辑中跳过首帧：

```rust
if self.ctx_menu_just_opened {
    self.ctx_menu_just_opened = false;
} else if close_menu {
    self.ctx_menu_visible = false;
} else if ui.input(|i| i.pointer.any_click()) {
    // ... existing click-outside check
}
```

---

### 6. `sysmon.rs` 直接依赖 `irtool_pcap`，违反分层约定

**位置**: [sysmon.rs#L6](/e:/Code/IRtool-next/crates/irtool-egui/src/pages/sysmon.rs#L6)
**来源**: 完整性 + 影响审查（设计方案 §3.2 明确要求 egui 只依赖 irtool-service）

**问题**: `use irtool_pcap::PcapConfig` 直接引用底层 crate。`PcapConfig` 已在 `irtool-service/src/types.rs` 通过 `pub use` 正确 re-export，但 sysmon.rs 绕过了该路径。这是整个 egui 代码库中唯一的分层违规。

**修复**:

```rust
// 替换 L6:
use irtool_pcap::PcapConfig;
// 为:
use irtool_service::types::PcapConfig;
```

同时从 `crates/irtool-egui/Cargo.toml` 移除 `irtool-pcap` 直接依赖。

---

### 7. Heartbeat 重绘间隔 500ms vs 设计规范 1000ms

**位置**: [app.rs#L757](/e:/Code/IRtool-next/crates/irtool-egui/src/app.rs#L757)
**来源**: 完整性审查（DESIGN.md §10.3 规范 1000ms）

**问题**: EventBridge 事件驱动重绘（B4）已正确实现，heartbeat 仅为兜底。当前 500ms 是设计预期的两倍，增加不必要的空闲帧渲染和 CPU 消耗。

**修复**:

```rust
ctx.request_repaint_after(Duration::from_millis(1000));
```

---

### 8. Sidebar 宽度 140px vs 设计规范 180px

**位置**: [theme.rs#L95](/e:/Code/IRtool-next/crates/irtool-egui/src/theme.rs#L95)
**来源**: 完整性审查（DESIGN.md §4.2 规范 180px）

**问题**: `SIDEBAR_WIDTH = 140.0`，与 DESIGN.md 明确规范的 180px 不一致。中文导航标签（如"持久化检测"、"后台监控"）在 140px 下可能拥挤。

**修复**:

```rust
pub const SIDEBAR_WIDTH: f32 = 180.0;
```

---

### 9. `FindWindowW` 硬编码窗口标题 `"IRtool"` 有误匹配风险

**位置**: [app.rs#L811](/e:/Code/IRtool-next/crates/irtool-egui/src/app.rs#L811), [app.rs#L829](/e:/Code/IRtool-next/crates/irtool-egui/src/app.rs#L829)
**来源**: 影响审查

**问题**: `FindWindowW(None, w!("IRtool"))` 返回第一个匹配的顶级窗口。若用户有其他标题含 "IRtool" 的窗口，`post_close_to_window()` 发送 `WM_CLOSE` 可能导致无关应用被关闭。

**修复**: 在 `IrtoolApp::new` 时记录 winit 窗口的 HWND，存入 `Arc<AtomicIsize>` 供托盘闭包读取。短期可改为同时指定窗口类名：

```rust
FindWindowW(w!("winit-window-class-name"), w!("IRtool"))
```

---

### 10. `is_running_as_admin` 中 Token 句柄泄漏

**位置**: [app.rs#L777-L793](/e:/Code/IRtool-next/crates/irtool-egui/src/app.rs#L777-L793)
**来源**: 正确性审查

**问题**: `OpenProcessToken` 返回的 `token` HANDLE 从未通过 `CloseHandle` 关闭。每次调用泄漏一个内核句柄。虽然只在启动时调用一次，但属于资源管理缺陷。

**修复**:

```rust
unsafe {
    let mut token: HANDLE = HANDLE::default();
    if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
        return false;
    }
    let mut elevation = TOKEN_ELEVATION::default();
    let mut size = 0u32;
    let result = GetTokenInformation(
        token, TokenElevation,
        Some(&mut elevation as *mut _ as *mut _),
        std::mem::size_of::<TOKEN_ELEVATION>() as u32,
        &mut size,
    ).is_ok() && elevation.TokenIsElevated != 0;
    let _ = windows::Win32::Foundation::CloseHandle(token);
    result
}
```

---

### 11. `set_event_handler` 设置进程级全局静态回调

**位置**: [app.rs#L573-L588](/e:/Code/IRtool-next/crates/irtool-egui/src/app.rs#L573-L588)
**来源**: 影响审查

**问题**: `TrayIconEvent::set_event_handler` 和 `MenuEvent::set_event_handler` 是进程级全局静态函数。任何后续调用会静默替换之前的处理器，导致托盘"显示窗口"和"退出"功能完全失效。`tray_handler_set` 标志仅防止同一实例重复设置，无法阻止外部代码覆盖。

**修复**: 在代码注释中明确记录此全局约束。如 crate 提供 `try_set_event_handler` 则优先使用。长期可考虑在 handler 中添加防御性校验。

---

## Suggestions (CONSIDER)

### 12. DESIGN.md 功能对等矩阵未更新

**位置**: [DESIGN.md#L627-L641](/e:/Code/IRtool-next/crates/irtool-egui/DESIGN.md#L627-L641)
**来源**: 完整性审查

**问题**: 矩阵仍将 Monitor/Database/Workspace/Settings 四个页面标记为 `❌ TODO`，但实际上这四个页面均已完整实现。建议更新为实际状态以维持文档的跟踪意义。

---

### 13. CJK 字体仅尝试单一路径，无回退

**位置**: [theme.rs#L113](/e:/Code/IRtool-next/crates/irtool-egui/src/theme.rs#L113)
**来源**: 影响审查

**问题**: 仅尝试 `C:\Windows\Fonts\msyh.ttc`。在 N 版 Windows 或非中文语言系统上，中文将显示为方块。建议增加候选路径（`simsun.ttc`、`msjh.ttc`、`msgothic.ttc`）依次尝试。

---

### 14. Database 页面每帧完整克隆 events 向量

**位置**: [database.rs#L441](/e:/Code/IRtool-next/crates/irtool-egui/src/pages/database.rs#L441)
**来源**: 正确性审查

**问题**: `let items = self.events.clone()` 每帧执行。`DbEvent` 含 ~17 个 `String` 字段，1000 条事件 × 10 FPS = 每秒 170,000 次字符串克隆。建议改为先提取 `sel_id`，再借用 `&self.events` 渲染。

---

### 15. Network 排序中 Proto/Family 每次比较分配字符串

**位置**: [network.rs#L1007-L1008](/e:/Code/IRtool-next/crates/irtool-egui/src/pages/network.rs#L1007-L1008)
**来源**: 正确性审查

**问题**: `format!("{:?}", a.proto)` 在每次比较时分配新字符串。建议改为静态字符串映射：

```rust
let proto_key = |p: &Proto| -> &'static str {
    match p { Proto::Tcp => "tcp", Proto::Udp => "udp" }
};
```

---

### 16. `init_logger` 通过 `mem::forget` 泄漏 guard

**位置**: [lib.rs#L141](/e:/Code/IRtool-next/crates/irtool-egui/src/lib.rs#L141)
**来源**: 影响审查

**问题**: 泄漏 `WorkerGuard` 意味着进程退出时 non_blocking 缓冲区不会 flush。`on_exit` 中记录的最后几条日志可能丢失。建议将 guard 存入 `IrtoolApp` 字段，让其在退出时自然 drop。

---

### 17. Sidebar 折叠功能（48px）未实现

**位置**: [DESIGN.md#L77](/e:/Code/IRtool-next/crates/irtool-egui/DESIGN.md#L77)
**来源**: 完整性审查

**问题**: DESIGN.md 提到 `Sidebar 宽度 | 180px (折叠: 48px)`，当前仅有固定宽度，无折叠功能。作为 fallback UI 的后续增强项。

---

## 变更摘要

- **共发现 17 项问题**：4 项 Critical、7 项 Warning、6 项 Suggestion
- **安全风险**: 剪贴板 PowerShell 命令注入（#1）和 `mask_url` panic（#2）为最高优先级
- **功能缺陷**: 选中缓存 PID-only 匹配（#3）导致详情面板显示错误连接，右键菜单闪烁（#5）导致功能不可用
- **分层违规**: sysmon.rs 直接依赖 irtool_pcap（#6）是唯一的分层遗留问题，修复简单
- **设计规范偏差**: Heartbeat 间隔（#7）、Sidebar 宽度（#8）与 DESIGN.md 不一致，调整量小
