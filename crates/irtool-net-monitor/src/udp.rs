use crate::types::{now_epoch_secs, ConnState, Family, NetConn, NetEndpoint, Proto};
use irtool_core::IrError;
use std::mem::size_of;

#[cfg(windows)]
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedUdpTable, MIB_UDP6ROW_OWNER_PID, MIB_UDP6TABLE_OWNER_PID, MIB_UDPROW_OWNER_PID, MIB_UDPTABLE_OWNER_PID,
    UDP_TABLE_OWNER_PID,
};
#[cfg(windows)]
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};

#[cfg(windows)]
pub fn enumerate_udp_v4() -> Result<Vec<NetConn>, IrError> {
    unsafe {
        let mut buf_len: u32 = 0;
        let _ = GetExtendedUdpTable(None, &mut buf_len, false, AF_INET.0 as u32, UDP_TABLE_OWNER_PID, 0);

        if buf_len == 0 {
            return Ok(Vec::new());
        }

        const MAX_ATTEMPTS: usize = 3;
        let mut attempts = 0;
        #[allow(unused_assignments)]
        let mut buf: Vec<u8> = Vec::new();
        let rc = loop {
            attempts += 1;
            buf = vec![0u8; buf_len as usize];
            let rc = GetExtendedUdpTable(
                Some(buf.as_mut_ptr() as *mut _),
                &mut buf_len,
                false,
                AF_INET.0 as u32,
                UDP_TABLE_OWNER_PID,
                0,
            );
            if rc == 0 || attempts >= MAX_ATTEMPTS {
                break rc;
            }
            // Table grew between calls; increase buffer and retry
            buf_len = ((buf_len as f64) * 1.5) as u32;
        };
        if rc != 0 {
            return Err(IrError::Internal(format!("GetExtendedUdpTable v4 failed: {}", rc)));
        }

        let table = &*(buf.as_ptr() as *const MIB_UDPTABLE_OWNER_PID);
        let count = table.dwNumEntries as usize;
        let row_size = size_of::<MIB_UDPROW_OWNER_PID>();
        let header_size = size_of::<u32>();
        if buf.len() < header_size + count * row_size {
            return Err(IrError::Internal("udp v4 table size insufficient".into()));
        }

        let rows_ptr = buf.as_ptr().add(header_size) as *const MIB_UDPROW_OWNER_PID;

        let now = now_epoch_secs();
        let mut conns = Vec::with_capacity(count);
        for i in 0..count {
            let row = &*rows_ptr.add(i);
            let local = NetEndpoint::from_v4(row.dwLocalAddr, row.dwLocalPort as u16);
            let remote = NetEndpoint {
                addr: String::new(),
                port: 0,
            };
            conns.push(NetConn {
                proto: Proto::Udp,
                family: Family::V4,
                local,
                remote,
                state: ConnState::None,
                pid: row.dwOwningPid,
                process_name: None,
                process_path: None,
                process_cmdline: None,
                cmdline_status: crate::types::CmdlineStatus::Unknown,
                first_seen: now,
                last_seen: now,
                is_current: true,
            });
        }
        Ok(conns)
    }
}

#[cfg(windows)]
pub fn enumerate_udp_v6() -> Result<Vec<NetConn>, IrError> {
    unsafe {
        let mut buf_len: u32 = 0;
        let _ = GetExtendedUdpTable(None, &mut buf_len, false, AF_INET6.0 as u32, UDP_TABLE_OWNER_PID, 0);

        if buf_len == 0 {
            return Ok(Vec::new());
        }

        const MAX_ATTEMPTS: usize = 3;
        let mut attempts = 0;
        #[allow(unused_assignments)]
        let mut buf: Vec<u8> = Vec::new();
        let rc = loop {
            attempts += 1;
            buf = vec![0u8; buf_len as usize];
            let rc = GetExtendedUdpTable(
                Some(buf.as_mut_ptr() as *mut _),
                &mut buf_len,
                false,
                AF_INET6.0 as u32,
                UDP_TABLE_OWNER_PID,
                0,
            );
            if rc == 0 || attempts >= MAX_ATTEMPTS {
                break rc;
            }
            // Table grew between calls; increase buffer and retry
            buf_len = ((buf_len as f64) * 1.5) as u32;
        };
        if rc != 0 {
            return Err(IrError::Internal(format!("GetExtendedUdpTable v6 failed: {}", rc)));
        }

        let table = &*(buf.as_ptr() as *const MIB_UDP6TABLE_OWNER_PID);
        let count = table.dwNumEntries as usize;
        let row_size = size_of::<MIB_UDP6ROW_OWNER_PID>();
        let header_size = size_of::<u32>();
        if buf.len() < header_size + count * row_size {
            return Err(IrError::Internal("udp v6 table size insufficient".into()));
        }

        let rows_ptr = buf.as_ptr().add(header_size) as *const MIB_UDP6ROW_OWNER_PID;

        let now = now_epoch_secs();
        let mut conns = Vec::with_capacity(count);
        for i in 0..count {
            let row = &*rows_ptr.add(i);
            let local = NetEndpoint::from_v6(row.ucLocalAddr, row.dwLocalPort as u16);
            let remote = NetEndpoint {
                addr: String::new(),
                port: 0,
            };
            conns.push(NetConn {
                proto: Proto::Udp,
                family: Family::V6,
                local,
                remote,
                state: ConnState::None,
                pid: row.dwOwningPid,
                process_name: None,
                process_path: None,
                process_cmdline: None,
                cmdline_status: crate::types::CmdlineStatus::Unknown,
                first_seen: now,
                last_seen: now,
                is_current: true,
            });
        }
        Ok(conns)
    }
}

#[cfg(not(windows))]
pub fn enumerate_udp_v4() -> Result<Vec<NetConn>, IrError> {
    Err(IrError::Internal("udp v4 only supported on Windows".into()))
}

#[cfg(not(windows))]
pub fn enumerate_udp_v6() -> Result<Vec<NetConn>, IrError> {
    Err(IrError::Internal("udp v6 only supported on Windows".into()))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn udp_v4_returns_results() {
        let conns = enumerate_udp_v4().expect("query failed");
        for c in &conns {
            assert_eq!(c.proto, Proto::Udp);
            assert_eq!(c.family, Family::V4);
            assert_eq!(c.state, ConnState::None);
        }
    }
}
