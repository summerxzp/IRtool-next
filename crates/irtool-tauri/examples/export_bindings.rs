//! 重新生成 `ui/src/lib/bindings.ts`。
//!
//! 运行：`cargo run -p irtool-tauri --example export_bindings`
//!
//! dev 模式下 `cargo run`（即 `irtool_lib::run()`）启动时也会自动重新生成，
//! 本 example 用于不启动完整 Tauri 应用的独立重新生成场景。
//!
//! 需要嵌入 Common-Controls v6 manifest（见 `build.rs` 的
//! `cargo:rustc-link-arg-examples`），否则 Tauri 静态导入的 `TaskDialogIndirect`
//! 在 `comctl32.dll` v5 下缺失，导致加载时 `0xc0000139`。

use irtool_lib::{command_builder, export_bindings};

fn main() {
    let builder = command_builder();
    export_bindings(&builder);
    println!("bindings.ts exported successfully");
}
