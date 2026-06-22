# 进程快照与三向关联 — 最终设计方案

> 本文档基于 `process-snapshot-design.md`（初版方案）与 `process-snapshot-design-review.md`（Review 改进方案），结合代码库逐项验证、业界工具对标（Process Hacker / System Informer / Sysmon）及使用场景反推，给出最终落地设计。

---

## 实施进度

### 一期 ✅ 已完成

**后端**：
- [x] `ProcessEntry` 扩展 `is_suspicious` / `suspicious_reason` 字段
- [x] `query_exe_path` 改为 `pub(crate)`
- [x] 新增 `take_snapshot_enriched()` — 快照时批量查询 exe 路径并标记可疑
- [x] `ProcessService` 切换到 enriched 版本
- [x] TypeScript bindings 更新

**前端**：
- [x] 进程列表视图（PID/PPID/名称/路径/可疑标记 + 应用图标）
- [x] 进程树视图（默认视图，孤儿进程黄边，可疑进程黄底+⚠，可逐节点展开/收起）
- [x] 进程链追溯（树形缩进样式，目标节点蓝色高亮，点击展开详情）
- [x] 工具栏：刷新 / 自动刷新（Play/Pause + 间隔选择）/ 列表·树切换 / 展开全部 / 筛选 / 搜索
- [x] 左右分栏布局 + detailPosition 切换
- [x] 路由 + Sidebar + i18n

### 一期优化 ✅ 已完成

- [x] 应用图标（复用 autoruns 的 `cmdAutorunsExtractIcon` + reactive cache 模式）
- [x] 进程链改为日志采集页的树形缩进样式（带选中展开详情）
- [x] 图标 `object-contain` 防裁切
- [x] 默认树视图
- [x] 蓝色高亮减淡（`bg-accent/15`）
- [x] 树视图可疑标记与列表一致（`bg-warning/8` + `text-warning` + ⚠ tooltip）
- [x] 展开/收起全部（一次性操作，之后可手动逐节点切换）
- [x] 自动刷新（Play/Pause + 1s/2s/5s/10s 间隔选择）

### 二期 ✅ 已完成

**后端**：
- [x] `irtool-net-monitor` HistoryStore 新增 `query_by_pid(pid)` 方法
- [x] `irtool-autoruns` AutorunsStore 新增 `query_by_path(exe_path)` 方法（含路径归一化）
- [x] Tauri 命令 `cmd_network_query_by_pid` + `cmd_autoruns_query_by_path`（薄包装，crate 层逻辑）

**前端**：
- [x] ProcessDetail 四标签页：进程链 / 网络 / 日志 / 持久化
- [x] 网络关联面板（按 PID 查询 NetConn，展示协议/本地/远程/状态/时间）
- [x] Sysmon 事件关联面板（从前端 Zustand store 按 PID 过滤，支持网络连接/DNS/进程创建等事件类型）
- [x] 持久化关联面板（按 exe 路径匹配 AutorunItem，含路径归一化）
- [x] 交叉跳转：
  - 进程详情 → 网络监控（pid 参数）
  - 进程详情 → 日志采集
  - 进程详情 → 持久化检测（imagePath 参数）
  - 网络详情 → 进程链（"查看进程链"按钮）
  - 持久化详情 → 进程（"查看运行状态"按钮）
- [x] 跳转后目标页面自动选中匹配项（validateSearch + useEffect）

### 未实施（设计文档中提到但未落地）

- ETW 实时进程监控 — 场景不适配（事后溯源 vs 实时捕获），复杂度过高
- 虚拟化 — DataTable 已内置 `@tanstack/react-virtual`，无需额外引入
- 签名验证（`WinVerifyTrust`）— 一期不做，`SuspiciousFlag` 枚举已具备扩展性

## 一、使用场景回顾

依据 `docs/使用场景.md`：

- **核心场景**：中毒电脑应急响应处置，捕获恶意进程痕迹，溯源、取样、处置
- **用户规模**：安全人员单用户使用
- **监控周期**：短期（1~2 天）
- **硬约束**：体积 < 50MB、兼容 Win10/11 及 Server 2016+、网络捕获 + 域名捕获 + 持久化扫描

**场景对设计的反推**：
1. 短期应急 → 不需要常驻后台的实时进程监控（ETW/Sysmon 模式过重）
2. 单用户 → 不需要并发安全考虑，但需要快速定位异常
3. 应急溯源 → 进程链追溯 + 三向关联是核心价值，超越任务管理器
4. 体积约束 → 不能引入重型依赖（如 Sysmon 驱动、ETW 持久化）

---

## 二、初版方案与 Review 方案的客观评估

### 2.1 初版方案评估

**结论：方向正确，整体可行，但存在表述矛盾与多处实现细节遗漏。**

#### 优势

| 方面 | 评价 |
|------|------|
| 分期思路 | 一期（进程列表+链）/ 二期（三向关联）切分合理，增量价值清晰 |
| 后端复用 | 正确识别了 `take_snapshot` / `get_process_chain` / `check_suspicious` 已就绪 |
| 关联可行性分析 | 准确识别了 `PcapEvent` 无 PID 的关键限制，关联仅限 `NetConn` |
| 前端结构 | 基本遵循现有 feature 模式（api/hooks/store/types） |
| 路径归一化 | 考虑了大小写、分隔符、环境变量展开，实用 |

#### 问题（经代码库验证）

1. **表述自相矛盾**：一期"后端改动"先写"无需改动"，紧接着列出需要扩展 `ProcessEntry` 和新增 `take_snapshot_enriched()`。实际验证：`ProcessEntry` 确实只有 4 个字段（`pid/ppid/name/exe`），`exe` 在 `take_snapshot()` 中始终为 `None`，**必须改后端**。

2. **`image_path` 行号错误**：设计文档标注 `irtool-autoruns/src/types.rs:52`，实际在第 45 行（第 52 行是 `md5` 字段）。

3. **`query_exe_path` 可见性遗漏**：设计拟在 `snapshot.rs` 调用 `query_exe_path()`，但该函数定义在 `chain.rs:86` 且是模块私有 `fn`，未提及需改为 `pub(crate)`。

4. **缺少 `columns.tsx`**：现有 `network` / `autoruns` feature 均有独立 `columns.tsx` 定义 `@tanstack/react-table` 列配置，初版文件结构遗漏。

5. **布局不一致**：初版描述"底部面板展开"（上下分栏），但 `NetworkPage` / `AutorunsPage` 均采用 `react-resizable-panels` 的左右分栏（表格 + 详情），且支持 `detailPosition` 配置（`right` / `bottom` 可切换）。

6. **二期关联数据依赖脆弱**：进程→网络依赖 `useNetworkStore` 缓存，进程→持久化依赖 autoruns 扫描结果缓存。若用户未访问过对应页面，缓存为空，关联失效。

7. **URL 参数跳转缺实现路径**：提议 `router.navigate({ to: "/process", search: { pid } })`，但现有路由均未配置 `validateSearch`，无法解析 search params。

8. **遗漏项**：未讨论刷新策略、数据量虚拟化、进程树视图、右键菜单、默认排序、i18n 具体文件、Sidebar 图标导入。

### 2.2 Review 改进方案评估

**结论：质量高，事实核查严谨，补充点均命中要害，个别建议需结合场景再权衡。**

#### 高价值贡献

| 补充点 | 评价 |
|--------|------|
| 行号纠错（52→45） | 事实正确，已验证 |
| `query_exe_path` 可见性 | 关键实现细节，必须处理 |
| 业界对标（Toolhelp32 vs NtQuerySystemInformation vs WMI） | 结论正确：当前方案可行，无需更换。`NtQuerySystemInformation` 的 `ImageName` 仅短名，省不掉 `OpenProcess` 查路径的开销 |
| 补充 `columns.tsx` | 一致性必需 |
| 左右分栏布局 | 与现有页面一致，应采纳 |
| 进程树视图 | **高价值**：IR 场景下孤儿进程、异常父子关系一目了然，且纯前端用 `pid/ppid` 构建，零后端改动 |
| 二期独立 API 建议 | 正确识别了缓存依赖的脆弱性 |
| `validateSearch` 补充 | TanStack Router 的标准做法，必需 |
| 手动刷新 + 时间戳 | 符合短期应急场景，避免轮询开销 |

#### 需再权衡的建议

1. **ETW 实时进程监控（二期补充项）**
   - Review 在二期清单提到"ETW 实时进程监控（ProcessStart/ProcessStop 事件订阅）"
   - **场景不适配**：ETW 订阅需要管理员权限、增加复杂度、且与"1~2 天短期应急"场景不匹配
   - **建议**：不在本方案范围内。若未来需要实时进程创建监控，应单独评估 Sysmon 集成或 ETW 内核会话

2. **虚拟化（`@tanstack/react-virtual`）**
   - Review 建议"评估现有 DataTable 在 500 行时的渲染性能"
   - **实际考量**：Windows 通常 200~500 进程，现代浏览器渲染 500 行表格无压力；现有 `network` / `autoruns` 页面未启用虚拟化且工作正常
   - **建议**：一期不引入虚拟化，保持与现有页面一致；若实测卡顿再引入

3. **可疑规则扩展（签名验证、进程注入）**
   - Review 建议一期保持现有规则，预留扩展空间
   - **同意**：签名验证（`WinVerifyTrust`）有价值但增加体积和复杂度，一期不做；`SuspiciousFlag` 枚举已具备扩展性

---

## 三、最终设计方案

### 3.1 核心决策汇总

| 决策点 | 最终选择 | 理由 |
|--------|---------|------|
| 进程枚举方案 | 维持 `Toolhelp32` + `OpenProcess` | 业界对标验证可行，500 进程 50~250ms 可接受 |
| `ProcessEntry` 扩展 | 增加 `is_suspicious` / `suspicious_reason` | 快照需自带可疑标记，避免前端逐进程再查 |
| `query_exe_path` 可见性 | 改为 `pub(crate) fn` | 最小改动，不污染公共 API |
| 页面布局 | 左右分栏（与 network/autoruns 一致） | 一致性优先，支持 `detailPosition` 切换 |
| 视图模式 | 列表视图 + 进程树视图切换 | 树视图对 IR 场景价值大，零后端改动 |
| 刷新策略 | 手动刷新 + 显示快照时间戳 | 短期应急场景，避免轮询开销 |
| 虚拟化 | 不引入 | 500 行无需虚拟化，保持与现有页面一致 |
| 二期关联数据 | 独立查询（crate 层 + Tauri 薄包装） | 不依赖其他页面缓存，数据可靠；核心逻辑在 crate 层，egui 可复用 |
| URL 参数跳转 | `validateSearch` 配置 | TanStack Router 标准做法 |
| ETW 实时监控 | 不做 | 场景不适配，复杂度过高 |

### 3.2 一期：进程列表页（含进程树）

#### 3.2.1 后端改动

**改动 1：扩展 `ProcessEntry` 结构**

文件：`crates/irtool-process/src/types.rs`

```rust
pub struct ProcessEntry {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    pub exe: Option<String>,
    pub is_suspicious: bool,               // 新增
    pub suspicious_reason: Option<String>, // 新增
}
```

**改动 2：`query_exe_path` 改为 `pub(crate)`**

文件：`crates/irtool-process/src/chain.rs:86`

```rust
// 原: fn query_exe_path(pid: u32) -> Option<String>
pub(crate) fn query_exe_path(pid: u32) -> Option<String>
```

**改动 3：新增 `take_snapshot_enriched()`**

文件：`crates/irtool-process/src/snapshot.rs`

```rust
pub fn take_snapshot_enriched() -> Result<ProcessSnapshot, IrError> {
    let mut snap = take_snapshot()?;
    for entry in &mut snap.processes {
        if let Some(exe) = crate::chain::query_exe_path(entry.pid) {
            let flag = crate::suspicious::check_suspicious(&entry.name, &exe);
            entry.exe = Some(exe);
            entry.is_suspicious = flag.is_some();
            entry.suspicious_reason = flag.map(|f| f.reason().to_string());
        }
    }
    Ok(snap)
}
```

**改动 4：Tauri 命令切换到 enriched 版本**

文件：`crates/irtool-tauri/src/commands/process.rs:8`

```rust
#[tauri::command]
pub async fn cmd_process_snapshot(ctx: State<'_, AppContext>) -> Result<ProcessSnapshot, IrError> {
    let rt = ctx.rt.lock().await;
    rt.spawn_blocking(|| irtool_process::snapshot::take_snapshot_enriched())
        .await
        .map_err(|e| IrError::Internal(e.to_string()))?
}
```

**改动 5：重新生成 bindings**

执行 `cargo run --bin export-bindings`（或项目对应的 bindings 生成命令），更新 `ui/src/lib/bindings.ts`。

#### 3.2.2 前端实现

**文件结构（遵循现有 feature 模式）**：

```
ui/src/features/process/
├── pages/
│   └── ProcessPage.tsx          # 主页面（左右分栏）
├── components/
│   ├── ProcessTable.tsx         # 进程表格（列表视图）
│   ├── ProcessTreeView.tsx      # 进程树视图
│   ├── ProcessDetail.tsx        # 右侧详情面板（进程链）
│   └── ProcessToolbar.tsx       # 工具栏（刷新+视图切换+筛选+搜索）
├── columns.tsx                  # @tanstack/react-table 列定义
├── api.ts                       # Tauri 命令封装
├── hooks.ts                     # React Query hooks
├── store.ts                     # zustand store
└── types.ts                     # 类型定义

ui/src/routes/process.tsx        # 路由文件（含 validateSearch）
```

**页面布局（左右分栏，与 network/autoruns 一致）**：

```
┌──────────────────────────────────────────────────────┐
│ ProcessToolbar [刷新] [列表|树] [筛选:全部▼] [搜索...] │
├──────────────────────┬───────────────────────────────┤
│ ProcessTable/TreeView│ ProcessDetail                 │
│ (可切换列表/树视图)   │ 进程链: 目标→父→祖父→...→System│
│                      │ 每个节点: 名称/路径/命令行/可疑│
│                      │ 快照时间: 2026/06/22 14:30:00 │
└──────────────────────┴───────────────────────────────┘
```

**关键实现点**：

1. **列表/树视图切换**：
   - 列表视图：`ProcessTable` 基于 `@tanstack/react-table`，列定义在 `columns.tsx`
   - 树视图：`ProcessTreeView` 用 `pid/ppid` 递归构建树结构，纯前端实现
   - 树视图特殊标记：孤儿进程（`ppid` 不在快照中）标黄，可疑进程标红

2. **进程树构建算法**（前端）：
   ```typescript
   function buildProcessTree(processes: ProcessEntry[]): ProcessTreeNode[] {
     const map = new Map<number, ProcessTreeNode>();
     const roots: ProcessTreeNode[] = [];
     // 第一遍：建节点
     for (const p of processes) {
       map.set(p.pid, { ...p, children: [] });
     }
     // 第二遍：建父子关系
     for (const p of processes) {
       const node = map.get(p.pid)!;
       const parent = map.get(p.ppid);
       if (parent) {
         parent.children.push(node);
       } else {
         roots.push(node); // ppid 不在快照中 → 孤儿或根
       }
     }
     return roots;
   }
   ```

3. **默认排序策略**：可疑进程置顶（`is_suspicious` desc），其次按 PID 升序

4. **快照时间戳显示**：`ProcessDetail` 顶部显示 `ProcessSnapshot.timestamp` 格式化为 `YYYY/MM/DD HH:MM:SS`

5. **进程已退出处理**：`cmdProcessChain(pid)` 返回空链时，详情面板显示"进程已退出，无法追溯"

6. **刷新策略**：手动刷新按钮重新调用 `cmdProcessSnapshot`，`refetchOnMount: false`（遵循项目约定）

**路由文件（含 `validateSearch`）**：

```typescript
// ui/src/routes/process.tsx
import { createFileRoute } from "@tanstack/react-router";
import { ProcessPage } from "@/features/process/pages/ProcessPage";

export const Route = createFileRoute("/process")({
  component: ProcessPage,
  validateSearch: (search: Record<string, unknown>) => ({
    pid: search.pid ? Number(search.pid) : undefined,
    imagePath: search.imagePath as string | undefined,
  }),
});
```

**Sidebar 导航集成**：

文件：`ui/src/components/layout/Sidebar.tsx`
- 导入补充：`import { ..., Cpu } from "lucide-react";`
- `NAV_ITEMS` 添加：`{ to: "/process", icon: Cpu, i18nKey: "nav.process" }`

**i18n 翻译键**（`ui/src/locales/zh-CN.json` 和 `en-US.json`）：

```json
{
  "nav.process": "进程",
  "process.toolbar.refresh": "刷新",
  "process.toolbar.view.list": "列表",
  "process.toolbar.view.tree": "树",
  "process.toolbar.filter.all": "全部",
  "process.toolbar.filter.suspicious": "可疑",
  "process.toolbar.search.placeholder": "搜索进程...",
  "process.table.pid": "PID",
  "process.table.ppid": "父PID",
  "process.table.name": "名称",
  "process.table.path": "路径",
  "process.table.suspicious": "可疑",
  "process.chain.title": "进程链",
  "process.chain.empty": "进程已退出，无法追溯",
  "process.detail.select-row": "选择一个进程查看详情",
  "process.detail.snapshot-time": "快照时间"
}
```

### 3.3 二期：三向关联

#### 3.3.1 关联数据获取策略

**决策：核心查询逻辑放在 crate 层，Tauri 命令仅为薄包装，确保 egui fallback 可复用。**

理由：
- 初版方案依赖 `useNetworkStore` / autoruns 缓存，若用户未访问过对应页面则关联失效
- Review 已正确指出此问题
- 项目已有 `irtool-monitor` 引擎将网络事件持久化到 SQLite，可复用查询
- 项目需支持 egui fallback，核心逻辑不能耦合在 Tauri 层

**分层原则**：

| 层 | 职责 | 位置 |
|---|---|---|
| 核心逻辑 | 查询函数本身 | `irtool-monitor` 提供 `query_connections_by_pid(pid)`，`irtool-autoruns` 提供 `query_items_by_path(exe_path)` |
| Tauri 适配 | 薄包装，调核心函数 | `irtool-tauri/commands/process.rs` 只做参数转换 + `spawn_blocking` |
| egui 适配 | 调同一核心函数 | egui 页面直接调 crate 层函数，或降级为简化版 |

**新增核心层查询函数**：

```rust
// crates/irtool-monitor/src/ (或 irtool-net-monitor)
/// 按 PID 查询网络连接（从 SQLite 历史或实时连接表）
pub fn query_connections_by_pid(pid: u32) -> Result<Vec<NetConn>, IrError> { ... }

// crates/irtool-autoruns/src/
/// 按 exe 路径查询持久化条目
pub fn query_items_by_path(exe_path: &str) -> Result<Vec<AutorunItem>, IrError> { ... }
```

**Tauri 命令（薄包装）**：

```rust
// crates/irtool-tauri/src/commands/process.rs

#[tauri::command]
pub async fn cmd_network_query_by_pid(
    ctx: State<'_, AppContext>,
    pid: u32,
) -> Result<Vec<NetConn>, IrError> {
    let rt = ctx.rt.lock().await;
    rt.spawn_blocking(move || irtool_monitor::query_connections_by_pid(pid))
        .await
        .map_err(|e| IrError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn cmd_autoruns_query_by_path(
    ctx: State<'_, AppContext>,
    exe_path: String,
) -> Result<Vec<AutorunItem>, IrError> {
    let rt = ctx.rt.lock().await;
    rt.spawn_blocking(move || irtool_autoruns::query_items_by_path(&exe_path))
        .await
        .map_err(|e| IrError::Internal(e.to_string()))?
}
```

**egui fallback 策略**：
- 一期（进程列表+树+链）：核心逻辑全在 crate 层，egui 可完整实现
- 二期（三向关联）：关联查询在 crate 层，egui 可实现但 UI 复杂度较高。若成本过高可降级为简化版（只展示关联数据，不做跨页面跳转）

#### 3.3.2 关联面板设计

`ProcessDetail` 增加标签页：

```
┌─────────────────────────────────────────────────────┐
│ ProcessDetail                                        │
│ [进程链] [网络连接] [持久化条目]  ← 标签切换        │
├─────────────────────────────────────────────────────┤
│ 进程链: PID 5678 → 4321 → 1234 → System             │
│   每个节点: 名称/路径/命令行/可疑标记               │
│                                                     │
│ 网络连接 (调用 cmd_network_query_by_pid):            │
│   协议  本地          远程           状态    时间   │
│   TCP  192.168.1.5:443 1.2.3.4:80    ESTABLISHED ... │
│                                                     │
│ 持久化 (调用 cmd_autoruns_query_by_path):            │
│   类别    条目         位置          操作            │
│   Logon  payload.exe   HKLM\...\Run  [跳转]         │
└─────────────────────────────────────────────────────┘
```

#### 3.3.3 路径归一化

```typescript
// ui/src/features/process/utils.ts
function normalizePath(p: string): string {
  return p.toLowerCase().replace(/\//g, "\\").replace(/\\+/g, "\\");
}

function pathsMatch(a: string | null, b: string | null): boolean {
  if (!a || !b) return false;
  return normalizePath(a) === normalizePath(b);
}
```

**不处理**：短路径（`C:\PROGRA~1`）— 罕见，autorunsc 和 `query_exe_path` 均返回长路径。

#### 3.3.4 交叉跳转

| 来源 | 目标 | 触发 | 实现 |
|------|------|------|------|
| 进程详情 | 网络监控 | "查看网络连接"按钮 | `router.navigate({ to: "/network", search: { pid } })` |
| 进程详情 | 持久化 | "查看持久化条目"按钮 | `router.navigate({ to: "/autoruns", search: { imagePath } })` |
| 持久化详情 | 进程 | "查看运行状态"按钮 | `router.navigate({ to: "/process", search: { pid } })` |
| 网络详情 | 进程链 | "查看进程链"按钮 | `router.navigate({ to: "/process", search: { pid } })` |

**目标页面需同步增加 `validateSearch`** 并在 `useEffect` 中处理参数：
- `routes/network.tsx`：增加 `pid` search param
- `routes/autoruns.tsx`：增加 `imagePath` search param

#### 3.3.5 应急响应闭环

```
扫描持久化（autoruns）
    → 发现可疑条目
    → 查看运行状态（跳转进程页，自动选中匹配进程）
    → 查看进程链（追溯来源，判断入侵入口）
    → 查看网络连接（判断是否 C2 通信）
    → 删除持久化条目 + 结束进程
```

---

## 四、关键实现方向与注意事项

### 4.1 后端关键点

1. **`take_snapshot_enriched` 性能**：500 进程逐个 `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` + `QueryFullProcessImageNameW`，单次约 0.1~0.5ms，总计 50~250ms。**必须用 `spawn_blocking`** 避免阻塞 Tauri 异步运行时（现有 `cmd_process_snapshot` 已是此模式）。

2. **`query_exe_path` 权限**：`PROCESS_QUERY_LIMITED_INFORMATION` 不需要管理员权限即可查询大部分进程，但部分受保护进程（如部分系统服务）会失败，返回 `None` 即可，不报错。

3. **`check_suspicious` 调用时机**：在 `take_snapshot_enriched` 中对每个进程调用，规则简单（字符串匹配），性能影响可忽略。

4. **bindings 重新生成**：扩展 `ProcessEntry` 后必须重新生成 `bindings.ts`，否则前端类型不匹配。

### 4.2 前端关键点

1. **进程树性能**：500 节点递归构建树结构，前端 O(n) 算法无压力。但深层递归渲染需注意 React 调和性能，建议树视图默认折叠非可疑分支。

2. **`refetchOnMount: false`**：遵循项目约定，防止组件重新挂载时重复请求。

3. **`detailPosition` 配置**：复用 `useUIStore` 的 `detailPositions` 机制，支持用户切换详情面板位置（右/下），与 network/autoruns 一致。

4. **进程链加载状态**：`cmdProcessChain` 含 WMI 查询（查命令行），可能较慢（200~2000ms），需展示 loading 态。

5. **URL 参数处理**：`ProcessPage` 中用 `Route.useSearch()` 获取 `pid` / `imagePath`，在 `useEffect` 中调用 `store.setSelectedPid(pid)`。注意 `pid` 可能在快照中不存在（进程已退出），需友好提示。

6. **标签不可选中**：遵循项目 UI 约定，所有 label 文本设置 `user-select: none`。

7. **时间格式**：快照时间戳显示用 `YYYY/MM/DD HH:MM:SS`（项目约定）。

### 4.3 注意事项

1. **`PcapEvent` 无 PID 的限制**：TLS SNI / DNS 事件无法关联到进程，只有 `NetConn`（TCP/UDP 连接表）含 PID。二期网络关联仅限连接表数据，需在 UI 上明确标注此限制，避免用户误解。

2. **进程快照是时间点数据**：快照后进程可能立即退出或新建，`cmdProcessChain(pid)` 对已退出进程返回空链。UI 需处理此场景，显示"进程已退出"。

3. **`SuspiciousFlag` 扩展性**：当前仅 2 条规则，未来可扩展签名验证、进程注入检测等。`SuspiciousFlag` 枚举已具备 `serde` 序列化能力，扩展时同步更新 `reason()` 即可。

4. **二期独立命令的 SQLite 查询**：`cmd_network_query_by_pid` 需确认 `irtool-monitor` 的 SQLite schema 是否支持按 PID 查询历史连接。若不支持，需先扩展存储层。

5. **路径归一化边界**：Windows 路径大小写不敏感但保留原始大小写，`normalizePath` 统一转小写比较即可。注意 UNC 路径（`\\server\share`）和设备路径（`\\.\C:`）的兼容性，一期可暂不处理。

6. **不要过度设计**：一期聚焦"超越任务管理器的进程视角"，二期聚焦"三向关联闭环"。ETW 实时监控、签名验证、进程注入检测等高级能力不在当前范围。

---

## 五、实施清单

### 一期

**后端**：
- [ ] 扩展 `ProcessEntry` 增加 `is_suspicious` / `suspicious_reason` 字段（`crates/irtool-process/src/types.rs`）
- [ ] `query_exe_path` 改为 `pub(crate) fn`（`crates/irtool-process/src/chain.rs:86`）
- [ ] 新增 `take_snapshot_enriched()` 函数（`crates/irtool-process/src/snapshot.rs`）
- [ ] `cmd_process_snapshot` 切换到 `take_snapshot_enriched`（`crates/irtool-tauri/src/commands/process.rs:8`）
- [ ] 重新生成 bindings（`ui/src/lib/bindings.ts`）
- [ ] 验证：`cargo check --workspace` + `cargo fmt --all` + `cargo clippy`

**前端**：
- [ ] 创建 `ui/src/features/process/` 目录结构（pages/components/columns/api/hooks/store/types）
- [ ] 实现 `api.ts` / `hooks.ts` / `store.ts` / `types.ts`
- [ ] 实现 `ProcessTable`（列表视图，含可疑标记、筛选、搜索、默认排序）
- [ ] 实现 `ProcessTreeView`（树视图，孤儿进程标黄、可疑标红）
- [ ] 实现 `ProcessDetail`（进程链展示、快照时间戳、空链处理）
- [ ] 实现 `ProcessToolbar`（刷新 + 列表/树切换 + 筛选 + 搜索）
- [ ] 实现 `ProcessPage`（左右分栏，复用 `detailPosition` 机制）
- [ ] 创建 `routes/process.tsx`（含 `validateSearch`）
- [ ] Sidebar 添加 `Cpu` 图标导入和导航项
- [ ] i18n 补充 `zh-CN.json` 和 `en-US.json` 翻译键
- [ ] 验证：`npx tsc --noEmit` + `pnpm lint`

### 二期

**后端（crate 层）**：
- [ ] `irtool-monitor`（或 `irtool-net-monitor`）新增 `query_connections_by_pid(pid) -> Vec<NetConn>`
- [ ] `irtool-autoruns` 新增 `query_items_by_path(exe_path) -> Vec<AutorunItem>`
- [ ] 确认/扩展 `irtool-monitor` SQLite schema 支持按 PID 查询

**后端（Tauri 适配层）**：
- [ ] 新增 `cmd_network_query_by_pid`（薄包装，调 crate 层函数）
- [ ] 新增 `cmd_autoruns_query_by_path`（薄包装，调 crate 层函数）
- [ ] 重新生成 bindings

**前端**：
- [ ] `ProcessDetail` 增加标签页（进程链 / 网络连接 / 持久化条目）
- [ ] 实现网络连接关联（调用 `cmd_network_query_by_pid`）
- [ ] 实现持久化关联（调用 `cmd_autoruns_query_by_path` + 路径归一化）
- [ ] `routes/network.tsx` 增加 `pid` 的 `validateSearch`
- [ ] `routes/autoruns.tsx` 增加 `imagePath` 的 `validateSearch`
- [ ] `NetworkDetail` 增加"查看进程链"按钮
- [ ] `AutorunsDetail` 增加"查看运行状态"按钮
- [ ] 验证：端到端关联流程测试（持久化→进程→网络→进程链闭环）

---

## 六、与初版/Review 的差异总结

| 项目 | 初版方案 | Review 建议 | 最终方案 |
|------|---------|------------|---------|
| 后端改动表述 | 自相矛盾 | 纠正为"需要改动" | 明确列出 4 项改动 |
| `image_path` 行号 | 52（错误） | 45（正确） | 45 |
| `query_exe_path` 可见性 | 未提及 | 改 `pub(crate)` | 采纳 |
| `columns.tsx` | 遗漏 | 补充 | 采纳 |
| 页面布局 | 上下分栏 | 左右分栏 | 采纳左右分栏 |
| 进程树视图 | 无 | 建议一期加入 | 采纳，一期含列表/树切换 |
| 刷新策略 | 未讨论 | 手动刷新+时间戳 | 采纳 |
| 虚拟化 | 未讨论 | 评估引入 | 不引入（500 行无需） |
| 二期关联数据 | 依赖缓存 | 独立 API | 采纳独立 API |
| URL 参数 | 未提 `validateSearch` | 补充 | 采纳 |
| ETW 实时监控 | 无 | 二期补充项 | 不做（场景不适配） |
| i18n 文件 | 未指明 | 指明具体文件 | 采纳 |
| Sidebar 图标 | 未提导入 | 补充 `Cpu` 导入 | 采纳 |
| 默认排序 | 未讨论 | 建议可疑置顶 | 采纳 |
