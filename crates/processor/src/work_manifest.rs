//! Work unit manifest for resumable task execution.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnitStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkUnit {
    pub job_key: String,
    pub unit_type: UnitType,
    pub status: UnitStatus,
    pub attempts: u32,
    pub input_fingerprint: Option<String>,
    pub param_hash: Option<String>,
    pub output_checksum: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    pub last_updated_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnitType {
    Convert,
    Rebuild,
    Texture,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkManifest {
    pub version: u32,
    pub task_id: String,
    pub tool_versions: ToolVersions,
    pub units: HashMap<String, WorkUnit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolVersions {
    pub processor_version: String,
    pub converter_version: Option<String>,
}

impl WorkManifest {
    pub fn new(task_id: String) -> Self {
        Self {
            version: 1,
            task_id,
            tool_versions: ToolVersions {
                processor_version: env!("CARGO_PKG_VERSION").to_string(),
                converter_version: None,
            },
            units: HashMap::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read manifest: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("failed to parse manifest: {}", e))
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let content = serde_json::to_vec_pretty(self)
            .map_err(|e| format!("failed to serialize manifest: {}", e))?;
        
        let temp_path = path.with_extension("tmp");
        std::fs::write(&temp_path, &content)
            .map_err(|e| format!("failed to write temp manifest: {}", e))?;
        
        std::fs::rename(&temp_path, path)
            .map_err(|e| format!("failed to rename manifest: {}", e))
    }

    pub fn mark_running(&mut self, job_key: &str) {
        if let Some(unit) = self.units.get_mut(job_key) {
            unit.status = UnitStatus::Running;
            unit.attempts += 1;
            unit.last_updated_ms = current_time_ms();
        }
    }

    pub fn mark_succeeded(
        &mut self,
        job_key: &str,
        output_checksum: Option<String>,
    ) {
        if let Some(unit) = self.units.get_mut(job_key) {
            unit.status = UnitStatus::Succeeded;
            unit.output_checksum = output_checksum;
            unit.last_updated_ms = current_time_ms();
        }
    }

    pub fn mark_failed(&mut self, job_key: &str) {
        if let Some(unit) = self.units.get_mut(job_key) {
            unit.status = UnitStatus::Failed;
            unit.last_updated_ms = current_time_ms();
        }
    }

    pub fn reset_running_to_pending(&mut self) {
        for unit in self.units.values_mut() {
            if unit.status == UnitStatus::Running {
                unit.status = UnitStatus::Pending;
            }
        }
    }

    pub fn can_reuse(&self, job_key: &str, input_fingerprint: Option<&str>, param_hash: Option<&str>) -> bool {
        let Some(unit) = self.units.get(job_key) else {
            return false;
        };
        
        if unit.status != UnitStatus::Succeeded {
            return false;
        }

        if let Some(expected) = input_fingerprint {
            if unit.input_fingerprint.as_deref() != Some(expected) {
                return false;
            }
        }

        if let Some(expected) = param_hash {
            if unit.param_hash.as_deref() != Some(expected) {
                return false;
            }
        }

        true
    }

    pub fn add_unit(&mut self, job_key: String, unit: WorkUnit) {
        self.units.insert(job_key, unit);
    }

    pub fn pending_or_failed_units(&self) -> Vec<String> {
        self.units
            .iter()
            .filter(|(_, unit)| {
                matches!(unit.status, UnitStatus::Pending | UnitStatus::Failed)
            })
            .map(|(key, _)| key.clone())
            .collect()
    }
}

impl WorkUnit {
    pub fn new(job_key: String, unit_type: UnitType) -> Self {
        Self {
            job_key,
            unit_type,
            status: UnitStatus::Pending,
            attempts: 0,
            input_fingerprint: None,
            param_hash: None,
            output_checksum: None,
            dependencies: Vec::new(),
            last_updated_ms: current_time_ms(),
        }
    }

    pub fn with_fingerprints(
        mut self,
        input_fingerprint: Option<String>,
        param_hash: Option<String>,
    ) -> Self {
        self.input_fingerprint = input_fingerprint;
        self.param_hash = param_hash;
        self
    }
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn manifest_path(temp_dir: &Path) -> PathBuf {
    temp_dir.join(".geoforge-work-manifest.json")
}

pub fn compute_param_hash(params: &serde_json::Value) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let canonical = serde_json::to_string(params).unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    canonical.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

pub fn compute_input_fingerprint(path: &Path) -> Result<String, String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let metadata = std::fs::metadata(path)
        .map_err(|e| format!("failed to read metadata: {}", e))?;
    
    let mut hasher = DefaultHasher::new();
    metadata.len().hash(&mut hasher);
    
    if let Ok(modified) = metadata.modified() {
        if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
            duration.as_secs().hash(&mut hasher);
        }
    }
    
    Ok(format!("{:x}", hasher.finish()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_empty_manifest() {
        let manifest = WorkManifest::new("task-001".to_string());
        assert_eq!(manifest.version, 1);
        assert_eq!(manifest.task_id, "task-001");
        assert!(manifest.units.is_empty());
    }

    #[test]
    fn adds_and_updates_work_unit() {
        let mut manifest = WorkManifest::new("task-002".to_string());
        let unit = WorkUnit::new("block-01".to_string(), UnitType::Convert);
        manifest.add_unit("block-01".to_string(), unit);

        assert_eq!(manifest.units.len(), 1);
        assert_eq!(manifest.units["block-01"].status, UnitStatus::Pending);

        manifest.mark_running("block-01");
        assert_eq!(manifest.units["block-01"].status, UnitStatus::Running);
        assert_eq!(manifest.units["block-01"].attempts, 1);

        manifest.mark_succeeded("block-01", Some("abc123".to_string()));
        assert_eq!(manifest.units["block-01"].status, UnitStatus::Succeeded);
        assert_eq!(manifest.units["block-01"].output_checksum, Some("abc123".to_string()));
    }

    #[test]
    fn resets_running_to_pending() {
        let mut manifest = WorkManifest::new("task-003".to_string());
        manifest.add_unit("unit-1".to_string(), WorkUnit::new("unit-1".to_string(), UnitType::Convert));
        manifest.add_unit("unit-2".to_string(), WorkUnit::new("unit-2".to_string(), UnitType::Convert));
        
        manifest.mark_running("unit-1");
        manifest.mark_succeeded("unit-2", None);

        assert_eq!(manifest.units["unit-1"].status, UnitStatus::Running);
        assert_eq!(manifest.units["unit-2"].status, UnitStatus::Succeeded);

        manifest.reset_running_to_pending();

        assert_eq!(manifest.units["unit-1"].status, UnitStatus::Pending);
        assert_eq!(manifest.units["unit-2"].status, UnitStatus::Succeeded);
    }

    #[test]
    fn can_reuse_checks_fingerprints() {
        let mut manifest = WorkManifest::new("task-004".to_string());
        let unit = WorkUnit::new("unit-1".to_string(), UnitType::Convert)
            .with_fingerprints(Some("input-abc".to_string()), Some("param-xyz".to_string()));
        manifest.add_unit("unit-1".to_string(), unit);
        manifest.mark_succeeded("unit-1", None);

        assert!(manifest.can_reuse("unit-1", Some("input-abc"), Some("param-xyz")));
        assert!(!manifest.can_reuse("unit-1", Some("input-different"), Some("param-xyz")));
        assert!(!manifest.can_reuse("unit-1", Some("input-abc"), Some("param-different")));
        assert!(!manifest.can_reuse("unit-2", None, None));
    }

    #[test]
    fn pending_or_failed_units_filters_correctly() {
        let mut manifest = WorkManifest::new("task-005".to_string());
        manifest.add_unit("pending".to_string(), WorkUnit::new("pending".to_string(), UnitType::Convert));
        manifest.add_unit("running".to_string(), WorkUnit::new("running".to_string(), UnitType::Convert));
        manifest.add_unit("succeeded".to_string(), WorkUnit::new("succeeded".to_string(), UnitType::Convert));
        manifest.add_unit("failed".to_string(), WorkUnit::new("failed".to_string(), UnitType::Convert));

        manifest.mark_running("running");
        manifest.mark_succeeded("succeeded", None);
        manifest.mark_failed("failed");

        let pending_or_failed = manifest.pending_or_failed_units();
        assert_eq!(pending_or_failed.len(), 2);
        assert!(pending_or_failed.contains(&"pending".to_string()));
        assert!(pending_or_failed.contains(&"failed".to_string()));
    }

    #[test]
    fn compute_param_hash_stable() {
        let params1 = serde_json::json!({"threads": 4, "mode": "keep"});
        let params2 = serde_json::json!({"threads": 4, "mode": "keep"});
        let params3 = serde_json::json!({"threads": 2, "mode": "keep"});

        let hash1 = compute_param_hash(&params1);
        let hash2 = compute_param_hash(&params2);
        let hash3 = compute_param_hash(&params3);

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }
}
