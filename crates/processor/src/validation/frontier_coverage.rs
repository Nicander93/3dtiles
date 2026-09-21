use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

use super::ValidationError;

#[derive(Debug, Serialize, Deserialize)]
pub struct FrontierCoverageResult {
    pub checked: bool,
    pub passed: bool,
    pub grid_size: String,
    pub blocks_expected: usize,
    pub blocks_found: usize,
    pub gaps: Vec<(i32, i32)>,
}

#[derive(Debug, Deserialize)]
struct Tileset {
    root: Tile,
}

#[derive(Debug, Deserialize)]
struct Tile {
    #[serde(default)]
    content: Option<Content>,
    
    #[serde(default)]
    children: Vec<Tile>,
}

#[derive(Debug, Deserialize)]
struct Content {
    uri: Option<String>,
}

pub fn check_frontier_coverage(tileset_path: &Path) -> Result<FrontierCoverageResult, ValidationError> {
    let content = std::fs::read_to_string(tileset_path)?;
    let tileset: Tileset = serde_json::from_str(&content)?;
    
    let mut coords = HashSet::new();
    extract_tile_coords(&tileset.root, &mut coords);
    
    // For 2x2 grid, expect (0,0), (1,0), (0,1), (1,1)
    let expected = [(0, 0), (1, 0), (0, 1), (1, 1)];
    let mut gaps = Vec::new();
    
    for &coord in &expected {
        if !coords.contains(&coord) {
            gaps.push(coord);
        }
    }
    
    Ok(FrontierCoverageResult {
        checked: true,
        passed: gaps.is_empty(),
        grid_size: "2x2".to_string(),
        blocks_expected: 4,
        blocks_found: coords.len(),
        gaps,
    })
}

fn extract_tile_coords(tile: &Tile, coords: &mut HashSet<(i32, i32)>) {
    if let Some(content) = &tile.content {
        if let Some(uri) = &content.uri {
            if let Some(coord) = parse_tile_coord(uri) {
                coords.insert(coord);
            }
        }
    }
    
    for child in &tile.children {
        extract_tile_coords(child, coords);
    }
}

fn parse_tile_coord(uri: &str) -> Option<(i32, i32)> {
    // Parse various formats:
    // "Tile_+000_+001/tileset.json" -> (0, 1)
    // "./Data/Tile_+000_+001/tileset.json" -> (0, 1)
    // "Tile_+000_+001" -> (0, 1)
    
    // Split by / and find the Tile_* part
    for part in uri.split('/') {
        if part.starts_with("Tile_") {
            let coords_str = &part[5..]; // skip "Tile_"
            let coords: Vec<&str> = coords_str.split('_').collect();
            
            if coords.len() != 2 {
                continue;
            }
            
            let col = coords[0].trim_start_matches('+').trim_start_matches('-').parse::<i32>().ok()?;
            let row = coords[1].trim_start_matches('+').trim_start_matches('-').parse::<i32>().ok()?;
            
            // Handle negative signs
            let col = if coords[0].starts_with('-') { -col } else { col };
            let row = if coords[1].starts_with('-') { -row } else { row };
            
            return Some((col, row));
        }
    }
    
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_parse_tile_coord() {
        assert_eq!(parse_tile_coord("Tile_+000_+000/tileset.json"), Some((0, 0)));
        assert_eq!(parse_tile_coord("Tile_+001_+000/tileset.json"), Some((1, 0)));
        assert_eq!(parse_tile_coord("Tile_+000_+001/tileset.json"), Some((0, 1)));
        assert_eq!(parse_tile_coord("Tile_+001_+001/tileset.json"), Some((1, 1)));
        assert_eq!(parse_tile_coord("Tile_+010_+020"), Some((10, 20)));
        assert_eq!(parse_tile_coord("./Data/Tile_+000_+000/tileset.json"), Some((0, 0)));
        assert_eq!(parse_tile_coord("./Data/Tile_+001_+001/tileset.json"), Some((1, 1)));
        assert_eq!(parse_tile_coord("invalid"), None);
        assert_eq!(parse_tile_coord("Tile_+000"), None);
    }
    
    #[test]
    fn test_frontier_2x2_complete() {
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
                        "content": {"uri": "Tile_+000_+000/tileset.json"}
                    },
                    {
                        "geometricError": 50,
                        "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                        "content": {"uri": "Tile_+001_+000/tileset.json"}
                    },
                    {
                        "geometricError": 50,
                        "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                        "content": {"uri": "Tile_+000_+001/tileset.json"}
                    },
                    {
                        "geometricError": 50,
                        "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                        "content": {"uri": "Tile_+001_+001/tileset.json"}
                    }
                ]
            }
        }"#;
        
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(tileset_json.as_bytes()).unwrap();
        
        let result = check_frontier_coverage(file.path()).unwrap();
        
        assert!(result.checked);
        assert!(result.passed);
        assert_eq!(result.grid_size, "2x2");
        assert_eq!(result.blocks_expected, 4);
        assert_eq!(result.blocks_found, 4);
        assert_eq!(result.gaps.len(), 0);
    }
    
    #[test]
    fn test_frontier_2x2_missing_block() {
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
                        "content": {"uri": "Tile_+000_+000/tileset.json"}
                    },
                    {
                        "geometricError": 50,
                        "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                        "content": {"uri": "Tile_+001_+000/tileset.json"}
                    },
                    {
                        "geometricError": 50,
                        "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                        "content": {"uri": "Tile_+000_+001/tileset.json"}
                    }
                ]
            }
        }"#;
        
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(tileset_json.as_bytes()).unwrap();
        
        let result = check_frontier_coverage(file.path()).unwrap();
        
        assert!(result.checked);
        assert!(!result.passed);
        assert_eq!(result.blocks_expected, 4);
        assert_eq!(result.blocks_found, 3);
        assert_eq!(result.gaps.len(), 1);
        assert!(result.gaps.contains(&(1, 1)));
    }
    
    #[test]
    fn test_frontier_empty() {
        let tileset_json = r#"{
            "asset": {"version": "1.0"},
            "geometricError": 100,
            "root": {
                "geometricError": 100,
                "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                "children": []
            }
        }"#;
        
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(tileset_json.as_bytes()).unwrap();
        
        let result = check_frontier_coverage(file.path()).unwrap();
        
        assert!(result.checked);
        assert!(!result.passed);
        assert_eq!(result.blocks_found, 0);
        assert_eq!(result.gaps.len(), 4);
    }
}
