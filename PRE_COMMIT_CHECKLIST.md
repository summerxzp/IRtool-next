# Pre-Commit Checklist

提交到 GitHub 前，按顺序执行以下检查。全部通过即可推送。

## 快速一键检查（推荐）

```powershell
# 在项目根目录执行
./scripts/pre-commit.ps1
```

如果脚本不存在或需要逐项排查，按下表手动执行：

---

## 检查清单

| # | 检查项 | 命令 | 对应 CI Job | 常见失败原因 |
|---|--------|------|-------------|-------------|
| 1 | **Rust 格式化** | `cargo fmt --all -- --check` | `rust` | 代码不符合 rustfmt 规范 |
| 2 | **Clippy 静态分析** | `cargo clippy --workspace --all-targets --exclude irtool-tauri -- -D warnings` | `rust` | 警告被 `-D warnings` 提升为错误 |
| 3 | **Rust 测试** | `cargo test --workspace --exclude irtool-tauri --no-fail-fast` | `rust` | 单元测试失败 |
| 4 | **前端类型检查** | `cd ui && pnpm lint` | `ui` | TypeScript 类型错误 |
| 5 | **前端构建** | `cd ui && pnpm build` | `ui` | 构建失败（缺少导入、语法错误等） |
| 6 | **Tauri Release 构建** | `cargo build --release -p irtool-tauri --features irtool-tauri/egui-fallback` | `tauri-build` | 链接错误、Windows API 不兼容、egui 降级 UI 集成代码编译失败 |

---

## 各检查项详解

### 1. cargo fmt（格式化）

```powershell
cargo fmt --all -- --check
# 失败时自动修复：
cargo fmt
```

**常见陷阱**：
- 函数调用参数超过行宽限制时，rustfmt 会强制每个参数单独一行
- `format!` 宏中多余的空格会被检测
- 短函数调用如果被手动拆行，rustfmt 会要求合并

### 2. cargo clippy（静态分析）

```powershell
cargo clippy --workspace --all-targets --exclude irtool-tauri -- -D warnings
```

**注意**：`irtool-tauri` 被排除在外（因为 Tauri 生成的代码会触发大量 clippy 警告）。

**常见陷阱**：
- `RUSTFLAGS="-D warnings"` 在 CI 中生效，本地编译的 warning 在 CI 会变成 error
- `unwrap()` / `expect()` 在某些 clippy 配置下会触发警告
- 未使用的变量 / 导入（`unused_imports`, `unused_variables`）
- `let _ = ...` 忽略 Result 可能触发 `let_underscore_drop` 警告

### 3. cargo test（测试）

```powershell
cargo test --workspace --exclude irtool-tauri --no-fail-fast
```

**注意**：
- `irtool-tauri` 被排除（Tauri 命令需要运行时环境）
- `--no-fail-fast` 表示即使某个测试失败也继续运行其他测试
- 某些测试可能需要管理员权限（如注册表操作）

### 4. pnpm lint（前端类型检查）

```powershell
cd ui && pnpm lint
```

**常见陷阱**：
- specta 生成的 `bindings.ts` 与后端类型不匹配时，需先重新生成
- 新增 Rust 命令后，前端调用处参数可能不一致

### 5. pnpm build（前端构建）

```powershell
cd ui && pnpm build
```

**常见陷阱**：
- 动态 import 路径错误（Vite 相对路径计算）
- 缺少 Tailwind 类名或主题变量
- TanStack Router 路由树未更新（运行 `pnpm dev` 会自动生成）

### 6. Tauri Release 构建（含 egui-fallback）

```powershell
cargo build --release -p irtool-tauri --features irtool-tauri/egui-fallback
```

**注意**：
- `--features irtool-tauri/egui-fallback` 启用 egui 降级 UI 集成代码，与打包时的配置一致
- 这是最耗时的检查，通常本地只需确认 `cargo check -p irtool-tauri --features irtool-tauri/egui-fallback` 通过即可。完整 release 构建交给 CI。
- 如果只改了 egui crate 本身的代码，可用 `cargo check -p irtool-egui` 快速验证

---

## 快速排查流程

```
提交前 → 改了 Rust 代码？
  ├─ 是 → 执行 #1 #2 #3
  │       └─ 改了 egui 或 irtool-tauri 集成代码？→ 额外执行 #6
  └─ 否 → 改了前端代码？
      ├─ 是 → 执行 #4 #5
      └─ 都改了 → 执行 #1 #2 #3 #4 #5 #6
```

---

## 已知 CI 环境差异

| 项目 | 本地开发 | CI 环境 |
|------|---------|---------|
| RUSTFLAGS | 默认（警告不中断） | `-D warnings`（警告即错误） |
| OS | Windows 11 24H2 | `windows-latest` |
| Rust 版本 | 本地 stable | CI stable（可能更新） |
| Node.js | 本地版本 | 20.x |
| pnpm | 本地版本 | 9.x |

**关键差异**：本地编译通过不代表 CI 通过，因为 CI 启用了 `-D warnings`。务必在本地也运行 clippy 确认无警告。
