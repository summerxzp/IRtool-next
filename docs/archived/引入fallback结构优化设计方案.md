结合你的实际情况：

- 已经有 Tauri + React
- 目标是 IR/DFIR 工具
- 需要便携版
- 需要兼容 Server 2016 / 无 WebView2 环境
- 希望长期维护成本可控
- 未来可能接入更多工具（Sysmon、Autoruns、Hayabusa、Chainsaw 等）

我建议采用下面这套架构作为正式方案。

------

# 一、总体架构

```text
IRTool

                    ┌─────────────────┐
                    │ irtool-models   │
                    └────────┬────────┘
                             │

                    ┌────────▼────────┐
                    │ irtool-config   │
                    └────────┬────────┘
                             │

                    ┌────────▼────────┐
                    │ irtool-platform │
                    └────────┬────────┘
                             │

                    ┌────────▼────────┐
                    │ irtool-services │
                    └────────┬────────┘
                             │

         ┌───────────────────┼───────────────────┐
         │                   │                   │

 ┌───────▼───────┐   ┌───────▼───────┐   ┌───────▼───────┐
 │ irtool-tauri  │   │ irtool-egui   │   │ irtool-cli    │
 └───────────────┘   └───────────────┘   └───────────────┘
```

核心原则：

```text
业务逻辑永远不依赖UI

UI依赖业务

业务不知道自己被谁调用
```

------

# 二、Workspace目录结构

推荐直接这样定版：

```text
irtool/

Cargo.toml

crates/

├── irtool-models/
│
├── irtool-config/
│
├── irtool-platform/
│
├── irtool-services/
│
├── irtool-tauri/
│
├── irtool-egui/
│
└── irtool-cli/
```

------

# 三、各模块职责

------

## irtool-models

只放数据结构

例如：

```rust
pub struct PersistenceItem;

pub struct NetworkConnection;

pub struct SysmonEvent;

pub struct AlertRecord;
```

禁止：

```rust
fn scan()
fn query()
fn save()
```

------

作用：

```text
统一DTO

避免前端自己定义结构

避免序列化不一致
```

------

## irtool-config

统一配置管理

例如：

```rust
pub struct AppConfig;

pub fn load();

pub fn save();
```

配置：

```text
config/

settings.toml

monitor.toml

sysmon.xml
```

------

禁止：

```text
Tauri自己读配置

egui自己读配置
```

------

统一：

```rust
config::load()
config::save()
```

------

## irtool-platform

平台相关能力

例如：

```rust
Registry

ETW

Sysmon

WMI

WinVerifyTrust

Windows Service

Task Scheduler
```

------

示例：

```rust
pub struct RegistryProvider;

impl RegistryProvider {
    pub fn query_run_keys();
}
```

------

特点：

```text
只做能力封装

不做业务判断
```

------

## irtool-services

真正的业务层

例如：

```rust
PersistenceService

NetworkService

MonitorService

SysmonService

ToolManagerService
```

------

例如：

```rust
pub struct PersistenceService;

impl PersistenceService {
    pub fn scan() -> Vec<PersistenceItem>;
}
```

------

这里负责：

```text
关联分析

过滤

规则判断

风险评级
```

------

# 四、ToolManager设计

建议单独模块：

```text
ToolManagerService
```

负责：

```text
检测工具

下载工具

验证签名

解压

版本记录

更新
```

------

目录：

```text
tools/

manifest.json

autoruns/
sigcheck/
sysmon/
```

------

验证方式：

```rust
enum VerifyMethod {
    Authenticode,
    Sha256,
    None,
}
```

当前：

```text
Sysinternals

默认使用

Authenticode
```

------

# 五、Tauri层

职责：

```text
界面展示

用户交互

调用Service
```

禁止：

```rust
直接访问数据库

直接解析Sysmon

直接调用Autoruns
```

------

示例：

```rust
#[tauri::command]
async fn scan_persistence()
{
    PersistenceService::scan()
}
```

------

# 六、egui Emergency Mode

定位：

```text
备用UI

不是第二套完整版UI
```

------

目标：

```text
Autoruns

TCPView

Process Explorer

Everything
```

这种风格。

------

支持：

```text
持久化检测

网络连接

Sysmon查询

IOC搜索

日志导出
```

------

不追求：

```text
动画

复杂图表

高级仪表盘

拖拽布局
```

------

UI建议：

```text
Top Toolbar

SearchBox

TableView

Detail Panel

Status Bar
```

即可。

------

# 七、CLI

不要省略。

建议保留：

```text
irtool-cli.exe
```

------

用途：

```text
Server Core

计划任务

SOAR

自动化

批量分析
```

------

示例：

```powershell
irtool-cli persistence

irtool-cli network

irtool-cli export

irtool-cli collect
```

------

很多企业最终反而会大量使用CLI。

------

# 八、事件系统

不要把：

```rust
app.emit(...)
```

写进业务层。

------

定义：

```rust
pub trait EventSink {
    fn emit(
        &self,
        event: &str,
        payload: &str,
    );
}
```

------

实现：

```rust
TauriEventSink

EguiEventSink

CliEventSink
```

------

业务层：

```rust
service.emit(...)
```

即可。

------

# 九、便携版目录

最终建议：

```text
IRTool.exe

README.md

LICENSE

首次运行自动创建

config/
data/
logs/
tools/
```

------

运行后：

```text
config/
├── settings.toml
├── monitor.toml
└── sysmon.xml

data/
└── monitor.db

logs/
├── app.log
├── monitor.log
└── tools.log

tools/
├── manifest.json
├── autoruns/
├── sigcheck/
└── sysmon/
```

------

# 十、发布策略

发布三个产物：

### 主版本

```text
IRTool.exe
```

Tauri

适合：

```text
Win10
Win11
分析机
```

------

### Emergency

```text
IRTool-Emergency.exe
```

egui

适合：

```text
Server2016

无WebView2

离线环境
```

------

### CLI

```text
IRTool-CLI.exe
```

适合：

```text
自动化

脚本

Server Core
```

------

# 十一、开发优先级（重要）

不要立刻开发 egui 页面。

顺序应该是：

```text
第一阶段
-------------
解耦

models
config
platform
services

第二阶段
-------------
Tauri改为调用services

第三阶段
-------------
CLI

第四阶段
-------------
egui Emergency Mode

第五阶段
-------------
工具下载管理器
```

原因：

```text
解耦完成后

CLI和egui开发难度都会大幅下降

否则后面一定返工
```

如果按这套方案执行，IRTool 后续无论是接 Hayabusa、Chainsaw、Velociraptor，还是增加 Web API、Agent 模式，基本都不需要再做架构级重构，只是在 `services` 和 `platform` 层持续扩展能力即可。对于一个长期维护的应急响应工具，这是比较稳妥且可扩展的路线。