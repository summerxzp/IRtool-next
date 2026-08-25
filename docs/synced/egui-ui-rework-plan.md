# 前端统一 egui 重构方案

> 状态：定稿待执行 · 分支 `egui-ui` · 2026-08-23
> 结论先行：React/Tauri 前端退役，`irtool-egui` 从降级 UI 升级为唯一主力前端。
> 策略：**双轨制** —— 风格在 demo 从零探索，页面在主项目渐进迁移，任何时刻主分支可用。

---

## 1. 背景与选型结论

### 1.1 为什么要动前端

- React 前端依赖 WebView2，部分 Win10 机器未预装，被迫维护双前端（`irtool-tauri` 19.3k 行 + `irtool-egui` 14.2k 行），长期成本不可接受。
- 应急响应场景要求：**单绿色 exe、体积 ≤50MB、Win10 全系 + RDP 远程桌面可用、大量记录实时渲染**。

### 1.2 为什么是 egui（调查结论摘要）

| 维度 | 结论 |
|---|---|
| 兼容性 | eframe wgpu 后端走 D3D12，Win10 全系 + RDP 实测可用（glow/OpenGL 在 RDP 下崩溃，egui issue #2573） |
| 体积 | 纯 egui 单 exe 实测 19.7MB（wgpu 版，P1）；参照：旧 glow 双前端便携 exe 19.2MB |
| 性能 | demo 实测 500 行滚动流畅，--stress 连续重绘 207FPS；数据量级（万条缓冲、百条/秒）远低于 egui 验证上限（rerun.io 百万点级） |
| 表格生态 | `egui_extras::TableBuilder` 内建虚拟化/固定表头/列宽拖拽，与工具形态（多列大表格）完全匹配 |
| 复用资产 | 主项目已有 14.2k 行、9 页全覆盖，业务层 `irtool-service` 与 UI 解耦 |
| 许可 | MIT/Apache 零义务 |

淘汰项速记：Slint（无官方 TableView 须自研、滚动帧率波动 72–180、skia 后端 Windows 需装 LLVM、免费版须署名）；CEF/WebView2 Fixed Runtime（超体积 3 倍+）；Sciter（商业收费 + 不兼容现有 React 栈）；Qt/CXX-Qt（工具链+LGPL 负担）；Flutter（放弃 service 直连，双语言栈）；Blitz/Dioxus-native（beta，CSS WPT 通过率 ~20%，保持关注）。

### 1.3 已确认的关键技术事实

- 主项目 egui 栈为 0.31.1 + **glow(OpenGL) 后端**，存在 RDP 崩溃风险，必须切 wgpu。
- demo（`E:\Code\solo\ir-ui-demos\egui-demo`）已是 eframe **0.36**，默认 wgpu 后端，且已有 light/dark 双主题、msyh/consolas 字体链、组件雏形 —— 但 0.31→0.36 有 API 变化（如 `egui::Panel`），**主项目版本对齐是 demo 组件搬运的前提**。
- Slint 对照实验中的教训同样适用于 egui 侧：数据刷新严禁全量重建（demo 曾每次 `refresh()` 重建整个 `VecModel`），必须增量更新。

## 2. 目标 / 非目标

**目标**

1. 单 exe ≤25MB（实测锚定 ~20MB，P1 实测纯 egui 入口 19.7MB 含 wgpu），零外部运行时依赖，Win10 全系 + RDP + 无 GPU 虚拟机可用（P1 已实测通过）。
2. 视觉与交互对齐现 React 版水准：复刻为起点，满意后统一演化出自有风格。
3. 功能对齐 React 版清单（见 §6 范式与附录 B），含深浅主题、zh/en 双语、告警弹窗、表格高级能力。
4. 工具链收敛为纯 cargo（退役 Node/pnpm/Vite/Tauri 链）。

**非目标**

- Win7 支持（glow 后端唯一的存在理由，放弃）。
- Linux/macOS（未来再说）。
- React 版新增功能（React 进入冻结维护，仅修严重 bug，作为视觉基准保留到 P8）。

## 3. 总体策略：双轨制

```
┌─ 风格轨道（沙盒）────────────────────────────┐
│ ir-ui-demos/egui-demo（独立仓库，秒级编译）      │
│ 职责：风格迭代、组件孵化、截图对比              │
│ 产物：定稿的 token/组件 → 搬入主项目 design 模块 │
└──────────────────────────────────────────────┘
                    ↓ 定稿搬运
┌─ 迁移轨道（主干）────────────────────────────┐
│ IRtool-next @ egui-ui 分支（16 crates）        │
│ 职责：渐进迁移 9 个页面 + 横向能力              │
│ 约束：任何提交点 cargo build --release 可过     │
└──────────────────────────────────────────────┘
```

- demo 不迁业务逻辑，只养"皮肤"；主项目不试新风格，只接定稿组件。
- 每个阶段独立可交付、可停；React 版全程保底，无"工具不可用"窗口。

## 4. 阶段计划

每阶段一个 PR 粒度；验收通过才进下一阶段。

### P0 分支与基线（本文件）✅

- [x] 建立 `egui-ui` 分支
- [x] 方案文档评审通过

### P1 渲染底座升级（egui 0.31.1 → 0.36+，glow → wgpu）— 技术项完成，实测待用户

一次性吸收版本 breaking（demo 已验证 0.36 可行）。**这是后续一切搬运的前置条件。**

- [x] `irtool-egui` 依赖升级到 0.36 系，feature 从 glow 切 wgpu，逐页修 API 编译错误（提交 `dd3c24e`，egui/eframe/egui_extras 0.36.1 + wgpu 30.0.1，`egui_glow` 已移出依赖图）
- [x] RDP 会话实测（2026-08-23 用户实测通过：Win10 VM 未开 VT-x/未开 3D 加速，RDP 远程执行，9 页点击/滚动无闪退）
- [x] 无 GPU 虚拟机实测（wgpu WARP 软件回退可用，同上环境）
- [x] 记录 release 体积基线：`irtool-egui.exe`（纯 egui 独立入口）19.7MB ← **P8 目标形态实测锚点**；`irtool-tauri.exe`（双前端含 wgpu）27.5MB；参照系：旧 glow 双前端便携 exe 19.2MB（dist/IRTool-v2.0.3-portable），wgpu 增重约 7MB，P8 删 tauri 后预计 ~20MB（方案早期"8–14MB"预期偏乐观，以实测为准）
- 已验收：release 双目标构建零警告、diff review 无行为/样式变更、运行冒烟 8s 无 panic、RDP/VM 实测通过 ✅（2026-08-23 P1 完结）
- 已知行为差异（0.31→0.36，观察即可）：默认字号 12.5→13.0（未补偿，P2 字号阶梯统一处理）；面板出现动画；pixels_per_point→zoom 映射基准微差
- P1 实测发现的功能问题（转 P7）：① autoruns 采集"扫描中"卡住（VM 普通权限复现，疑似无提权相关，UAC 落地后复测）；② egui 独立入口无 UAC manifest（requireAdministrator 仅嵌在 tauri exe）

### P2 design 模块入库（不动任何页面）✅

- [x] 前置：按 [ui-design-spec.md](ui-design-spec.md) 校准 demo 色值（以 tokens.css 为准覆盖，含 ok/listen/tw/cw/mute → success/info/warning/danger/neutral 改名），补齐语义角色三件套（fg + bg@12% + border@25% 派生）
- [x] `crates/irtool-egui/src/design/` 新模块：`tokens.rs`（Palette 双主题 + 8 角色三件套，值严格取自 tokens.css）、`fonts.rs`（msyh/consolas 兜底链 + 字号阶梯，内建 TextStyle + Name 扩展档）、`widgets.rs`（badge 两态/chip/empty_state/panel_frame/separator，无硬编码色）、`preview.rs`（design_board 样板）+ `examples/design_board.rs` 预览入口（main.rs 零改动，lib.rs 仅 +2 行）
- [x] demo 与主项目渲染同一设计样板，截图对照：像素残差 light 1.68% / dark 1.18%，全部集中于窗口标题栏文字与 DWM 阴影边缘，正文/色板/组件零结构差异（截图存 `ir-ui-demos/shots-p2/`）
- 验收通过（2026-08-23）：release 构建零警告；`git diff` 除新增文件外仅 lib.rs +2 行，9 个现有页面零改动 ✅
- 备注：demo 仓库（E:\Code\solo\ir-ui-demos）无版本管理，建议 git init（待用户决定）；spec §7 已回填字重降级/取整规则，遗留 4 个待定稿项转 P3

### P3 全局壳切换 + 主题三态 ✅

- [x] 主题运行时（`design/theme.rs`）：Light/Dark/System 三态（默认 System，注册表 AppsUseLightTheme 判定），OnceLock+RwLock 全局态，持久化 `<数据目录>/config/ui-state.json`（serde_json 原子写，跟随 portable 机制）；已登记限制：运行期不监听系统主题变化
- [x] 旧 `theme.rs` 运行时化：const→fn（363 处机械替换，映射表注释齐全，9 页面 365+/364- 纯机械），全 app 主题一致，无"黑壳白页"割裂
- [x] 壳切换：顶栏/侧栏/状态栏接 design 模块（视觉复刻 demo 壳），顶栏主题切换按钮（浅色→深色→跟随系统循环）
- [x] 图标方案落地：lucide SVG 内嵌 + egui_extras svg 管线（`design/icon.rs` + `assets/`，tint 着色缓存，无新增直接依赖）
- [x] spec 三项裁决定稿：selected.bg 15%（P5 落地目检）/ hover 与 bg-elev-2 收窄并存 / 交互态 18% 派生（见 spec §7）
- 验收记录（2026-08-23）：release 双目标构建零警告；真实采集数据下深色全 app 一致（顶栏/侧栏图标/表格/语义徽章），持久化写入→重启→生效链路验证通过；截图 `target/shots-p3/`

### P4 i18n 就位（必须在逐页迁移前，避免每页摸两遍）✅

- [x] rust-i18n 3.x 接入（编译期宏，locales 嵌入二进制）
- [x] 键表搬运：`scripts/i18n-egui-sync.mjs`（React locale → egui locales 同步 + `scripts/egui-locales-extra/` egui 特有键 merge 源）；zh/en 各 743 键、差集为空；键名沿用 React 原键名（P6 直接用）
- [x] 全局壳接线（app 31 / nav 9 / layout 6 处 t!）；settings 页全量接线（含事件类型标签动态化、异步消息占位符 %{e}/%{path}），非注释中文残留 0
- [x] 语言切换：settings 侧栏沉底 ComboBox（各语言自称 native_label），UiState.language 持久化（ui-state.json），切换即时生效
- 验收记录（2026-08-25）：双目标 release 构建零警告；写 en-US → 重启 → 全英文壳（顶栏/工具栏/状态栏）截图验证通过（`target/shots-p4/`）；其余 8 页零 diff（P6 迁移时接线）
- 备注：React locale 自身有键不齐现象（如 ioc-matches 缺 en），搬运时已修补；后续 React 侧补键建议同步回脚本源

### P5 reference 页迁移：network（1,292 行）

定死迁移范式（§6），产出范式文档片段回填本文档。

- [ ] 表格新组件 `design/table.rs` 首次实战：列宽拖拽、表头排序、密度切换（28/34px）、键盘 ↑↓ 导航、行选中/右键、条件行样式（risk 高亮）
- [ ] 数据侧增量更新（事件到达改 store，禁止全量重建）
- 验收：与 React 版 network 页并排截图对照 + 滚动/过滤/导出功能清单逐项打勾

### P6 逐页迁移（按使用频率排序，复杂度递增）

顺序与预估（行数为现状）：

| 序 | 页面 | 行数 | 备注 |
|---|---|---|---|
| 1 | network | 1292 | P5 完成 |
| 2 | sysmon | 1367 | 对应 React 日志采集页，实时流主战场 |
| 3 | autoruns | 1345 | 分类树 |
| 4 | process | 1365 | 进程树 |
| 5 | database | 1180 | 修 W11（raw_json 重复解析） |
| 6 | monitor | 708 | 告警 |
| 7 | settings | 926 | |
| 8 | workspace | 2366 | 修 S8（规则持久化）、S7（恶意 IP 配置化） |
| 9 | browser_forensics | 1672 | React 侧 3963 行差距最大，最后攻坚（CDP 捕获控制、扩展归因等） |

- 验收（每页相同）：功能对齐清单 + 截图对照 + 该页 i18n 接线完成

### P7 横向能力补齐

- [ ] 独立告警弹窗（eframe viewport 多窗口，替代现 banner；行为对齐 React 版 10s 自动关、点击跳页）
- [ ] 表格列宽/列序 localStorage 等价持久化（exe 目录 JSON）
- [ ] CSV 导出路径统一（rfd 系统对话框，对齐 `ui/src/lib/csv.ts` 行为）
- [ ] UAC 自提升：manifest 改 asInvoker + 启动检测非提权则 ShellExecuteW runas 重启提权；用户拒绝 UAC 则普通模式继续运行（P1 实测发现的既定期望行为）
- [ ] autoruns 采集"扫描中"卡住排查（P1 VM 实测发现；先在提权环境复测排除权限因素）
- [ ] deferred 审查遗留清零（docs/archived/ 两轮审查剩余项）

### P8 切换默认前端与退役

- [ ] `irtool-tauri/src/main.rs` 的 WebView2 检测与 fallback 逻辑移除，egui 为唯一前端（`egui-fallback` feature 删除）
- [ ] 删除 `crates/irtool-tauri`、`ui/`、Node 工具链配置；`scripts/build-portable.ps1` 改纯 cargo
- [ ] MSIX/NSIS 安装包重建；版本号机制不变
- [ ] 终验：体积 ≤15MB、启动 <1s、万条表格滚动 ≥60fps、RDP/VM/60Hz+高刷屏实测、AV 误报抽查（Virustotal）
- 验收：发布 IRTool vNext portable

## 5. 风格探索与定稿流程

### 5.0 规范前置（已建立，见 ui-design-spec.md）

先行固化结构与约束（令牌三层架构、语义角色三件套、字体阶梯、组件公共样式、图标颜色分级、硬约束），具体值以 React 版为基准源，经 §5.2 流程在 demo 定稿后回填 spec §7。**页面迁移（P5+）开始前 spec 必须达 v1.0**（色值对齐 + role 枚举映射全表 + 字号校准）。

方法论框架沿用作者 UI 知识库（审计→原则→视觉探索→token→页面生产→多页演化）：

- "先复刻后演化"对应：审计（tokens.css/截图）→ token（翻译）→ 生产（逐页迁移）→ 演化（统一一轮风格升级）。
- Pattern 目录（Table / Investigation / Work Queue / State 三态 / 图表一图一问）作为页面结构选型参考——IRtool 9 页全部落在 Table 为主 Pattern、Investigation 为详情面板辅助、State Pattern 全局适用的组合上。

### 5.1 原则：先复刻，后演化

- 第一阶段**逐像素复刻 React 版**（它已是满意的基准）：色板/间距/圆角/字号/交互节奏全部以 React 版截图为准。
- 复刻达标后，再统一做一轮"自有风格演化"（一次改 token 全局生效，不做单页特例）。

### 5.2 demo 迭代循环

```
改 demo → capture.ps1 截图 → 与 React 版并排对照 → 不满意继续改（秒级迭代）
   → 满意 → 组件搬入主项目 design/ → 主项目截图复核 → 定稿记录
```

- 风格决策记录在 demo 仓库 `POLISH-PLAN.md`（已存在），定稿条目同步到本文档 §5.5。

### 5.3 设计 token 基线（来源 `ui/src/styles/tokens.css`）

- 双套色板：`--bg-base / bg-elev-1 / bg-elev-2 / border / fg-primary / fg-secondary / fg-tertiary / accent / success / warning / danger / critical` + color-mix 派生背景 → 翻译为 `tokens.rs` 的 dark/light 两组 `Color32` 常量 + 派生函数。
- 字体：Proportional = 微软雅黑（`msyh.ttc` 候选链），Monospace = Consolas → 雅黑兜底（demo main.rs 已实现，直接搬）。
- 新增 token 必须先进 demo 定稿，禁止在主项目页面里直接写 `Color32` 字面量。

### 5.4 组件清单（design/widgets 范围）

Button / IconButton / Badge(severity 五级，对齐主项目 severity 规范) / Chip(env/count) / Toolbar / Select / SearchInput / EmptyState / DetailRow / Banner / TableShell（列宽拖拽、排序表头、密度切换、键盘导航、行选中/右键、虚拟滚动、空状态）。

### 5.5 定稿记录（迁移过程中回填）

- （空，待 demo 定稿后登记）

## 6. 迁移范式（P5 定稿，全页强制）

- 页面骨架：每页一个 `pages/*.rs` 模块，页面状态集中 struct，禁止跨页共享可变全局。
- 数据流：`EventBus → event_bridge（已有）→ 页面 store（Mutex + VecDeque 环形）→ request_repaint 驱动`，事件到达只改数据不碰 UI。
- 性能红线（每页 PR 自查项）：
  1. 行渲染闭包**零分配**：进表格前预格式化列字符串（修 S3 类问题），渲染闭包内禁止 `format!`/`clone`；
  2. 环形缓冲上限（沿用 `MAX_EVENTS = 10000` 模式），超限弹头丢弃；
  3. 表格用 `TableBody::rows`（固定行高），禁用 `heterogeneous_rows`；
  4. 刷新增量更新，禁止整表/整模型重建；
  5. 查询解析结果缓存（修 W11 类问题）。
- i18n：键名 `page.widget.key` 对齐 React locale 文件，迁移该页时同步接线。
- 交互对齐清单（每页核对 React 版）：排序/过滤/搜索/右键菜单/详情面板/导出/空状态/加载态/键盘导航。

## 7. 风险与对策

| 风险 | 对策 |
|---|---|
| wgpu 在老 GPU / RDP / VM 失败 | P1 阶段专项实测清单（RDP 会话、无 GPU VM、60Hz/高刷屏）；wgpu 失败时尝试 WARP 软件适配器；极端情况保留 glow 作为编译期备选（不进常规发布） |
| CJK 字体缺失（LTSC 精简系统） | 字体候选链（msyh → msyhbd → simhei → egui 默认），启动日志记录实际命中字体 |
| AV / EDR 误报 | 不加壳（禁 UPX）、不嵌可疑资源、终验 VT 抽查；如条件允许申请代码签名证书 |
| 0.31→0.36 升级 breaking 面超预期 | demo 已在 0.36 验证核心 API；P1 独立 PR，可整体回退 |
| 单人迁移疲劳（9 页 × 范式执行） | 每页独立 PR、React 保底可随时停；P5 范式固化后机械执行 |
| 风格反复（demo 长期不定稿） | §5.1 强制"先复刻后演化"，复刻即 P5 验收标准，演化另起一轮 |

## 8. 度量与里程碑

| 里程碑 | 度量 |
|---|---|
| P1 完成 | RDP/VM 实测通过，体积基线记录 |
| P1 完成 | RDP/VM 实测通过，体积基线记录 ✅（2026-08-23，见 §4-P1） |
| P5 完成 | network 页与 React 版并排对照通过，范式文档回填 |
| P8 完成 | 单 exe ≤25MB（实测锚定 ~20MB）、启动 <1s、万条滚动 ≥60fps、VT 误报抽查通过 |

## 9. 文档体系

重构期间长出的文档按 [INDEX.md](INDEX.md) 的结构分布（synced 进库方案规范 / wiki 领域知识 / superpowers 过程产物 / archived 归档）；AI 协作红线沉淀在根目录 AGENTS.md。新增文档必须登记 INDEX.md。

重构相关核心文档链：本方案（总控）→ [ui-design-spec.md](ui-design-spec.md)（设计规范）→ demo 仓库 `POLISH-PLAN.md`（风格迭代现场）。

---

## 附录 A：选型证据索引

- egui RDP 崩溃（glow）与 wgpu 可用：emilk/egui#2573、#2545
- egui_extras 虚拟化：docs.rs/egui_extras TableBody
- Slint 大列表 a11y 性能：slint-ui/slint#3867；femtovg 文本渲染弱：#6365；无 TableView：#4561
- 性能实测记录（本机，2026-08-23）：egui --stress 207fps；Slint femtovg（关 a11y）滚动 96–180 波动
- 体积对照：CEF ~150–200MB / WebView2 Fixed Runtime ~+150MB / egui 预计 8–14MB

## 附录 B：现状资产清单（迁移输入）

- egui 侧：`crates/irtool-egui/` 24 文件 14,240 行；theme.rs 204 行（硬编码浅色）；widgets ~160 行；DESIGN.md 功能矩阵全 ✅
- React 侧（基准）：`ui/src` 19,250 行；tokens.css 72 行；DataTable.tsx 379 行（能力基准）；locales 817 键 ×2；浏览器取证 3,963 行为最大差距页
- 桥接：`event_bus.rs`（broadcast 1024）、`event_bridge.rs`（mpsc + request_repaint）
- 已知遗留（迁移时清零）：S3 每帧 format!、S7 恶意 IP 硬编码、S8 规则不持久化、W11 raw_json 重复解析、签名/哈希扫描进度条未实现（app.rs 空分支）
