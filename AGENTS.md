# AGENTS.md — AI 协作入口

> 给 AI 助手（和未来的自己）的最小上下文。细节不在此展开，只给指针和红线。

## 项目是什么

IRtool：Windows 应急响应工具（Rust workspace，16 crates）。网络监控、Sysmon 事件、持久化检测、浏览器取证、后台监控。当前双前端（Tauri/React 主 + egui 降级），正按 [docs/synced/egui-ui-rework-plan.md](docs/synced/egui-ui-rework-plan.md) 统一到 egui（分支 `dev` 进行中）。

**改架构前必读**：[CONTEXT.md](CONTEXT.md)（crate 关系/EventBus 数据流/构建发布）。
**找任何文档**：[docs/synced/INDEX.md](docs/synced/INDEX.md)。

## 红线（违反即返工）

### UI（详见 docs/synced/ui-design-spec.md）

1. 页面代码禁止硬编码色值/裸字号——只用 `design::tokens` 与字体阶梯。
2. 新增状态/分级必须先登记语义角色（spec §2.2），禁止自造状态色。
3. 图标颜色只允许 `role.{x}.fg` 或 `fg-*` 文字层级，禁止单独调色。
4. 样式变更必须先经 demo（`E:\Code\solo\ir-ui-demos`）定稿，禁止在主项目页面试样式。
5. 表格行渲染闭包零分配（`format!`/`clone` 禁入）；刷新增量更新，禁止整表重建；用 `TableBody::rows`。

### egui 本地参考（疑难先查，别瞎猜）

`E:\Code\refs\egui-0.36` — egui 0.36 官方仓库浅克隆（完整源码带文档注释 / `examples/` 官方示例 / `crates/egui_demo_lib`）。用法：自绘标题栏等交互先看 `examples/custom_window_frame.rs`；间距/字体默认值查 `crates/egui/src/style.rs`；表格行为查 `egui_extras/src/table.rs` + `layout.rs`。已踩坑的根因与修法见 `docs/synced/ui-design-spec.md` §4.3 疑难排查手册。

### 工程

- 提交信息：中文 + conventional 前缀（`feat:` / `fix:` / `docs:` / `refactor:`）。
- 分支：`dev` 开发，阶段性合回 `main`（PR 或 merge）。
- 提交前过 [PRE_COMMIT_CHECKLIST.md](PRE_COMMIT_CHECKLIST.md)。
- `docs/` 下新文件默认不进库，进库的放 `docs/synced/` 并登记 INDEX.md。

## 常用命令

```bash
cargo build --release                                    # 全量构建
cargo tauri build --no-bundle --features irtool-tauri/egui-fallback   # 发布构建（含前端）
# egui 快速迭代：E:\Code\solo\ir-ui-demos\egui-demo（独立秒级编译）
```

## 文档分布速查

进库方案/规范 → `docs/synced/` · 领域知识 → `docs/wiki/` · 过程产物 → `docs/superpowers/` · 归档 → `docs/archived/`
