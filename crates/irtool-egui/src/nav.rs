/// Navigation page enum.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Network,
    Autoruns,
    Sysmon,
    Monitor,
    Database,
    Workspace,
    Settings,
}

impl Page {
    pub const ALL: [Page; 7] = [
        Page::Network,
        Page::Autoruns,
        Page::Sysmon,
        Page::Monitor,
        Page::Database,
        Page::Workspace,
        Page::Settings,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Page::Network => "网络监控",
            Page::Autoruns => "持久化检测",
            Page::Sysmon => "日志采集",
            Page::Monitor => "后台监控",
            Page::Database => "数据库",
            Page::Workspace => "工作台",
            Page::Settings => "设置",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Page::Network => "",
            Page::Autoruns => "",
            Page::Sysmon => "",
            Page::Monitor => "",
            Page::Database => "",
            Page::Workspace => "",
            Page::Settings => "",
        }
    }
}
