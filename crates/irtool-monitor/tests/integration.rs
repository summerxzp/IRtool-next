use irtool_monitor::{EventQuery, EventSource, MonitorConfig, MonitorEngine, MonitorEvent, RuntimeMode, RuntimeTelemetry};
use std::time::Duration;

fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("临时目录创建失败")
}

fn make_event(
    timestamp: i64,
    source: EventSource,
    event_type: &str,
    process_name: &str,
    key_field: &str,
    raw_json: &str,
) -> MonitorEvent {
    MonitorEvent {
        id: 0,
        timestamp,
        source,
        event_type: event_type.to_string(),
        process_name: process_name.to_string(),
        key_field: key_field.to_string(),
        raw_json: raw_json.to_string(),
    }
}

fn build_engine_in_background(app_dir: &std::path::Path) -> MonitorEngine {
    let mut engine = MonitorEngine::new(app_dir);
    engine.enter_background_mode(app_dir).expect("进入后台模式失败");
    engine
}

#[tokio::test]
async fn test_end_to_end_event_pipeline() {
    let dir = temp_dir();
    let engine = build_engine_in_background(dir.path());
    assert_eq!(engine.get_telemetry().mode, RuntimeMode::Background);

    let events: Vec<MonitorEvent> = (0..5)
        .map(|i| {
            make_event(
                1_000_000_000_000 + i * 1000,
                EventSource::Sysmon,
                "dns",
                "chrome.exe",
                "example.com",
                r#"{"process_id": 1234}"#,
            )
        })
        .collect();

    for e in &events {
        engine.process_monitor_event(e).await;
    }
    tokio::time::sleep(Duration::from_millis(700)).await;

    let count = engine.get_event_count().expect("查询事件总数失败");
    assert_eq!(count, 5);

    let recent = engine.get_recent_events(10).expect("查询最近事件失败");
    assert_eq!(recent.len(), 5);
    assert!(recent.iter().all(|e| e.id > 0));
    assert!(recent.iter().all(|e| e.event_type == "dns"));
    assert!(engine.get_telemetry().last_event_at.is_some());
}

#[tokio::test]
async fn test_telemetry_updates_on_events() {
    let dir = temp_dir();
    let engine = build_engine_in_background(dir.path());
    let before = engine.get_telemetry();

    engine.process_monitor_event(&make_event(2_000_000_000_000, EventSource::DnsClient, "dns_client", "firefox.exe", "test.com", "{}")).await;
    engine.process_monitor_event(&make_event(2_000_000_001_000, EventSource::NetMonitor, "network_connect", "curl.exe", "1.2.3.4:443", "{}")).await;

    tokio::time::sleep(Duration::from_millis(700)).await;

    let after = engine.get_telemetry();
    assert!(after.events_written >= before.events_written + 2);
    assert_eq!(after.last_event_at, Some(2_000_000_001_000));
    assert_eq!(after.mode, RuntimeMode::Background);
    assert!(after.started_at.is_some());
}

#[tokio::test]
async fn test_event_type_counts_aggregates_by_type() {
    let dir = temp_dir();
    let engine = build_engine_in_background(dir.path());

    for _ in 0..3 {
        engine.process_monitor_event(&make_event(3_000_000_000_000, EventSource::Sysmon, "dns", "app.exe", "a.com", "{}")).await;
    }
    for _ in 0..5 {
        engine.process_monitor_event(&make_event(3_000_000_000_001, EventSource::DnsClient, "dns_client", "svc.exe", "b.com", "{}")).await;
    }
    for _ in 0..2 {
        engine.process_monitor_event(&make_event(3_000_000_000_002, EventSource::NetMonitor, "network_connect", "net.exe", "3.3.3.3:80", "{}")).await;
    }

    tokio::time::sleep(Duration::from_millis(700)).await;

    let counts = engine.get_event_type_counts().expect("查询类型统计失败");
    let lookup: std::collections::HashMap<String, u64> = counts.into_iter().collect();

    assert_eq!(lookup.get("dns").copied(), Some(3));
    assert_eq!(lookup.get("dns_client").copied(), Some(5));
    assert_eq!(lookup.get("network_connect").copied(), Some(2));
}

#[tokio::test]
async fn test_search_with_multiple_filters() {
    let dir = temp_dir();
    let engine = build_engine_in_background(dir.path());

    let e1 = make_event(4_000_000_000_000, EventSource::Sysmon, "dns", "chrome.exe", "evil.example.com", "{}");
    let e2 = make_event(4_000_000_000_001, EventSource::Sysmon, "dns", "chrome.exe", "safe.example.com", "{}");
    let e3 = make_event(4_000_000_000_002, EventSource::DnsClient, "dns_client", "chrome.exe", "evil.example.com", "{}");
    let e4 = make_event(4_000_000_000_003, EventSource::Sysmon, "network_connect", "firefox.exe", "evil.example.com", "{}");

    for e in &[e1, e2, e3, e4] {
        engine.process_monitor_event(e).await;
    }
    tokio::time::sleep(Duration::from_millis(700)).await;

    // 确认数据被写入了
    let total = engine.get_event_count().unwrap();
    assert_eq!(total, 4, "应当有 4 条事件写入");

    // 仅按 source=sysmon 过滤（不加其他）
    let q_source_only = EventQuery {
        source: Some("sysmon".to_string()),
        event_type: None,
        process_name: None,
        key_field: None,
        is_external: None,
        search_text: None,
        limit: 10,
        offset: 0,
    };
    let r_source = engine.search_events(&q_source_only).expect("source-only 搜索失败");
    assert_eq!(r_source.len(), 3, "3 条 sysmon 事件");

    let query = EventQuery {
        source: Some("sysmon".to_string()),
        event_type: Some("dns".to_string()),
        process_name: None,
        key_field: None,
        is_external: None,
        search_text: Some("evil".to_string()),
        limit: 10,
        offset: 0,
    };
    let results = engine.search_events(&query).expect("搜索事件失败");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].event_type, "dns");
    assert_eq!(results[0].key_field, "evil.example.com");

    let query2 = EventQuery {
        source: Some("dns_client".to_string()),
        event_type: None,
        process_name: None,
        key_field: None,
        is_external: None,
        search_text: None,
        limit: 10,
        offset: 0,
    };
    let results2 = engine.search_events(&query2).expect("按 source 搜索失败");
    assert_eq!(results2.len(), 1);
    assert_eq!(results2[0].source, EventSource::DnsClient);
}

#[tokio::test]
async fn test_pagination_correctness() {
    let dir = temp_dir();
    let engine = build_engine_in_background(dir.path());

    for i in 0..7 {
        engine
            .process_monitor_event(&make_event(
                5_000_000_000_000 + i * 1000,
                EventSource::Sysmon,
                "dns",
                &format!("proc{}.exe", i),
                "a.com",
                "{}",
            ))
            .await;
    }
    tokio::time::sleep(Duration::from_millis(700)).await;

    let page1 = engine
        .search_events_page(&EventQuery {
            source: None, event_type: None, process_name: None,
            key_field: None, is_external: None, search_text: None,
            limit: 3, offset: 0,
        })
        .expect("分页查询失败");
    assert_eq!(page1.total, 7);
    assert_eq!(page1.items.len(), 3);

    let page2 = engine
        .search_events_page(&EventQuery {
            source: None, event_type: None, process_name: None,
            key_field: None, is_external: None, search_text: None,
            limit: 3, offset: 3,
        })
        .expect("分页查询失败");
    assert_eq!(page2.items.len(), 3);
    assert_eq!(page2.total, 7);

    let page3 = engine
        .search_events_page(&EventQuery {
            source: None, event_type: None, process_name: None,
            key_field: None, is_external: None, search_text: None,
            limit: 3, offset: 6,
        })
        .expect("分页查询失败");
    assert_eq!(page3.items.len(), 1);

    let page4 = engine
        .search_events_page(&EventQuery {
            source: None, event_type: None, process_name: None,
            key_field: None, is_external: None, search_text: None,
            limit: 3, offset: 100,
        })
        .expect("分页查询失败");
    assert_eq!(page4.items.len(), 0);
    assert_eq!(page4.total, 7);

    assert!(page1.items[0].timestamp >= page1.items[2].timestamp);
}

#[tokio::test]
async fn test_clear_events_and_repopulate() {
    let dir = temp_dir();
    let engine = build_engine_in_background(dir.path());

    for i in 0..4 {
        engine
            .process_monitor_event(&make_event(
                6_000_000_000_000 + i * 1000,
                EventSource::Sysmon,
                "dns",
                "a.exe",
                "a.com",
                "{}",
            ))
            .await;
    }
    tokio::time::sleep(Duration::from_millis(700)).await;
    assert_eq!(engine.get_event_count().unwrap(), 4);

    let deleted = engine.clear_events().expect("清空事件失败");
    assert_eq!(deleted, 4);
    assert_eq!(engine.get_event_count().unwrap(), 0);

    for i in 0..2 {
        engine
            .process_monitor_event(&make_event(
                6_000_000_010_000 + i * 1000,
                EventSource::Sysmon,
                "network_connect",
                "b.exe",
                "2.2.2.2:80",
                "{}",
            ))
            .await;
    }
    tokio::time::sleep(Duration::from_millis(700)).await;

    assert_eq!(engine.get_event_count().unwrap(), 2);
    let recent = engine.get_recent_events(10).unwrap();
    assert!(recent.iter().all(|e| e.event_type == "network_connect"));
}

#[tokio::test]
async fn test_config_persistence_roundtrip() {
    let dir = temp_dir();

    {
        let engine = MonitorEngine::new(dir.path());
        let mut cfg = engine.get_config();
        cfg.retention_days = 30;
        cfg.load_limit = 500;
        cfg.enable_sni = true;
        cfg.notify_config.popup_duration_secs = 15;
        engine.update_config(cfg).expect("写入配置失败");
    }

    let engine2 = MonitorEngine::new(dir.path());
    let cfg2 = engine2.get_config();
    assert_eq!(cfg2.retention_days, 30);
    assert_eq!(cfg2.load_limit, 500);
    assert!(cfg2.enable_sni);
    assert_eq!(cfg2.notify_config.popup_duration_secs, 15);
}

#[tokio::test]
async fn test_alert_flow() {
    let dir = temp_dir();
    let engine = build_engine_in_background(dir.path());

    let rule_id = "rule_alert_flow".to_string();
    let mut cfg = engine.get_config();

    cfg.rules.push(irtool_monitor::MonitorRule {
        id: rule_id.clone(),
        name: "检测恶意域名".to_string(),
        targets: vec!["*.evil.com".to_string()],
        event_types: vec!["dns".to_string()],
        enabled: true,
    });
    cfg.notify_config.popup_rule_ids.push(rule_id);
    engine.update_config(cfg).expect("写入告警规则配置失败");

    let matched = make_event(
        8_000_000_000_000,
        EventSource::Sysmon,
        "dns",
        "malware.exe",
        "foo.evil.com",
        r#"{"process_id": 999}"#,
    );
    let alerts_from_process = engine.process_monitor_event(&matched).await;
    assert!(!alerts_from_process.is_empty());
    assert_eq!(alerts_from_process[0].rule_name, "检测恶意域名");
    assert_eq!(alerts_from_process[0].key_field, "foo.evil.com");
    assert_eq!(alerts_from_process[0].event_type, "dns");
    assert!(alerts_from_process[0].action_taken.contains("popup"));

    let wrong_type = make_event(
        8_000_000_000_001,
        EventSource::Sysmon,
        "network_connect",
        "malware.exe",
        "bar.evil.com",
        "{}",
    );
    let no_alert = engine.process_monitor_event(&wrong_type).await;
    assert!(no_alert.is_empty());

    let safe = make_event(
        8_000_000_000_002,
        EventSource::Sysmon,
        "dns",
        "safe.exe",
        "safe.example.com",
        "{}",
    );
    let no_alert2 = engine.process_monitor_event(&safe).await;
    assert!(no_alert2.is_empty());

    tokio::time::sleep(Duration::from_millis(700)).await;

    let stored_alerts = engine.get_recent_alerts(10).expect("查询告警失败");
    assert!(stored_alerts.iter().any(|a| a.key_field == "foo.evil.com"));
    assert!(stored_alerts.iter().any(|a| a.rule_name == "检测恶意域名"));
    assert_eq!(engine.get_event_count().unwrap(), 3);

    let deleted = engine.clear_alerts().expect("清空告警失败");
    assert!(deleted >= 1);
    assert_eq!(engine.get_recent_alerts(10).unwrap().len(), 0);
}

#[tokio::test]
async fn test_runtime_telemetry_debug_output() {
    let dir = temp_dir();
    let engine = build_engine_in_background(dir.path());
    let t: RuntimeTelemetry = engine.get_telemetry();
    let debug_str = format!("{:?}", t);
    assert!(debug_str.contains("Background"));
}

#[tokio::test]
async fn test_config_rules_persist_across_engine_instances() {
    let dir = temp_dir();

    {
        let engine = MonitorEngine::new(dir.path());
        let mut cfg: MonitorConfig = engine.get_config();
        cfg.rules.push(irtool_monitor::MonitorRule {
            id: "persist_rule".to_string(),
            name: "持久化规则".to_string(),
            targets: vec!["persist.example.com".to_string()],
            event_types: vec!["dns".to_string()],
            enabled: true,
        });
        engine.update_config(cfg).expect("写入规则失败");
    }

    let engine2 = MonitorEngine::new(dir.path());
    let cfg2 = engine2.get_config();
    assert_eq!(cfg2.rules.len(), 1);
    assert_eq!(cfg2.rules[0].name, "持久化规则");
    assert_eq!(cfg2.rules[0].targets, vec!["persist.example.com"]);
}

#[tokio::test]
async fn test_cleanup_expires_old_events() {
    let dir = temp_dir();
    let engine = build_engine_in_background(dir.path());

    let now_ms = chrono::Utc::now().timestamp_millis();
    engine
        .process_monitor_event(&make_event(
            now_ms - 10 * 24 * 3600 * 1000,
            EventSource::Sysmon,
            "dns",
            "old.exe",
            "old.com",
            "{}",
        ))
        .await;
    engine
        .process_monitor_event(&make_event(
            now_ms - 1 * 24 * 3600 * 1000,
            EventSource::Sysmon,
            "dns",
            "recent.exe",
            "recent.com",
            "{}",
        ))
        .await;

    tokio::time::sleep(Duration::from_millis(700)).await;
    assert_eq!(engine.get_event_count().unwrap(), 2);

    let deleted = engine.cleanup().expect("清理失败");
    assert!(deleted >= 1);
    let remaining = engine.get_recent_events(10).unwrap();
    assert!(remaining.iter().all(|e| e.process_name != "old.exe"));
    assert!(remaining.iter().any(|e| e.process_name == "recent.exe"));
}
