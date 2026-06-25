//! irtool-native-messaging-host — Chrome Native Messaging Host 二进制入口
//!
//! 由 Chrome 浏览器通过 Native Messaging 协议启动，
//! 读取 stdin 上的消息并写入队列文件供 IRtool 主进程消费。

use std::path::PathBuf;

fn main() {
    // 使用 %TEMP%\irtool\attr-queue 作为队列目录
    let queue_dir = PathBuf::from(std::env::temp_dir())
        .join("irtool")
        .join("attr-queue");

    // 初始化日志：默认 info 级别，可通过 RUST_LOG 环境变量覆盖
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        )
        .with_writer(std::io::stderr) // 写入 stderr 避免干扰 stdout 的 Native Messaging 协议
        .init();

    // 运行事件循环（阻塞），直到 stdin 关闭
    if let Err(e) = irtool_native_messaging::run_event_loop(&queue_dir) {
        eprintln!("Native Messaging Host error: {:?}", e);
        std::process::exit(1);
    }
}
