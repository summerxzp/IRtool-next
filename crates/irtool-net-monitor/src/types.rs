use serde::{Deserialize, Serialize};
use specta::Type;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum Proto {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum Family {
    V4,
    V6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConnState {
    Closed,
    Listen,
    SynSent,
    SynRcvd,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
    DeleteTcb,
    None,
}

impl ConnState {
    pub fn from_mib_tcp_state(state: u32) -> Self {
        match state {
            1 => Self::Closed,
            2 => Self::Listen,
            3 => Self::SynSent,
            4 => Self::SynRcvd,
            5 => Self::Established,
            6 => Self::FinWait1,
            7 => Self::FinWait2,
            8 => Self::CloseWait,
            9 => Self::Closing,
            10 => Self::LastAck,
            11 => Self::TimeWait,
            12 => Self::DeleteTcb,
            _ => Self::None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Closed => "CLOSED",
            Self::Listen => "LISTEN",
            Self::SynSent => "SYN_SENT",
            Self::SynRcvd => "SYN_RCVD",
            Self::Established => "ESTABLISHED",
            Self::FinWait1 => "FIN_WAIT1",
            Self::FinWait2 => "FIN_WAIT2",
            Self::CloseWait => "CLOSE_WAIT",
            Self::Closing => "CLOSING",
            Self::LastAck => "LAST_ACK",
            Self::TimeWait => "TIME_WAIT",
            Self::DeleteTcb => "DELETE_TCB",
            Self::None => "",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
pub struct NetEndpoint {
    pub addr: String,
    pub port: u16,
}

impl NetEndpoint {
    pub fn from_v4(addr: u32, port: u16) -> Self {
        let ip = std::net::Ipv4Addr::from(u32::from_be(addr));
        Self {
            addr: ip.to_string(),
            port: u16::from_be(port),
        }
    }

    pub fn from_v6(addr: [u8; 16], port: u16) -> Self {
        let ip = std::net::Ipv6Addr::from(addr);
        Self {
            addr: ip.to_string(),
            port: u16::from_be(port),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NetConnKey {
    pub proto: Proto,
    pub family: Family,
    pub local_addr: String,
    pub local_port: u16,
    pub remote_addr: String,
    pub remote_port: u16,
    pub pid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NetConn {
    pub proto: Proto,
    pub family: Family,
    pub local: NetEndpoint,
    pub remote: NetEndpoint,
    pub state: ConnState,
    pub pid: u32,
    pub process_name: Option<String>,
    pub process_path: Option<String>,
    pub process_cmdline: Option<String>,
    pub first_seen: u64,
    pub last_seen: u64,
    pub is_current: bool,
}

impl NetConn {
    pub fn key(&self) -> NetConnKey {
        NetConnKey {
            proto: self.proto,
            family: self.family,
            local_addr: self.local.addr.clone(),
            local_port: self.local.port,
            remote_addr: self.remote.addr.clone(),
            remote_port: self.remote.port,
            pid: self.pid,
        }
    }
}

pub fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_v4_converts_byte_order() {
        let raw_be: u32 = u32::from_be_bytes([127, 0, 0, 1]);
        let ep = NetEndpoint::from_v4(raw_be.to_be(), 8080u16.to_be());
        assert_eq!(ep.addr, "127.0.0.1");
        assert_eq!(ep.port, 8080);
    }

    #[test]
    fn endpoint_v6_loopback() {
        let mut bytes = [0u8; 16];
        bytes[15] = 1;
        let ep = NetEndpoint::from_v6(bytes, 80u16.to_be());
        assert_eq!(ep.addr, "::1");
        assert_eq!(ep.port, 80);
    }

    #[test]
    fn conn_state_maps_mib() {
        assert_eq!(ConnState::from_mib_tcp_state(5), ConnState::Established);
        assert_eq!(ConnState::from_mib_tcp_state(2), ConnState::Listen);
        assert_eq!(ConnState::from_mib_tcp_state(99), ConnState::None);
    }

    #[test]
    fn key_differs_by_pid() {
        let mut conn = NetConn {
            proto: Proto::Tcp,
            family: Family::V4,
            local: NetEndpoint { addr: "127.0.0.1".into(), port: 80 },
            remote: NetEndpoint { addr: "1.1.1.1".into(), port: 443 },
            state: ConnState::Established,
            pid: 100,
            process_name: None,
            process_path: None,
            process_cmdline: None,
            first_seen: 0,
            last_seen: 0,
            is_current: true,
        };
        let k1 = conn.key();
        conn.pid = 200;
        let k2 = conn.key();
        assert_ne!(k1, k2);
    }
}
