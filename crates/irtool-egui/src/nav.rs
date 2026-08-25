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

    pub fn label(&self) -> &'static str {
        match self {
            Page::Network => "网络监控",
            Page::Autoruns => "持久化检测",
            Page::BrowserForensics => "浏览器取证",
            Page::Sysmon => "日志采集",
            Page::Process => "进程",
            Page::Monitor => "后台监控",
            Page::Database => "数据库",
            Page::Workspace => "工作台",
            Page::Settings => "设置",
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
