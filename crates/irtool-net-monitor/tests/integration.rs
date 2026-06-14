use irtool_net_monitor::{
    CmdlineEnricher, ConnState, Family, HistoryStore, NetCollector, NetConn, NetEndpoint, ProcessInfoCache, Proto,
    RetentionPolicy, WindowsNetCollector,
};

#[allow(clippy::too_many_arguments)]
fn make_conn(
    proto: Proto,
    family: Family,
    local_addr: &str,
    local_port: u16,
    remote_addr: &str,
    remote_port: u16,
    pid: u32,
    state: ConnState,
) -> NetConn {
    NetConn {
        proto,
        family,
        local: NetEndpoint {
            addr: local_addr.to_string(),
            port: local_port,
        },
        remote: NetEndpoint {
            addr: remote_addr.to_string(),
            port: remote_port,
        },
        state,
        pid,
        process_name: None,
        process_path: None,
        process_cmdline: None,
        cmdline_status: Default::default(),
        first_seen: 0,
        last_seen: 0,
        is_current: true,
    }
}

#[test]
fn test_historystore_merge_basic() {
    let store = HistoryStore::new();
    let retention = RetentionPolicy::Seconds(3600);

    let snapshot1 = vec![
        make_conn(
            Proto::Tcp,
            Family::V4,
            "192.168.1.1",
            10000,
            "10.0.0.1",
            80,
            1001,
            ConnState::Established,
        ),
        make_conn(
            Proto::Tcp,
            Family::V4,
            "192.168.1.1",
            10001,
            "10.0.0.2",
            443,
            1002,
            ConnState::Established,
        ),
    ];

    let merged1 = store.merge(snapshot1, retention);
    assert_eq!(merged1.len(), 2, "first merge should have 2 connections");
    assert_eq!(store.len(), 2);

    let snapshot2 = vec![make_conn(
        Proto::Tcp,
        Family::V4,
        "192.168.1.1",
        10002,
        "10.0.0.3",
        8080,
        1003,
        ConnState::Established,
    )];

    let merged2 = store.merge(snapshot2, retention);
    assert_eq!(merged2.len(), 3, "after second merge store should have 3 connections");
    assert_eq!(store.len(), 3);
}

#[test]
fn test_historystore_retention_trims_old() {
    let store = HistoryStore::new();
    let retention_forever = RetentionPolicy::Forever;

    let conns: Vec<NetConn> = (0..5u32)
        .map(|i| {
            make_conn(
                Proto::Tcp,
                Family::V4,
                "192.168.1.1",
                (10000 + i) as u16,
                "10.0.0.1",
                80,
                1000 + i,
                ConnState::Established,
            )
        })
        .collect();

    let merged = store.merge(conns, retention_forever);
    assert_eq!(merged.len(), 5);
    assert_eq!(store.len(), 5);

    let merged_none = store.merge(Vec::<NetConn>::new(), RetentionPolicy::None);
    assert_eq!(
        merged_none.len(),
        0,
        "with RetentionPolicy::None all historical connections should be dropped"
    );
    assert_eq!(store.len(), 0);
}

#[test]
fn test_historystore_clear_then_merge() {
    let store = HistoryStore::new();
    let retention = RetentionPolicy::Forever;

    let snapshot = vec![
        make_conn(
            Proto::Tcp,
            Family::V4,
            "192.168.1.1",
            10000,
            "10.0.0.1",
            80,
            1001,
            ConnState::Established,
        ),
        make_conn(
            Proto::Tcp,
            Family::V4,
            "192.168.1.1",
            10001,
            "10.0.0.2",
            443,
            1002,
            ConnState::Established,
        ),
    ];

    let merged = store.merge(snapshot, retention);
    assert_eq!(merged.len(), 2);
    assert_eq!(store.len(), 2);

    let only_first = vec![make_conn(
        Proto::Tcp,
        Family::V4,
        "192.168.1.1",
        10000,
        "10.0.0.1",
        80,
        1001,
        ConnState::Established,
    )];
    let merged2 = store.merge(only_first, retention);
    assert_eq!(merged2.len(), 2, "historical + current still 2");
    let historical_count = merged2.iter().filter(|c| !c.is_current).count();
    assert_eq!(historical_count, 1, "missing conn should be marked historical");

    store.clear_history();
    assert_eq!(
        store.len(),
        1,
        "after clear_history, only current entries should remain"
    );

    let new_snapshot = vec![make_conn(
        Proto::Udp,
        Family::V4,
        "192.168.1.1",
        20000,
        "10.0.0.2",
        443,
        2001,
        ConnState::None,
    )];

    let merged3 = store.merge(new_snapshot, retention);
    assert_eq!(merged3.len(), 2, "after clear_history + new merge = 2");
    assert_eq!(store.len(), 2);
}

#[test]
fn test_historystore_merge_dedup_same_conn() {
    let store = HistoryStore::new();
    let retention = RetentionPolicy::Forever;

    let conn_a = make_conn(
        Proto::Tcp,
        Family::V4,
        "192.168.1.1",
        10000,
        "10.0.0.1",
        80,
        1001,
        ConnState::Established,
    );

    let snapshot1 = vec![conn_a.clone()];
    let merged1 = store.merge(snapshot1, retention);
    assert_eq!(merged1.len(), 1);
    assert_eq!(store.len(), 1);

    let snapshot2 = vec![conn_a.clone()];
    let merged2 = store.merge(snapshot2, retention);
    assert_eq!(
        merged2.len(),
        1,
        "same connection merged multiple times should remain at 1 entry"
    );
    assert_eq!(store.len(), 1);
}

#[cfg(windows)]
#[test]
fn test_collector_snapshot_returns_data() {
    let collector = WindowsNetCollector::new();
    let conns = collector.snapshot().expect("snapshot should succeed");
    assert!(
        !conns.is_empty(),
        "snapshot should return at least one connection on Windows"
    );

    let has_tcp = conns.iter().any(|c| c.proto == Proto::Tcp);
    let has_udp = conns.iter().any(|c| c.proto == Proto::Udp);
    assert!(has_tcp || has_udp, "should have TCP or UDP connections");

    for conn in &conns {
        assert!(
            conn.process_name.is_some(),
            "every connection should have a process name enriched"
        );
    }
}

#[test]
fn test_cmdline_enricher_on_test_conns() {
    let enricher = CmdlineEnricher::new();
    let current_pid = std::process::id();
    let fake_pids = vec![current_pid, 12345, 67890];

    enricher.enqueue(&fake_pids);
    let pending = enricher.drain_pending(10);
    assert_eq!(pending.len(), 3, "all three PIDs should be queued");

    let mut conns = vec![make_conn(
        Proto::Tcp,
        Family::V4,
        "127.0.0.1",
        12345,
        "127.0.0.1",
        80,
        current_pid,
        ConnState::Established,
    )];

    let cache = ProcessInfoCache::new();
    for c in &mut conns {
        let info = cache.get(c.pid);
        c.process_name = Some(info.name);
        if let Some(path) = info.path {
            c.process_path = Some(path.to_string_lossy().into_owned());
        }
    }

    assert!(
        conns[0].process_name.is_some(),
        "process name should be enriched via ProcessInfoCache"
    );
}

#[test]
fn test_netconn_key_uniqueness() {
    let conn1 = make_conn(
        Proto::Tcp,
        Family::V4,
        "192.168.1.1",
        10000,
        "10.0.0.1",
        80,
        1001,
        ConnState::Established,
    );

    let conn2 = make_conn(
        Proto::Tcp,
        Family::V4,
        "192.168.1.1",
        10001,
        "10.0.0.2",
        443,
        1002,
        ConnState::Established,
    );

    let conn3 = make_conn(
        Proto::Udp,
        Family::V6,
        "::1",
        10000,
        "fe80::1",
        80,
        1003,
        ConnState::None,
    );

    assert_ne!(conn1.key(), conn2.key(), "different ports produce different keys");
    assert_ne!(
        conn1.key(),
        conn3.key(),
        "different protocols/families produce different keys"
    );
    assert_ne!(conn2.key(), conn3.key());

    let mut seen = std::collections::HashSet::new();
    assert!(seen.insert(conn1.key()));
    assert!(seen.insert(conn2.key()));
    assert!(seen.insert(conn3.key()));
    assert_eq!(seen.len(), 3);
}

#[cfg(windows)]
#[test]
fn test_process_info_lookup_for_current() {
    let current_pid = std::process::id();
    let cache = ProcessInfoCache::new();
    let info = cache.get(current_pid);

    assert!(!info.name.is_empty(), "current process should have a non-empty name");
}

#[test]
fn test_process_info_lookup_invalid_pid_returns_unknown() {
    let cache = ProcessInfoCache::new();
    let info = cache.get(999_999_999);
    assert!(
        !info.name.is_empty(),
        "even invalid PIDs should return a label (e.g. 已结束 / 权限不足)"
    );
}

#[test]
fn test_historystore_clear_all() {
    let store = HistoryStore::new();
    let retention = RetentionPolicy::Forever;

    let snapshot = vec![
        make_conn(
            Proto::Tcp,
            Family::V4,
            "192.168.1.1",
            10000,
            "10.0.0.1",
            80,
            1001,
            ConnState::Established,
        ),
        make_conn(
            Proto::Udp,
            Family::V6,
            "::1",
            20000,
            "fe80::1",
            443,
            1002,
            ConnState::None,
        ),
    ];

    let _ = store.merge(snapshot, retention);
    assert_eq!(store.len(), 2);

    store.clear_all();
    assert_eq!(store.len(), 0, "clear_all should remove all entries");
}

#[test]
fn test_netconn_fields_v6() {
    let conn = make_conn(
        Proto::Udp,
        Family::V6,
        "::1",
        53,
        "2001:db8::1",
        5353,
        500,
        ConnState::None,
    );

    assert_eq!(conn.proto, Proto::Udp);
    assert_eq!(conn.family, Family::V6);
    assert_eq!(conn.local.addr, "::1");
    assert_eq!(conn.local.port, 53);
    assert_eq!(conn.remote.addr, "2001:db8::1");
    assert_eq!(conn.remote.port, 5353);
    assert_eq!(conn.pid, 500);
    assert_eq!(conn.state, ConnState::None);
}

#[test]
fn test_retention_seconds_marks_historical() {
    let store = HistoryStore::new();
    let retention_secs = RetentionPolicy::Seconds(60);

    let first_snapshot = vec![make_conn(
        Proto::Tcp,
        Family::V4,
        "192.168.1.1",
        10000,
        "10.0.0.1",
        80,
        1001,
        ConnState::Established,
    )];

    let merged1 = store.merge(first_snapshot, retention_secs);
    assert_eq!(merged1.len(), 1);
    assert!(merged1[0].is_current, "first snapshot entry should be current");

    let merged2 = store.merge(Vec::<NetConn>::new(), retention_secs);
    assert_eq!(merged2.len(), 1, "within 60s historical entries should be kept");
    assert!(!merged2[0].is_current, "missing entry should be marked historical");
}

#[test]
fn test_connstate_variants() {
    let _ = ConnState::Established;
    let _ = ConnState::Listen;
    let _ = ConnState::Closed;
    let _ = ConnState::SynSent;
    let _ = ConnState::SynRcvd;
    let _ = ConnState::FinWait1;
    let _ = ConnState::FinWait2;
    let _ = ConnState::CloseWait;
    let _ = ConnState::Closing;
    let _ = ConnState::LastAck;
    let _ = ConnState::TimeWait;
    let _ = ConnState::DeleteTcb;
    let _ = ConnState::None;
}

#[test]
fn test_historystore_is_empty_after_clear() {
    let store = HistoryStore::new();
    assert!(store.is_empty(), "new store should be empty");

    let _ = store.merge(
        vec![make_conn(
            Proto::Tcp,
            Family::V4,
            "192.168.1.1",
            10000,
            "10.0.0.1",
            80,
            1001,
            ConnState::Established,
        )],
        RetentionPolicy::Forever,
    );

    assert!(!store.is_empty(), "after merge, store should not be empty");

    store.clear_all();
    assert!(store.is_empty(), "after clear_all, store should be empty again");
}
