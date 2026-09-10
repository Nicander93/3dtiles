//! Output validation before commit.

use crate::protocol::{Emitter, Stage};
use std::path::Path;

pub fn validate_tileset_dir(emitter: &Emitter, dir: &Path) -> Result<(), String> {
    emitter.stage(Stage::Validate, "Verifying output");
    let tileset = dir.join("tileset.json");
    if !tileset.is_file() {
        return Err(format!("tileset.json missing under {}", dir.display()));
    }
    // Basic JSON parse
    let text = std::fs::read_to_string(&tileset).map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        format!("tileset.json is not valid JSON: {e}")
    })?;
    if v.get("root").is_none() && v.get("asset").is_none() {
        emitter.warning(
            "TILESET_SHAPE",
            "tileset.json missing typical root/asset keys; continuing",
        );
    }
    emitter.log(&format!("[validate] OK {}", tileset.display()));
    Ok(())
}
