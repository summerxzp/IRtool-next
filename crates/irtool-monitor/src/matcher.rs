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
    // 通配符 *.evil.com
    if let Some(suffix) = target.strip_prefix("*.") {
        return key_field == suffix || key_field.ends_with(&format!(".{}", suffix));
    }
    // 精确匹配（域名或 IP:Port）
    if key_field == target {
        return true;
    }
    // IPv4:Port 匹配 IP 部分（排除 IPv6 中的冒号）
    if let Some(ip_part) = key_field.rsplit_once(':') {
        // 只有当冒号后面是纯数字时才视为 port 分隔符
        if ip_part.1.parse::<u16>().is_ok() {
            if ip_part.0 == target {
                return true;
            }
        }
    }
    false
}

fn matches_cidr(key_field: &str, cidr: &str) -> bool {
    // 提取 IP 部分（去掉 :Port）
    let ip_str = key_field.rsplit_once(':').map(|(ip, _)| ip).unwrap_or(key_field);
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
    fn test_cidr_match() {
        assert!(matches_target("10.0.0.5:80", "10.0.0.0/8"));
        assert!(!matches_target("192.168.1.1:80", "10.0.0.0/8"));
    }
}
