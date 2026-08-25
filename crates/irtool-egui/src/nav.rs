use std::borrow::Cow;

use crate::design::icon::Icon;

/// Navigation page enum.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Network,
    Autoruns,
    BrowserForensics,
    Sysmon,
    Process,
    Monitor,
    Database,
    Workspace,
    Settings,
}

impl Page {
    /// 主导航项（rail 上部；设置项单独沉底，与 demo side_rail 一致）。
    pub const MAIN: [Page; 8] = [
        Page::Network,
        Page::Sysmon,
        Page::Autoruns,
        Page::Process,
        Page::BrowserForensics,
        Page::Workspace,
        Page::Monitor,
        Page::Database,
    ];

    /// 导航名（i18n：键名与 React ui/src/locales nav.* 一致，P4 起 t! 动态取）。
    pub fn label(&self) -> Cow<'static, str> {
        match self {
            Page::Network => rust_i18n::t!("nav.network"),
            Page::Autoruns => rust_i18n::t!("nav.autoruns"),
            Page::BrowserForensics => rust_i18n::t!("nav.browser-forensics"),
            Page::Sysmon => rust_i18n::t!("nav.log-collector"),
            Page::Process => rust_i18n::t!("nav.process"),
            Page::Monitor => rust_i18n::t!("nav.background-monitoring"),
            Page::Database => rust_i18n::t!("nav.database-search"),
            Page::Workspace => rust_i18n::t!("nav.workspace"),
            Page::Settings => rust_i18n::t!("nav.settings"),
        }
    }

    /// 导航图标（lucide 线性图标，spec §4.1；颜色规则见 spec §2.2-3）。
    pub fn icon(&self) -> Icon {
        match self {
            Page::Network => Icon::Activity,
            Page::Autoruns => Icon::Shield,
            Page::BrowserForensics => Icon::Search,
            Page::Sysmon => Icon::FileText,
            Page::Process => Icon::Cpu,
            Page::Monitor => Icon::Bell,
            Page::Database => Icon::Database,
            Page::Workspace => Icon::Briefcase,
            Page::Settings => Icon::Sliders,
        }
    }
}
