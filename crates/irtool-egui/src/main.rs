#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    irtool_egui::run(irtool_egui::StartupMode::Normal);
}
