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

/// Typed options for the `convert-model` operation. Existing operations retain
/// their JSON options so adding model conversion does not reinterpret legacy
/// task files.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelTaskOptions {
    pub model: ModelImportOptions,
    #[serde(default)]
    pub georeference: GeoReferenceOptions,
    #[serde(default)]
    pub texture: ModelTextureOptions,
    #[serde(default)]
    pub model_output: ModelOutputOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelImportOptions {
    pub format: ModelFormat,
    #[serde(default)]
    pub unit: ModelUnit,
    #[serde(default)]
    pub axes: ModelAxes,
    #[serde(default)]
    pub missing_texture_policy: MissingTexturePolicy,
    #[serde(default)]
    pub texture_roots: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelFormat {
    Fbx,
    Obj,
}

impl ModelFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Fbx => "fbx",
            Self::Obj => "obj",
        }
    }
}

impl ModelTaskOptions {
    pub fn validate(&self) -> Result<(), String> {
        if self.model.format == ModelFormat::Obj && self.model.unit == ModelUnit::FromMetadata {
            return Err("OBJ requires an explicit model.unit; OBJ does not reliably declare units".into());
        }
        if self.model.format == ModelFormat::Obj && self.model.axes == ModelAxes::FromMetadata {
            return Err("OBJ requires explicit model.axes; OBJ does not reliably declare axes".into());
        }
        match &self.georeference {
            GeoReferenceOptions::Local => Ok(()),
            GeoReferenceOptions::Anchor {
                longitude_deg,
                latitude_deg,
                ellipsoid_height_m,
                heading_deg,
                pitch_deg,
                roll_deg,
                ..
            } => {
                let values = [
                    *longitude_deg,
                    *latitude_deg,
                    *ellipsoid_height_m,
                    *heading_deg,
                    *pitch_deg,
                    *roll_deg,
                ];
                if values.iter().any(|value| !value.is_finite()) {
                    return Err("anchor georeference values must be finite numbers".into());
                }
                if !(-180.0..=180.0).contains(longitude_deg) {
                    return Err("anchor longitudeDeg must be between -180 and 180".into());
                }
                if !(-90.0..=90.0).contains(latitude_deg) {
                    return Err("anchor latitudeDeg must be between -90 and 90".into());
                }
                Ok(())
            }
            GeoReferenceOptions::Projected {
                source_crs,
                origin_offset,
                ..
            } => {
                if source_crs.trim().is_empty() {
                    return Err("projected georeference requires sourceCrs".into());
                }
                if origin_offset.iter().any(|value| !value.is_finite()) {
                    return Err("projected originOffset values must be finite numbers".into());
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelUnit {
    FromMetadata,
    Meters,
    Centimeters,
    Millimeters,
    Feet,
}

impl Default for ModelUnit {
    fn default() -> Self {
        Self::FromMetadata
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelAxes {
    FromMetadata,
    YUpRightHanded,
    ZUpRightHanded,
}

impl Default for ModelAxes {
    fn default() -> Self {
        Self::FromMetadata
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MissingTexturePolicy {
    Error,
    Warn,
}

impl Default for MissingTexturePolicy {
    fn default() -> Self {
        Self::Error
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum GeoReferenceOptions {
    Local,
    Anchor {
        #[serde(rename = "longitudeDeg")]
        longitude_deg: f64,
        #[serde(rename = "latitudeDeg")]
        latitude_deg: f64,
        #[serde(rename = "ellipsoidHeightM")]
        ellipsoid_height_m: f64,
        #[serde(default)]
        pivot: ModelPivot,
        #[serde(default)]
        heading_deg: f64,
        #[serde(default)]
        pitch_deg: f64,
        #[serde(default)]
        roll_deg: f64,
    },
    Projected {
        #[serde(rename = "sourceCrs")]
        source_crs: String,
        #[serde(rename = "axisMapping")]
        axis_mapping: ProjectedAxisMapping,
        #[serde(rename = "originOffset", default)]
        origin_offset: [f64; 3],
    },
}

impl Default for GeoReferenceOptions {
    fn default() -> Self {
        Self::Local
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelPivot {
    Original,
    BottomCenter,
    BoundingBoxCenter,
}

impl Default for ModelPivot {
    fn default() -> Self {
        Self::Original
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectedAxisMapping {
    EastNorthHeight,
    NorthEastHeight,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelTextureOptions {
    #[serde(default)]
    pub mode: ModelTextureMode,
}

impl Default for ModelTextureOptions {
    fn default() -> Self {
        Self { mode: ModelTextureMode::Keep }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelTextureMode {
    Keep,
}

impl Default for ModelTextureMode {
    fn default() -> Self {
        Self::Keep
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelOutputOptions {
    #[serde(default)]
    pub format: ModelOutputFormat,
    #[serde(default)]
    pub tiling: ModelTiling,
    #[serde(default)]
    pub lod: bool,
}

impl Default for ModelOutputOptions {
    fn default() -> Self {
        Self {
            format: ModelOutputFormat::Tiles3d10,
            tiling: ModelTiling::Single,
            lod: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelOutputFormat {
    #[serde(rename = "3dtiles-1.0")]
    Tiles3d10,
}

impl Default for ModelOutputFormat {
    fn default() -> Self {
        Self::Tiles3d10
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelTiling {
    Single,
}

impl Default for ModelTiling {
    fn default() -> Self {
        Self::Single
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExecutionOptions {
    pub cpu_workers: CpuWorkers,
    pub memory_budget_mib: Option<u64>,
    pub io_workers: Option<u32>,
    pub resume_policy: ResumePolicy,
}

impl Default for ExecutionOptions {
    fn default() -> Self {
        Self {
            cpu_workers: CpuWorkers::Auto,
            memory_budget_mib: None,
            io_workers: None,
            resume_policy: ResumePolicy::Off,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResumePolicy {
    Off,
    RetainOnFailure,
    Resume,
}

impl Default for ResumePolicy {
    fn default() -> Self {
        Self::Off
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CpuWorkers {
    Auto,
    Count(u32),
}

impl Default for CpuWorkers {
    fn default() -> Self {
        Self::Auto
    }
}

impl CpuWorkers {
    pub fn is_auto(self) -> bool {
        matches!(self, CpuWorkers::Auto)
    }

    pub fn as_explicit(self) -> Option<u32> {
        match self {
            CpuWorkers::Auto => None,
            CpuWorkers::Count(n) => Some(n),
        }
    }
}

impl From<u32> for CpuWorkers {
    fn from(value: u32) -> Self {
        if value == 0 {
            CpuWorkers::Auto
        } else {
            CpuWorkers::Count(value)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedExecutionOptions {
    pub cpu_workers: u32,
    pub memory_budget_mib: u64,
    pub io_workers: u32,
}

impl ExecutionOptions {
    pub fn parse(options: &Value) -> Result<Self, String> {
        let mut exec_opts = ExecutionOptions::default();
        
        let exec_value = options.get("execution");
        let legacy_threads = options
            .get("convert")
            .and_then(|v| v.get("threads"));

        if let Some(exec) = exec_value {
            if let Some(workers) = exec.get("cpuWorkers").and_then(|v| v.as_u64()) {
                if workers > u32::MAX as u64 {
                    return Err("execution.cpuWorkers exceeds maximum value".into());
                }
                exec_opts.cpu_workers = CpuWorkers::from(workers as u32);
            }

            if let Some(mem) = exec.get("memoryBudgetMiB").and_then(|v| v.as_u64()) {
                exec_opts.memory_budget_mib = Some(mem);
            }

            if let Some(io) = exec.get("ioWorkers").and_then(|v| v.as_u64()) {
                if io > u32::MAX as u64 {
                    return Err("execution.ioWorkers exceeds maximum value".into());
                }
                exec_opts.io_workers = Some(io as u32);
            }

            if let Some(resume) = exec.get("resumePolicy").and_then(|v| v.as_str()) {
                exec_opts.resume_policy = match resume {
                    "off" => ResumePolicy::Off,
                    "retain-on-failure" => ResumePolicy::RetainOnFailure,
                    "resume" => ResumePolicy::Resume,
                    _ => return Err(format!("invalid execution.resumePolicy: {}", resume)),
                };
            }
        }

        if let Some(threads_val) = legacy_threads {
            let legacy_count = if let Some(n) = threads_val.as_u64() {
                if n > u32::MAX as u64 {
                    return Err("convert.threads exceeds maximum value".into());
                }
                Some(n as u32)
            } else {
                None
            };

            if let Some(legacy) = legacy_count {
                match exec_opts.cpu_workers {
                    CpuWorkers::Auto => {
                        exec_opts.cpu_workers = CpuWorkers::from(legacy);
                    }
                    CpuWorkers::Count(_explicit) => {
                    }
                }
            }
        }

        exec_opts.validate()?;
        Ok(exec_opts)
    }

    pub fn validate(&self) -> Result<(), String> {
        if let Some(mem) = self.memory_budget_mib {
            if mem == 0 {
                return Err("execution.memoryBudgetMiB must be positive when specified".into());
            }
        }
        if let Some(io) = self.io_workers {
            if io == 0 {
                return Err("execution.ioWorkers must be positive when specified".into());
            }
        }
        Ok(())
    }

    pub fn resolve(&self) -> ResolvedExecutionOptions {
        let available_parallelism = std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(4);

        let cpu_workers = match self.cpu_workers {
            CpuWorkers::Auto => {
                let auto_value = (available_parallelism / 2).max(1).min(8);
                auto_value
            }
            CpuWorkers::Count(n) => n.max(1).min(available_parallelism),
        };

        let memory_budget_mib = self.memory_budget_mib.unwrap_or_else(|| {
            4096
        });

        let io_workers = self.io_workers.unwrap_or(2).max(1);

        ResolvedExecutionOptions {
            cpu_workers,
            memory_budget_mib,
            io_workers,
        }
    }

    pub fn describe_resolution(&self, resolved: &ResolvedExecutionOptions) -> String {
        let cpu_desc = match self.cpu_workers {
            CpuWorkers::Auto => format!("auto → {}", resolved.cpu_workers),
            CpuWorkers::Count(n) => {
                if n == resolved.cpu_workers {
                    format!("{} (explicit)", n)
                } else {
                    format!("{} (requested) → {} (capped)", n, resolved.cpu_workers)
                }
            }
        };

        let mem_desc = if let Some(req) = self.memory_budget_mib {
            format!("{} MiB (explicit)", req)
        } else {
            format!("{} MiB (default)", resolved.memory_budget_mib)
        };

        let io_desc = if let Some(req) = self.io_workers {
            format!("{} (explicit)", req)
        } else {
            format!("{} (default)", resolved.io_workers)
        };

        format!(
            "cpu_workers: {}, memory_budget: {}, io_workers: {}",
            cpu_desc, mem_desc, io_desc
        )
    }
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

    pub fn model_options(&self) -> Result<ModelTaskOptions, String> {
        let options = serde_json::from_value(self.options.clone())
            .map_err(|error| format!("invalid convert-model options: {error}"))?;
        Ok(options)
    }

    pub fn execution_options(&self) -> Result<ExecutionOptions, String> {
        ExecutionOptions::parse(&self.options)
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
    Merge,
    Clip,
    Flatten,
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
            Stage::Merge => "merge",
            Stage::Clip => "clip",
            Stage::Flatten => "flatten",
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

    #[test]
    fn parses_anchor_model_options_at_the_geographic_origin() {
        let cfg = TaskConfig {
            schema_version: Some(1),
            task_id: "model-001".into(),
            operation: "convert-model".into(),
            input: PathRef { path: "building.fbx".into() },
            output: PathRef { path: "result".into() },
            options: json!({
                "model": { "format": "fbx" },
                "georeference": {
                    "mode": "anchor",
                    "longitudeDeg": 0.0,
                    "latitudeDeg": 0.0,
                    "ellipsoidHeightM": 0.0
                }
            }),
        };

        let result = cfg.model_options().and_then(|options| options.validate());
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn obj_requires_explicit_unit_and_axes() {
        let cfg = TaskConfig {
            schema_version: Some(1),
            task_id: "model-002".into(),
            operation: "convert-model".into(),
            input: PathRef { path: "building.obj".into() },
            output: PathRef { path: "result".into() },
            options: json!({ "model": { "format": "obj" } }),
        };

        let error = cfg
            .model_options()
            .and_then(|options| options.validate())
            .expect_err("OBJ metadata is insufficient");
        assert!(error.contains("model.unit"));
    }

    #[test]
    fn execution_options_default() {
        let cfg = TaskConfig {
            schema_version: Some(1),
            task_id: "exec-001".into(),
            operation: "convert-osgb".into(),
            input: PathRef { path: "data".into() },
            output: PathRef { path: "out".into() },
            options: json!({}),
        };
        let exec = cfg.execution_options().unwrap();
        assert!(exec.cpu_workers.is_auto());
        assert!(exec.memory_budget_mib.is_none());
        assert!(exec.io_workers.is_none());
    }

    #[test]
    fn execution_options_explicit_cpu_workers() {
        let cfg = TaskConfig {
            schema_version: Some(1),
            task_id: "exec-002".into(),
            operation: "convert-osgb".into(),
            input: PathRef { path: "data".into() },
            output: PathRef { path: "out".into() },
            options: json!({ "execution": { "cpuWorkers": 4 } }),
        };
        let exec = cfg.execution_options().unwrap();
        assert_eq!(exec.cpu_workers.as_explicit(), Some(4));
    }

    #[test]
    fn execution_options_explicit_auto() {
        let cfg = TaskConfig {
            schema_version: Some(1),
            task_id: "exec-003".into(),
            operation: "convert-osgb".into(),
            input: PathRef { path: "data".into() },
            output: PathRef { path: "out".into() },
            options: json!({ "execution": { "cpuWorkers": 0 } }),
        };
        let exec = cfg.execution_options().unwrap();
        assert!(exec.cpu_workers.is_auto());
    }

    #[test]
    fn execution_options_legacy_threads() {
        let cfg = TaskConfig {
            schema_version: Some(1),
            task_id: "exec-004".into(),
            operation: "convert-osgb".into(),
            input: PathRef { path: "data".into() },
            output: PathRef { path: "out".into() },
            options: json!({ "convert": { "threads": 2 } }),
        };
        let exec = cfg.execution_options().unwrap();
        assert_eq!(exec.cpu_workers.as_explicit(), Some(2));
    }

    #[test]
    fn execution_options_explicit_wins_over_legacy() {
        let cfg = TaskConfig {
            schema_version: Some(1),
            task_id: "exec-005".into(),
            operation: "convert-osgb".into(),
            input: PathRef { path: "data".into() },
            output: PathRef { path: "out".into() },
            options: json!({
                "convert": { "threads": 2 },
                "execution": { "cpuWorkers": 4 }
            }),
        };
        let exec = cfg.execution_options().unwrap();
        assert_eq!(exec.cpu_workers.as_explicit(), Some(4));
    }

    #[test]
    fn execution_options_invalid_zero_memory() {
        let cfg = TaskConfig {
            schema_version: Some(1),
            task_id: "exec-006".into(),
            operation: "convert-osgb".into(),
            input: PathRef { path: "data".into() },
            output: PathRef { path: "out".into() },
            options: json!({ "execution": { "memoryBudgetMiB": 0 } }),
        };
        let err = cfg.execution_options().expect_err("zero memory should fail");
        assert!(err.contains("memoryBudgetMiB"));
    }

    #[test]
    fn execution_options_invalid_zero_io_workers() {
        let cfg = TaskConfig {
            schema_version: Some(1),
            task_id: "exec-007".into(),
            operation: "convert-osgb".into(),
            input: PathRef { path: "data".into() },
            output: PathRef { path: "out".into() },
            options: json!({ "execution": { "ioWorkers": 0 } }),
        };
        let err = cfg.execution_options().expect_err("zero io workers should fail");
        assert!(err.contains("ioWorkers"));
    }

    #[test]
    fn execution_options_resolve_auto() {
        let exec = ExecutionOptions::default();
        let resolved = exec.resolve();
        assert!(resolved.cpu_workers >= 1 && resolved.cpu_workers <= 8);
        assert!(resolved.memory_budget_mib > 0);
        assert!(resolved.io_workers >= 1);
    }

    #[test]
    fn execution_options_resolve_explicit() {
        let exec = ExecutionOptions {
            cpu_workers: CpuWorkers::Count(2),
            memory_budget_mib: Some(8192),
            io_workers: Some(2),
            resume_policy: ResumePolicy::Off,
        };
        let resolved = exec.resolve();
        assert_eq!(resolved.cpu_workers, 2);
        assert_eq!(resolved.memory_budget_mib, 8192);
        assert_eq!(resolved.io_workers, 2);
    }

    #[test]
    fn execution_options_describe_resolution() {
        let exec = ExecutionOptions {
            cpu_workers: CpuWorkers::Count(4),
            memory_budget_mib: Some(8192),
            io_workers: Some(2),
            resume_policy: ResumePolicy::Off,
        };
        let resolved = exec.resolve();
        let desc = exec.describe_resolution(&resolved);
        assert!(desc.contains("cpu_workers"));
        assert!(desc.contains("memory_budget"));
        assert!(desc.contains("io_workers"));
    }
}
