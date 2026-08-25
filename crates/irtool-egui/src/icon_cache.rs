use std::collections::{HashMap, HashSet};
use std::sync::mpsc;

use eframe::egui;

use crate::theme;

/// 可执行文件图标缓存：路径 -> 已加载的 egui 纹理。
///
/// 与主 UI 的 `iconCache` 对应：异步批量提取图标（复用 `irtool_autoruns::batch_extract_icons`），
/// 在主线程解码为 egui TextureHandle 后缓存。
#[derive(Default)]
pub struct IconCache {
    textures: HashMap<String, Option<egui::TextureHandle>>,
    pending: HashSet<String>,
    #[allow(clippy::type_complexity)]
    rx: Option<mpsc::Receiver<Vec<(String, Option<String>)>>>,
}

impl IconCache {
    /// 预加载指定路径的图标。跳过已缓存、正在加载或空路径。
    pub fn preload(&mut self, rt: &tokio::runtime::Handle, paths: Vec<String>) {
        let uncached: Vec<String> = paths
            .into_iter()
            .filter(|p| !p.is_empty())
            .filter(|p| !self.textures.contains_key(p) && !self.pending.contains(p))
            .collect();

        if uncached.is_empty() {
            return;
        }

        for p in &uncached {
            self.pending.insert(p.clone());
        }

        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        rt.spawn_blocking(move || {
            let results = irtool_autoruns::batch_extract_icons(&uncached);
            let _ = tx.send(results);
        });
    }

    /// 轮询异步提取结果并生成纹理（必须在主线程调用，需要 egui::Context）。
    pub fn poll(&mut self, ctx: &egui::Context) {
        let rx = match self.rx.take() {
            Some(r) => r,
            None => return,
        };

        let mut all_results = Vec::new();
        while let Ok(results) = rx.try_recv() {
            all_results.extend(results);
        }
        self.rx = Some(rx);

        if !all_results.is_empty() {
            self.apply_batch(ctx, all_results);
        }
    }

    fn apply_batch(&mut self, ctx: &egui::Context, results: Vec<(String, Option<String>)>) {
        for (path, maybe_b64) in results {
            self.pending.remove(&path);
            let texture = maybe_b64
                .as_deref()
                .and_then(|b64| decode_icon_texture(ctx, &path, b64));
            self.textures.insert(path, texture);
        }
    }

    /// 获取已缓存的图标纹理。
    pub fn get(&self, path: &str) -> Option<&egui::TextureHandle> {
        self.textures.get(path).and_then(|opt| opt.as_ref())
    }

    /// 渲染图标；未加载或不存在时渲染占位方块。
    pub fn icon_or_placeholder(&self, ui: &mut egui::Ui, path: Option<&str>, size: f32) {
        if let Some(path) = path {
            if let Some(tex) = self.get(path) {
                let sized = egui::load::SizedTexture::new(tex.id(), egui::vec2(size, size));
                ui.add(egui::Image::new(sized).fit_to_exact_size(egui::vec2(size, size)));
                return;
            }
        }
        let rect = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover()).0;
        ui.painter().rect_filled(rect, 2.0, theme::bg_elevated());
    }
}

fn decode_icon_texture(ctx: &egui::Context, debug_name: &str, data_url: &str) -> Option<egui::TextureHandle> {
    let b64 = data_url.strip_prefix("data:image/png;base64,")?;
    let png_bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64).ok()?;
    let img = image::load_from_memory_with_format(&png_bytes, image::ImageFormat::Png).ok()?;
    let rgba = img.to_rgba8();
    let size = [rgba.width() as _, rgba.height() as _];
    let image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture(debug_name.to_string(), image, Default::default()))
}
