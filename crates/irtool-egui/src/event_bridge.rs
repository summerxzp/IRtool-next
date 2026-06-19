use std::sync::{Arc, Mutex};

use eframe::egui;
use irtool_service::event_bus::AppEvent;
use irtool_service::AppContext;

/// Bridges `EventBus` (tokio broadcast) into egui's synchronous update loop.
///
/// Holds a lazily-attached `egui::Context` so the tokio background task
/// can call `request_repaint()` on event arrival instead of relying on
/// a fixed repaint interval.
pub struct EventBridge {
    rx: std::sync::mpsc::Receiver<AppEvent>,
    egui_ctx: Arc<Mutex<Option<egui::Context>>>,
}

impl EventBridge {
    pub fn new(ctx: &AppContext, rt: &tokio::runtime::Handle) -> Self {
        let mut bus_rx = ctx.event_bus.subscribe();
        let (tx, rx) = std::sync::mpsc::channel::<AppEvent>();
        let egui_ctx = Arc::new(Mutex::new(None::<egui::Context>));
        let egui_ctx_clone = egui_ctx.clone();

        rt.spawn(async move {
            loop {
                match bus_rx.recv().await {
                    Ok(event) => {
                        if tx.send(event).is_err() {
                            break;
                        }
                        if let Ok(guard) = egui_ctx_clone.lock() {
                            if let Some(ctx) = guard.as_ref() {
                                ctx.request_repaint();
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("egui event bridge lagged: {} events", n);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        Self { rx, egui_ctx }
    }

    pub fn attach_context(&self, ctx: egui::Context) {
        if let Ok(mut guard) = self.egui_ctx.lock() {
            *guard = Some(ctx);
        }
    }

    pub fn drain(&self) -> Vec<AppEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.rx.try_recv() {
            events.push(event);
        }
        events
    }
}
