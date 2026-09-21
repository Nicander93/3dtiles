use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("Invalid tileset: {0}")]
    InvalidTileset(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeMonotonicityResult {
    pub checked: bool,
    pub passed: bool,
    pub violations: Vec<GeViolation>,
    pub total_tiles: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeViolation {
    pub parent_uri: String,
    pub parent_ge: f64,
    pub child_uri: String,
    pub child_ge: f64,
}

#[derive(Debug, Deserialize)]
struct Tileset {
    root: Tile,
}

#[derive(Debug, Deserialize)]
struct Tile {
    #[serde(rename = "geometricError")]
    geometric_error: f64,
    
    #[serde(default)]
    content: Option<Content>,
    
    #[serde(default)]
    children: Vec<Tile>,
}

#[derive(Debug, Deserialize)]
struct Content {
    uri: Option<String>,
}

pub fn check_ge_monotonicity(tileset_path: &Path) -> Result<GeMonotonicityResult, ValidationError> {
    let content = std::fs::read_to_string(tileset_path)?;
    let tileset: Tileset = serde_json::from_str(&content)?;
    
    let mut violations = Vec::new();
    let mut total_tiles = 0;
    
    check_tile(&tileset.root, "root", None, &mut violations, &mut total_tiles);
    
    Ok(GeMonotonicityResult {
        checked: true,
        passed: violations.is_empty(),
        violations,
        total_tiles,
    })
}

fn check_tile(
    tile: &Tile,
    tile_uri: &str,
    parent_ge: Option<f64>,
    violations: &mut Vec<GeViolation>,
    total_tiles: &mut usize,
) {
    *total_tiles += 1;
    
    // Check if parent GE >= current GE
    if let Some(p_ge) = parent_ge {
        if p_ge < tile.geometric_error {
            // Violation: parent GE < child GE
            violations.push(GeViolation {
                parent_uri: "parent".to_string(), // In real impl, track parent URI
                parent_ge: p_ge,
                child_uri: tile_uri.to_string(),
                child_ge: tile.geometric_error,
            });
        }
    }
    
    // Recursively check children
    for (i, child) in tile.children.iter().enumerate() {
        let child_uri = if let Some(ref content) = child.content {
            content.uri.clone().unwrap_or_else(|| format!("child-{}", i))
        } else {
            format!("child-{}", i)
        };
        
        check_tile(
            child,
            &child_uri,
            Some(tile.geometric_error),
            violations,
            total_tiles,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_ge_monotonicity_pass() {
        let tileset_json = r#"{
            "asset": {"version": "1.0"},
            "geometricError": 100,
            "root": {
                "geometricError": 100,
                "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                "children": [
                    {
                        "geometricError": 50,
                        "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                        "content": {"uri": "child1.b3dm"}
                    },
                    {
                        "geometricError": 50,
                        "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                        "content": {"uri": "child2.b3dm"},
                        "children": [
                            {
                                "geometricError": 25,
                                "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                                "content": {"uri": "grandchild.b3dm"}
                            }
                        ]
                    }
                ]
            }
        }"#;
        
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(tileset_json.as_bytes()).unwrap();
        
        let result = check_ge_monotonicity(file.path()).unwrap();
        
        assert!(result.checked);
        assert!(result.passed);
        assert_eq!(result.violations.len(), 0);
        assert_eq!(result.total_tiles, 4); // root + 2 children + 1 grandchild
    }
    
    #[test]
    fn test_ge_monotonicity_violation() {
        let tileset_json = r#"{
            "asset": {"version": "1.0"},
            "geometricError": 100,
            "root": {
                "geometricError": 50,
                "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                "children": [
                    {
                        "geometricError": 100,
                        "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                        "content": {"uri": "child.b3dm"}
                    }
                ]
            }
        }"#;
        
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(tileset_json.as_bytes()).unwrap();
        
        let result = check_ge_monotonicity(file.path()).unwrap();
        
        assert!(result.checked);
        assert!(!result.passed);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].parent_ge, 50.0);
        assert_eq!(result.violations[0].child_ge, 100.0);
    }
}
