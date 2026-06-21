# Autoruns 持久化条目删除指南

本文档记录 autorunsc 输出的各类持久化条目的数据结构、删除逻辑、已知问题及解决方案。

## 章节索引

- [autorunsc CSV 输出字段](#autorunsc-csv-输出字段)
- [持久化条目类别与删除逻辑](#持久化条目类别与删除逻辑)
  - [1. Services（服务）](#1-services服务)
  - [2. Drivers（驱动）](#2-drivers驱动)
  - [3. Scheduled Tasks（计划任务）](#3-scheduled-tasks计划任务)
  - [4. Logon（登录项）](#4-logon登录项)
    - [4a. Active Setup\Installed Components](#4a-active-setupinstalled-components)
  - [5. Winlogon](#5-winlogon)
    - [5a. LSA_MULTI_SZ（Userinit, Shell, Taskman, System）](#5a-lsa_multi_szuserinit-shell-taskman-system)
    - [5b. GpExtensions / Notify](#5b-gpextensions--notify)
    - [5c. Credential Provider Filters](#5c-credential-provider-filters)
  - [6. Internet Explorer（IE 加载项）](#6-internet-explorerie-加载项)
    - [Browser Helper Objects (BHO)](#browser-helper-objects-bho)
  - [7. Explorer（Shell 扩展）](#7-explorershell-扩展)
  - [8. Boot Execute](#8-boot-execute)
  - [9. AppInit DLLs](#9-appinit-dlls)
  - [10. Image Hijacks（IFEO）](#10-image-hijacksifeo)
  - [11. LSA Security Packages](#11-lsa-security-packages)
  - [12. Known DLLs](#12-known-dlls)
  - [13. Winsock Providers](#13-winsock-providers)
  - [14. WMI](#14-wmi)
  - [15. Print Monitors](#15-print-monitors)
  - [16. Office / COM](#16-office--com)
  - [17. Generic（Logon、Explorer、Codecs 等）](#17-genericlogonexplorercodecs-等)
- [删除结果处理](#删除结果处理)
  - [假删除问题（已修复）](#假删除问题已修复)
  - [\(Default) 默认值处理（新增）](#default-默认值处理新增)
  - [权限不足错误处理（新增）](#权限不足错误处理新增)
  - [schtasks stderr 编码处理（新增）](#schtasks-stderr-编码处理新增)
  - [CLSID 提取（已优化）](#clsid-提取已优化)
  - [子键枚举匹配方案（新增）](#子键枚举匹配方案新增)
  - [日志记录](#日志记录)
- [测试要点](#测试要点)

## autorunsc CSV 输出字段

autorunsc 输出的 CSV 包含以下字段（由 `csv_parser.rs` 解析为 `RawEntry`）：

| 字段 | CSV 列名 | 说明 |
|------|---------|------|
| `location` | Entry Location | 注册表路径或标识（如 `HKLM\...`、`Task Scheduler`） |
| `entry` | Entry | 条目名（可能是值名、描述名、CLSID、服务名等） |
| `enabled` | Enabled | 启用状态 |
| `category` | Category | 类别（如 Logon、Services、Drivers） |
| `description` | Description | 描述 |
| `publisher` | Company | 发布者 |
| `image_path` | Image Path | 可执行文件路径 |
| `launch_string` | Launch String | 启动命令（可能包含 CLSID、路径、参数） |
| `timestamp` | Time | 时间戳 |
| `md5` | MD5 | MD5 哈希 |
| `sha256` | SHA-256 | SHA-256 哈希 |
| `signer` | Signer | 签名者 |
| `version` | Version | 版本 |

## 持久化条目类别与删除逻辑

### 1. Services（服务）

**数据结构**：
- `location`: `HKLM\System\CurrentControlSet\Services`（不含服务名）
- `entry`: 服务名（如 `edgeupdate`、`edgeupdatem`）
- `service_name`: 从 `entry` 提取（与 `entry` 相同）

**删除逻辑**（`delete_service`）：
1. `OpenSCManagerW` 打开 SCM
2. `OpenServiceW` 打开服务（SERVICE_DELETE | SERVICE_STOP）
3. 如果服务存在：`DeleteService` 删除
4. 如果服务不存在（`ERROR_SERVICE_DOES_NOT_EXIST = 0x80070424`）：fallback 到删除注册表项 `SYSTEM\CurrentControlSet\Services\{service_name}`

**已知问题**：
- ❌ ~~服务名从 exe 路径提取导致错误~~（已修复）：多个服务可能共享同一 exe（如 `edgeupdate` 和 `edgeupdatem` 都用 `MicrosoftEdgeUpdate.exe`），必须用 `entry` 字段
- ✅ 服务已卸载但注册表残留时，fallback 删除注册表项

### 2. Drivers（驱动）

**数据结构**：
- `location`: `HKLM\System\CurrentControlSet\Services`（不含服务名）
- `entry`: 驱动服务名（如 `scfilter`）
- `service_name`: 从 `entry` 提取

**删除逻辑**：同 Services，调用 `delete_service`

**已知问题**：
- ⚠️ 系统核心驱动（如 `scfilter`）需要 SYSTEM 权限，管理员权限不足，返回"拒绝访问"（预期行为）

### 3. Scheduled Tasks（计划任务）

**数据结构**：
- `location`: `Task Scheduler`
- `entry`: 任务路径（如 `\MicrosoftEdgeUpdateTaskMachineUA`）

**删除逻辑**（`delete_scheduled_task`）：
1. 调用 `schtasks /delete /tn "{entry}" /f` 删除
2. 失败时用 `encoding_rs::GBK` 解码 stderr（schtasks 输出使用系统 ANSI 编码）

**已知问题**：
- ⚠️ 受 Windows 保护的系统任务（如 `\Microsoft\Windows\UpdateOrchestrator\*`）即使管理员权限也会被拒绝，返回"拒绝访问"（预期行为）
- ❌ ~~stderr 乱码~~（已修复）：schtasks 输出 GBK 编码，用 `encoding_rs::GBK` 解码

### 4. Logon（登录项）

#### 4a. Active Setup\Installed Components

**数据结构**：
- `location`: `HKLM\SOFTWARE\Microsoft\Active Setup\Installed Components`（父键路径）
- `entry`: 描述名（如 `Microsoft Edge`、`Microsoft Windows Media Player`）
- `launch_string`: 可能包含 CLSID（如 `C:\Windows\System32\ie4uinit.exe,BaseSetup`）
- `image_path`: 可执行文件路径

**注册表结构**：
```
HKLM\SOFTWARE\Microsoft\Active Setup\Installed Components
  └── {CLSID}                    ← 子键名是 CLSID
      ├── StubPath               ← 值名，数据是启动命令
      ├── Version
      └── IsInstalled
```

**删除逻辑**：
1. 检测 `subkey` 包含 `active setup\installed components`
2. 从 `launch_string` / `image_path` / `entry` / `location` 提取 CLSID
3. 如果找到 CLSID：删除子键 `...\Installed Components\{CLSID}`
4. 如果未找到 CLSID：调用 `find_and_delete_subkey_by_value` 遍历子键，读取每个子键的 `StubPath` 值，与 `launch_string`（或 `image_path`）做不区分大小写的包含匹配，找到后删除整个子键

**已知问题**：
- ❌ ~~entry 是描述名而非 CLSID，无法定位子键~~（已修复）：现在从 `launch_string` 和 `image_path` 也提取 CLSID
- ❌ ~~所有字段都不包含 CLSID 时返回"暂不支持自动删除"~~（已修复）：改用子键枚举 + StubPath 值匹配方案
- ⚠️ autorunsc CSV 输出完全不包含 CLSID（实测 Microsoft Edge 条目：`entry`="Microsoft Edge"，`launch_string`=setup.exe 命令，`image_path`=setup.exe 路径，均无 CLSID），CLSID 只存在于注册表子键名中

### 5. Winlogon

#### 5a. LSA_MULTI_SZ（Userinit, Shell, Taskman, System）

**数据结构**：
- `location`: `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon`
- `entry`: 值名（如 `Userinit`、`Shell`）

**注册表结构**：
```
HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon
  ├── Userinit    REG_MULTI_SZ    "C:\Windows\system32\userinit.exe,"
  ├── Shell       REG_MULTI_SZ    "explorer.exe"
  └── Taskman
```

**删除逻辑**（`delete_lsa_multi_sz`）：
1. 读取 MULTI_SZ 值
2. 过滤掉要删除的条目
3. 写回新的 MULTI_SZ

**保护机制**：
- `Shell` 和 `Userinit` 是系统关键值，删除后将无法登录，返回"请手动处理"

#### 5b. GpExtensions / Notify

**数据结构**：
- `location`: `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon\GpExtensions`
- `entry`: 描述名（如 `组策略缓解选项`、`MDM Policy`）
- `image_path` / `launch_string`: DLL 路径（如 `C:\Windows\System32\gpprefcl.dll`）

**注册表结构**：
```
HKLM\...\Winlogon\GpExtensions
  └── {CLSID}                    ← 子键名是 CLSID
      ├── DllName                ← 值名，数据是 DLL 路径
      └── ...
```

**删除逻辑**：
1. 检测 `subkey` 以 `\gpextensions` 或 `\notify` 结尾
2. 调用 `find_and_delete_subkey_by_value` 遍历子键，读取每个子键的 `DllName` 值，与 `launch_string`（或 `image_path`）做包含匹配，找到后删除整个子键

**已知问题**：
- ❌ ~~entry 是描述名，CLSID 在子键名中，无法定位~~（已修复）：改用子键枚举 + DllName 值匹配方案
- ⚠️ autorunsc CSV 输出完全不包含 CLSID（实测"组策略缓解选项"条目：`entry`="组策略缓解选项"，`launch_string`=DLL 路径，均无 CLSID），CLSID 只存在于注册表子键名中

#### 5c. Credential Provider Filters

**数据结构**：
- `location`: `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Authentication\Credential Provider Filters`
- `entry`: 空（`""`）

**删除逻辑**：
- 返回"entry 为空，无法定位删除目标，请手动处理"

### 6. Internet Explorer（IE 加载项）

#### Browser Helper Objects (BHO)

**数据结构**：
- `location`: `HKLM\Software\Microsoft\Windows\CurrentVersion\Explorer\Browser Helper Objects`
- `entry`: 描述名（如 `IEToEdge BHO`）
- `launch_string`: 可能包含 CLSID

**注册表结构**：
```
HKLM\...\Browser Helper Objects
  └── {CLSID}                    ← 子键名是 CLSID
      └── (默认值) = 描述名
```

**删除逻辑**（`delete_ie_addon`）：
1. 从 `item` 提取 CLSID（尝试 location、entry、launch_string、image_path）
2. 如果找到 CLSID：
   - 删除 BHO 子键 `...\Browser Helper Objects\{CLSID}`
   - 删除 CLSID 类注册 `SOFTWARE\Classes\CLSID\{CLSID}`
3. 如果未找到 CLSID：调用 `find_and_delete_subkey_by_value` 遍历子键，读取每个子键的默认值（描述名），与 `entry` 做不区分大小写的包含匹配，找到后删除整个子键

**已知问题**：
- ❌ ~~entry 是描述名，CLSID 在子键名中，无法定位~~（已修复）：现在从 `launch_string` 和 `image_path` 也提取 CLSID
- ❌ ~~所有字段都不包含 CLSID 时 fallback 到删除注册表值（必然失败）~~（已修复）：改用子键枚举 + 默认值匹配方案
- ⚠️ autorunsc CSV 输出完全不包含 CLSID（实测 IEToEdge BHO 条目：`entry`="IEToEdge BHO"，`launch_string`=DLL 路径，`image_path`=DLL 路径，均无 CLSID），CLSID 只存在于注册表子键名中

### 7. Explorer（Shell 扩展）

**数据结构**：
- `location`: 注册表路径
- `entry`: 可能是 CLSID 或描述名

**删除逻辑**（`delete_explorer_addon`）：
1. 从 `item` 提取 CLSID
2. 如果找到 CLSID：删除子键 `...\{CLSID}` + CLSID 类注册
3. 如果未找到 CLSID：fallback 到删除注册表值

### 8. Boot Execute

**数据结构**：
- `location`: `HKLM\System\CurrentControlSet\Control\Session Manager`
- `entry`: DLL 文件名

**删除逻辑**（`delete_boot_execute`）：
1. 读取 `BootExecute` MULTI_SZ 值
2. 过滤掉要删除的条目
3. 写回新的 MULTI_SZ

### 9. AppInit DLLs

**数据结构**：
- `location`: `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Windows`
- `entry`: DLL 文件名

**删除逻辑**（`delete_appinit_dlls`）：
1. 读取 `AppInit_DLLs` MULTI_SZ 值
2. 过滤掉要删除的条目
3. 写回新的 MULTI_SZ

### 10. Image Hijacks（IFEO）

**数据结构**：
- `location`: `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options\{exe}` 或 `HKLM\SOFTWARE\Classes\{filetype}\Shell\Open\Command\(Default)`
- `entry`: 值名（如 `Debugger`）或空（`\(Default)` 情况）

**删除逻辑**：
1. 如果 `location` 以 `\(Default)` 结尾：分离出真正的 subkey，调用 `delete_registry_default_value` 删除默认值（用 `PCWSTR::null()`）
2. 如果 `entry` 为空：返回"IFEO 项 entry 为空，无法定位子键，请手动处理"（安全检查，防止误删父键）
3. 否则：删除整个子键 `...\Image File Execution Options\{entry}`

**已知问题**：
- ❌ ~~`\(Default)` 被当作子键路径的一部分，导致删除不存在的子键~~（已修复）：统一在 `delete_entry_inner` 中检测 `\(Default)` 后缀，分离出真正的 subkey 并删除默认值
- ❌ ~~`entry` 为空时构造 `parent_subkey\` 路径，可能误删父键~~（已修复）：`delete_ifeo` 增加 `entry` 为空的安全检查

### 11. LSA Security Packages

**数据结构**：
- `location`: `HKLM\System\CurrentControlSet\Control\Lsa`
- `entry`: 包名（数据中的条目，非值名）

**删除逻辑**（`delete_lsa_multi_sz_at`）：
1. 读取 `Security Packages` MULTI_SZ 值
2. 过滤掉 `entry` 对应的包名
3. 写回新的 MULTI_SZ

### 12. Known DLLs

**数据结构**：
- `location`: `HKLM\System\CurrentControlSet\Control\Session Manager`
- `entry`: DLL 文件名

**删除逻辑**（`delete_lsa_multi_sz_at`）：
1. 读取 `KnownDLLs` MULTI_SZ 值
2. 过滤掉 `entry` 对应的 DLL
3. 写回新的 MULTI_SZ

**已知问题**：
- ⚠️ KnownDLLs 键需要 SYSTEM 权限，管理员权限不足，返回"权限不足，无法打开注册表项（可能需要 SYSTEM 权限）"（预期行为）

### 13. Winsock Providers

**数据结构**：
- `location`: `HKLM\System\CurrentControlSet\Services\WinSock2\Parameters\NameSpace_Catalog5\Catalog_Entries`
- `entry`: 描述名（如 `Bluetooth Namespace`）

**注册表结构**：
```
HKLM\...\NameSpace_Catalog5\Catalog_Entries
  └── 000000000001              ← 子键名是数字编号
      ├── LibraryPath
      ├── DisplayString = "Bluetooth Namespace"
      └── ...
```

**删除逻辑**：
- 返回"Winsock/网络提供程序项结构复杂，建议使用专用工具手动处理"
- 原因：子键名是数字编号，entry 是描述名，需要遍历子键匹配 DisplayString

### 14. WMI

**删除逻辑**：
- 返回"WMI 持久化项暂不支持删除，请手动处理"
- 原因：需要 WMI API，非注册表操作

### 15. Print Monitors

**数据结构**：
- `location`: 注册表路径
- `entry`: 值名

**删除逻辑**：
- 删除注册表值

### 16. Office / COM

**数据结构**：
- `location`: 注册表路径
- `entry`: 可能是 CLSID 或描述名

**删除逻辑**（`delete_com_or_office`）：
1. 从 `item` 提取 CLSID
2. 如果找到 CLSID：删除 `SOFTWARE\Classes\CLSID\{CLSID}`
3. 否则：删除注册表值

### 17. Generic（Logon、Explorer、Codecs 等）

**数据结构**：
- `location`: 注册表路径
- `entry`: 值名

**删除逻辑**（`delete_registry_value`）：
1. 打开注册表项
2. 删除值

## 删除结果处理

### 假删除问题（已修复）

**根因**：`delete_registry_value` 和 `delete_registry_key_fallback` 在 `ERROR_FILE_NOT_FOUND (2)` 时返回 `success=true`，把"不存在"当作"已删除"。

**修复**：
- `delete_registry_value`：值/键不存在时返回 `success=false`
- `delete_registry_key_fallback`：键不存在时返回 `Err`

### `\(Default)` 默认值处理（新增）

**背景**：autorunsc 对某些条目（如 Htmlfile 的 Shell\Open\Command）用 `location` 以 `\(Default)` 结尾来表示注册表默认值。直接解析会把 `\(Default)` 当作子键路径的一部分，导致删除不存在的子键。

**修复**：在 `delete_entry_inner` 中，解析 subkey 后、类别分发前，统一检测 `\(Default)` 后缀：
1. 如果 subkey 以 `\(Default)` 结尾（不区分大小写），分离出真正的 subkey
2. 调用 `delete_registry_default_value` 删除默认值（用 `PCWSTR::null()` 表示默认值）

**安全检查**：`delete_ifeo` 增加 `entry` 为空的检查，防止构造 `parent_subkey\` 路径误删父键。

### 权限不足错误处理（新增）

**背景**：某些注册表键（如 `KnownDLLs`）需要 SYSTEM 权限，管理员权限不足。之前的错误信息"无法打开注册表项"不够明确。

**修复**：在 `delete_registry_value`、`delete_registry_default_value`、`delete_lsa_multi_sz_at` 中检测 `ERROR_ACCESS_DENIED (5)`，返回"权限不足，无法打开注册表项（可能需要 SYSTEM 权限）"。

### schtasks stderr 编码处理（新增）

**背景**：`schtasks` 输出使用系统 ANSI 编码（中文 Windows 为 GBK），用 `String::from_utf8_lossy` 解码会导致乱码。

**修复**：用 `encoding_rs::GBK.decode()` 解码 stderr。

### CLSID 提取（已优化）

**优化前**：仅从 `location` 和 `entry` 提取 CLSID
**优化后**：按以下顺序提取：
1. `location`
2. `entry`
3. `launch_string`
4. `image_path`

### 子键枚举匹配方案（新增）

**背景**：autorunsc 对某些条目类型（Active Setup、BHO）完全不输出 CLSID，`entry` 是描述名，`launch_string`/`image_path` 是 exe/dll 路径，都不是 CLSID。CLSID 只存在于注册表子键名中，无法通过 CSV 字段直接定位。

**方案**：`find_and_delete_subkey_by_value` 函数 — 遍历父键的子键，读取每个子键的指定值名，与 search 字符串做不区分大小写的包含匹配，找到后删除整个子键。

**函数签名**：
```rust
fn find_and_delete_subkey_by_value(
    hive: HKEY,
    parent_subkey: &str,  // 父键路径（如 "SOFTWARE\Microsoft\Active Setup\Installed Components"）
    value_name: &str,     // 要读取的值名（Active Setup 用 "StubPath"，BHO 用 "" 表示默认值）
    search: &str,         // 匹配字符串（Active Setup 用 launch_string，BHO 用 entry）
) -> Result<DeleteResult, IrError>
```

**应用场景**：
| 条目类型 | value_name | search | 匹配逻辑 |
|---------|-----------|--------|---------|
| Active Setup | `StubPath` | `launch_string` 或 `image_path` | 子键的 StubPath 值包含启动命令 |
| BHO | `""`（默认值） | `entry` | 子键的默认值包含描述名 |
| GpExtensions / Notify | `DllName` | `launch_string` 或 `image_path` | 子键的 DllName 值包含 DLL 路径 |

**实现要点**（踩过的坑）：
1. **HSTRING 生命周期**：`PCWSTR` 只是裸指针，不持有底层 `HSTRING` 的生命周期。如果在 `if/else` 块内创建 `HSTRING` 再取 `as_ptr()`，块结束时 `HSTRING` 被 drop，指针变成悬垂指针。必须在循环外创建 `Option<HSTRING>` 并保持其生命周期覆盖整个枚举过程。
2. **PWSTR 类型**：`RegEnumKeyExW` 的 `lpName` 参数需要 `PWSTR` 而非 `&mut [u16]`，需用 `PWSTR(name_buf.as_mut_ptr())` 转换。
3. **模块可见性**：`find_and_delete_subkey_by_value` 是模块级函数（非 `win_impls` 内部函数），不能调用 `win_impls` 内的 `delete_registry_key`，需调用模块级的 `delete_registry_key_fallback` 并手动构造 `DeleteResult`。
4. **缓冲区大小**：值读取缓冲区从 2048 扩大到 4096 字节，避免长路径被截断。
5. **调试日志**：枚举过程中记录每个子键的检查和值内容（DEBUG 级别），匹配成功时记录完整路径（INFO 级别），方便排查匹配失败问题。

### 日志记录

删除失败时输出完整条目信息（WARN 级别）：
```
delete entry failed, full item: id=X, category=X, entry=X, location=X,
         enabled=X, image_path=X, launch_string=X, service_name=X,
         description=X, publisher=X
```

## 测试要点

1. **Services**：删除已卸载但注册表残留的服务（如 `edgeupdate`）
2. **Active Setup**：删除 entry 是描述名的条目（如 `Microsoft Edge`），验证子键枚举匹配 StubPath
3. **IE BHO**：删除 entry 是描述名的条目（如 `IEToEdge BHO`），验证子键枚举匹配默认值
4. **GpExtensions**：删除 entry 是描述名的条目（如 `组策略缓解选项`），验证子键枚举匹配 DllName
5. **Image Hijacks \(Default)**：删除 location 以 `\(Default)` 结尾的条目，验证删除默认值而非子键
6. **Scheduled Tasks**：删除受保护任务（如 `UpdateOrchestrator`），验证 stderr 正确解码为中文
7. **Known DLLs**：删除 KnownDLLs 条目，验证返回"权限不足"而非"无法打开"
8. **Winsock**：返回"暂不支持自动删除"
9. **Winlogon**：entry 为空时返回"无法定位删除目标"
10. **假删除**：删除不存在的条目，验证返回 `success=false`
11. **子键枚举日志**：删除 Active Setup / BHO / GpExtensions 时查看 `monitor.log`，应有 `find_and_delete_subkey_by_value: checking subkey '...'` 和 `find_and_delete_subkey_by_value: subkey '...' {value_name}='...'` 的 DEBUG 日志
