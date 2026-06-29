//! CDP 抓包端到端测试。
//!
//! 前置条件：Chrome/Edge 已以 `--remote-debugging-port=9222` 启动。
//! 本程序会：
//! 1. 启动 CdpCaptureService 抓包
//! 2. 通过 CDP 控制浏览器导航到测试 URL（触发请求）
//! 3. 等待并打印抓到的 ExtensionAttribution 事件
//! 4. 停止服务
//!
//! 运行：`cargo run --example cdp_capture_e2e -p irtool-service`

use std::time::Duration;

use irtool_cdp::discovery::discover_targets;
use irtool_cdp::CdpClient;
use irtool_service::event_bus::{AppEvent, EventBus};
use irtool_service::services::cdp_capture::CdpCaptureService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 tracing 日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "irtool_service=debug,irtool_cdp=debug,info".into()),
        )
        .init();

    println!("[E2E] 步骤 1: 发现浏览器调试端口...");
    let targets = discover_targets().await;
    if targets.is_empty() {
        println!("[E2E] 失败：未发现浏览器调试端口。请先启动 Chrome：");
        println!("      chrome --remote-debugging-port=9222 --user-data-dir=<临时目录>");
        return Err("no CDP target".into());
    }
    let browser_ws = &targets[0].web_socket_debugger_url;
    println!("[E2E] 发现浏览器: {:?} (ws: {})", targets[0].browser, browser_ws);

    println!("[E2E] 步骤 2: 启动 CdpCaptureService...");
    let event_bus = EventBus::new();
    let mut rx = event_bus.subscribe();
    let capture_service = CdpCaptureService::start(event_bus.clone()).await?;
    println!("[E2E] CdpCaptureService 已启动，等待 attach 完成...");
    tokio::time::sleep(Duration::from_secs(2)).await;

    println!("[E2E] 步骤 3: 通过 CDP 触发导航请求...");
    // 连接 browser-level WebSocket，创建新 tab 导航到测试 URL
    let client = CdpClient::connect(browser_ws).await?;
    // Target.createTarget 创建新标签页
    let test_url = "https://sec.summer233.dpdns.org/";
    let result = client
        .send_command("Target.createTarget", serde_json::json!({ "url": test_url }))
        .await?;
    let target_id = result.get("targetId").and_then(|v| v.as_str());
    println!("[E2E] 新 tab targetId: {:?}", target_id);

    println!("[E2E] 步骤 4: 等待抓包事件（最多 10 秒）...");
    let mut got_events = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(1), rx.recv()).await {
            Ok(Ok(AppEvent::ExtensionAttribution(evt))) => {
                println!(
                    "[E2E] 抓到事件: {} {} | type={:?} | status={} | ext_id={:?}",
                    evt.method, evt.url, evt.resource_type, evt.attribution_status, evt.extension_id
                );
                got_events.push(evt);
                if got_events.len() >= 3 {
                    break;
                }
            }
            Ok(Ok(other)) => {
                // 其他事件忽略
                tracing::debug!(?other, "其他事件");
            }
            Ok(Err(e)) => {
                println!("[E2E] broadcast error: {}", e);
                break;
            }
            Err(_) => {
                // timeout，继续等
            }
        }
    }

    println!("[E2E] 步骤 5: 停止 CdpCaptureService...");
    capture_service.stop().await?;

    // 汇总
    println!("\n========== 端到端测试结果 ==========");
    println!("抓到事件数: {}", got_events.len());
    if got_events.is_empty() {
        println!("失败：未抓到任何事件");
        return Err("no events captured".into());
    }
    for (i, evt) in got_events.iter().enumerate() {
        println!(
            "  #{}: {} {} | resource_type={:?} | attribution={} | ext={:?}",
            i + 1,
            evt.method,
            evt.url,
            evt.resource_type,
            evt.attribution_status,
            evt.extension_id
        );
    }
    // 验证：至少有一个事件的 url 包含测试域名
    let has_test_domain = got_events.iter().any(|e| e.url.contains("summer233.dpdns.org"));
    if has_test_domain {
        println!("\n成功：抓到了测试域名的请求，CDP 抓包通路已打通！");
    } else {
        println!("\n警告：抓到事件但无测试域名请求（可能只是其他资源请求）");
    }
    Ok(())
}
