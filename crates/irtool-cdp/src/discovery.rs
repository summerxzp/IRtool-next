//! BrowserDiscovery — 端口扫描 + `/json/version` 探测（P2.2 填充实现）。
//!
//! ## 设计要点
//!
//! - 扫描本机 9222/9223/9229 端口
//! - 调 `GET http://127.0.0.1:<port>/json/version` 判断浏览器类型
//! - **不启动浏览器**：只连接已开启 `--remote-debugging-port` 的实例
//! - 若无端口监听，返回空列表

use serde::Deserialize;

/// CDP 支持的浏览器类型（从 `/json/version` 的 `Browser` 字段解析）
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrowserKind {
    Chrome,
    Edge,
    /// 未知浏览器（理论上不该出现，但防御性处理）
    Unknown,
}

/// `/json/version` 响应（仅保留需要的字段）
#[derive(Debug, Clone, Deserialize)]
pub struct VersionResponse {
    /// 例如 "Chrome/120.0.6099.109"
    #[serde(rename = "Browser")]
    pub browser: String,
    /// WebSocket 调试 URL，例如 "ws://127.0.0.1:9222/devtools/browser/..."
    #[serde(rename = "webSocketDebuggerUrl")]
    pub web_socket_debugger_url: String,
}

/// Discovery 探测到的 CDP 目标（一个浏览器实例）
#[derive(Debug, Clone)]
pub struct CdpTarget {
    /// 监听端口（9222/9223/9229 等）
    pub port: u16,
    /// 浏览器类型
    pub browser: BrowserKind,
    /// WebSocket 调试 URL
    pub web_socket_debugger_url: String,
}

/// Discovery 错误
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("port {port} not responding: {source}")]
    PortNotResponding {
        port: u16,
        #[source]
        source: reqwest::Error,
    },

    #[error("invalid version response from port {port}: {message}")]
    InvalidResponse { port: u16, message: String },
}

/// 扫描的默认端口列表（Chrome/Edge 常用远程调试端口）
pub const DEFAULT_CDP_PORTS: &[u16] = &[9222, 9223, 9229];

/// 从 `Browser` 字段字符串解析浏览器类型
///
/// 例如 "Chrome/120.0.6099.109" → Chrome，"Edge/120.0.6099.109" → Edge
pub fn parse_browser_kind(browser_field: &str) -> BrowserKind {
    let lower = browser_field.to_lowercase();
    if lower.starts_with("chrome") {
        BrowserKind::Chrome
    } else if lower.starts_with("edge") || lower.contains("edg") {
        BrowserKind::Edge
    } else {
        BrowserKind::Unknown
    }
}

/// 扫描本机 CDP 端口并探测浏览器类型。
///
/// 对 `DEFAULT_CDP_PORTS`（9222/9223/9229）中的每个端口：
/// 1. TCP connect 探测端口是否监听（`tokio::net::TcpStream::connect`）
/// 2. 若监听，发 `GET http://127.0.0.1:<port>/json/version` HTTP 请求
/// 3. 解析响应 JSON 为 `VersionResponse`，提取 `Browser` 和 `webSocketDebuggerUrl`
/// 4. 根据 `Browser` 字段解析 `BrowserKind`
///
/// **不启动浏览器**：只连接已开启 `--remote-debugging-port` 的实例。
/// 若无端口监听或 HTTP 探测失败，返回空 Vec（不返回错误，调用方按需处理）。
///
/// 所有端口并行探测，单个端口失败不影响其他端口。
pub async fn discover_targets() -> Vec<CdpTarget> {
    discover_targets_with_ports(DEFAULT_CDP_PORTS).await
}

/// 扫描指定端口列表（测试可注入端口）。
pub async fn discover_targets_with_ports(ports: &[u16]) -> Vec<CdpTarget> {
    let futures: Vec<_> = ports.iter().map(|&port| probe_port(port)).collect();
    let results = futures_util::future::join_all(futures).await;
    results.into_iter().flatten().collect()
}

/// 探测单个端口：先 TCP 探测监听，再 HTTP 探测版本。
///
/// 失败时返回 None（调用方忽略），但用 tracing::debug 记录原因便于调试。
async fn probe_port(port: u16) -> Option<CdpTarget> {
    // 1. TCP connect 探测（快速失败，不阻塞）
    if !is_port_listening(port).await {
        return None;
    }

    // 2. HTTP 探测 /json/version
    let url = format!("http://127.0.0.1:{}/json/version", port);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .ok()?;

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("port {} HTTP probe failed: {}", port, e);
            return None;
        }
    };

    if !resp.status().is_success() {
        tracing::debug!("port {} /json/version returned status {}", port, resp.status());
        return None;
    }

    let version: VersionResponse = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!("port {} /json/version parse failed: {}", port, e);
            return None;
        }
    };

    let browser = parse_browser_kind(&version.browser);
    let target = CdpTarget {
        port,
        browser,
        web_socket_debugger_url: version.web_socket_debugger_url,
    };
    tracing::info!(
        "discovered CDP target: port={}, browser={:?}, wsUrl={}",
        target.port,
        target.browser,
        target.web_socket_debugger_url
    );
    Some(target)
}

/// TCP connect 探测端口是否监听（1s 超时）。
pub async fn is_port_listening(port: u16) -> bool {
    let addr = format!("127.0.0.1:{}", port);
    matches!(
        tokio::time::timeout(std::time::Duration::from_secs(1), tokio::net::TcpStream::connect(&addr)).await,
        Ok(Ok(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_browser_kind_chrome() {
        assert_eq!(parse_browser_kind("Chrome/120.0.6099.109"), BrowserKind::Chrome);
    }

    #[test]
    fn parse_browser_kind_edge() {
        assert_eq!(parse_browser_kind("Edge/120.0.6099.109"), BrowserKind::Edge);
        assert_eq!(parse_browser_kind("Microsoft Edge/120.0.6099.109"), BrowserKind::Edge);
    }

    #[test]
    fn parse_browser_kind_unknown() {
        assert_eq!(parse_browser_kind("Firefox/120.0"), BrowserKind::Unknown);
        assert_eq!(parse_browser_kind(""), BrowserKind::Unknown);
    }

    #[test]
    fn version_response_deserialize() {
        let json =
            r#"{"Browser":"Chrome/120.0.6099.109","webSocketDebuggerUrl":"ws://127.0.0.1:9222/devtools/browser/abc"}"#;
        let resp: VersionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.browser, "Chrome/120.0.6099.109");
        assert_eq!(resp.web_socket_debugger_url, "ws://127.0.0.1:9222/devtools/browser/abc");
    }

    #[tokio::test]
    async fn is_port_listening_returns_false_for_unlistened_port() {
        // 端口 19999 通常未监听（避开常用端口）
        let result = is_port_listening(19999).await;
        assert!(!result, "port 19999 should not be listening");
    }

    #[tokio::test]
    async fn probe_port_returns_none_for_unlistened_port() {
        let result = probe_port(19999).await;
        assert!(result.is_none(), "probe_port should return None for unlistened port");
    }

    #[tokio::test]
    async fn discover_targets_with_empty_ports_returns_empty() {
        let result = discover_targets_with_ports(&[]).await;
        assert!(result.is_empty());
    }
}
