//! GeoForge processor library — TaskConfig, JSONL protocol, pipelines.

pub mod cancel;
pub mod geo;
pub mod pipeline;
pub mod protocol;
pub mod stages;
pub mod util;

pub use cancel::CancelFlag;
pub use pipeline::{run_task, RunOutcome};
pub use protocol::{Emitter, Stage, TaskConfig, EXIT_CANCELLED, EXIT_FAILED, EXIT_OK};
pub use stages::scan::scan_osgb;
