//! Process enumeration, chain tracing, and suspicious process detection.

pub mod chain;
pub mod snapshot;
pub mod suspicious;
pub mod types;

pub use chain::get_process_chain;
pub use snapshot::{take_snapshot, take_snapshot_enriched};
pub use types::{ProcessChain, ProcessEntry, ProcessNode, ProcessSnapshot, SuspiciousFlag};
