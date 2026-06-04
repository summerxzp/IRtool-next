//! P2 持久化检测：autorunsc 调用 + WinTrust 签名验证 + 风险评估 + 删除操作

pub mod types;
pub mod csv_parser;
pub mod risk;
pub mod sigcheck;
pub mod delete;
pub mod scanner;
pub mod store;

pub use types::*;
pub use scanner::AutorunsScanner;
pub use scanner::find_sigcheck;
pub use scanner::open_in_explorer;
pub use scanner::open_regedit;
pub use scanner::open_services_msc;
pub use store::AutorunsStore;
