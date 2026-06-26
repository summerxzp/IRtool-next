//! Phase 1 集成测试：本机浏览器数据读取验证

use irtool_browser_forensics::{attribute_history, enumerate_profiles, scan_extensions, BrowserKind, TimeTier};

/// 验证本机 Chrome Profile 枚举和扩展扫描
#[test]
fn local_chrome_extension_scan() {
    let profiles = enumerate_profiles(BrowserKind::Chrome);
    if profiles.is_empty() {
        eprintln!("SKIP: Chrome not installed or no profiles found");
        return;
    }

    for profile in &profiles {
        let inventory = scan_extensions(profile);
        eprintln!(
            "[Chrome/{}] Found {} extensions",
            profile.name,
            inventory.extensions.len()
        );

        for ext in &inventory.extensions {
            let risk = if ext.risk_flags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", ext.risk_flags.join(", "))
            };
            eprintln!(
                "  - {} v{} ({}) {}{}",
                ext.name,
                ext.version,
                ext.id,
                if ext.enabled { "enabled" } else { "disabled" },
                risk
            );
        }

        // 至少应该有一些基本断言
        assert_eq!(inventory.browser, BrowserKind::Chrome);
    }
}

/// 验证本机 Edge Profile 枚举和扩展扫描
#[test]
fn local_edge_extension_scan() {
    let profiles = enumerate_profiles(BrowserKind::Edge);
    if profiles.is_empty() {
        eprintln!("SKIP: Edge not installed or no profiles found");
        return;
    }

    for profile in &profiles {
        let inventory = scan_extensions(profile);
        eprintln!(
            "[Edge/{}] Found {} extensions",
            profile.name,
            inventory.extensions.len()
        );

        for ext in &inventory.extensions {
            let risk = if ext.risk_flags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", ext.risk_flags.join(", "))
            };
            eprintln!(
                "  - {} v{} ({}) {}{}",
                ext.name,
                ext.version,
                ext.id,
                if ext.enabled { "enabled" } else { "disabled" },
                risk
            );
        }

        assert_eq!(inventory.browser, BrowserKind::Edge);
    }
}

/// 验证本机 Chrome History 读取
#[test]
fn local_chrome_history_scan() {
    let profiles = enumerate_profiles(BrowserKind::Chrome);
    if profiles.is_empty() {
        eprintln!("SKIP: Chrome not installed or no profiles found");
        return;
    }

    // 使用当前时间作为锚点，±30s 窗口内可能有记录
    let now = chrono::Utc::now();
    for profile in &profiles {
        let result = attribute_history(profile, now, "");
        eprintln!(
            "[Chrome/{}] History: {} recent activities, {} nav chain nodes",
            profile.name,
            result.recent_browser_activity.len(),
            result.navigation_chain.len()
        );

        for act in &result.recent_browser_activity {
            eprintln!(
                "  - [{}] {} ({}) +{}ms",
                match act.tier {
                    TimeTier::Immediate => "IMM",
                    TimeTier::Nearby => "NEAR",
                    TimeTier::Recent => "REC",
                },
                act.title,
                act.url,
                act.time_distance_ms
            );
        }

        assert_eq!(result.browser, BrowserKind::Chrome);
    }
}
