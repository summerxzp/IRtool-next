use crate::types::MonitorEvent;
use crate::types::MonitorRule;

/// 检查事件是否匹配规则
pub fn matches_rule(event: &MonitorEvent, rule: &MonitorRule) -> bool {
    if !rule.enabled {
        return false;
    }
    // 事件类型过滤
    if !rule.event_types.is_empty() && !rule.event_types.contains(&event.event_type) {
        return false;
    }
    // 目标匹配
    for target in &rule.targets {
        if matches_target(&event.key_field, target) {
            return true;
        }
    }
    false
}

fn matches_target(key_field: &str, target: &str) -> bool {
    // CIDR 匹配
    if target.contains('/') {
        return matches_cidr(key_field, target);
    }
    // 中间层级通配符 *.doubao.*
    if target.starts_with("*.") && target.ends_with(".*") {
        let middle = &target[2..target.len() - 2];
        if middle.is_empty() {
            return false;
        }
        let key = key_field
            .rsplit_once(':')
            .map(|(k, _)| k)
            .unwrap_or(key_field);
        return key == middle
            || key.starts_with(&format!("{}.", middle))
            || key.ends_with(&format!(".{}", middle))
            || key.contains(&format!(".{}.", middle));
    }
    // 前缀通配符 *.evil.com
    if let Some(suffix) = target.strip_prefix("*.") {
        return key_field == suffix || key_field.ends_with(&format!(".{}", suffix));
    }
    // 精确匹配（域名或 IP:Port）
    if key_field == target {
        return true;
    }
    // IPv4:Port 匹配 IP 部分（排除 IPv6 中的冒号）
    if let Some(ip_part) = key_field.rsplit_once(':') {
        if ip_part.1.parse::<u16>().is_ok() && !ip_part.0.contains(':') && ip_part.0 == target {
            return true;
        }
    }
    false
}

fn matches_cidr(key_field: &str, cidr: &str) -> bool {
    // 提取 IP 部分（去掉 :Port），但排除 IPv6 地址中的冒号
    let ip_str = key_field
        .rsplit_once(':')
        .and_then(|(ip, port)| {
            if port.parse::<u16>().is_ok() && !ip.contains(':') {
                Some(ip)
            } else {
                None
            }
        })
        .unwrap_or(key_field);
    let Ok(network) = cidr.parse::<ipnetwork::IpNetwork>() else {
        return false;
    };
    let Ok(ip) = ip_str.parse::<std::net::IpAddr>() else {
        return false;
    };
    network.contains(ip)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EventSource;

    #[test]
    fn test_exact_match() {
        assert!(matches_target("evil.com", "evil.com"));
        assert!(!matches_target("good.com", "evil.com"));
    }

    #[test]
    fn test_wildcard_match() {
        assert!(matches_target("sub.evil.com", "*.evil.com"));
        assert!(matches_target("evil.com", "*.evil.com"));
        assert!(!matches_target("notevil.com", "*.evil.com"));
    }

    #[test]
    fn test_ip_port_match() {
        assert!(matches_target("1.2.3.4:443", "1.2.3.4"));
        assert!(!matches_target("1.2.3.5:443", "1.2.3.4"));
    }

    #[test]
    fn test_middle_wildcard_match() {
        assert!(matches_target("www.doubao.com", "*.doubao.*"));
        assert!(matches_target("doubao.com", "*.doubao.*"));
        assert!(matches_target("api.doubao.cn", "*.doubao.*"));
        assert!(matches_target("a.b.doubao.c.d", "*.doubao.*"));
        assert!(!matches_target("notdoubao.com", "*.doubao.*"));
        assert!(!matches_target("doubao2.com", "*.doubao.*"));
    }

    #[test]
    fn test_cidr_match() {
        assert!(matches_target("10.0.0.5:80", "10.0.0.0/8"));
        assert!(!matches_target("192.168.1.1:80", "10.0.0.0/8"));
    }

    #[test]
    fn test_empty_key_field_no_match() {
        assert!(!matches_target("", "evil.com"));
        assert!(!matches_target("", "1.2.3.4"));
        assert!(!matches_target("", "10.0.0.0/8"));
    }

    #[test]
    fn test_empty_targets_no_match() {
        let event = MonitorEvent {
            id: 1,
            timestamp: 0,
            source: EventSource::Sysmon,
            event_type: "dns".into(),
            process_name: "test".into(),
            key_field: "evil.com".into(),
            raw_json: "{}".into(),
        };
        let rule = MonitorRule {
            id: "r1".into(),
            name: "test".into(),
            targets: vec![],
            event_types: vec![],
            enabled: true,
        };
        assert!(!matches_rule(&event, &rule));
    }

    #[test]
    fn test_disabled_rule_no_match() {
        let event = MonitorEvent {
            id: 1,
            timestamp: 0,
            source: EventSource::Sysmon,
            event_type: "dns".into(),
            process_name: "test".into(),
            key_field: "evil.com".into(),
            raw_json: "{}".into(),
        };
        let rule = MonitorRule {
            id: "r1".into(),
            name: "test".into(),
            targets: vec!["evil.com".into()],
            event_types: vec![],
            enabled: false,
        };
        assert!(!matches_rule(&event, &rule));
    }

    #[test]
    fn test_event_type_filter_blocks_non_matching() {
        let event = MonitorEvent {
            id: 1,
            timestamp: 0,
            source: EventSource::Sysmon,
            event_type: "network_connect".into(),
            process_name: "test".into(),
            key_field: "evil.com".into(),
            raw_json: "{}".into(),
        };
        let rule = MonitorRule {
            id: "r1".into(),
            name: "test".into(),
            targets: vec!["evil.com".into()],
            event_types: vec!["dns".into()],
            enabled: true,
        };
        assert!(!matches_rule(&event, &rule));
    }

    #[test]
    fn test_event_type_filter_allows_matching() {
        let event = MonitorEvent {
            id: 1,
            timestamp: 0,
            source: EventSource::Sysmon,
            event_type: "dns".into(),
            process_name: "test".into(),
            key_field: "evil.com".into(),
            raw_json: "{}".into(),
        };
        let rule = MonitorRule {
            id: "r1".into(),
            name: "test".into(),
            targets: vec!["evil.com".into()],
            event_types: vec!["dns".into()],
            enabled: true,
        };
        assert!(matches_rule(&event, &rule));
    }

    #[test]
    fn test_empty_event_types_matches_all() {
        let event = MonitorEvent {
            id: 1,
            timestamp: 0,
            source: EventSource::Sysmon,
            event_type: "network_connect".into(),
            process_name: "test".into(),
            key_field: "evil.com".into(),
            raw_json: "{}".into(),
        };
        let rule = MonitorRule {
            id: "r1".into(),
            name: "test".into(),
            targets: vec!["evil.com".into()],
            event_types: vec![],
            enabled: true,
        };
        assert!(matches_rule(&event, &rule));
    }

    #[test]
    fn test_multiple_targets_any_match() {
        let event = MonitorEvent {
            id: 1,
            timestamp: 0,
            source: EventSource::Sysmon,
            event_type: "dns".into(),
            process_name: "test".into(),
            key_field: "b.com".into(),
            raw_json: "{}".into(),
        };
        let rule = MonitorRule {
            id: "r1".into(),
            name: "test".into(),
            targets: vec!["a.com".into(), "b.com".into(), "c.com".into()],
            event_types: vec![],
            enabled: true,
        };
        assert!(matches_rule(&event, &rule));
    }

    #[test]
    fn test_mixed_target_types() {
        let rule = MonitorRule {
            id: "r1".into(),
            name: "test".into(),
            targets: vec!["evil.com".into(), "10.0.0.0/8".into()],
            event_types: vec![],
            enabled: true,
        };
        // 域名匹配
        let event1 = MonitorEvent {
            id: 1,
            timestamp: 0,
            source: EventSource::Sysmon,
            event_type: "dns".into(),
            process_name: "test".into(),
            key_field: "evil.com".into(),
            raw_json: "{}".into(),
        };
        assert!(matches_rule(&event1, &rule));
        // CIDR 匹配
        let event2 = MonitorEvent {
            id: 2,
            timestamp: 0,
            source: EventSource::Sysmon,
            event_type: "network_connect".into(),
            process_name: "test".into(),
            key_field: "10.1.2.3:443".into(),
            raw_json: "{}".into(),
        };
        assert!(matches_rule(&event2, &rule));
    }

    #[test]
    fn test_ipv6_address_not_confused_with_port() {
        // IPv6 地址中的冒号不应被误认为 port 分隔符
        assert!(!matches_target("::1", "1"));
        assert!(!matches_target("fe80::1", "1"));
        // IPv6 精确匹配应正常工作
        assert!(matches_target("::1", "::1"));
    }

    #[test]
    fn test_wildcard_suffix_matches_exact_domain() {
        // *.evil.com 也匹配 evil.com 本身
        assert!(matches_target("evil.com", "*.evil.com"));
        assert!(matches_target("sub.evil.com", "*.evil.com"));
        assert!(!matches_target("notevil.com", "*.evil.com"));
    }

    #[test]
    fn test_cidr_ipv6() {
        assert!(matches_target("::1", "::1/128"));
        assert!(matches_target("fe80::1", "fe80::/10"));
        assert!(!matches_target("::1", "fe80::/10"));
    }
}
