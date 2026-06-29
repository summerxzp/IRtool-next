//! 最小化 CDP 诊断：列出所有 target，尝试 attach 一个 page target 并 Network.enable。

use irtool_cdp::discovery::discover_targets;
use irtool_cdp::CdpClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let targets = discover_targets().await;
    let ws_url = &targets[0].web_socket_debugger_url;
    println!("ws_url: {}", ws_url);

    let client = CdpClient::connect(ws_url).await?;

    // 列出所有 target
    let target_list = client.get_targets().await?;
    println!("\n=== 所有 target (Target.getTargets) ===");
    for t in &target_list {
        println!("  type={:20} id={} url={}", t.target_type, t.target_id, t.url);
    }

    // 找一个 page target
    let page_target = target_list.iter().find(|t| t.target_type == "page");
    if let Some(pt) = page_target {
        println!("\n=== 尝试 attach page target: {} ===", pt.target_id);
        match client.attach_to_target(&pt.target_id).await {
            Ok(sid) => {
                println!("attach 成功: sessionId={}", sid.0);
                println!("尝试 Network.enable...");
                match client.enable_network(&sid).await {
                    Ok(_) => println!("Network.enable 成功！"),
                    Err(e) => println!("Network.enable 失败: {:?}", e),
                }
                // 等 2 秒看有没有事件
                println!("等待 2 秒收集事件...");
                for _ in 0..2 {
                    if let Some(evt) = client.recv_event().await {
                        println!("收到事件: method={} session_id={:?}", evt.method, evt.session_id);
                    }
                }
                let _ = client.detach_from_target(&sid).await;
            }
            Err(e) => println!("attach 失败: {:?}", e),
        }
    } else {
        println!("没有 page target");
    }

    Ok(())
}
