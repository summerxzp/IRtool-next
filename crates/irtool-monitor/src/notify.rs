use crate::types::{Alert, NotifyAction};
use irtool_core::IrError;
use tracing::{info, warn};

fn no_proxy_client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default()
}

pub async fn send_notification(alert: &Alert, actions: &[NotifyAction]) {
    for action in actions {
        match action {
            NotifyAction::Popup => {
                // 弹窗通知由 Tauri 前端处理，通过事件 emit
                info!("Popup alert: {} - {}", alert.rule_name, alert.key_field);
            }
            NotifyAction::Feishu { webhook_url } => {
                if let Err(e) = send_feishu(webhook_url, alert).await {
                    warn!("飞书通知发送失败: {}", e);
                }
            }
        }
    }
}

pub async fn test_feishu_webhook(webhook_url: &str) -> Result<(), IrError> {
    let payload = serde_json::json!({
        "msg_type": "interactive",
        "card": {
            "header": {
                "title": { "tag": "plain_text", "content": "🧪 IRtool 飞书通知测试" }
            },
            "elements": [{
                "tag": "div",
                "text": {
                    "tag": "plain_text",
                    "content": "如果您看到此消息，说明飞书 Webhook 配置正确！"
                }
            }]
        }
    });
    let resp = no_proxy_client()
        .post(webhook_url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| IrError::Network(format!("请求失败: {}", e)))?;
    if !resp.status().is_success() {
        return Err(IrError::Network(format!("HTTP {}: {}", resp.status(), resp.text().await.unwrap_or_default())));
    }
    Ok(())
}

async fn send_feishu(webhook_url: &str, alert: &Alert) -> Result<(), String> {
    let time_str = chrono::DateTime::from_timestamp_millis(alert.timestamp)
        .map(|dt| dt.format("%Y/%m/%d %H:%M:%S").to_string())
        .unwrap_or_default();

    let raw: Option<serde_json::Value> = serde_json::from_str(&alert.raw_json).ok();
    let raw = match raw {
        Some(v) => v,
        None => {
            // raw_json 解析失败时回退到简单文本
            let payload = serde_json::json!({
                "msg_type": "interactive",
                "card": {
                    "header": {
                        "title": { "tag": "plain_text", "content": format!("🚨 IRtool 告警: {}", alert.rule_name) },
                        "template": "red"
                    },
                    "elements": [{
                        "tag": "div",
                        "text": { "tag": "lark_md", "content": format!("**目标**: {}\n**类型**: {}\n**时间**: {}", alert.key_field, alert.event_type, time_str) }
                    }]
                }
            });
            no_proxy_client().post(webhook_url).json(&payload).send().await.map_err(|e| format!("HTTP 请求失败: {}", e))?;
            return Ok(());
        }
    };

    let mut elements: Vec<serde_json::Value> = Vec::new();

    // 判断事件类型：Pcap 事件有 event_kind 字段
    let event_kind = raw.get("event_kind").and_then(|v| v.as_str()).unwrap_or("");

    if event_kind == "tls_sni" || event_kind == "dns_query" {
        // ---- Pcap 事件布局 ----
        // 第一行：源地址 / 目标地址
        let src_ip = raw.get("src_ip").and_then(|v| v.as_str()).unwrap_or("");
        let src_port = raw.get("src_port").and_then(|v| v.as_u64()).map(|p| format!(":{}", p)).unwrap_or_default();
        let dst_ip = raw.get("dst_ip").and_then(|v| v.as_str()).unwrap_or("");
        let dst_port = raw.get("dst_port").and_then(|v| v.as_u64()).map(|p| format!(":{}", p)).unwrap_or_default();

        elements.push(column_set(
            lark_md_col("**源地址**", &format!("{}{}", src_ip, src_port)),
            lark_md_col("**目标地址**", &format!("{}{}", dst_ip, dst_port)),
        ));

        // 第二行：域名 / 类型
        let domain = raw.get("domain").and_then(|v| v.as_str()).unwrap_or("-");
        let type_label = if event_kind == "tls_sni" { "TLS SNI" } else { "DNS查询" };
        elements.push(column_set(
            lark_md_col("**域名**", domain),
            lark_md_col("**类型**", type_label),
        ));

        // 第三行：时间
        elements.push(single_div(&format!("**时间**: {}", time_str)));

        // DNS 查询类型（仅 dns_query）
        if event_kind == "dns_query" {
            if let Some(qtype) = raw.get("query_type").and_then(|v| v.as_str()) {
                elements.push(single_div(&format!("**查询类型**: {}", qtype)));
            }
        }
    } else {
        // ---- Sysmon 事件布局 ----
        // 第一行：协议 / 用户
        let protocol = raw.get("protocol").and_then(|v| v.as_str()).unwrap_or("-");
        let user = raw.get("user").and_then(|v| v.as_str()).unwrap_or("-");
        elements.push(column_set(
            lark_md_col("**协议**", protocol),
            lark_md_col("**用户**", user),
        ));

        // 第二行：源地址 / 目标地址
        let src_ip = raw.get("source_ip").and_then(|v| v.as_str()).unwrap_or("-");
        let src_port = raw.get("source_port").and_then(|v| v.as_u64()).map(|p| format!(":{}", p)).unwrap_or_default();
        let dst_ip = raw.get("destination_ip").and_then(|v| v.as_str()).unwrap_or("-");
        let dst_port = raw.get("destination_port").and_then(|v| v.as_u64()).map(|p| format!(":{}", p)).unwrap_or_default();
        elements.push(column_set(
            lark_md_col("**源地址**", &format!("{}{}", src_ip, src_port)),
            lark_md_col("**目标地址**", &format!("{}{}", dst_ip, dst_port)),
        ));

        // 第三行：时间 / 进程
        let pid = raw.get("process_id").and_then(|v| v.as_u64()).map(|p| format!(" ({})", p)).unwrap_or_default();
        let proc_display = if alert.process_name.is_empty() {
            "-".to_string()
        } else {
            format!("{}{}", alert.process_name, pid)
        };
        elements.push(column_set(
            lark_md_col("**时间**", &time_str),
            lark_md_col("**进程**", &proc_display),
        ));

        // 路径（全宽）
        if let Some(image) = raw.get("process_path").and_then(|v| v.as_str()) {
            elements.push(single_div(&format!("**路径**\n{}", image)));
        }

        // 进程链（全宽）
        if let Some(chain) = raw.get("process_chain").and_then(|v| v.as_str()) {
            elements.push(single_div(&format!("**进程链**\n{}", chain)));
        }
    }

    let payload = serde_json::json!({
        "msg_type": "interactive",
        "card": {
            "header": {
                "title": { "tag": "plain_text", "content": format!("🚨 IRtool 告警: {}", alert.rule_name) },
                "template": "red"
            },
            "elements": elements
        }
    });

    no_proxy_client()
        .post(webhook_url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("HTTP 请求失败: {}", e))?;
    Ok(())
}

/// 构造两列 column_set
fn column_set(left: serde_json::Value, right: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "tag": "column_set",
        "flex_mode": "bisect",
        "background_style": "default",
        "columns": [
            {
                "tag": "column",
                "width": "weighted",
                "weight": 1,
                "elements": [left]
            },
            {
                "tag": "column",
                "width": "weighted",
                "weight": 1,
                "elements": [right]
            }
        ]
    })
}

/// 构造 lark_md 列内容（标签 + 值）
fn lark_md_col(label: &str, value: &str) -> serde_json::Value {
    serde_json::json!({
        "tag": "div",
        "text": { "tag": "lark_md", "content": format!("{}\n{}", label, value) }
    })
}

/// 构造全宽 lark_md div
fn single_div(content: &str) -> serde_json::Value {
    serde_json::json!({
        "tag": "div",
        "text": { "tag": "lark_md", "content": content }
    })
}
