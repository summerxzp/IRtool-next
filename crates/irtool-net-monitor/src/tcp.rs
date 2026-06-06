use crate::types::{now_epoch_secs, ConnState, Family, NetConn, NetEndpoint, Proto};
use irtool_core::IrError;
use std::mem::size_of;

#[cfg(windows)]
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCP6TABLE_OWNER_PID, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID,
    TCP_TABLE_OWNER_PID_ALL,
};
#[cfg(windows)]
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};

#[cfg(windows)]
pub fn enumerate_tcp_v4() -> Result<Vec<NetConn>, IrError> {
    unsafe {
        let mut buf_len: u32 = 0;
        let _ = GetExtendedTcpTable(None, &mut buf_len, false, AF_INET.0 as u32, TCP_TABLE_OWNER_PID_ALL, 0);

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
            let rc = GetExtendedTcpTable(
                Some(buf.as_mut_ptr() as *mut _),
                &mut buf_len,
                false,
                AF_INET.0 as u32,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            );
            if rc == 0 || attempts >= MAX_ATTEMPTS {
                break rc;
            }
            // Table grew between calls; increase buffer and retry
            buf_len = ((buf_len as f64) * 1.5) as u32;
        };
        if rc != 0 {
            return Err(IrError::Internal(format!("GetExtendedTcpTable v4 failed: {}", rc)));
        }

        let table = &*(buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
        let count = table.dwNumEntries as usize;

        let row_size = size_of::<MIB_TCPROW_OWNER_PID>();
        let header_size = size_of::<u32>();
        if buf.len() < header_size + count * row_size {
            return Err(IrError::Internal("tcp v4 table size insufficient".into()));
        }

        let rows_ptr = buf.as_ptr().add(header_size) as *const MIB_TCPROW_OWNER_PID;

        let now = now_epoch_secs();
        let mut conns = Vec::with_capacity(count);
        for i in 0..count {
            let row = &*rows_ptr.add(i);
            let local = NetEndpoint::from_v4(row.dwLocalAddr, row.dwLocalPort as u16);
            let remote = NetEndpoint::from_v4(row.dwRemoteAddr, row.dwRemotePort as u16);
            let state = ConnState::from_mib_tcp_state(row.dwState);
            conns.push(NetConn {
                proto: Proto::Tcp,
                family: Family::V4,
                local,
                remote,
                state,
                pid: row.dwOwningPid,
                process_name: None,
                process_path: None,
                process_cmdline: None,
                first_seen: now,
                last_seen: now,
                is_current: true,
            });
        }
        Ok(conns)
    }
}

#[cfg(windows)]
pub fn enumerate_tcp_v6() -> Result<Vec<NetConn>, IrError> {
    unsafe {
        let mut buf_len: u32 = 0;
        let _ = GetExtendedTcpTable(None, &mut buf_len, false, AF_INET6.0 as u32, TCP_TABLE_OWNER_PID_ALL, 0);

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
            let rc = GetExtendedTcpTable(
                Some(buf.as_mut_ptr() as *mut _),
                &mut buf_len,
                false,
                AF_INET6.0 as u32,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            );
            if rc == 0 || attempts >= MAX_ATTEMPTS {
                break rc;
            }
            // Table grew between calls; increase buffer and retry
            buf_len = ((buf_len as f64) * 1.5) as u32;
        };
        if rc != 0 {
            return Err(IrError::Internal(format!("GetExtendedTcpTable v6 failed: {}", rc)));
        }

        let table = &*(buf.as_ptr() as *const MIB_TCP6TABLE_OWNER_PID);
        let count = table.dwNumEntries as usize;

        let row_size = size_of::<MIB_TCP6ROW_OWNER_PID>();
        let header_size = size_of::<u32>();
        if buf.len() < header_size + count * row_size {
            return Err(IrError::Internal("tcp v6 table size insufficient".into()));
        }

        let rows_ptr = buf.as_ptr().add(header_size) as *const MIB_TCP6ROW_OWNER_PID;

        let now = now_epoch_secs();
        let mut conns = Vec::with_capacity(count);
        for i in 0..count {
            let row = &*rows_ptr.add(i);
            let local = NetEndpoint::from_v6(row.ucLocalAddr, row.dwLocalPort as u16);
            let remote = NetEndpoint::from_v6(row.ucRemoteAddr, row.dwRemotePort as u16);
            let state = ConnState::from_mib_tcp_state(row.dwState);
            conns.push(NetConn {
                proto: Proto::Tcp,
                family: Family::V6,
                local,
                remote,
                state,
                pid: row.dwOwningPid,
                process_name: None,
                process_path: None,
                process_cmdline: None,
                first_seen: now,
                last_seen: now,
                is_current: true,
            });
        }
        Ok(conns)
    }
}

#[cfg(not(windows))]
pub fn enumerate_tcp_v4() -> Result<Vec<NetConn>, IrError> {
    Err(IrError::Internal("tcp v4 only supported on Windows".into()))
}

#[cfg(not(windows))]
pub fn enumerate_tcp_v6() -> Result<Vec<NetConn>, IrError> {
    Err(IrError::Internal("tcp v6 only supported on Windows".into()))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn tcp_v4_returns_some_listening() {
        let conns = enumerate_tcp_v4().expect("query failed");
        assert!(!conns.is_empty(), "expected at least one TCP v4 connection");
        for c in &conns {
            assert_eq!(c.proto, Proto::Tcp);
            assert_eq!(c.family, Family::V4);
        }
    }

    #[test]
    fn tcp_v6_query_does_not_panic() {
        let _ = enumerate_tcp_v6().expect("query failed");
    }
}
