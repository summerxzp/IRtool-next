use serde::Serialize;
use specta::Type;

#[derive(Serialize, Type)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub is_admin: bool,
}
