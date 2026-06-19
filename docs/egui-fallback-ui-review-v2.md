# egui Fallback UI 审查报告 v2

> 审查基准：`docs/fallback机制与分层优化设计方案.md`
> 审查范围：`crates/irtool-egui/` 全部源码 + `crates/irtool-tauri/src/main.rs` fallback 入口
> 审查日期：2026-06-19
> 前置报告：`docs/egui-fallback-ui-review.md`（v1，17 项问题）

---

## 一、概述

本次审查在 v1 报告基础上进行，主要目标：

1. 确认 v1 报告中 17 项问题的修复状态
2. 识别 fallback UI 主要功能完成后新引入的问题、bug 与改进建议
3. 不修改代码，仅输出审查结论

整体结论：**fallback 机制主干（WebView2 检测 → StartupMode 传递 → egui 启动 → 单实例 → 托盘 → 各页面渲染）已基本可用**，v1 报告中大部分问题已修复。但本次审查发现 **2 项 Critical 问题**（单实例机制实际失效、workspace 事件数据永远为空）会直接影响功能正确性，以及若干 Warning/Suggestion 级问题需后续处理。

---

## 二、v1 报告问题修复确认

| # | 问题 | 级别 | 状态 | 证据 |
|---|------|------|------|------|
| 1 | 剪贴板命令注入（PowerShell） | Critical | ✅ 已修复 | `autoruns.rs` 改用 `arboard` 库 |
| 2 | `mask_url` panic（非字符边界切片） | Critical | ✅ 已修复 | `settings.rs` 改用 `char_indices` |
| 3 | 网络选中 PID-only 匹配 | Critical | ✅ 已修复 | `network.rs` 使用 `(pid, local, remote)` 三元组 |
| 4 | `tracing::warn!` 在 fallback 早期丢失 | Warning | ✅ 已修复 | `main.rs` 改用 `eprintln!` |
| 5 | 右键菜单闪烁 | Warning | ✅ 已修复 | `autoruns.rs` 添加 `ctx_menu_just_opened` 标志 |
| 6 | `sysmon.rs` 分层违规（直接依赖 pcap 类型） | Warning | ✅ 已修复 | 改用 `irtool_service::types::PcapConfig` |
| 7 | heartbeat 500ms 过频 | Suggestion | ✅ 已修复 | `app.rs` 改为 1000ms |
| 8 | sidebar 140px 过窄 | Suggestion | ✅ 已修复 | `theme.rs` `SIDEBAR_WIDTH = 180.0` |
| 9 | `FindWindowW` 误匹配任意窗口 | Critical | ✅ 已修复 | 使用 `winit-window-class-name` 精确匹配 |
| 10 | Token 句柄泄漏 | Warning | ✅ 已修复 | `app.rs` 添加 `CloseHandle` |
| 11 | CJK 字体单路径 | Suggestion | ✅ 已修复 | `theme.rs` 多候选（msyh/msyhbd/simsun/msjh/msgothic） |
| 12 | database 每帧 clone 全量 events | Warning | ✅ 已修复 | `database.rs` 改为借用 `&self.events` |
| 13 | proto 字符串每帧分配 | Suggestion | ✅ 已修复 | `network.rs` 使用静态字符串 |
| 14 | `init_logger` `mem::forget` guard | Warning | ✅ 已修复 | `lib.rs` 通过 `IrtoolApp` 字段持有 `log_guard` |
| 15 | `set_event_handler` 全局静态 | Suggestion | ✅ 已修复 | `event_bridge.rs` 持有 `egui::Context` 事件驱动重绘 |
| 16 | DESIGN.md 矩阵未更新 | Suggestion | ⚠️ 未确认 | 本次未检查 DESIGN.md 矩阵更新状态 |
| 17 | sidebar 折叠未实现 | Suggestion | ⚠️ 未确认 | 本次未检查 sidebar 折叠实现 |

**修复率：14/17 确认修复，2 项未确认，0 项未修复。** 修复质量良好。

---

## 三、新发现问题

### Critical

#### C1. 单实例互斥锁 guard 未持有，机制实际失效

- **位置**：[lib.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/lib.rs#L38-L40)
- **现象**：
  ```rust
  // Check for existing instance
  if single_instance::check_and_acquire().is_none() {
      std::process::exit(1);
  }
  ```
- **问题**：`check_and_acquire()` 返回的 guard（持有 Windows 互斥锁句柄）未被保存到 `IrtoolApp` 或任何长期持有的结构中。`if` 表达式结束后，guard 立即被 drop，互斥锁释放。这意味着：
  - 单实例检测只在启动瞬间生效
  - 启动完成后，第二个实例可以正常启动并获取互斥锁
  - 单实例机制实际完全失效
- **对比**：同文件中 `log_guard` 已正确通过 `IrtoolApp` 字段持有（v1 #14 修复），但 `single_instance guard` 未采用相同模式，存在不一致。
- **修复建议**：
  ```rust
  let single_instance_guard = match single_instance::check_and_acquire() {
      Some(g) => g,
      None => std::process::exit(1),
  };
  // ... 后续将 guard 传入 IrtoolApp::new(...) 作为字段持有
  ```
  在 `IrtoolApp` 结构体中添加 `single_instance_guard: Option<single_instance::SingleInstanceGuard>` 字段。

---

#### C2. workspace `event_items` 永远为空，Events tab 与事件规则扫描失效

- **位置**：[workspace.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/workspace.rs#L533-L578)
- **现象**：`trigger_refresh` 只拉取 `autorun_items` 和 `network_items`：
  ```rust
  pub fn trigger_refresh(&self, ctx: &AppContext, rt: &tokio::runtime::Handle) {
      // Fetch autoruns
      rt.spawn(async move { ... AutorunsService.get_result() ... });
      // Fetch network snapshot
      rt.spawn(async move { ... NetworkService.snapshot() ... });
      // ❌ 缺少：Fetch sysmon events
  }
  ```
- **问题**：
  - `WorkspacePageState.event_items` 初始为空，`apply_refresh` 中也无 `event_items` 字段更新逻辑
  - Events tab 永远显示空状态提示"无事件数据，请在 Sysmon 页面开始采集"
  - `do_rule_scan` 中的 `scan_events(&self.event_items, ...)` 永远返回空 HashMap
  - 默认规则 `default-malicious-ip-event`（Event target）永远无法命中
  - `WorkspaceRefresh` 结构体缺少 `event_items: Option<Vec<SysmonEvent>>` 字段
- **影响**：workspace 页面的"事件"功能完全不可用，用户无法在此页面查看 sysmon 事件或对事件应用规则。
- **修复建议**：
  1. 在 `WorkspaceRefresh` 添加 `event_items: Option<Vec<SysmonEvent>>` 字段
  2. 在 `trigger_refresh` 添加第三个 spawn 拉取 sysmon events（调用 `SysmonService` 或直接从 `EventBus` 订阅）
  3. 在 `apply_refresh` 添加 `event_items` 更新逻辑

---

### Warning

#### W1. `AppEvent::CloseRequested` 处理为空 TODO

- **位置**：[app.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/app.rs#L271-L273)
- **现象**：
  ```rust
  AppEvent::CloseRequested => {
      // TODO: handle close
  }
  ```
- **问题**：窗口关闭事件未处理。结合项目内存中"Application must reset to non-background mode on forced exit to prevent inconsistent state on restart"约束，此处应至少重置后台模式标志。当前实现可能导致：
  - 强制退出时后台模式状态未清理
  - 退出流程不完整
- **修复建议**：实现关闭处理逻辑，至少包含：重置后台模式、刷新配置、通知服务停止。

---

#### W2. workspace 规则导入按钮未实现

- **位置**：[workspace.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/workspace.rs#L1713-L1715)
- **现象**：
  ```rust
  if ui.button("导入").clicked() {
      // TODO: import rules from JSON file
  }
  ```
- **问题**：规则管理对话框中"导入"按钮点击无任何效果，但 UI 上无禁用提示。用户会误以为功能故障。
- **修复建议**：要么实现导入逻辑（参考 `settings.rs` 的 `import_rules`），要么将按钮设为 `add_enabled(false, ...)` 并添加 tooltip 说明"未实现"。

---

#### W3. workspace Regex 条件退化为子串匹配，与 UI 标签不符

- **位置**：[workspace.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/workspace.rs#L210-L220)
- **现象**：
  ```rust
  ConditionType::Regex => {
      // Simplified: case-insensitive substring match.
      // Full regex support requires adding `regex` to Cargo.toml.
      s.to_lowercase().contains(&pattern.to_lowercase())
  }
  ```
- **问题**：
  - 规则编辑对话框中条件类型可选"正则"（[workspace.rs:1948](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/workspace.rs#L1948)）
  - 但实际执行的是大小写不敏感子串匹配，与"正则"语义完全不同
  - 用户编写 `^C:\\Windows\\.*\.exe$` 等正则表达式将无法按预期工作
  - 注释承认需要 `regex` crate 但未添加
- **影响**：功能语义不一致，用户预期与实际行为偏差，可能导致漏报或误报。
- **修复建议**：
  - 方案 A：在 `irtool-egui/Cargo.toml` 添加 `regex` 依赖，实现真正的正则匹配
  - 方案 B：移除 UI 中的"正则"选项，只保留"包含"和"等于"，避免误导
  - 推荐方案 A，因为 `irtool-service` 或 `irtool-core` 可能已依赖 `regex`，可复用

---

#### W4. monitor `cmdline_enrich` 字段未持久化到 config

- **位置**：[monitor.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/monitor.rs#L46)
- **现象**：`MonitorPageState.cmdline_enrich: u32` 字段在 state 中，UI 可切换（[monitor.rs:526-533](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/monitor.rs#L526-L533)），但未在 `trigger_config_load` / `update_config` 流程中读写。
- **问题**：用户切换"命令行富化"设置后，重启应用会丢失。这与项目内存中"所有前端状态同步必须使用 refetchOnMount: false"的精神一致——状态应在 config 中持久化。
- **修复建议**：将 `cmdline_enrich` 映射到 `MonitorConfig` 字段，在加载/保存 config 时同步。

---

#### W5. monitor `trigger_poll` 每 3 秒创建 4 个独立 spawn

- **位置**：[monitor.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/monitor.rs#L112-L180)
- **现象**：`trigger_poll` 内有 4 个 `rt.spawn(...)` 分别拉取 telemetry / background / event_count / db_size。
- **问题**：
  - 每 3 秒产生 4 个独立 tokio 任务，任务调度开销累积
  - 4 个任务各自 clone `ctx` 和 `tx`，无共享
  - 若某个任务卡顿，其他任务无法感知，可能导致 refresh 通道积压
- **修复建议**：合并为单个 spawn，内部并发执行 4 个查询（使用 `tokio::join!`），单次 send 聚合结果：
  ```rust
  rt.spawn(async move {
      let svc = MonitorService { ctx: &ctx_clone };
      let (t, b, e, d) = tokio::join!(
          svc.get_telemetry(),
          svc.is_background(),
          svc.get_event_count(),
          svc.get_db_size(),
      );
      // 聚合后单次 send
  });
  ```

---

#### W6. sysmon `trim_events` 使用 `drain(0..excess)` 在 Vec 头部删除 O(n)

- **位置**：[sysmon.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/sysmon.rs#L120-L125)
- **现象**：
  ```rust
  fn trim_events(&mut self) {
      if self.events.len() > MAX_EVENTS {
          let excess = self.events.len() - MAX_EVENTS;
          self.events.drain(0..excess);
      }
  }
  ```
- **问题**：`MAX_EVENTS = 10000`，当事件累积超过上限时，`drain(0..excess)` 在 Vec 头部删除，需要将后续所有元素前移，O(n) 复杂度。在高频事件场景（pcap 抓包）下，每次 `handle_pcap_event` 都可能触发 trim，导致性能抖动。
- **修复建议**：
  - 方案 A：改用 `VecDeque`，头部弹出 O(1)
  - 方案 B：批量删除时使用 `events.drain(..excess)`（与现有等价）但降低 `MAX_EVENTS` 或增加 trim 频率控制
  - 方案 C：环形缓冲区
  - 推荐方案 A，改动最小

---

#### W7. network `find_selected_conn` 每帧多次 `format!` 字符串比较

- **位置**：[network.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/network.rs#L141-L142) 及 [network.rs:173-174](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/network.rs#L173-L174)、[network.rs:557-560](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/network.rs#L557-L560)
- **现象**：
  ```rust
  c.pid == pid
      && format!("{}:{}", c.local.addr, c.local.port) == *local
      && format!("{}:{}", c.remote.addr, c.remote.port) == *remote
  ```
- **问题**：每帧对每个连接执行 2 次 `format!`（分配 String）用于比较，在连接数多时（如几百条）会产生显著堆分配压力。
- **修复建议**：直接比较字段，避免 `format!`：
  ```rust
  c.pid == pid
      && c.local.addr == local_addr_str
      && c.local.port == local_port
      && c.remote.addr == remote_addr_str
      && c.remote.port == remote_port
  ```
  或将 `selected_local`/`selected_remote` 拆分为 `(addr, port)` 元组存储。

---

#### W8. autoruns `paint_row_bg` 空实现，行背景着色功能失效

- **位置**：[autoruns.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/autoruns.rs#L1195-L1198)
- **现象**：
  ```rust
  fn paint_row_bg(ui: &mut egui::Ui, color: egui::Color32, alpha: f32) {
      let _ = (color, alpha);
      // Row tinting handled via row.set_selected; this is a placeholder for
      // custom background which egui_extras doesn't expose easily.
  }
  ```
- **问题**：
  - 调用点（[autoruns.rs:474](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/autoruns.rs#L474)、[autoruns.rs:479](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/autoruns.rs#L479)、[autoruns.rs:484](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/autoruns.rs#L484)）期望对 `file_missing`/`unsigned`/`disabled` 行着色提示
  - 但函数体为空，UI 上看不到任何视觉区分
  - 注释说"handled via row.set_selected"，但 `set_selected` 只影响选中态，无法表达三种不同状态
- **影响**：用户无法通过行背景快速识别异常项，降低可用性。
- **修复建议**：
  - 方案 A：在 `row.col(...)` 内部对每个 cell 的 `ui` 设置背景色（通过 `ui.painter().rect_filled`）
  - 方案 B：使用 egui_extras 的 `row.set_selected` 配合自定义选中色（但无法区分三种状态）
  - 方案 C：在行首添加状态图标 badge 替代背景色
  - 推荐方案 A 或 C

---

#### W9. settings import/export 使用 `std::env::current_dir()` 而非 app_dirs

- **位置**：[settings.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/settings.rs#L873)、[settings.rs:901](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/settings.rs#L901)、[settings.rs:945](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/settings.rs#L945)、[settings.rs:972](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/settings.rs#L972)
- **现象**：
  ```rust
  let path = std::env::current_dir()
      .unwrap_or_default()
      .join("irtool-config.json");
  ```
- **问题**：
  - 当应用通过托盘、开机自启或 fallback 路径启动时，`current_dir()` 可能为 `C:\Windows\System32`（服务启动）或不可写目录
  - 导入/导出文件会静默失败或写入意外位置
  - 与项目其他部分使用 `AppDirs` 的约定不一致
- **修复建议**：改用 `ctx.app_dirs.root()` 或 `ctx.app_dirs.config_dir()` 作为基目录。

---

#### W10. workspace `export_rules_json` 使用 `std::env::temp_dir()`

- **位置**：[workspace.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/workspace.rs#L2210)
- **现象**：
  ```rust
  let dir = std::env::temp_dir();
  let path = dir.join("irtool_rules_export.json");
  ```
- **问题**：
  - 与 W9 同类问题，应使用 app_dirs
  - 文件名固定为 `irtool_rules_export.json`，多次导出会覆盖，无时间戳
  - 用户无法知道导出位置（仅日志记录）
- **修复建议**：改用 `ctx.app_dirs.root().join("exports")`，文件名加时间戳。

---

#### W11. database `monitor_event_to_db_event` 每次解析 `raw_json` 无缓存

- **位置**：[database.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/database.rs#L879-L880)
- **现象**：
  ```rust
  fn monitor_event_to_db_event(me: &MonitorEvent) -> DbEvent {
      let raw: serde_json::Value = serde_json::from_str(&me.raw_json).unwrap_or_default();
      ...
  }
  ```
- **问题**：每次分页查询都对所有 item 重新解析 `raw_json`，无缓存。在 `search_event_page` 返回大量结果时，JSON 解析开销显著。
- **修复建议**：
  - 方案 A：在 `DbEvent` 构建时缓存解析后的字段（但 `DbEvent` 是值类型，需调整结构）
  - 方案 B：在 service 层缓存最近查询结果
  - 方案 C：接受现状，但在 `tracing` 中记录解析耗时，确认是否为瓶颈
  - 推荐先做方案 C 评估，再决定是否优化

---

### Suggestion

#### S1. `AppEvent::AutorunsSignatureProgress` / `AutorunsHashProgress` 未实现

- **位置**：[app.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/app.rs#L212-L217)
- **现象**：两个事件分支仅注释"not shown in toolbar; could be added later"。
- **建议**：若短期不实现，建议在 UI 上隐藏签名/哈希进度；若计划实现，可在 topbar 添加进度条。

---

#### S2. sysmon `PCAP_ID_BASE + pcap_seq` 溢出风险（理论）

- **位置**：[sysmon.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/sysmon.rs#L113)
- **现象**：`let id = PCAP_ID_BASE + self.pcap_seq;` 其中 `PCAP_ID_BASE = 1_000_000_000`，`pcap_seq: u64`。
- **建议**：虽然 u64 实际不会溢出（需 10^9 + 1.8×10^10 次事件才溢出），但可添加 `checked_add` 或注释说明"pcap_seq 在应用生命周期内不会接近 u64::MAX"。

---

#### S3. workspace `network_key` / `event_key` 每帧重复计算

- **位置**：[workspace.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/workspace.rs#L386-L406)
- **现象**：`render_network_table` 和 `render_events_table` 中对每个 item 调用 `network_key(item)` / `event_key(item)` 生成 String 用于过滤和选中比较，每帧重复。
- **建议**：在 `apply_refresh` 时预计算并缓存 `(key, item)` 映射，避免每帧分配。

---

#### S4. lib.rs `log_guard` 持有但 `single_instance guard` 未持有（一致性）

- **位置**：[lib.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/lib.rs#L30-L40)
- **现象**：`log_guard` 已通过 `IrtoolApp` 字段持有（v1 #14 修复），但 `single_instance guard` 未采用相同模式（见 C1）。
- **建议**：统一两者持有方式，体现代码一致性。

---

#### S5. workspace `export_csv` 使用 `SystemTime::UNIX_EPOCH` 而非 chrono

- **位置**：[workspace.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/workspace.rs#L2146-L2149)
- **现象**：
  ```rust
  let secs = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .map(|d| d.as_secs())
      .unwrap_or(0);
  ```
- **建议**：项目已依赖 `chrono`（`theme.rs` 的 `fmt_time` 使用），可直接用 `chrono::Local::now().format("%Y%m%d_%H%M%S")` 生成更友好的时间戳文件名。

---

#### S6. monitor 4 个独立 spawn 可合并（与 W5 关联）

- **位置**：[monitor.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/monitor.rs#L121-L180)
- **建议**：见 W5 修复建议，合并为单个 `tokio::join!` 任务。

---

#### S7. workspace 规则 `default_rules` 硬编码恶意 IP 列表

- **位置**：[workspace.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/workspace.rs#L70-L80)
- **现象**：10 个恶意 IP 硬编码在源码中。
- **建议**：考虑改为从外部文件（如 `rules/malicious_ips.txt`）加载，便于更新；或至少添加注释说明来源和更新日期。

---

#### S8. workspace `Rule` / `Condition` 等类型未持久化

- **位置**：[workspace.rs](file:///e:/Code/IRtool-next/crates/irtool-egui/src/pages/workspace.rs#L47-L65)
- **现象**：`Rule` 实现了 `Clone/Debug` 但未实现 `Serialize/Deserialize`，`default_rules` 每次启动重新生成。用户编辑的规则在重启后丢失。
- **建议**：为 `Rule` / `Condition` 派生 `serde::Serialize/Deserialize`，持久化到 `app_dirs.config_dir().join("workspace_rules.json")`。

---

## 四、问题汇总表

| # | 级别 | 位置 | 简述 |
|---|------|------|------|
| C1 | Critical | lib.rs:38-40 | single_instance guard 未持有，单实例机制失效 |
| C2 | Critical | workspace.rs:533-578 | event_items 永远为空，Events tab 与事件规则扫描失效 |
| W1 | Warning | app.rs:271-273 | CloseRequested 处理为空 TODO |
| W2 | Warning | workspace.rs:1713-1715 | 规则导入按钮未实现 |
| W3 | Warning | workspace.rs:210-220 | Regex 退化为子串匹配，与 UI 标签不符 |
| W4 | Warning | monitor.rs:46 | cmdline_enrich 未持久化到 config |
| W5 | Warning | monitor.rs:112-180 | trigger_poll 每 3 秒 4 个 spawn，任务泛滥 |
| W6 | Warning | sysmon.rs:120-125 | trim_events drain(0..excess) O(n) 头部删除 |
| W7 | Warning | network.rs:141-142 等 | find_selected_conn 每帧多次 format! |
| W8 | Warning | autoruns.rs:1195-1198 | paint_row_bg 空实现，行背景着色失效 |
| W9 | Warning | settings.rs:873 等 | import/export 使用 current_dir 而非 app_dirs |
| W10 | Warning | workspace.rs:2210 | export_rules_json 使用 temp_dir |
| W11 | Warning | database.rs:879-880 | monitor_event_to_db_event 每次解析 raw_json 无缓存 |
| S1 | Suggestion | app.rs:212-217 | Signature/HashProgress 未实现 |
| S2 | Suggestion | sysmon.rs:113 | PCAP_ID_BASE + pcap_seq 理论溢出 |
| S3 | Suggestion | workspace.rs:386-406 | network_key/event_key 每帧重复计算 |
| S4 | Suggestion | lib.rs:30-40 | log_guard 与 single_instance guard 持有不一致 |
| S5 | Suggestion | workspace.rs:2146-2149 | export_csv 用 SystemTime 而非 chrono |
| S6 | Suggestion | monitor.rs:121-180 | 4 个 spawn 可合并（关联 W5） |
| S7 | Suggestion | workspace.rs:70-80 | 恶意 IP 列表硬编码 |
| S8 | Suggestion | workspace.rs:47-65 | Rule 类型未持久化，用户编辑重启丢失 |

**统计：2 Critical / 11 Warning / 8 Suggestion**

---

## 五、优先级建议

### 立即修复（影响功能正确性）
1. **C1**：单实例机制失效——第二个实例可正常启动，违背单实例设计意图
2. **C2**：workspace Events tab 完全不可用——用户无法查看事件或应用事件规则

### 短期修复（影响用户体验或数据一致性）
3. **W3**：Regex 退化为子串——功能语义不符，可能导致漏报
4. **W8**：paint_row_bg 空实现——用户无法视觉识别异常项
5. **W9/W10**：import/export 路径问题——托盘启动时可能静默失败
6. **W1**：CloseRequested 未处理——退出状态可能不一致
7. **W2**：规则导入未实现——UI 误导

### 中期优化（性能与规范）
8. **W5/S6**：monitor spawn 合并
9. **W6**：sysmon trim_events 改 VecDeque
10. **W7**：network find_selected_conn 去除 format!
11. **W4**：cmdline_enrich 持久化
12. **W11**：database raw_json 解析缓存评估
13. **S8**：Rule 持久化（用户编辑重启丢失是较大体验问题）

### 长期改进
14. S1/S2/S3/S4/S5/S7

---

## 六、总体评价

fallback UI 主干功能完成度良好，v1 报告 17 项问题修复 14 项（修复率 82%），修复质量高。本次发现的 2 项 Critical 问题（C1/C2）属于"功能完成度高但关键路径未闭环"的典型情况：

- **C1** 是 v1 修复 `log_guard` 时遗漏了 `single_instance guard` 的对称处理
- **C2** 是 workspace 三 tab 架构中 Events tab 的数据流未接通

建议优先修复 C1/C2 后再进入下一轮功能开发。Warning 级问题中 W3（Regex 退化）和 W8（paint_row_bg 空实现）涉及功能语义与 UI 承诺不符，建议一并修复。其余 Warning/Suggestion 可按优先级逐步处理。

审查完毕，未修改任何代码。
