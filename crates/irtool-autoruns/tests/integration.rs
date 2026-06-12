use irtool_autoruns::risk::{evaluate, FileInfo};
use irtool_autoruns::{AutorunItem, AutorunsStore, RiskLevel, SignatureStatus};

fn make_item(id: u64, image_path: &str) -> AutorunItem {
    AutorunItem {
        id,
        category: "Logon".into(),
        entry: "test_entry".into(),
        enabled: true,
        location: "HKLM\\Software\\Microsoft\\Windows\\CurrentVersion\\Run".into(),
        description: "Test description".into(),
        publisher: "Test Publisher".into(),
        image_path: Some(image_path.into()),
        launch_string: Some(format!("\"{}\"", image_path)),
        timestamp: None,
        file_exists: true,
        file_size: Some(102400),
        file_version: Some("1.0.0".into()),
        service_name: None,
        md5: None,
        sha256: None,
        risk: RiskLevel::Safe,
        risk_reasons: vec![],
        signature: SignatureStatus::NotVerified,
    }
}

#[test]
fn test_store_put_and_get() {
    let store = AutorunsStore::new();
    let items = vec![
        make_item(1, r"C:\Program Files\A\app.exe"),
        make_item(2, r"C:\Users\test\AppData\Local\B\b.exe"),
        make_item(3, r"C:\Windows\System32\driver.sys"),
    ];
    store.clear_and_put(items);
    assert_eq!(store.len(), 3);

    let all = store.get_all();
    assert_eq!(all.len(), 3);
    assert!(all.iter().any(|i| i.id == 1));
    assert!(all.iter().any(|i| i.id == 2));
    assert!(all.iter().any(|i| i.id == 3));
}

#[test]
fn test_store_remove_existing() {
    let store = AutorunsStore::new();
    store.clear_and_put(vec![make_item(1, "a.exe"), make_item(2, "b.exe")]);
    assert_eq!(store.len(), 2);

    let removed = store.remove(1);
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().id, 1);
    assert_eq!(store.len(), 1);
    assert!(store.get(1).is_none());
}

#[test]
fn test_store_get_nonexistent_returns_none() {
    let store = AutorunsStore::new();
    store.clear_and_put(vec![make_item(1, "a.exe")]);
    assert!(store.get(999).is_none());
    assert!(store.remove(999).is_none());
}

#[test]
fn test_store_update_signature() {
    let store = AutorunsStore::new();
    let path = r"C:\Program Files\MyApp\app.exe";
    store.clear_and_put(vec![make_item(1, path), make_item(2, "other.exe")]);

    store.update_signature(
        path,
        SignatureStatus::Valid {
            signer: "Microsoft Corporation".into(),
        },
    );

    let item1 = store.get(1).unwrap();
    match item1.signature {
        SignatureStatus::Valid { signer } => assert_eq!(signer, "Microsoft Corporation"),
        _ => panic!("Expected Valid signature"),
    }

    let item2 = store.get(2).unwrap();
    assert!(matches!(item2.signature, SignatureStatus::NotVerified));
}

#[test]
fn test_store_update_hash() {
    let store = AutorunsStore::new();
    store.clear_and_put(vec![make_item(1, "a.exe")]);

    let md5 = "d41d8cd98f00b204e9800998ecf8427e".to_string();
    let sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string();
    store.update_hash(1, md5.clone(), sha256.clone());

    let item = store.get(1).unwrap();
    assert_eq!(item.md5.as_ref().unwrap(), &md5);
    assert_eq!(item.sha256.as_ref().unwrap(), &sha256);
}

#[test]
fn test_store_update_hash_nonexistent_no_panic() {
    let store = AutorunsStore::new();
    store.clear_and_put(vec![make_item(1, "a.exe")]);
    store.update_hash(999, "md5".into(), "sha256".into());
    assert_eq!(store.len(), 1);
}

#[test]
fn test_store_clear_and_repopulate() {
    let store = AutorunsStore::new();
    store.clear_and_put(vec![make_item(1, "a.exe"), make_item(2, "b.exe")]);
    assert_eq!(store.len(), 2);

    store.clear_and_put(vec![
        make_item(3, "c.exe"),
        make_item(4, "d.exe"),
        make_item(5, "e.exe"),
    ]);
    assert_eq!(store.len(), 3);
    assert!(store.get(1).is_none());
    assert!(store.get(2).is_none());
    assert!(store.get(3).is_some());
}

#[test]
fn test_store_is_empty_and_len() {
    let store = AutorunsStore::new();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);

    store.clear_and_put(vec![make_item(1, "a.exe")]);
    assert!(!store.is_empty());
    assert_eq!(store.len(), 1);

    store.clear_and_put(vec![]);
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
}

#[test]
fn test_store_clear_with_empty_vector() {
    let store = AutorunsStore::new();
    store.clear_and_put(vec![make_item(1, "a.exe")]);
    assert_eq!(store.len(), 1);
    store.clear_and_put(vec![]);
    assert_eq!(store.len(), 0);
}

#[test]
fn test_risk_trusted_system_path_safe() {
    use irtool_autoruns::csv_parser::RawEntry;

    let entry = RawEntry {
        location: "HKLM".into(),
        entry: "test".into(),
        enabled: "enabled".into(),
        category: "Logon".into(),
        description: "".into(),
        publisher: "Microsoft".into(),
        image_path: r"c:\windows\system32\svchost.exe".into(),
        launch_string: "".into(),
        timestamp: "".into(),
        md5: "".into(),
        sha256: "".into(),
        signer: "Microsoft Corporation".into(),
        version: "".into(),
    };
    let file_info = FileInfo {
        exists: true,
        size: Some(102400),
    };

    let (risk, reasons) = evaluate(&entry, Some(&file_info));
    // 受信任路径 + Microsoft 签名 → 应该是 Safe 或至少 Suspicious
    assert_ne!(risk, RiskLevel::HighRisk);
    assert!(reasons.len() <= 2);
}

#[test]
fn test_risk_high_risk_path_with_temp() {
    use irtool_autoruns::csv_parser::RawEntry;

    let entry = RawEntry {
        location: "HKCU".into(),
        entry: "test".into(),
        enabled: "enabled".into(),
        category: "Logon".into(),
        description: "".into(),
        publisher: "Unknown".into(),
        image_path: r"c:\users\test\appdata\local\temp\malware.exe".into(),
        launch_string: "".into(),
        timestamp: "".into(),
        md5: "".into(),
        sha256: "".into(),
        signer: "".into(),
        version: "".into(),
    };
    let file_info = FileInfo {
        exists: true,
        size: Some(50000),
    };

    let (risk, reasons) = evaluate(&entry, Some(&file_info));
    // 高风险路径 + 无签名 + 小文件 → 应该是 HighRisk
    assert_eq!(risk, RiskLevel::HighRisk);
    assert!(!reasons.is_empty());
}

#[test]
fn test_risk_unsigned_file() {
    use irtool_autoruns::csv_parser::RawEntry;

    let entry = RawEntry {
        location: "HKLM".into(),
        entry: "test".into(),
        enabled: "enabled".into(),
        category: "Logon".into(),
        description: "".into(),
        publisher: "Unknown Publisher".into(),
        image_path: r"c:\program files\custom\app.exe".into(),
        launch_string: "".into(),
        timestamp: "".into(),
        md5: "".into(),
        sha256: "".into(),
        signer: "".into(),
        version: "".into(),
    };
    let file_info = FileInfo {
        exists: true,
        size: Some(5000000),
    };

    let (risk, reasons) = evaluate(&entry, Some(&file_info));
    // 不在受信任路径 + 无签名 → 应该是 Suspicious 或更高
    assert_ne!(risk, RiskLevel::Safe);
    assert!(!reasons.is_empty());
}

#[test]
fn test_risk_multiple_items_different_risk() {
    use irtool_autoruns::csv_parser::RawEntry;

    let file_info = FileInfo {
        exists: true,
        size: Some(100000),
    };

    // 1. 受信任系统路径 + Microsoft
    let safe_entry = RawEntry {
        location: "".into(),
        entry: "".into(),
        enabled: "".into(),
        category: "".into(),
        description: "".into(),
        publisher: "Microsoft".into(),
        image_path: r"c:\windows\system32\explorer.exe".into(),
        launch_string: "".into(),
        timestamp: "".into(),
        md5: "".into(),
        sha256: "".into(),
        signer: "Microsoft".into(),
        version: "".into(),
    };

    // 2. 临时目录
    let risky_entry = RawEntry {
        location: "".into(),
        entry: "".into(),
        enabled: "".into(),
        category: "".into(),
        description: "".into(),
        publisher: "Unknown".into(),
        image_path: r"c:\users\test\appdata\local\temp\something.exe".into(),
        launch_string: "".into(),
        timestamp: "".into(),
        md5: "".into(),
        sha256: "".into(),
        signer: "".into(),
        version: "".into(),
    };

    // 3. 下载目录
    let download_entry = RawEntry {
        location: "".into(),
        entry: "".into(),
        enabled: "".into(),
        category: "".into(),
        description: "".into(),
        publisher: "Unknown".into(),
        image_path: r"c:\users\test\downloads\installer.exe".into(),
        launch_string: "".into(),
        timestamp: "".into(),
        md5: "".into(),
        sha256: "".into(),
        signer: "".into(),
        version: "".into(),
    };

    let (risk1, _) = evaluate(&safe_entry, Some(&file_info));
    let (risk2, _) = evaluate(&risky_entry, Some(&file_info));
    let (risk3, _) = evaluate(&download_entry, Some(&file_info));

    // 验证风险评级的相对关系
    assert_ne!(risk1, RiskLevel::HighRisk);
    assert_ne!(risk2, RiskLevel::Safe);
    assert_ne!(risk3, RiskLevel::Safe);
}

#[test]
fn test_signature_status_default_is_not_verified() {
    let status = SignatureStatus::default();
    assert!(matches!(status, SignatureStatus::NotVerified));
}

#[test]
fn test_risk_level_as_str() {
    assert_eq!(RiskLevel::Safe.as_str(), "safe");
    assert_eq!(RiskLevel::Suspicious.as_str(), "suspicious");
    assert_eq!(RiskLevel::HighRisk.as_str(), "high_risk");
}

#[test]
fn test_store_get_returns_correct_data() {
    let store = AutorunsStore::new();
    let mut item = make_item(42, r"C:\Custom\Path\program.exe");
    item.description = "A test program".into();
    item.publisher = "Test Corp".into();
    item.enabled = true;

    store.clear_and_put(vec![item.clone()]);

    let retrieved = store.get(42).unwrap();
    assert_eq!(retrieved.id, 42);
    assert_eq!(retrieved.image_path, Some(r"C:\Custom\Path\program.exe".into()));
    assert_eq!(retrieved.description, "A test program");
    assert_eq!(retrieved.publisher, "Test Corp");
    assert!(retrieved.enabled);
}

#[test]
fn test_store_remove_then_repopulate() {
    let store = AutorunsStore::new();
    store.clear_and_put(vec![make_item(1, "a.exe"), make_item(2, "b.exe")]);
    store.remove(1);
    assert_eq!(store.len(), 1);

    store.clear_and_put(vec![make_item(1, "a.exe"), make_item(2, "b.exe"), make_item(3, "c.exe")]);
    assert_eq!(store.len(), 3);
    assert!(store.get(1).is_some());
}

#[test]
fn test_risk_none_file_info() {
    use irtool_autoruns::csv_parser::RawEntry;

    let entry = RawEntry {
        location: "".into(),
        entry: "".into(),
        enabled: "".into(),
        category: "".into(),
        description: "".into(),
        publisher: "Unknown".into(),
        image_path: r"c:\some\path.exe".into(),
        launch_string: "".into(),
        timestamp: "".into(),
        md5: "".into(),
        sha256: "".into(),
        signer: "".into(),
        version: "".into(),
    };

    // file_info 为 None 时不 panic
    let (_, reasons) = evaluate(&entry, None);
    assert!(!reasons.is_empty() || reasons.is_empty()); // 只要不 panic 就好
}

#[test]
fn test_risk_file_not_exists() {
    use irtool_autoruns::csv_parser::RawEntry;

    let entry = RawEntry {
        location: "".into(),
        entry: "".into(),
        enabled: "".into(),
        category: "".into(),
        description: "".into(),
        publisher: "Unknown".into(),
        image_path: r"c:\missing\file.exe".into(),
        launch_string: "".into(),
        timestamp: "".into(),
        md5: "".into(),
        sha256: "".into(),
        signer: "".into(),
        version: "".into(),
    };
    let file_info = FileInfo {
        exists: false,
        size: None,
    };

    // 文件不存在时不 panic
    let (_, reasons) = evaluate(&entry, Some(&file_info));
    // 文件不存在可能是高危
    assert!(reasons.len() >= 1);
}
