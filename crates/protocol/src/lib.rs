//! Pure task configuration and event field contract.
//! No stdout I/O, Tauri, SQLite, or engine dependencies.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SCHEMA_VERSION: u32 = 1;

/// Exit codes: 0 success, 1 failed, 2 cancelled.
pub const EXIT_OK: i32 = 0;
pub const EXIT_FAILED: i32 = 1;
pub const EXIT_CANCELLED: i32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskConfig {
    pub schema_version: Option<u32>,
    pub task_id: String,
    pub operation: String,
    pub input: PathRef,
    pub output: PathRef,
    #[serde(default)]
    pub options: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathRef {
    pub path: String,
}

impl TaskConfig {
    pub fn input_path(&self) -> &str {
        &self.input.path
    }

    pub fn output_path(&self) -> &str {
        &self.output.path
    }

    pub fn options_obj(&self) -> &Value {
        &self.options
    }

    /// Reject unsupported schema versions. Missing version is treated as v1 (legacy).
    pub fn validate_schema(&self) -> Result<(), String> {
        match self.schema_version {
            None | Some(SCHEMA_VERSION) => Ok(()),
            Some(v) => Err(format!(
                "unsupported schemaVersion {v}; expected {SCHEMA_VERSION} or omit for legacy"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Scan,
    Convert,
    RebuildIndex,
    RebuildProxy,
    Rebuild,
    Texture,
    Validate,
    Commit,
    Done,
}

impl Stage {
    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Scan => "scan",
            Stage::Convert => "convert",
            Stage::RebuildIndex => "rebuild-index",
            Stage::RebuildProxy => "rebuild-proxy",
            Stage::Rebuild => "rebuild",
            Stage::Texture => "texture",
            Stage::Validate => "validate",
            Stage::Commit => "commit",
            Stage::Done => "done",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn task_config_roundtrip() {
        let cfg = TaskConfig {
            schema_version: Some(1),
            task_id: "t1".into(),
            operation: "convert-osgb".into(),
            input: PathRef {
                path: r"D:\data\in".into(),
            },
            output: PathRef {
                path: r"D:\data\out".into(),
            },
            options: json!({ "texture": { "mode": "keep" } }),
        };
        let s = serde_json::to_string(&cfg).unwrap();
        let back: TaskConfig = serde_json::from_str(&s).unwrap();
        assert_eq!(back.task_id, "t1");
        assert_eq!(back.input_path(), r"D:\data\in");
        assert!(back.validate_schema().is_ok());
    }

    #[test]
    fn rejects_unknown_schema() {
        let cfg = TaskConfig {
            schema_version: Some(99),
            task_id: "t".into(),
            operation: "x".into(),
            input: PathRef { path: "a".into() },
            output: PathRef { path: "b".into() },
            options: json!({}),
        };
        assert!(cfg.validate_schema().is_err());
    }
}
