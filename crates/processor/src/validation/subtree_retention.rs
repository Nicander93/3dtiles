use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

use super::ValidationError;

#[derive(Debug, Serialize, Deserialize)]
pub struct SubtreeRetentionResult {
    pub checked: bool,
    pub passed: bool,
    pub blocks_expected: usize,
    pub blocks_retained: usize,
    pub blocks_lost: Vec<String>,
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

pub fn check_subtree_retention(
    input_tileset: &Path,
    output_tileset: &Path,
) -> Result<SubtreeRetentionResult, ValidationError> {
    let input_content = std::fs::read_to_string(input_tileset)?;
    let input: Tileset = serde_json::from_str(&input_content)?;
    
    let output_content = std::fs::read_to_string(output_tileset)?;
    let output: Tileset = serde_json::from_str(&output_content)?;
    
    let input_blocks = extract_external_blocks(&input.root);
    let output_blocks = extract_external_blocks(&output.root);
    
    let mut lost = Vec::new();
    for block in &input_blocks {
        if !output_blocks.contains(block) {
            lost.push(block.clone());
        }
    }
    
    Ok(SubtreeRetentionResult {
        checked: true,
        passed: lost.is_empty(),
        blocks_expected: input_blocks.len(),
        blocks_retained: output_blocks.len(),
        blocks_lost: lost,
    })
}

fn extract_external_blocks(tile: &Tile) -> HashSet<String> {
    let mut blocks = HashSet::new();
    
    if let Some(content) = &tile.content {
        if let Some(uri) = &content.uri {
            // Find Tile_* pattern in URI (handle ./Data/Tile_* or just Tile_*)
            for part in uri.split('/') {
                if part.starts_with("Tile_") {
                    blocks.insert(part.to_string());
                    break;
                }
            }
        }
    }
    
    for child in &tile.children {
        blocks.extend(extract_external_blocks(child));
    }
    
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_subtree_all_retained() {
        let input_json = r#"{
            "asset": {"version": "1.0"},
            "geometricError": 100,
            "root": {
                "geometricError": 100,
                "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                "children": [
                    { "content": { "uri": "Tile_+000_+000/tileset.json" } },
                    { "content": { "uri": "Tile_+001_+000/tileset.json" } },
                    { "content": { "uri": "Tile_+000_+001/tileset.json" } },
                    { "content": { "uri": "Tile_+001_+001/tileset.json" } }
                ]
            }
        }"#;
        
        let output_json = r#"{
            "asset": {"version": "1.0"},
            "geometricError": 100,
            "root": {
                "geometricError": 100,
                "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                "children": [
                    { "content": { "uri": "Tile_+000_+000/tileset.json" } },
                    { "content": { "uri": "Tile_+001_+000/tileset.json" } },
                    { "content": { "uri": "Tile_+000_+001/tileset.json" } },
                    { "content": { "uri": "Tile_+001_+001/tileset.json" } }
                ]
            }
        }"#;
        
        let mut input_file = NamedTempFile::new().unwrap();
        input_file.write_all(input_json.as_bytes()).unwrap();
        
        let mut output_file = NamedTempFile::new().unwrap();
        output_file.write_all(output_json.as_bytes()).unwrap();
        
        let result = check_subtree_retention(input_file.path(), output_file.path()).unwrap();
        
        assert!(result.checked);
        assert!(result.passed);
        assert_eq!(result.blocks_expected, 4);
        assert_eq!(result.blocks_retained, 4);
        assert_eq!(result.blocks_lost.len(), 0);
    }
    
    #[test]
    fn test_subtree_one_lost() {
        let input_json = r#"{
            "asset": {"version": "1.0"},
            "geometricError": 100,
            "root": {
                "geometricError": 100,
                "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                "children": [
                    { "content": { "uri": "Tile_+000_+000/tileset.json" } },
                    { "content": { "uri": "Tile_+001_+000/tileset.json" } },
                    { "content": { "uri": "Tile_+000_+001/tileset.json" } },
                    { "content": { "uri": "Tile_+001_+001/tileset.json" } }
                ]
            }
        }"#;
        
        let output_json = r#"{
            "asset": {"version": "1.0"},
            "geometricError": 100,
            "root": {
                "geometricError": 100,
                "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                "children": [
                    { "content": { "uri": "Tile_+000_+000/tileset.json" } },
                    { "content": { "uri": "Tile_+001_+000/tileset.json" } },
                    { "content": { "uri": "Tile_+000_+001/tileset.json" } }
                ]
            }
        }"#;
        
        let mut input_file = NamedTempFile::new().unwrap();
        input_file.write_all(input_json.as_bytes()).unwrap();
        
        let mut output_file = NamedTempFile::new().unwrap();
        output_file.write_all(output_json.as_bytes()).unwrap();
        
        let result = check_subtree_retention(input_file.path(), output_file.path()).unwrap();
        
        assert!(result.checked);
        assert!(!result.passed);
        assert_eq!(result.blocks_expected, 4);
        assert_eq!(result.blocks_retained, 3);
        assert_eq!(result.blocks_lost.len(), 1);
        assert!(result.blocks_lost.contains(&"Tile_+001_+001".to_string()));
    }
    
    #[test]
    fn test_subtree_empty_input() {
        let input_json = r#"{
            "asset": {"version": "1.0"},
            "geometricError": 100,
            "root": {
                "geometricError": 100,
                "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                "children": []
            }
        }"#;
        
        let output_json = r#"{
            "asset": {"version": "1.0"},
            "geometricError": 100,
            "root": {
                "geometricError": 100,
                "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                "children": []
            }
        }"#;
        
        let mut input_file = NamedTempFile::new().unwrap();
        input_file.write_all(input_json.as_bytes()).unwrap();
        
        let mut output_file = NamedTempFile::new().unwrap();
        output_file.write_all(output_json.as_bytes()).unwrap();
        
        let result = check_subtree_retention(input_file.path(), output_file.path()).unwrap();
        
        assert!(result.checked);
        assert!(result.passed); // No blocks expected, so pass
        assert_eq!(result.blocks_expected, 0);
        assert_eq!(result.blocks_retained, 0);
        assert_eq!(result.blocks_lost.len(), 0);
    }
}
