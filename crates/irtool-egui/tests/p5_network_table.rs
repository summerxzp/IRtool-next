//! P5 network 页 TableShell 交互自动化验证（egui_kittest 无头渲染，spec §6 范式验收）。
//!
//! 覆盖功能清单（docs/superpowers/plans/p5-network-reference.md 验收第 2 条）中
//! 依赖 GUI 交互的项：行选中→详情面板、右键菜单（含终止确认框开/关）、键盘
//! ↑↓+Enter、表头点击排序、密度切换（28/34px 行距断言）、搜索过滤、等待态。
//! risk 行高亮与深浅主题由实机截图验证（target/shots-p5/network-*.png）。
//!
//! 运行：`cargo test -p irtool-egui --test p5_network_table -- --nocapture`
//! 截图输出：`target/shots-p5/kittest-*.png`。
//!
//! 注意：密度切换/排序会触发表格持久化写盘（AppDirs::detect 的真实 config 目录），
//! 测试前后对 ui-state.json 做备份/恢复。

use std::path::PathBuf;

use eframe::egui;
use egui_kittest::{Harness, kittest::Queryable};
use irtool_egui::pages::network::NetworkPageState;
use irtool_net_monitor::{CmdlineStatus, ConnState, Family, NetConn, NetEndpoint, Proto};
use irtool_service::context::AppContext;

/// 备份/恢复真实 ui-state.json 的守卫（测试期间表格持久化会写该文件）。
struct StateFileGuard {
    path: PathBuf,
    backup: Option<Vec<u8>>,
}

impl StateFileGuard {
    fn take() -> Self {
        let dir = std::env::var("APPDATA")
            .map(|d| PathBuf::from(d).join("IRtool").join("config"))
            .unwrap_or_else(|_| PathBuf::from("."));
        let path = dir.join("ui-state.json");
        let backup = std::fs::read(&path).ok();
        // 写入空对象：load_table_state 读到全默认值，保证测试确定性
        let _ = std::fs::write(&path, b"{}");
        StateFileGuard { path, backup }
    }
}

impl Drop for StateFileGuard {
    fn drop(&mut self) {
        match &self.backup {
            Some(bytes) => {
                let _ = std::fs::write(&self.path, bytes);
            }
            None => {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
}

/// 构造一条合成连接记录（first_seen = 基准 + pid，决定默认「首次出现降序」视图序）。
fn conn(pid: u32, name: &str, state: ConnState, remote: (&str, u16), is_current: bool, proto: Proto) -> NetConn {
    NetConn {
        proto,
        family: if remote.0.contains(':') { Family::V6 } else { Family::V4 },
        local: NetEndpoint { addr: "127.0.0.1".into(), port: 5000 + pid as u16 },
        remote: NetEndpoint { addr: remote.0.into(), port: remote.1 },
        state,
        pid,
        process_name: Some(name.into()),
        process_path: Some(format!("C:\\test\\{}", name)),
        process_cmdline: Some(format!("{} -flag", name)),
        cmdline_status: CmdlineStatus::Ready,
        first_seen: 1787000000 + pid as u64,
        last_seen: 1787000000 + pid as u64,
        is_current,
    }
}

/// 合成快照：12 条，含 CLOSE_WAIT（warning risk）、恶意 IP（danger risk）、
/// 历史行（is_current=false）、UDP 行。
fn snapshot() -> irtool_service::dto::network::NetworkSnapshotPayload {
    use irtool_service::dto::network::NetworkSnapshotPayload;
    let items = vec![
        conn(100, "alpha.exe", ConnState::Established, ("8.130.222.211", 443), true, Proto::Tcp),
        conn(101, "bravo.exe", ConnState::CloseWait, ("172.64.146.88", 443), true, Proto::Tcp),
        conn(102, "charlie.exe", ConnState::Established, ("82.23.246.148", 443), true, Proto::Tcp), // 恶意 IP → danger 高亮
        conn(103, "delta.exe", ConnState::TimeWait, ("59.110.96.118", 443), false, Proto::Tcp),     // 历史行
        conn(104, "echo.exe", ConnState::Listen, ("0.0.0.0", 0), true, Proto::Tcp),
        conn(105, "foxtrot.exe", ConnState::None, ("::", 0), true, Proto::Udp),
        conn(106, "golf.exe", ConnState::Established, ("8.130.222.211", 443), true, Proto::Tcp),
        conn(107, "hotel.exe", ConnState::SynSent, ("127.0.0.1", 7890), true, Proto::Tcp),
        conn(108, "india.exe", ConnState::Established, ("192.168.1.8", 5353), true, Proto::Udp),
        conn(109, "juliet.exe", ConnState::Established, ("8.130.222.211", 443), true, Proto::Tcp),
        conn(110, "kilo.exe", ConnState::Established, ("8.130.222.211", 443), true, Proto::Tcp),
        conn(111, "mike.exe", ConnState::Established, ("8.130.222.211", 443), true, Proto::Tcp),
    ];
    NetworkSnapshotPayload { items, timestamp: 1787000000 }
}

struct Bench {
    page: NetworkPageState,
    ctx: AppContext,
    rt: tokio::runtime::Runtime,
    /// 测试在断言完等待态后置 true，下一帧喂入合成快照。
    feed_requested: bool,
    fed: bool,
    themed: bool,
    debug_hover: bool,
}

impl Bench {
    fn new() -> Self {
        // 主题确定性：固定 Light（全局 OnceLock，测试进程内有效）
        irtool_egui::design::theme::init(irtool_egui::design::theme::ThemeMode::Light);
        let mut page = NetworkPageState::default();
        page.load_table_state(
            &std::env::var("APPDATA")
                .map(|d| PathBuf::from(d).join("IRtool").join("config"))
                .unwrap_or_else(|_| PathBuf::from(".")),
        );
        Bench {
            page,
            ctx: AppContext::new(irtool_core::AppDirs::detect()),
            rt: tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap(),
            feed_requested: false,
            fed: false,
            themed: false,
            debug_hover: false,
        }
    }
}

fn shots_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/shots-p5")
}

fn save_shot(harness: &mut Harness<Bench>, name: &str) {
    let img = harness.render().expect("kittest render failed");
    let path = shots_dir().join(name);
    img.save(&path).expect("save screenshot failed");
    println!("screenshot: {}", path.display());
}

/// 每帧 UI：主题/字族安装 + 喂快照 + 页面渲染（详情面板接线对齐 app.rs）。
fn ui_frame(ui: &mut egui::Ui, bench: &mut Bench) {
    // 主题/字族只装一次（对齐真实 app：theme::apply 仅启动时调用；
    // 每帧 set_theme 会干扰 egui 的 hit-test widget rects）
    if !bench.themed {
        irtool_egui::design::theme::apply(ui.ctx());
        bench.themed = true;
    }
    if bench.debug_hover {
        // egui 0.36 移除了 Style.debug（debug_on_hover），kittest 查询走
        // accesskit 不受影响；此开关现仅保留用例占位，无渲染副作用。
    }
    if bench.feed_requested && !bench.fed {
        bench.page.handle_snapshot(snapshot());
        bench.fed = true;
    }
    bench.page.render(ui, &bench.ctx, bench.rt.handle());
    if bench.page.detail_visible {
        egui::Panel::bottom("detail_panel")
            .default_size(220.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(irtool_egui::design::theme::palette().bg_elev1)
                    .inner_margin(egui::Margin::symmetric(8, 4)),
            )
            .show(ui, |ui| {
                bench.page.render_detail_panel(ui, &bench.ctx, bench.rt.handle());
            });
    }
}

/// 收集首列时间标签的 y 坐标（行距断言用）。
fn time_label_ys(harness: &Harness<Bench>) -> Vec<f32> {
    let mut ys: Vec<f32> = harness
        .root()
        .query_all_by_label_contains("2026/")
        .map(|n| n.rect().top())
        .collect();
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ys.dedup_by(|a, b| (*a - *b).abs() < 1.0);
    ys
}

#[test]
fn p5_network_table_interactions() {
    let _guard = StateFileGuard::take();

    let mut harness = Harness::new_ui_state(ui_frame, Bench::new());
    harness.set_size(egui::vec2(1280.0, 760.0));

    // ── 1. 等待态（未收到快照）───────────────────────────────
    harness.run_steps(2);
    assert!(
        harness.root().query_by_label("等待网络数据…").is_some(),
        "未收到快照时应显示等待态文案"
    );

    // 请求喂数据，下一帧生效
    harness.state_mut().feed_requested = true;
    harness.run_steps(4);
    assert!(
        harness.root().query_by_label("alpha.exe").is_some(),
        "喂入快照后应渲染出行数据"
    );
    // 历史行 delta.exe（is_current=false）默认显示（show_history=true）
    assert!(harness.root().query_by_label("delta.exe").is_some());

    // ── 2. 密度切换：28px → 34px（行距断言 + 截图）────────────
    let ys_before = time_label_ys(&harness);
    assert!(ys_before.len() >= 10, "应渲染出足够多的行");
    let spacing_before = ys_before[2] - ys_before[1];
    assert!(
        (spacing_before - 28.0).abs() < 6.0,
        "compact 行距应约 28px，实际 {spacing_before}"
    );

    harness.root().get_by_label("紧凑").click();
    harness.run_steps(4);
    let ys_after = time_label_ys(&harness);
    let spacing_after = ys_after[2] - ys_after[1];
    assert!(
        (spacing_after - 34.0).abs() < 6.0,
        "standard 行距应约 34px，实际 {spacing_after}"
    );
    save_shot(&mut harness, "kittest-standard-density.png");

    // 切回 compact，后续截图统一
    harness.root().get_by_label("标准").click();
    harness.run_steps(4);


    // ── 3. 表头点击排序：首次出现 desc → asc ─────────────────
    harness.run_steps(3); // 布局沉降帧（密度切换后）
    let hdr = harness.root().get_all_by_label("首次出现").next().unwrap();
    hdr.click();
    harness.run_steps(4);
    let alpha_y = harness.root().get_by_label("alpha.exe").rect().top();
    let mike_y = harness.root().get_by_label("mike.exe").rect().top();
    assert!(
        alpha_y < mike_y,
        "点击「首次出现」表头升序后 alpha(pid100) 应排在 mike(pid111) 之前"
    );

    // ── 4. 行点击选中 → 详情面板 ─────────────────────────────
    harness.root().get_by_label("charlie.exe").click();
    harness.run_steps(4);
    assert!(
        harness.root().query_by_label("PID 102").is_some(),
        "点击 charlie.exe 行后详情面板应显示 PID 102"
    );
    save_shot(&mut harness, "kittest-detail-panel.png");

    // 点击已选行 → 取消选中、面板关闭（旧行为保持）。
    // 此时详情面板也在展示 charlie.exe，取树序第一个（表格在面板之前）。
    harness.root().get_all_by_label("charlie.exe").next().unwrap().click();
    harness.run_steps(4);
    assert!(
        harness.root().query_by_label("PID 102").is_none(),
        "再次点击已选行应收起详情面板"
    );

    // ── 5. 右键菜单（菜单项对齐 React 版）─────────────────────
    harness.root().get_by_label("bravo.exe").click_secondary();
    harness.run_steps(4);
    assert!(harness.root().query_by_label("复制行").is_some(), "右键菜单应含「复制行」");
    // 工具栏也有「终止进程」按钮（同名），菜单打开时应为两处
    assert!(
        harness.root().get_all_by_label("终止进程").count() >= 2,
        "右键菜单应含「终止进程」"
    );
    assert!(
        harness.root().query_by_label("在工作台搜索").is_some(),
        "右键菜单应含「在工作台搜索」（禁用项）"
    );
    save_shot(&mut harness, "kittest-context-menu.png");

    // 右键未选行 → 先选中该行（bravo → PID 101 详情）
    assert!(
        harness.root().query_by_label("PID 101").is_some(),
        "右键未选行时应自动选中并打开详情"
    );

    // ── 6. 菜单「终止进程」→ 确认框打开 → 取消关闭（不实际 kill）──
    // 同名节点有多处（工具栏/右键菜单/详情面板），取 y 最小者=工具栏按钮（同样打开确认框）
    harness
        .root()
        .get_all_by_label("终止进程")
        .min_by_key(|n| n.rect().top().to_bits())
        .unwrap()
        .click();
    harness.run_steps(4);
    assert!(
        harness.root().query_by_label("确认终止进程").is_some(),
        "菜单「终止进程」应打开确认框"
    );
    harness.root().get_by_label("取消").click();
    harness.run_steps(4);
    assert!(
        harness.root().query_by_label("确认终止进程").is_none(),
        "取消后确认框应关闭"
    );
    // 点击表格行，关闭可能残留的右键菜单
    harness.root().get_by_label("alpha.exe").click();
    harness.run_steps(2);

    // ── 7. 键盘导航：点击行激活表格 → ↓ → Enter 确认 ──────────
    harness.root().get_by_label("mike.exe").click(); // mike=pid111，asc 序最后一行
    harness.run_steps(4);
    assert!(harness.root().query_by_label("PID 111").is_some());

    harness.key_press(egui::Key::ArrowDown); // 末行下移应保持（clamp）
    harness.run_steps(2);
    harness.key_press(egui::Key::ArrowUp); // 上移一行（kilo, pid 110）
    harness.run_steps(2);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(4);
    assert!(
        harness.root().query_by_label("PID 110").is_some(),
        "↑↓ 移动焦点后 Enter 应确认选中 kilo.exe (PID 110)"
    );

    // ── 8. 搜索过滤 ──────────────────────────────────────────
    let search = harness.root().get_by_role(egui::accesskit::Role::TextInput);
    search.click();
    harness.run_steps(2);
    harness
        .root()
        .get_by_role(egui::accesskit::Role::TextInput)
        .type_text("charlie");
    harness.run_steps(4);
    assert!(
        harness.root().query_by_label("charlie.exe").is_some(),
        "搜索 charlie 应保留 charlie.exe 行"
    );
    assert!(
        harness.root().query_by_label("bravo.exe").is_none(),
        "搜索 charlie 应过滤掉 bravo.exe 行"
    );
    assert!(
        harness.root().query_by_label("alpha.exe").is_none(),
        "搜索 charlie 应过滤掉 alpha.exe 行"
    );
    save_shot(&mut harness, "kittest-search-filter.png");
}
