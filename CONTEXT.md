# IRtool-next 领域术语表

## 核心概念

### 工作台 (Workspace)
处置工作台。以"发现可疑 → 处置可疑"为核心流程的聚合页面。工作台是纯数据消费者，不触发扫描或采集，只对已有数据做搜索、规则匹配和关联发现。处置操作以原生 API 优先，命令模板兜底。

### 处置 (Disposal)
对可疑项执行的操作动作。包括：kill 进程、删除持久化项、跳转注册表、打开路径、取消隐藏属性、获取所有权、加密压缩取样等。操作按钮放在详情面板中，以下拉菜单 + 执行按钮形式呈现，命令预览以 tooltip 显示。

### 规则扫描 (Rule Scan)
基于用户配置的恶意特征规则，对已有数据进行匹配扫描。规则按数据源类型（RuleTarget）分类：Autorun / Network / Event，各自匹配对应数据源的结果。不做跨数据源组合规则。

### 关联发现 (Association)
选中一条记录后，基于严格匹配键（PID、进程路径、IP 地址）在其他数据源中查找相关记录。中等关联（同目录、同签名）作为可选扩展，不做宽松关联（时间窗口）。手动点击"关联发现"按钮触发，结果在详情面板"关联记录"折叠区域展示，按数据源类型分组显示摘要，点击跳转对应 tab。

### 取样 (Sampling)
加密压缩可疑文件/目录用于取证。默认打包"所在目录"，密码默认 "1"（可在设置页配置），取样成功后 toast 提示。"打开路径"指在资源管理器中打开进程文件所在目录。

### 命令模板 (Command Template)
当原生 Rust API 无法覆盖的处置操作，通过拼装 PowerShell/cmd 命令执行。作为兜底方案，不优先使用。

## 数据源类型

### 持久化项 (Autorun Item)
来自 autoruns 扫描的持久化条目。关键字段：entry, image_path, category, location, signature, sha256。典型操作：删除、跳转注册表、打开路径、取消隐藏、获取所有权、取样。

### 网络连接 (Network Connection)
来自网络监控的实时/历史连接记录。关键字段：local/remote addr, pid, process_name, process_path。典型操作：kill 进程、打开路径。

### 事件 (Event)
来自 Sysmon/DNS/SNI 等日志采集的事件记录。关键字段：event_type, timestamp, process_path, destination。主要用于搜索发现，操作较少。事件数据仅存于前端 Zustand store（上限 10000 条），后端无内存存储。

## 工作台数据访问

工作台通过分别调用各专题页面的现有 Tauri 命令获取数据，前端统一过滤：
- 持久化项：`cmd_autoruns_get_result` → 前端过滤
- 网络连接：`cmd_network_snapshot` → 前端过滤
- 日志事件：直接读前端 `useLogCollectorStore` → 前端过滤

不新增统一搜索命令。原因：日志事件后端无存储，统一命令搜不到；数据量小（百~千级），前端过滤与后端过滤性能差异可忽略；复用现有命令更简单。

## 规则模型

### RuleTarget
规则匹配的数据源类型枚举：Autorun / Network / Event。每条规则声明其 target，扫描时只匹配对应数据源。

### Condition
规则的单个匹配条件。包含 field（字段名）、type（匹配类型：contains/regex/equals）、value（匹配值）。新版暂不包含 hash 匹配条件，收益不大。

### Severity
规则严重级别：critical / high / medium / low。

### Family
规则所属家族/分类（如：银狐、APT37、黑猫）。

## 规则引擎实现

- 规则存储：JSON 文件（与原版兼容，支持 IOC 导入/导出）
- 规则扫描：前端执行。规则加载到前端，对已获取的数据做 JS 过滤（contains/regex/equals）
- 不新增后端扫描命令。原因：数据已在前端，规则条件简单，JS 完全能做，避免 IPC 开销
- 新版暂不包含 hash 规则，收益不大

### 各 RuleTarget 可匹配字段

**Autorun：** entry, image_path, launch_string, location, publisher, description
**Network：** remote.addr（支持 CIDR）, remote.port, process_name, process_path
**Event：** query_name, destination_ip, process_path, event_type

## 处置操作实现

### 已有原生 API（直接复用）
- kill 进程：`irtool-net-monitor::kill_process`
- 删除持久化项：`irtool-autoruns::delete_entry`
- 打开资源管理器：`irtool-autoruns::open_in_explorer`
- 跳转注册表：`irtool-autoruns::open_regedit`
- 打开服务管理器：`irtool-autoruns::open_services_msc`

### P3 阶段用命令模板实现
- 取消隐藏属性：`attrib -h -s <path>`
- 强制获取所有权：`takeown /f <path>` + `icacls <path> /grant admins:F`
- 加密压缩取样：`7z a -p1 <output> <dir>`

命令模板通过通用 `cmd_workspace_run_command(program, args)` 执行，前端拼好参数传给后端，后端执行并返回输出。不为每个操作写单独命令。前端需确保参数拼接正确。

后续逐步迁移到原生 Rust Win32 API，不影响前端接口。

## 页面职责边界

### 工作台页面
- 跨数据源关键字搜索 + 规则扫描 + 关联发现 + 处置操作
- 不触发扫描/采集，只消费已有数据
- 结果按数据源类型分 tab 展示，顺序：持久化项 → 网络连接 → 事件（按处置价值排序）
- 初始空状态提示"请先在各专题页面完成扫描/采集"，提供"刷新数据"按钮主动拉取
- 搜索/扫描结果替换上次结果，不叠加
- 切换 tab 时保留各 tab 的选中状态

### 数据库检索页面
- 专为后台监测场景设计
- 细粒度查询历史事件（按来源、事件类型、进程名、IP/域名等）
- 与工作台独立，服务不同场景

### 各专题页面（网络监控/持久化检测/日志采集）
- 负责数据采集和扫描触发
- 工作台消费这些页面产生的数据

## 架构决策

### 权限模型：默认请求管理员
Manifest 声明 `requireAdministrator`，启动即提权。原因：Sysmon 安装、Autoruns 删除条目等核心操作均需管理员权限，运行中弹 UAC 或重启应用会丢失内存数据（事件、网络历史等），打断工作流。非管理员环境下各模块降级运行，功能受限但不崩溃。

### 运行时依赖：系统 WebView2
依赖用户系统已安装的 WebView2 Runtime（Win10/11 通常预装），不嵌入 Fixed Runtime（+150MB）也不做 Bootstrapper 自动安装。便携版体积优先，缺失时弹错误提示并退出。NSIS 安装包同理，不在安装流程中捆绑 WebView2。

### 便携版设计：portable.flag 双模式
`portable.flag` 文件存在于 exe 同目录 → 便携模式，所有数据（config/data/logs/tools）存放在 exe 目录下；不存在 → 安装模式，数据存 `%APPDATA%/IRtool/`。首次启动自动创建子目录。设置存储用 `tauri-plugin-store`（JSON 文件）而非 localStorage，确保便携版数据随 exe 移动。

### 外部工具：Bootstrap Installer 模式
不内置/二次分发 Sysinternals 二进制（合规），首次启动检测缺失工具并引导下载。下载后做 Authenticode 签名验证（WinVerifyTrust + CertFindCertificateInStore），验证失败则删除已解压文件。EULA 自动接受（`-accepteula`）。支持离线 ZIP 导入。工具版本由内嵌 `manifest.json` 定义，`tools/manifest.json` 仅记录已安装版本。

### Sysmon 中文路径：8.3 短路径 + TEMP 回退
Sysmon64.exe 的 `-i`/`-c` 参数不支持中文路径。先用 `GetShortPathNameW` 转 8.3 短路径；若短路径仍含非 ASCII（如 8.3 被禁用），则复制 sysmon.xml 到 `%TEMP%\irtool-sysmon.xml` 再安装。

### 日志拆分
三个独立滚动日志：`irtool-app.log`（全局）、`irtool-monitor.log`（监测引擎）、`irtool-tools.log`（工具管理）。每日轮转，保留 7 天。Release 构建无控制台输出。

### Commit 规范

- Commit message 不添加 Co-Authored-By，仅使用作者 summerxzp

### 版本号管理规范

**单一来源原则**：版本号统一在 `Cargo.toml` 的 `workspace.package.version` 中维护。

| 文件 | 说明 |
|------|------|
| `Cargo.toml` | **唯一版本来源**，修改此文件 |
| `ui/package.json` | 运行 sync-version.js 同步 |
| `crates/irtool-tauri/tauri.conf.json` | 运行 sync-version.js 同步 |
| `crates/irtool-tauri/irtool.manifest` | 运行 sync-version.js 同步 |
| `README.md` 版本徽章 | 运行 sync-version.js 同步 |
| 前端左上角版本号 | 运行时从 API 自动获取，无需维护 |

**发布流程**：
1. 修改 `Cargo.toml` 中的 `version`
2. 运行 `node scripts/sync-version.js` 同步到其他文件
3. 提交并打 tag

**前端获取版本号**：通过 `cmdAppInfo()` API 从后端获取，使用 `env!("CARGO_PKG_VERSION")` 自动读取 Cargo.toml 版本。

## 主题默认浅色
新用户默认浅色主题。深色主题可选。持久化到 `tauri-plugin-store`。
