//! GeoForge processor library — TaskConfig, JSONL protocol, pipelines.

pub mod cancel;
pub mod capabilities;
pub mod geo;
pub mod path_policy;
pub mod pipeline;
pub mod protocol;
pub mod stages;
pub mod util;
pub mod validator;

pub use cancel::CancelFlag;
pub use capabilities::capabilities_json;
pub use path_policy::{validate_io_paths, ValidatedPaths};
pub use pipeline::{run_task, RunOutcome};
pub use protocol::{Emitter, Stage, TaskConfig, EXIT_CANCELLED, EXIT_FAILED, EXIT_OK};
pub use stages::scan::scan_osgb;
pub use validator::{ValidationCode, ValidationIssue, ValidationReport};
