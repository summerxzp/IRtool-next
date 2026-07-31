fn main() {
    // 使用 Tauri 的 WindowsAttributes 自定义 app manifest，包含 requireAdministrator
    // 和 Common-Controls v6 依赖。不能用 embed_resource 单独嵌入 manifest，
    // 因为 Tauri 的 try_build() 会生成自己的 manifest 资源（RT_MANIFEST），
    // 两者资源 ID 冲突时链接器优先使用 Tauri 的，导致 requireAdministrator 被覆盖。
    let mut attrs = tauri_build::Attributes::new();
    #[cfg(windows)]
    {
        attrs = attrs
            .windows_attributes(tauri_build::WindowsAttributes::new().app_manifest(include_str!("app-manifest.xml")));

        // example exe 默认无 app manifest，加载 comctl32 v5 时缺少
        // TaskDialogIndirect（Tauri/windows crate 静态导入）导致 0xc0000139。
        // 给 example 嵌入 Common-Controls v6 manifest 修复。
        // 不影响 main binary（已通过 app_manifest 嵌入）和 cdylib。
        let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("common-controls.manifest");
        let manifest_str = manifest.to_str().unwrap();
        println!("cargo:rustc-link-arg-examples=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-examples=/MANIFESTINPUT:{}", manifest_str);
    }
    tauri_build::try_build(attrs).expect("failed to run tauri-build");
}
