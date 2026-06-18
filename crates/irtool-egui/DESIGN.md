# IRtool egui Design System

## 1. Design Philosophy

- **即时模式 (IMGUI)**: state → render → interaction → state，每帧重新渲染
- **零延迟原则**: UI 操作不阻塞主线程，所有 Service 调用通过 `rt.spawn()` 异步化
- **数据驱动**: EventBus 事件驱动 UI 状态更新，`update()` 中 `drain()` 处理事件

---

## 2. Color System

### 2.1 Light Theme (默认)

| 用途 | 颜色 | 值 |
|------|------|-----|
| **背景 - 主** | 白色 | `#ffffff` |
| **背景 - 次** | 浅灰 | `#f5f5f5` |
| **背景 - 浮层** | 灰 | `#eaeaea` |
| **前景 - 主** | 深蓝黑 | `#1a1a2e` |
| **前景 - 次** | 深灰 | `#555555` |
| **前景 - 弱** | 浅灰 | `#999999` |
| **强调色 - 主** | 蓝 | `#2563eb` |
| **强调色 - hover** | 深蓝 | `#1d4ed8` |

### 2.2 语义色

| 语义 | 颜色 | 值 | 用途 |
|------|------|-----|------|
| **success** | 绿 | `#16a34a` | ESTABLISHED, 操作成功 |
| **info** | 蓝 | `#2563eb` | LISTEN, 提示信息 |
| **warning** | 黄 | `#ca8a04` | TIME_WAIT, 警告 |
| **danger** | 红 | `#dc2626` | CLOSE_WAIT, 错误, Kill |
| **default** | 灰 | `#6b7280` | 无特殊语义的状态 |

---

## 3. Typography

| 用途 | 字体 | 大小 |
|------|------|------|
| 标题 | Default (proportional) | 20px |
| 正文 | Default | 14px |
| 注释 | Default | 12px |
| 代码/数据 | Monospace | 13px |

**Monospace 场景**: PID、时间戳、IP 地址、端口、文件路径、命令行

---

## 4. Layout System

### 4.1 App Shell

```
+----------------------------------------------------+
| [IRtool v2.0.1]  [Admin]  [Polling: ON]            |  TopBar 32px
+----------+-----------------------------------------+
| Network  |                                          |
| Autoruns |         Content Area                     |
| Sysmon   |         (当前页面)                        |
| Monitor  |                                          |
| Database |                     +--------------------+
| Workspace|                     | Detail Panel 300px |
| Settings |                     | (选中行时展开)       |
+----------+---------------------+--------------------+
| Sidebar  |         Status Bar (可选)                 |
| (180px)  |                                          |
+----------+-----------------------------------------+
```

### 4.2 尺寸规范

| 元素 | 尺寸 |
|------|------|
| TopBar 高度 | 32px |
| Sidebar 宽度 | 180px (折叠: 48px) |
| Detail Panel 宽度 | 300px |
| Panel padding | 12px |
| Element gap | 8px |
| Table row height | 22px |
| Table header height | 24px |

---

## 4.3 Label Convention

**所有 `ui.label()` 必须添加 `.selectable(false)`**，理由：
- egui 的 Label 默认 `selectable_labels = true`，点击后进入文本选择模式（I 形光标），但 egui 文本模式下无法复制文本，纯属添麻烦
- 表格内的文本已通过 `cell_text()` 辅助函数统一使用 `.selectable(false)`
- 详情面板、工具栏、TopBar 也不应进入文本模式

```rust
// ❌ 错误 — 点击后进入无用编辑模式
ui.label("some text");

// ✅ 正确
ui.label(egui::RichText::new("some text").selectable(false));
// 或使用辅助函数
fn ui_label(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
    ui.add(egui::Label::new(text).selectable(false))
}
```

---

## 4.4 CopyButton Convention

**详情面板中的复制按钮**:
- 使用 `copy_button()` 辅助函数，渲染为无边框的 📋 图标，极小尺寸 (18x16)
- 短字段：使用 `detail_row_with_copy()` 在值末尾 inline 放置复制按钮
- 长字段（路径、命令行）：复制按钮放在标题行，与标题文字同一水平线

```rust
// 短字段 — inline 复制按钮
detail_row_with_copy(ui, "本地地址", &value, &copy_value);

// 长字段 — 复制按钮在标题行
ui.horizontal(|ui| {
    ui_label(ui, "路径".strong());
    if copy_button(ui, path) {
        ui.ctx().copy_text(path);
    }
});
ui_label(ui, egui::RichText::new(path).monospace().size(11.0));
```

### 5.1 Navigation

**SidebarItem**:
- icon + label 横排
- 选中: 强调色背景 + 白色文字
- hover: 浅色背景高亮
- 高度: 32px, padding: 8px

### 5.2 Data Display

**DataTable** (`egui_extras::TableBuilder`):
- 包裹在 `ScrollArea::both()` 中，支持水平和垂直滚动
- 所有列使用 `Column::exact()` 固定宽度，确保水平滚动
- 排序: 点击表头切换 ▲/▼
- 过滤: 搜索框 + 协议按钮组 + State 多选下拉
- 行点击: `Sense::click()` + `response.union()` 合并所有列响应
- 行选择: 单击行选中/取消, 选中行高亮背景
- 右键菜单: `secondary_clicked()` 弹出 `Area` 上下文菜单
- **不使用条纹行背景**: 所有行统一白色背景，仅选中行高亮
- Resizable: 列宽可拖拽

**Badge** (状态标签):
- 圆角矩形背景 + 文字
- 颜色跟随语义色 (success/info/warning/danger/default)
- 文字大写, 小号字体

**MonospaceText**:
- PID / 时间戳 / 路径 / 命令行
- 长文本截断 + tooltip

### 5.3 Inputs

**SearchBox**:
- 带清除按钮 (×) 的文本输入
- 实时过滤, 无 debounce (IMGUI 天然适配)

**FilterButtonGroup**:
- 按钮组: All / TCP / UDP
- 选中态: 强调色背景

**MultiSelectDropdown** (多选下拉):
- 按钮显示当前选中数量 (e.g. "State: All" / "State: 3")
- 点击按钮展开 `Area` 弹出层
- 包含 "Select All" / "Deselect All" 快捷按钮
- 每项为 Checkbox，可单独勾选/取消
- 点击弹出层外部自动关闭
- 使用 `egui::Order::Foreground` 确保在最上层

**ToggleButton**:
- 暂停 ⏸ / 恢复 ▶
- 切换按钮文字和颜色

### 5.4 Feedback

**ErrorBanner**:
- 页面顶部红色横幅
- 显示 `last_error` 内容
- 可关闭

**EmptyState**:
- 无数据时居中显示提示文字

---

## 6. Table Conventions

### 6.1 排序
- 点击表头排序，再次点击切换升/降序
- 排序箭头: ▲ (升序) / ▼ (降序), 显示在当前排序列
- 默认排序: `first_seen` DESC

### 6.2 过滤
- 搜索框: 匹配进程名/PID/IP/路径 (不区分大小写)
- 协议过滤: All / TCP / UDP 按钮组
- State 多选: 下拉菜单，支持任意组合，"None" 状态始终显示

### 6.3 行交互
- 单击行: 选中 + 展开 Detail Panel
- 再次单击: 取消选中 + 收起 Detail Panel
- 选中行: 强调色半透明背景 (`row.set_selected(true)`)
- 右键行: 弹出上下文菜单 (Copy Details / Copy IP:Port / Refresh Cmdline / Kill Process)
- **历史连接整行灰色**: 所有列的文本颜色统一使用 `FG_TERTIARY`，包括状态徽章也降级为纯文本灰色
- 点击暂停/恢复标签可切换轮询状态 (TopBar + ToolBar 双入口)

### 6.4 右键菜单 (ContextMenu)
- 使用 `egui::Area::new(id).order(Foreground).fixed_pos(ctx_menu_pos)` 实现
- **关键**: 菜单位置必须在右键点击时捕获并存入 state（`ctx_menu_pos: Option<Pos2>`），而不是每帧用 `interact_pos()` 读取当前鼠标位置，否则菜单会跟随光标移动
- 点击菜单项后自动关闭
- 点击菜单外部关闭
- 关闭时同时清空 `ctx_menu_pos`
- 典型菜单项: Copy Details / Copy IP:Port / Refresh Cmdline / Kill Process

---

## 7. State Management

### 7.1 数据流
```
EventBus (tokio broadcast)
    ↓
EventBridge (std::sync::mpsc)
    ↓
egui update() → drain()
    ↓
handle_event() → mutate PageState
    ↓
render()
```

### 7.2 异步操作
```
UI action (button click)
    ↓
rt.spawn(async { Service::method() })
    ↓
Service → EventBus::publish()
    ↓
handle_event() → update PageState
```

### 7.3 状态分层

| 层级 | 位置 | 内容 |
|------|------|------|
| **GlobalState** | `AppContext` (Arc) | 共享数据: collectors, stores, engines |
| **AppState** | `IrtoolApp` struct | 导航状态, 主题, sidebar |
| **PageState** | struct 字段 | 过滤/排序/选择/loading/error |
| **TransientState** | 局部变量 | 临时展开/折叠 |

---

## 8. Error & Loading Patterns

### 8.1 错误处理
- Service 调用失败 → `last_error = Some(msg)` → 渲染 ErrorBanner
- 网络断开 → 保留最后快照, 显示 banner
- 非关键错误 → tracing::warn, 不影响 UI

### 8.2 加载状态
- 初始加载: 无数据显示 EmptyState
- 后台操作: 按钮 disabled + Spinner
- 持续轮询: 无需 loading (直接显示最新数据)

---

## 9. Page Specifications

### 9.1 Network (POC)

**数据**: `NetworkSnapshotPayload { items: Vec<NetConn>, timestamp }`

**工具栏**: 搜索框 | [All] [TCP] [UDP] | ⏸/▶ | 🗑️ Clear

**表格列** (11列):

| 列 | 字段 | 宽度 | 字体 | 特殊 |
|---|---|---|---|---|
| First Seen | first_seen | 160 | Mono | fmtTime() |
| PID | pid | 55 | Mono | |
| Process | process_name | 160 | Default | |
| Local | local.addr:port | 200 | Mono | fmtAddr/fmtPort |
| Remote | remote.addr:port | 200 | Mono | fmtAddr/fmtPort |
| State | state | 110 | Default | 语义色 Badge |
| Proto | proto | 60 | Default | uppercase |
| Fam | family | 50 | Default | uppercase |
| Path | process_path | 280 | Mono | 截断+tooltip |
| Cmdline | process_cmdline | 200 | Mono | status icon+tooltip |
| Last Seen | last_seen | 160 | Mono | fmtTime() |

**State Badge 颜色**:
- ESTABLISHED → success (绿)
- LISTEN → info (蓝)
- TIME_WAIT → warning (黄)
- CLOSE_WAIT → danger (红)
- 其他 → default (灰)

**Detail Panel** (选中行时右侧展开):
- 完整连接信息
- 每个字段右侧有独立的小复制按钮 (📋)，短字段 inline 显示，长字段（路径/命令行）复制按钮在标题行
- 路径和命令行以等宽小字体显示
- 历史连接在标题行右侧显示 `Badge::Warning("历史")`
- Kill Process 按钮 (danger 色)
- Refresh Cmdline 按钮

### 9.2 ~ 9.7 其他页面 (未来)
- 均留 TODO 占位, 显示 "Coming Soon"

---

## 10. Performance Patterns

egui 的 IMGUI 模式容易写出"每帧全量计算"的代码，数据量一大就卡。以下规范从 POC 阶段就要遵守，避免后续每个页面重复同样的性能问题。

### 10.1 Filtered + Sorted 结果缓存

**问题**：`get_filtered_sorted_items()` 每帧执行 `filter().cloned().collect()` + `sort_by`，数百连接 × 10 FPS = 每秒数千次克隆+排序。

**规范**：所有页面用 dirty flag 缓存计算结果。

```rust
pub struct NetworkPageState {
    // 原始数据
    snapshot: Option<NetworkSnapshotPayload>,

    // 缓存
    cached_items: Vec<NetConn>,  // filtered + sorted 结果
    cache_dirty: bool,           // snapshot 更新或 filter/sort 变化时置 true
}

impl NetworkPageState {
    fn get_filtered_sorted_items(&mut self) -> &[NetConn] {
        if self.cache_dirty {
            self.cached_items = self.compute_filtered_sorted();
            self.cache_dirty = false;
        }
        &self.cached_items
    }

    fn handle_snapshot(&mut self, payload: NetworkSnapshotPayload) {
        self.snapshot = Some(payload);
        self.cache_dirty = true;  // 标记需重算
    }

    // 任何 filter/sort 变化的地方都要置 cache_dirty = true
}
```

**触发 dirty 的场景**：snapshot 更新、搜索框变化、协议过滤变化、状态过滤变化、排序列变化、历史开关变化。

### 10.2 选中行数据缓存

**问题**：`find_selected_conn()` 每帧被详情面板、右键菜单多次调用，每次 O(n) 扫描 + clone。

**规范**：选中时缓存 `NetConn`，snapshot 更新时刷新缓存。

```rust
pub struct NetworkPageState {
    selected_pid: Option<u32>,
    selected_conn: Option<NetConn>,  // 缓存，避免每帧扫描
}

// 选中行时：
self.selected_pid = Some(pid);
self.selected_conn = Some(conn.clone());

// snapshot 更新时：
if let Some(ref pid) = self.selected_pid {
    self.selected_conn = self.snapshot.as_ref()
        .and_then(|s| s.items.iter().find(|c| c.pid == *pid).cloned());
}
```

### 10.3 事件驱动重绘（替代固定重绘）

**问题**：`ctx.request_repaint_after(Duration::from_millis(100))` 无论有无事件都每 100ms 重绘。

**规范**：EventBridge 持有 `egui::Context`，事件到达时主动触发重绘；保留低频 heartbeat 作为兜底。

```rust
// crates/irtool-egui/src/event_bridge.rs
pub struct EventBridge {
    rx: std::sync::mpsc::Receiver<AppEvent>,
}

impl EventBridge {
    pub fn new(ctx: &AppContext, rt: &tokio::runtime::Handle, egui_ctx: egui::Context) -> Self {
        let mut bus_rx = ctx.event_bus.subscribe();
        let (tx, rx) = std::sync::mpsc::channel();
        rt.spawn(async move {
            loop {
                match bus_rx.recv().await {
                    Ok(event) => {
                        if tx.send(event).is_err() { break; }
                        egui_ctx.request_repaint();  // 事件到达，触发重绘
                    }
                    Err(Lagged(n)) => tracing::warn!("egui event bridge lagged: {}", n),
                    Err(Closed) => break,
                }
            }
        });
        Self { rx }
    }
}
```

```rust
// crates/irtool-egui/src/app.rs update()
// 改为低频 heartbeat 兜底（1秒），事件到达时会立即重绘
ctx.request_repaint_after(Duration::from_millis(1000));
```

**注意**：`egui::Context` 需要在第一帧 `update()` 中获取（`creation_context.egui_ctx` 或首次 update 的 `ctx`），因此 `EventBridge::new` 要延迟到首次 update 调用，或在 `eframe::AppCreator` 闭包中创建。

---

## 11. Fallback Mode

### 11.1 降级模式标识

当 egui 作为 WebView2 缺失的 fallback 启动时，TopBar 显示黄色 "降级模式" 徽章。

```rust
// app.rs
pub struct IrtoolApp {
    pub is_fallback: bool,  // 从 IRTOOL_FALLBACK 环境变量读取
}

// layout.rs render_topbar()
if self.is_fallback {
    badge::badge(ui, "降级模式", BadgeVariant::Warning);
}
```

### 11.2 功能对等矩阵

跟踪每个页面在 Tauri / egui 下的实现状态，避免遗漏。egui 作为 fallback，核心功能必须对等，非核心功能可缺失。

| 页面 | Tauri | egui | 说明 |
|------|-------|------|------|
| Network | ✅ | ✅ | 已实现 |
| Autoruns | ✅ | ❌ TODO | 核心功能，必须实现 |
| Sysmon | ✅ | ❌ TODO | 核心功能，必须实现 |
| Monitor | ✅ | ❌ TODO | 后台监控，egui 下可简化 |
| Database | ✅ | ❌ TODO | 数据库检索，egui 下可简化 |
| Workspace | ✅ | ❌ TODO | 工作台，egui 下可简化 |
| Settings | ✅ | ❌ TODO | 设置，egui 下可简化 |

**优先级**：Network → Autoruns → Sysmon → Monitor → 其他。前三个是 IR 核心功能，fallback 必须支持。

### 11.3 降级模式下的功能简化原则

egui fallback 不追求与 Tauri 版 100% 功能对等，遵循"核心功能可用即可"：

**功能对等目标**：Tauri 100%，egui 60-80%

**不做**（避免维护两套前端的痛苦）：
- 动画、复杂图表、拖拽布局
- 高级仪表盘、可视化
- 所有 Tauri 版有的次要功能

**必须做**（IR 核心功能）：
- 表格 + 详情面板 + 工具栏
- Network / Autoruns / Sysmon 三个核心页面
- 设置页只保留核心项（日志级别、数据保留策略）

**原则**：egui 是应急 fallback，不是第二套完整 UI。半年后维护两套 100% 对等的前端会非常痛苦。
