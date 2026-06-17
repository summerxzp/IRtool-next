use irtool_core::IrError;
use irtool_process::{get_process_chain, take_snapshot, ProcessChain, ProcessSnapshot};

use crate::context::AppContext;

pub struct ProcessService<'a> {
    pub ctx: &'a AppContext,
}

impl<'a> ProcessService<'a> {
    pub async fn snapshot(&self) -> Result<ProcessSnapshot, IrError> {
        let _ = self.ctx; // available for future use
        tokio::task::spawn_blocking(take_snapshot)
            .await
            .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    pub async fn chain(&self, pid: u32) -> Result<ProcessChain, IrError> {
        let _ = self.ctx;
        tokio::task::spawn_blocking(move || get_process_chain(pid))
            .await
            .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }
}
