//! 设计样板预览入口：`cargo run -p irtool-egui --example design_board [-- --dark]`。
//! 窗口标题含 "IRtool"，便于 capture.ps1 按标题定位截图。

fn main() -> eframe::Result<()> {
    let dark = std::env::args().any(|a| a == "--dark");
    irtool_egui::design::preview::run_preview(dark)
}
