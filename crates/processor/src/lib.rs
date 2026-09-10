//! GeoForge processor library — TaskConfig, JSONL protocol, pipelines.

pub mod cancel;
pub mod geo;
pub mod pipeline;
pub mod protocol;
pub mod stages;
pub mod util;
pub mod validator;

pub use cancel::CancelFlag;
pub use pipeline::{run_task, RunOutcome};
pub use protocol::{Emitter, Stage, TaskConfig, EXIT_CANCELLED, EXIT_FAILED, EXIT_OK};
pub use stages::scan::scan_osgb;

pub use validator::{validate_and_write_report, validate_tileset_tree, ValidationReport};

/// Phase 13 staging / atomic commit helpers (tests + callers).
pub use stages::commit::{
    backup_dir, cleanup_staging, cleanup_temp, commit_transaction, dir_fingerprint,
    inject_fail_stage_to_final, interrupted_marker, mark_interrupted, prepare_staging,
    recover_interrupted_commit, staging_dir, temp_work_dir,
};
