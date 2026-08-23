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
| — | role 枚举映射全表（Sysmon 22 类事件/告警/网络状态） | 待 P5 审计 |
| — | 图标实现方案（SVG vs 字符） | 待 P3 |
| — | selected.bg 深色可读性（12% vs 15%） | 待 P3 接线评审 |
| — | 交互态派生规则：accent_hover/danger_hover 无基准来源；demo 扩展 hover 与 bg-elev-2 语义重叠待收敛 | 待 P3 定稿 |
