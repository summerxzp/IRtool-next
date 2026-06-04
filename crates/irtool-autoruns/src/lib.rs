//! P2 持久化检测：autorunsc 调用 + WinTrust 签名验证 + 风险评估 + 删除操作

pub mod csv_parser;
pub mod delete;
pub mod icon;
pub mod risk;
pub mod scanner;
pub mod sigcheck;
pub mod store;
pub mod types;

pub use icon::batch_extract_icons;
pub use icon::extract_icon_base64;
pub use scanner::find_sigcheck;
pub use scanner::open_in_explorer;
pub use scanner::open_regedit;
pub use scanner::open_services_msc;
pub use scanner::AutorunsScanner;
pub use store::AutorunsStore;
pub use types::*;
