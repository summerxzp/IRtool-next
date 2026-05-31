fn main() {
    #[cfg(windows)]
    {
        embed_resource::compile("irtool.rc", embed_resource::NONE);
    }
    tauri_build::build()
}
