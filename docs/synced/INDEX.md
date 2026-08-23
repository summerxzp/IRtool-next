# IRtool 文档索引（INDEX）

> 本文件是全部文档的总入口。新增文档后必须在此登记；不知道东西放哪，先看 §2。

## 1. 文档体系结构

```
IRtool-next/
├── README.md                  # 项目门面（是什么、快速开始）
├── CONTEXT.md                 # 架构上下文（crate 关系/数据流/双前端现状）——改动架构前必读
├── AGENTS.md                  # AI 协作入口（硬约束/惯例/导航，保持轻量）
├── PRE_COMMIT_CHECKLIST.md    # 提交前检查清单
└── docs/
    ├── synced/                # ⭐ 进版本库的规划/规范/方案文档（本索引所在）
    ├── wiki/                  # 领域知识（浏览器取证原理等，按主题生长）
    ├── superpowers/           # 工作过程产物（plans/ 一次性计划、specs/ 阶段规格）
    └── archived/              # 已完结/过档资料（审查报告、旧版文档 bak）
```

放置规则：
- 会指导**未来开发**的（方案、规范、索引）→ `docs/synced/`
- 某功能/领域的**知识沉淀**（原理、调研、使用方案）→ `docs/wiki/`
- **一次性**过程产物（某次迁移的计划、某阶段验收规格）→ `docs/superpowers/plans|specs/`，完结后视价值归档或删除
- 不再维护但有史料价值的 → `docs/archived/`
- 约定俗成：`docs/*` 默认不进库（.gitignore），**只有 `docs/synced/` 被跟踪**；wiki/superpowers 如需共享再显式调整

## 2. 文档一览

### synced（进库，长期维护）

| 文档 | 内容 | 维护时机 |
|---|---|---|
| [egui-ui-rework-plan.md](egui-ui-rework-plan.md) | 前端统一 egui 重构总方案（双轨策略/8 阶段/范式） | 阶段推进时更新勾选与范式回填 |
| [ui-design-spec.md](ui-design-spec.md) | UI 设计规范（令牌三层/语义角色/字体/组件/硬约束） | demo 定稿、role 新增时 |
| INDEX.md | 本文件 | 新文档登记、结构变更 |

### 根级

| 文档 | 内容 |
|---|---|
| README.md | 项目介绍与快速开始 |
| CONTEXT.md | 架构上下文：crate 拓扑、EventBus 数据流、双前端、构建发布 |
| AGENTS.md | AI 协作入口：硬约束、惯例、本文档导航 |
| PRE_COMMIT_CHECKLIST.md | 提交检查清单 |

### wiki（领域知识，本地）

浏览器扩展取证（安装方式/通信原理/流量溯源）、autoruns 删除指引等。

### superpowers / archived（过程与归档，本地）

egui fallback 两轮审查报告（38 项）、浏览器取证旧文档 bak 等。

## 3. 文档间引用约定

- 方案引用规范：`egui-ui-rework-plan.md` → `ui-design-spec.md`（不复制内容，只链接）。
- 规范引用事实源：`ui-design-spec.md` 色值 → `ui/src/styles/tokens.css`（P8 退役 React 时改指 `crates/irtool-egui/src/design/tokens.rs`）。
- 过程文档（superpowers）完结后：有长期价值的结论提炼进 synced，原文归档。
