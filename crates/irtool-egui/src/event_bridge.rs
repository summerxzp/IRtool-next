use irtool_service::event_bus::AppEvent;
use irtool_service::AppContext;

/// Bridges `EventBus` (tokio broadcast) into egui's synchronous update loop
/// via a `std::sync::mpsc` channel.
pub struct EventBridge {
    rx: std::sync::mpsc::Receiver<AppEvent>,
}

impl EventBridge {
    pub fn new(ctx: &AppContext, rt: &tokio::runtime::Handle) -> Self {
        let mut bus_rx = ctx.event_bus.subscribe();
        let (tx, rx) = std::sync::mpsc::channel();

        rt.spawn(async move {
            loop {
                match bus_rx.recv().await {
                    Ok(event) => {
                        if tx.send(event).is_err() {
                            break; // UI closed
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("egui event bridge lagged: {} events", n);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        Self { rx }
    }

    /// Non-blocking drain. Call from egui `update()`.
    pub fn drain(&self) -> Vec<AppEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.rx.try_recv() {
            events.push(event);
        }
        events
    }
}
