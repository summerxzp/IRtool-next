use crate::dns_raw;
use crate::sni;
use crate::types::{PcapConfig, PcapEvent, PcapEventKind};
use irtool_core::IrError;
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use tokio::sync::mpsc;

#[cfg(windows)]
use windows::Win32::Networking::WinSock::*;

/// SIO_RCVALL constant for enabling promiscuous mode on raw sockets
#[cfg(windows)]
const SIO_RCVALL: u32 = 0x98000001;

// Raw FFI for WSAIoctl (not exposed by windows crate 0.59 with current features)
#[cfg(windows)]
#[link(name = "ws2_32")]
extern "system" {
    fn WSAIoctl(
        s: isize,
        dwIoControlCode: u32,
        lpvInBuffer: *const std::ffi::c_void,
        cbInBuffer: u32,
        lpvOutBuffer: *mut std::ffi::c_void,
        cbOutBuffer: u32,
        lpcbBytesReturned: *mut u32,
        lpOverlapped: *mut std::ffi::c_void,
        lpCompletionRoutine: *mut std::ffi::c_void,
    ) -> i32;
}

pub struct PcapCollector {
    running: Arc<AtomicBool>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl PcapCollector {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            thread_handle: None,
        }
    }

    /// 检查 raw socket 是否可用（需要管理员权限）
    #[cfg(windows)]
    pub fn is_available() -> bool {
        unsafe {
            let mut wsa_data = WSADATA::default();
            if WSAStartup(0x0202, &mut wsa_data) != 0 {
                return false;
            }
            match WSASocketW(
                AF_INET.0 as i32,
                SOCK_RAW.0 as i32,
                IPPROTO_IP.0,
                None,
                0,
                WSA_FLAG_OVERLAPPED,
            ) {
                Ok(s) => {
                    closesocket(s);
                    WSACleanup();
                    true
                }
                Err(_) => {
                    WSACleanup();
                    false
                }
            }
        }
    }

    #[cfg(not(windows))]
    pub fn is_available() -> bool {
        false
    }

    /// 获取本机默认出口 IP
    fn get_local_ip() -> Result<Ipv4Addr, IrError> {
        let socket = std::net::UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| IrError::Network(format!("创建 UDP socket 失败: {}", e)))?;
        socket
            .connect("8.8.8.8:53")
            .map_err(|e| IrError::Network(format!("连接失败: {}", e)))?;
        let local_addr = socket
            .local_addr()
            .map_err(|e| IrError::Network(format!("获取本地地址失败: {}", e)))?;
        match local_addr.ip() {
            std::net::IpAddr::V4(ip) => Ok(ip),
            std::net::IpAddr::V6(_) => Err(IrError::Network("不支持 IPv6".to_string())),
        }
    }

    /// 启动抓包
    #[cfg(windows)]
    pub fn start(
        &mut self,
        config: PcapConfig,
        tx: mpsc::UnboundedSender<PcapEvent>,
    ) -> Result<(), IrError> {
        if self.running.load(Ordering::SeqCst) {
            return Err(IrError::Internal("抓包已在运行".to_string()));
        }

        let local_ip = Self::get_local_ip()?;
        let running = self.running.clone();
        running.store(true, Ordering::SeqCst);

        let handle = std::thread::Builder::new()
            .name("irtool-pcap".to_string())
            .spawn(move || {
                Self::capture_loop(local_ip, config, tx, running);
            })
            .map_err(|e| IrError::Internal(format!("创建抓包线程失败: {}", e)))?;

        self.thread_handle = Some(handle);
        info!("Pcap collector started on {}", local_ip);
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn start(
        &mut self,
        _config: PcapConfig,
        _tx: mpsc::UnboundedSender<PcapEvent>,
    ) -> Result<(), IrError> {
        Err(IrError::FeatureDisabled(
            "Raw socket 仅支持 Windows".to_string(),
        ))
    }

    /// 停止抓包
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
        info!("Pcap collector stopped");
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    #[cfg(windows)]
    fn capture_loop(
        local_ip: Ipv4Addr,
        config: PcapConfig,
        tx: mpsc::UnboundedSender<PcapEvent>,
        running: Arc<AtomicBool>,
    ) {
        info!("capture_loop starting, local_ip={}", local_ip);
        unsafe {
            let mut wsa_data = WSADATA::default();
            if WSAStartup(0x0202, &mut wsa_data) != 0 {
                error!("WSAStartup failed");
                running.store(false, Ordering::SeqCst);
                return;
            }

            let socket = match WSASocketW(
                AF_INET.0 as i32,
                SOCK_RAW.0 as i32,
                IPPROTO_IP.0,
                None,
                0,
                WSA_FLAG_OVERLAPPED,
            ) {
                Ok(s) => s,
                Err(_) => {
                    error!("Failed to create raw socket (requires admin privileges)");
                    WSACleanup();
                    running.store(false, Ordering::SeqCst);
                    return;
                }
            };

            // Bind to local IP
            // S_un.S_addr expects network byte order (big-endian).
            // Ipv4Addr::octets() returns [u8; 4] in network byte order (e.g. [192, 168, 1, 8]).
            // u32::from_ne_bytes(octets) on little-endian x86 stores the bytes as-is in memory:
            //   memory layout = [192, 168, 1, 8], which IS the correct network byte order for S_addr.
            // Do NOT use from_be_bytes — it would swap bytes on little-endian, corrupting the address.
            let mut addr: SOCKADDR_IN = std::mem::zeroed();
            addr.sin_family = AF_INET;
            addr.sin_port = 0; // Raw socket: port is irrelevant, must be 0
            let octets = local_ip.octets();
            addr.sin_addr.S_un.S_addr = u32::from_ne_bytes(octets);
            info!(
                "Binding raw socket to {} (octets: {:?}, s_addr: 0x{:08X})",
                local_ip, octets, addr.sin_addr.S_un.S_addr
            );

            if bind(
                socket,
                &addr as *const _ as *const SOCKADDR,
                std::mem::size_of::<SOCKADDR_IN>() as i32,
            ) != 0
            {
                error!(
                    "Failed to bind raw socket to {} (error: {:?})",
                    local_ip,
                    WSAGetLastError()
                );
                closesocket(socket);
                WSACleanup();
                running.store(false, Ordering::SeqCst);
                return;
            }
            info!("Raw socket bound to {}", local_ip);

            // Enable SIO_RCVALL
            let mut rcvall_value: u32 = 1; // RCVALL_ON
            let mut bytes_returned: u32 = 0;
            let result = WSAIoctl(
                socket.0 as isize,
                SIO_RCVALL,
                &mut rcvall_value as *mut _ as *const std::ffi::c_void,
                std::mem::size_of::<u32>() as u32,
                std::ptr::null_mut(),
                0,
                &mut bytes_returned,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );

            if result != 0 {
                error!(
                    "Failed to set SIO_RCVALL (error: {:?})",
                    WSAGetLastError()
                );
                closesocket(socket);
                WSACleanup();
                running.store(false, Ordering::SeqCst);
                return;
            }
            info!("SIO_RCVALL enabled on raw socket");

            // Set non-blocking mode with timeout
            let mut non_blocking: u32 = 1;
            let _ = ioctlsocket(socket, FIONBIO as i32, &mut non_blocking);

            let mut buffer = [0u8; 65535];
            let mut packet_count: u64 = 0;
            let poll_timeout = TIMEVAL {
                tv_sec: 0,
                tv_usec: 100_000,
            }; // 100ms

            while running.load(Ordering::SeqCst) {
                // Use select to wait with timeout
                let mut read_fds: FD_SET = std::mem::zeroed();
                read_fds.fd_count = 1;
                read_fds.fd_array[0] = socket;

                let select_result = select(
                    0,
                    Some(&mut read_fds as *mut _),
                    None,
                    None,
                    Some(&poll_timeout as *const _),
                );
                if select_result == 0 {
                    continue; // Timeout, check running flag
                }
                if select_result == -1 {
                    if !running.load(Ordering::SeqCst) {
                        break;
                    }
                    warn!("select error: {:?}", WSAGetLastError());
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }

                let len = recv(socket, &mut buffer, SEND_RECV_FLAGS(0));
                if len < 0 {
                    let err = WSAGetLastError();
                    // WSAEWOULDBLOCK (10035) — no data available, not an error
                    if err != WSA_ERROR(10035) {
                        warn!("recv error: {:?}", err);
                    }
                    continue;
                }
                if len == 0 {
                    continue;
                }

                packet_count += 1;
                // Log first 5 packets, then every 1000th (debug level to avoid spam)
                if packet_count <= 5 || packet_count % 1000 == 0 {
                    debug!(
                        "Received packet #{}: {} bytes",
                        packet_count, len
                    );
                }

                let packet = &buffer[..len as usize];
                if let Some(event) = Self::parse_packet(packet, &config, local_ip, packet_count) {
                    let _ = tx.send(event);
                }
            }

            closesocket(socket);
            WSACleanup();
        }
    }

    /// 解析 IP 数据包
    fn parse_packet(
        packet: &[u8],
        config: &PcapConfig,
        _local_ip: Ipv4Addr,
        packet_count: u64,
    ) -> Option<PcapEvent> {
        // IP header minimum 20 bytes
        if packet.len() < 20 {
            return None;
        }

        let version = packet[0] >> 4;
        if version != 4 {
            return None;
        } // Only IPv4 for now

        let ihl = (packet[0] & 0x0F) as usize * 4;
        if ihl < 20 || packet.len() < ihl {
            return None;
        }

        let protocol = packet[9];
        let src_ip = format!(
            "{}.{}.{}.{}",
            packet[12], packet[13], packet[14], packet[15]
        );
        let dst_ip = format!(
            "{}.{}.{}.{}",
            packet[16], packet[17], packet[18], packet[19]
        );

        let now_ts = chrono::Utc::now().timestamp_millis();

        match protocol {
            6 => {
                // TCP
                if config.enable_sni {
                    let tcp_header = &packet[ihl..];
                    if tcp_header.len() < 20 {
                        return None;
                    }
                    let src_port = ((tcp_header[0] as u16) << 8) | (tcp_header[1] as u16);
                    let dst_port = ((tcp_header[2] as u16) << 8) | (tcp_header[3] as u16);

                    // Only process packets to port 443 (TLS)
                    if dst_port != 443 {
                        return None;
                    }

                    // Only log occasionally (debug level)
                    if packet_count <= 5 || packet_count % 1000 == 0 {
                        debug!(
                            "[pkt#{}] TCP {}:{} -> {}:{} (port 443, payload {} bytes)",
                            packet_count, src_ip, src_port, dst_ip, dst_port,
                            tcp_header.len().saturating_sub(20)
                        );
                    }

                    let data_offset = ((tcp_header[12] >> 4) as usize) * 4;
                    if tcp_header.len() < data_offset {
                        return None;
                    }
                    let payload = &tcp_header[data_offset..];

                    if let Some(domain) = sni::extract_sni(payload) {
                        info!("[pkt#{}] SNI extracted: {}", packet_count, domain);
                        return Some(PcapEvent {
                            timestamp: now_ts,
                            event_kind: PcapEventKind::TlsSni,
                            domain,
                            src_ip,
                            src_port,
                            dst_ip,
                            dst_port,
                            query_type: String::new(),
                        });
                    }
                }
                None
            }
            17 => {
                // UDP
                if config.enable_dns_pcap {
                    let udp_header = &packet[ihl..];
                    if udp_header.len() < 8 {
                        return None;
                    }
                    let src_port = ((udp_header[0] as u16) << 8) | (udp_header[1] as u16);
                    let dst_port = ((udp_header[2] as u16) << 8) | (udp_header[3] as u16);

                    // Only process DNS packets (port 53)
                    if dst_port != 53 {
                        return None;
                    }

                    if packet_count <= 5 || packet_count % 1000 == 0 {
                        debug!(
                            "[pkt#{}] UDP {}:{} -> {}:{} (port 53, payload {} bytes)",
                            packet_count, src_ip, src_port, dst_ip, dst_port,
                            udp_header.len().saturating_sub(8)
                        );
                    }

                    let payload = &udp_header[8..];

                    if let Some(dns_info) = dns_raw::extract_dns_query(payload) {
                        info!(
                            "[pkt#{}] DNS query extracted: {} ({})",
                            packet_count, dns_info.domain, dns_info.query_type
                        );
                        return Some(PcapEvent {
                            timestamp: now_ts,
                            event_kind: PcapEventKind::DnsQuery,
                            domain: dns_info.domain,
                            src_ip,
                            src_port,
                            dst_ip,
                            dst_port,
                            query_type: dns_info.query_type,
                        });
                    }
                }
                None
            }
            _ => None,
        }
    }
}

impl Default for PcapCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PcapCollector {
    fn drop(&mut self) {
        self.stop();
    }
}
