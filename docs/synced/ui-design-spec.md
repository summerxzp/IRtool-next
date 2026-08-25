# IRtool UI 设计规范

> 状态：v0.1 初版（结构定稿，具体值以 React 版为基准源，P2 期间校准回填）
> 适用：egui 重构全程（`egui-ui` 计划，见 [egui-ui-rework-plan.md](egui-ui-rework-plan.md)）
> 方法论来源：作者的 UI 设计方法论知识库（令牌三层架构 / 语义角色系统 / Pattern 目录 / 审计→原则→视觉探索→token→生产流程）
> 核心原则：**Design System 是设计过程的结果，不是起点** —— 本规范先固化"结构与约束"，具体值通过 demo 探索定稿后回填。

---

## 1. 令牌架构（三层）

| 层级 | egui 实现 | 用途 | 规则 |
|---|---|---|---|
| Raw | `Color32` 常量（私有） | 基础色值 | **禁止在页面代码中出现** |
| Semantic | `design/tokens.rs` 公开常量 | 业务含义色/字号/间距 | 页面只允许使用这一层 |
| Component | widget 内部引用 Semantic | 组件级派生（如 badge 的 fg+bg 组合） | 组件内部消化，不外泄 |

**基准源**：复刻阶段以 React 版 `ui/src/styles/tokens.css`（72 行，dark/light 双套）为唯一色值基准；demo（`ir-ui-demos/egui-demo/src/theme.rs`）当前色值与其存在偏差（如 accent `#2563EB` vs `#3A7AF0`），**P2 时以 tokens.css 为准对齐**，偏差处理在 demo 定稿流程中决策。（P2 已对齐完成，2026-08-23）

**egui 0.36 实现注意**：style 按主题分存（`ctx.set_style_of(Theme::Light/Dark, …)`），无单入口 `set_style`；接线时深浅两套都要写入。

## 2. 色彩语义

### 2.1 中性色阶梯（双主题，来源 tokens.css）

| 语义 | Light | Dark | 用途 |
|---|---|---|---|
| bg-base | #f7f8fa | #0b0d10 | 窗口底 |
| bg-elev-1 (surface) | #ffffff | #14171c | 卡片/面板/表格 |
| bg-elev-2 (muted) | #eef1f5 | #1c2127 | 次级表面/hover |
| border | #d8dde5 | #262c34 | 分隔线/描边 |
| fg-primary | #1a1d23 | #e6e8eb | 主文字 |
| fg-secondary | #4a5260 | #9aa3ad | 次级文字/表头 |
| fg-tertiary | #788090 | #6b7480 | 辅助文字/占位 |
| accent | #3a7af0 | #4c8dff | 品牌/选中/链接/info |

demo 侧扩展的中性阶（保留）：rail（侧栏底）、hover、border-strong、row-line（行分隔线）。

### 2.2 语义角色三件套（状态/图标/徽章配色的唯一来源）

每个语义角色（role）提供三个令牌：

```
role.{name}.fg        前景：文字/图标/状态点/实色徽章背景
role.{name}.bg        背景：软色徽章/高亮行（= fg 12% 透明度混合）
role.{name}.border    边框：软色徽章边框（= fg 25% 透明度混合）
```

透明度规则沿用 React 版现状：**两主题统一 12%（bg）/ 25%（border）**；深色下如可读性不足，允许调至 15%/30%，须在本文档登记。实现取整规则：alpha = round(p×255)，即 12%→31、25%→64（`tokens.rs` 的 const fn 派生按此实现）。

待评审（P3 接线时裁决）：`selected.bg`（accent 12% alpha）在深色下做行选中底色比旧实色更淡，若可读性不足按上述 15% 上调。

**IRtool 角色集（v0.1，迁移审计时补全枚举映射）**：

| Role | 基准色 | 含义 | 已知映射 |
|---|---|---|---|
| critical | critical #911818/#b91c1c | 最高危 | 高危告警、恶意确认 |
| danger(threat) | danger #d63838/#ef4444 | 威胁/错误/失败 | CLOSE_WAIT、危险操作、危险进程 |
| warning | warning #c98a07/#f0b429 | 可疑/需注意 | 可疑项、TIME_WAIT、降级状态 |
| success | success #20a04f/#2ecc71 | 正面/正常/运行中 | ESTABLISHED、采集正常、管理员权限 |
| info(progress) | accent | 进行中/中性动作 | LISTEN、DNS 请求类事件、链接 |
| neutral | fg-secondary | 默认/辅助/未知 | UDP "-"、空态、未扫描 |
| dim(terminal) | fg-tertiary | 终态/降权/历史 | 已结束、已忽略 |
| accent(选中) | accent | 选中/聚焦/当前页 | 行选中、导航当前项 |

**用法约定**（对应徽章两态）：
- 软色徽章（默认）：`bg` 背景 + `fg` 文字（+ `border` 可选）——如状态列 Badge。
- 实色徽章（强调）：`fg` 背景 + `on-accent/白` 文字——仅用于 critical/danger 的强提醒。

**约束（L1 级，违反即 bug）**：
1. 新增状态/事件分级，必须先在本表登记 role，再写代码。
2. 一个枚举只属一个 role；映射依据业务语义，不是视觉偏好。
3. 图标与文字同源：图标颜色只允许取 `role.{x}.fg` 或 `fg-primary/secondary/tertiary`，**禁止为图标单独调色**。

## 3. 字体排印

### 3.1 字族（egui 实现，demo main.rs 已验证）

| 用途 | 字族 | 兜底链 |
|---|---|---|
| 界面文字 | Microsoft YaHei (`msyh.ttc`) | msyh → simhei? → egui 内置 |
| 等宽（时间/IP:端口/PID/路径/哈希/控制台输出） | Consolas (`consola.ttf`) | consolas → yahei |

mono 使用清单：表格中时间戳、端点、PID、路径、哈希、命令行、原始 JSON。

### 3.2 字号阶梯（v0.1 初值，P2 demo 定稿校准）

| 阶梯 | egui FontId | 用途 |
|---|---|---|
| display | proportional 20 semibold | 告警弹窗标题 |
| title | proportional 16 semibold | 页面标题/对话框标题 |
| section | proportional 14 medium | 分组标题/详情区标题 |
| body | proportional 13 | 正文（demo 现值） |
| control | proportional 12.5 | 按钮/输入（demo 现值） |
| table | proportional 12 | 表格单元/工具栏 |
| caption | proportional 11.5 | 徽章/表头/辅助说明 |
| mono-* | monospace 同级 | 上述各级的等宽对应 |

字重设计四档：normal / medium(500) / semibold(600) / bold；标题不超过 semibold。
**egui 实现降级**（登记）：egui `FontId` 暂无 weight 字段（上游 TODO），msyh.ttc 多字重不可加载，实际仅 normal / strong 两态——medium→normal、semibold/bold→`strong()`；待 egui 支持 weight 后还原四档。

### 3.3 密度（对应 React DataTable）

| 模式 | 行高 | 切换 |
|---|---|---|
| compact（默认） | 28px | 表格工具栏 |
| standard | 34px | 同上 |

## 4. 组件公共样式（design/widgets 约定）

| 组件 | 关键规范 |
|---|---|
| Button | 圆角 6px；主按钮 accent 底 + 白字，普通按钮透明底 + border；危险操作 danger；高度 28px（compact） |
| IconButton | 16px 图标 + 4px 内边距；hover 用 bg-elev-2；颜色继承文字层级 |
| Select（下拉） | 沿用 d88eed7 收敛后的规范（egui 自定义 select 已审查通过）；弹出层 surface 底 + border + 圆角 6；选中项 accent-soft 底 |
| SearchInput | 圆角 6 + border；聚焦 ring accent；占位文字 fg-tertiary |
| Badge | 软色徽章为默认（§2.2）；圆角 5px；高度 20px；caption 字号 |
| Chip（状态点+文字） | 7px 圆点（role.fg）+ 12px 文字（fg-secondary）；用于状态栏/统计条 |
| TableShell | 表头 caption 字号 fg-secondary + 底部分隔线；行分隔 row-line；行选中 accent-soft；条件行高亮用 role.bg |
| EmptyState | 44px 图标（fg-tertiary 60%）+ 主文案 + 引导文案；回答"为什么空+下一步" |
| Toast/Banner | accent 文字（可淡出）；错误场景用 danger.fg |
| Tooltip | surface 底 + border + 圆角 6 + caption 字号 |

### 4.1 图标

- 来源沿用 React 侧 lucide 线性风格（egui 侧 SVG→纹理或字符图标，P2 定实现方案）。
- 尺寸两档：16px（工具栏/导航）、14px（行内/表头）。
- 颜色只走 §2.2 约束第 3 条；导航图标默认 fg-secondary，当前页 fg-primary + accent 指示条。

### 4.2 布局与间距规范（P5 实战沉淀，v1.0）

**egui 布局模型事实（官方源码 egui 0.36 style.rs）**：

- 默认 `item_spacing = (8.0, 3.0)`（横 8 / 纵 3）；官方注释明确 **item_spacing 在 widget 添加之后插入**——即 `add_space(pad)` 之后再加 widget，实际偏移 = pad + item_spacing.x。**禁止用"垫片法"（add_space 计算居中）**：永远差一个 item_spacing。
- 默认字体 Body/Button/Monospace 均 13pt（本项目字体阶梯覆盖后以 §3.2 为准）。

**居中规则（硬性）**：

1. 图标/按钮在容器内居中，必须用**整行响应 + 视觉块中心绘制**：`allocate_exact_size(整行宽, 高)` 后以 `full.center()` 为中心画视觉块——数学绝对居中，与容器宽/DPI 无关。参考 `layout.rs rail_row`。
2. 禁止 `add_space` 垫片凑居中；禁止假设 `available_width` 等于容器宽（horizontal 布局中它是"剩余宽"）。

**间距阶梯（8pt 系）**：4 / 8 / 12 / 16 / 24。组件内 padding 8-12；组件间 8-12；分组间 16-24。表格行距 = 行高（item_spacing.y 必须归零，见下）。

**对齐规则**：

1. 同一水平行的控件必须同轴对齐（垂直中心一致）；右对齐组用 `Layout::right_to_left` 且**必须先于弹性区添加**（先占边、后弹性），否则重叠。
2. 表格单元格：左内边距 `CELL_PAD_X`（8），相邻列间距 = 左 pad 之和（16）——左右均衡。
3. 行高即视觉行高：表格容器 `item_spacing.y = 0`；行背景（选中/risk）纵向扩 1px 覆盖亚像素缝（`paint_row_bg`）。

**尺寸规范（本项目定值）**：

| 组件 | 尺寸 |
|---|---|
| 工具栏按钮（胶囊） | 高 30、圆角 6、内边距 12 |
| 图标钮 | 30×30（圆角 7）/ 导航钮 42×42（圆角 10） |
| 顶栏窗口控制钮 | 42×28（关闭 hover 红底） |
| 侧栏 rail | 收起 64 / 展开 168；图标 20；行钮高 42、间距 4 |
| 表格行高 | 紧凑 26 / 标准 30；表头 28 |
| 徽章 | 高 20、圆角 5；状态点 7px |

**自绘控件硬约束**：

1. 自绘控件（allocate + painter）**必须** `resp.widget_info(|| WidgetInfo::labeled(..))` 注册无障碍 label——kittest 自动化与读屏依赖它。
2. 可点击节点的 label 不得随状态拼接（如表头"列名▲▼"），状态指示独立节点渲染。
3. `ui.painter()` 返回引用借用 ui——与 `&mut ui` API（如 `design::icon::draw`）混用时，painter 临时调用即用即放，不存局部变量。
4. `right_to_left` 与弹性区混排顺序：先 right_to_left 占边、后 allocate 弹性区（顺序反了必重叠）。

### 4.3 疑难排查手册（实战踩坑，附源码定位；P6 前必读）

以下每条都消耗过 ≥1 轮返工，根因均已用官方源码考证。源码路径以 egui/egui_extras/winit **0.36.1** 为准。

**① 表格行高亮断裂/上下不均（两轮才根治）**

- 现象：选中/risk 行背景在列间被切成碎块；行上下缝宽不一致。
- 根因 1（横向断裂）：egui_extras 的 StripLayout 给每个 cell 分配 `宽=列宽` 的矩形，然后 `advance_cursor` 推进 `item_spacing.x`——**列间有横向间隙**，每列背景只画自己那格。源码：`egui_extras-0.36.1/src/layout.rs` `StripLayout::add` + `end_line`（`cursor.x = max.x + item_spacing.x`）。
- 根因 2（纵向不均）：行推进 `cursor.y = max.y + item_spacing.y`（默认 3pt）+ 官方 gapless 公式 `expand2(0.5 * item_spacing).round_ui()` 在**非整数 DPI（1.25/1.5）**下上下取整方向不一致 → 上缝≠下缝。手动 `expand 1px` 补偿会因行绘制顺序（上行侵入被下行覆盖）加剧不对称。
- **修复**：表格容器 `item_spacing = Vec2::ZERO`（x/y 全归零，行紧贴、列紧贴，列间视觉间距由 CELL_PAD_X 提供）+ 行背景**物理像素对齐**（`(v * ppp).round() / ppp`——相邻行共享坐标则共享取整边界，数学无缝）。源码：`design/table.rs` `paint_row_bg`。

**② 窗口拖拽（StartDrag）无效**

- 官方文档原文（egui `src/viewport.rs` `ViewportCommand::StartDrag`）：
  > "Moves the window with the left mouse button until the button is released. **There's no guarantee that this will work unless the left mouse button was pressed immediately before this function is called.**"
- egui 的 `Response::dragged()` 带移动阈值（decidedly dragging），按住不动/微动时为 false → StartDrag 从未发出。
- egui_winit 侧还有 `window.has_focus()` 前提（`egui-winit-0.36.1/src/lib.rs` process_viewport_command）。
- **修复**：不用 drag 状态——`resp.contains_pointer() && pointer.primary_down()` 即发（每帧重发无害）。源码：`layout.rs` 顶栏拖拽区。

**③ decorations(false) 后失去系统边缘 resize**

- winit 在 Windows 上 decorations(false) 不提供隐形 resize 边框。
- **修复**：egui 0.36 有 `ViewportCommand::BeginResize(ResizeDirection)`（8 方向，走 winit 系统 resize 循环，Windows 可靠）。指针按下且处于边缘带（6pt）时发送；顶栏区排除（避开窗口控制钮）。源码：`app.rs` 边缘 resize 段；winit 映射见 `egui-winit-0.36.1/src/lib.rs`（`use winit::window::ResizeDirection`）。
- 注意：BeginResize 与 StartDrag 同语义（左键按下立即调用）。

**④ 自绘控件在 kittest/读屏中不可见**

- `allocate_exact_size + painter` 自绘不产生 accesskit label 节点。必须 `resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, label))`。
- 可点击节点的 label **不得随状态拼接**（如表头"列名▲▼"）——kittest `By::label` 是精确匹配。

**⑤ Unicode 特殊符号渲染为 tofu**

- 微软雅黑缺字形（⟳⊘✗ 等）→ 方块。一律用 `design::icon`（lucide 纹理），禁止裸 Unicode 符号做图标。

**⑥ egui Painter 与 &mut Ui 混用借用冲突**

- `ui.painter()` 返回引用借用 ui，与 `&mut ui` API（icon::draw 等）同作用域共存会 E0502。painter 临时调用即用即放，不存局部变量。

**⑦ 垫片法居中失效**

- 见 §4.2 居中规则（item_spacing 在 widget 之后插入，官方 style.rs 注释）。

## 5. 图表与数据画法（统计条/趋势适用）

- **一图一问**：一张图只回答一个问题；图表配色与状态语义色是两套，同图不混用。
- 缺失（null）：空白 + "无数据"标注，禁止显示为 0。
- 零：显示 0，不隐藏。
- 异常/超限：高亮 + 标注，禁止按正常值显示。
- 时间趋势用折线/柱状，占比用饼/堆叠柱，禁止趋势用饼图。

## 6. 硬约束（同步写入根目录 AGENTS.md）

1. 页面代码**禁止**出现 `Color32::from_rgb` 等硬编码色值与裸字号——只引用 `design::tokens` 与字体阶梯。
2. 新增状态/分级先登记 role（本文档 §2.2），再写代码；禁止自造状态色。
3. 图标色禁止独立配色（§2.2-3）。
4. 令牌/规范的修改必须先经 demo 定稿流程（rework-plan §5.2），禁止直接在主项目页面试样式。
5. mono 字族用途以 §3.1 清单为准，不随意等宽化。

## 7. 定稿记录（demo 探索回填）

| 日期 | 条目 | 状态 |
|---|---|---|
| 2026-08-23 | 规范框架建立，色值基准锁定 tokens.css | ✅ |
| 2026-08-23 | P2 色值对齐：demo 与主项目 design 模块同源 tokens.css，双端设计样板像素对照通过（残差 <2%，集中于标题栏文字/窗口阴影）；字号映射落定（内建 TextStyle + Name 扩展档）；字重降级与 alpha 取整规则登记（§2.2/§3.2） | ✅ |
| 2026-08-23 | 图标方案定稿（§4.1）：**SVG 内嵌（lucide）+ egui_extras svg 管线**——lucide SVG（24 viewBox 线性风格）`include_bytes!` 内嵌，加载时 `stroke="black"` 归一为白色后经 `egui_extras::image::load_svg_bytes_with_size`（resvg）光栅化，`(Icon, 物理像素宽)` 纹理缓存，绘制以乘法 tint 着色（主题切换不重光栅化）。无新增直接依赖 | ✅ |
| 2026-08-23 | P3 裁决定稿①：行选中底色采用 **selected.bg 15%**（alpha 38；12% 在深色表面偏淡），P5 页面迁移落地时目检 | ✅ 定稿 |
| 2026-08-23 | P3 裁决定稿②：`hover` 与 `bg-elev-2` 并存收窄——`hover` 仅用于悬停/按下交互态，`bg-elev-2` 仅用于静态次级表面；tokens.css 后续增补 `--hover` 后主项目 tokens.rs 转正 | ✅ 定稿 |
| 2026-08-23 | P3 裁决定稿③：accent_hover/danger_hover 派生规则 = 按主题方向线性混合 18%（浅色调暗、深色调亮，design/tokens.rs `hover_shift`）；tokens.css 增补 `--accent-hover/--danger-hover` 后转正 | ✅ 定稿 |
| — | role 枚举映射全表（Sysmon 22 类事件/告警/网络状态） | 待 P5 审计 |
