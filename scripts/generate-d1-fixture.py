#!/usr/bin/env python3
"""
Generate D1 small-grid synthetic fixture: 2x2 Tile blocks with simple geometry.
"""
import struct
import json
import os
from pathlib import Path

def create_glb_with_cube(size=1.0):
    """Create a minimal GLB with a simple cube mesh."""
    # Simple cube vertices (8 corners)
    vertices = [
        -size, -size, -size,  # 0
         size, -size, -size,  # 1
         size,  size, -size,  # 2
        -size,  size, -size,  # 3
        -size, -size,  size,  # 4
         size, -size,  size,  # 5
         size,  size,  size,  # 6
        -size,  size,  size,  # 7
    ]
    
    # Cube indices (12 triangles, 2 per face)
    indices = [
        # Front
        0, 1, 2, 0, 2, 3,
        # Back
        5, 4, 7, 5, 7, 6,
        # Left
        4, 0, 3, 4, 3, 7,
        # Right
        1, 5, 6, 1, 6, 2,
        # Top
        3, 2, 6, 3, 6, 7,
        # Bottom
        4, 5, 1, 4, 1, 0,
    ]
    
    # Pack vertex data (float32)
    vertex_bytes = struct.pack(f'<{len(vertices)}f', *vertices)
    
    # Pack index data (uint16)
    index_bytes = struct.pack(f'<{len(indices)}H', *indices)
    
    # Align to 4 bytes
    vertex_bytes_padded = vertex_bytes + b'\x00' * ((4 - len(vertex_bytes) % 4) % 4)
    index_bytes_padded = index_bytes + b'\x00' * ((4 - len(index_bytes) % 4) % 4)
    
    # Create buffer
    buffer_data = vertex_bytes_padded + index_bytes_padded
    buffer_length = len(buffer_data)
    
    # GLB JSON (minimal glTF with cube mesh)
    gltf_json = {
        "asset": {"version": "2.0"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0}],
        "meshes": [{
            "primitives": [{
                "attributes": {"POSITION": 0},
                "indices": 1,
                "mode": 4  # TRIANGLES
            }]
        }],
        "accessors": [
            {
                "bufferView": 0,
                "componentType": 5126,  # FLOAT
                "count": 8,
                "type": "VEC3",
                "min": [-size, -size, -size],
                "max": [size, size, size]
            },
            {
                "bufferView": 1,
                "componentType": 5123,  # UNSIGNED_SHORT
                "count": len(indices),
                "type": "SCALAR"
            }
        ],
        "bufferViews": [
            {
                "buffer": 0,
                "byteOffset": 0,
                "byteLength": len(vertex_bytes_padded),
                "target": 34962  # ARRAY_BUFFER
            },
            {
                "buffer": 0,
                "byteOffset": len(vertex_bytes_padded),
                "byteLength": len(index_bytes_padded),
                "target": 34963  # ELEMENT_ARRAY_BUFFER
            }
        ],
        "buffers": [{"byteLength": buffer_length}]
    }
    
    gltf_json_str = json.dumps(gltf_json, separators=(',', ':'))
    gltf_json_bytes = gltf_json_str.encode('utf-8')
    gltf_json_padded = gltf_json_bytes + b' ' * ((4 - len(gltf_json_bytes) % 4) % 4)
    
    # GLB chunks
    json_chunk = struct.pack('<II', len(gltf_json_padded), 0x4E4F534A) + gltf_json_padded
    bin_chunk = struct.pack('<II', len(buffer_data), 0x004E4942) + buffer_data
    
    # GLB header
    glb_header = struct.pack('<III', 0x46546C67, 2, 12 + len(json_chunk) + len(bin_chunk))
    
    return glb_header + json_chunk + bin_chunk

def create_b3dm(glb_data):
    """Wrap GLB in B3DM format."""
    # Feature table (minimal: BATCH_LENGTH = 0)
    ft_json = {"BATCH_LENGTH": 0}
    ft_json_bytes = json.dumps(ft_json, separators=(',', ':')).encode('utf-8')
    ft_json_padded = ft_json_bytes + b' ' * ((8 - len(ft_json_bytes) % 8) % 8)
    
    # B3DM header
    b3dm_magic = b'b3dm'
    b3dm_version = 1
    b3dm_byte_length = 28 + len(ft_json_padded) + len(glb_data)
    
    header = struct.pack('<4sIIIIIII',
        b3dm_magic, b3dm_version, b3dm_byte_length,
        len(ft_json_padded), 0, 0, 0, 0)
    
    return header + ft_json_padded + glb_data

def create_tile_block(base_dir, tile_id, grid_x, grid_y, levels=2):
    """Create a single Tile block with multiple LOD levels."""
    tile_dir = base_dir / f"Tile_{grid_x:+04d}_{grid_y:+04d}"
    tile_dir.mkdir(parents=True, exist_ok=True)
    
    # Create B3DM files for each level (decreasing size)
    tile_contents = []
    for level in range(levels):
        size = 5.0 / (2 ** level)  # L0=5m, L1=2.5m, L2=1.25m
        glb = create_glb_with_cube(size)
        b3dm = create_b3dm(glb)
        
        filename = f"L{level}.b3dm"
        filepath = tile_dir / filename
        filepath.write_bytes(b3dm)
        
        tile_contents.append({
            "level": level,
            "uri": filename,
            "geometricError": 10.0 / (2 ** level),  # L0=10, L1=5, L2=2.5
            "size": len(b3dm)
        })
    
    # Create tileset.json for this block
    root_tile = {
        "boundingVolume": {
            "box": [
                grid_x * 20.0, grid_y * 20.0, 2.5,  # center
                10, 0, 0,  # half-axes
                0, 10, 0,
                0, 0, 2.5
            ]
        },
        "geometricError": tile_contents[0]["geometricError"],
        "content": {"uri": tile_contents[0]["uri"]}
    }
    
    # Add children if multiple levels
    if len(tile_contents) > 1:
        root_tile["children"] = []
        for tc in tile_contents[1:]:
            root_tile["children"].append({
                "boundingVolume": {
                    "box": [
                        grid_x * 20.0, grid_y * 20.0, 2.5,
                        10, 0, 0,
                        0, 10, 0,
                        0, 0, 2.5
                    ]
                },
                "geometricError": tc["geometricError"],
                "content": {"uri": tc["uri"]}
            })
    
    tileset = {
        "asset": {"version": "1.0"},
        "geometricError": root_tile["geometricError"],
        "root": root_tile
    }
    
    tileset_path = tile_dir / "tileset.json"
    tileset_path.write_text(json.dumps(tileset, indent=2))
    
    return {
        "tile_id": tile_id,
        "grid_x": grid_x,
        "grid_y": grid_y,
        "levels": levels,
        "total_size": sum(tc["size"] for tc in tile_contents)
    }

def create_root_tileset(base_dir, blocks):
    """Create root tileset.json referencing all blocks."""
    children = []
    
    for block in blocks:
        gx, gy = block["grid_x"], block["grid_y"]
        children.append({
            "boundingVolume": {
                "box": [
                    gx * 20.0, gy * 20.0, 2.5,
                    10, 0, 0,
                    0, 10, 0,
                    0, 0, 2.5
                ]
            },
            "geometricError": 10.0,
            "content": {
                "uri": f"Tile_{gx:+04d}_{gy:+04d}/tileset.json"
            }
        })
    
    root_tileset = {
        "asset": {"version": "1.0"},
        "geometricError": 50.0,
        "root": {
            "boundingVolume": {
                "box": [
                    10.0, 10.0, 2.5,  # center of 2x2 grid
                    20, 0, 0,
                    0, 20, 0,
                    0, 0, 2.5
                ]
            },
            "geometricError": 20.0,
            "children": children
        }
    }
    
    root_path = base_dir / "tileset.json"
    root_path.write_text(json.dumps(root_tileset, indent=2))

def generate_small_grid(output_dir):
    """Generate complete 2x2 small-grid fixture."""
    base_dir = Path(output_dir)
    base_dir.mkdir(parents=True, exist_ok=True)
    
    # Create 2x2 grid of blocks
    blocks = []
    for gx in [0, 1]:
        for gy in [0, 1]:
            block_info = create_tile_block(base_dir, f"{gx}_{gy}", gx, gy, levels=2)
            blocks.append(block_info)
            print(f"Created block Tile_{gx:+04d}_{gy:+04d}: {block_info['total_size']} bytes")
    
    # Create root tileset
    create_root_tileset(base_dir, blocks)
    
    total_size = sum(b["total_size"] for b in blocks)
    print(f"\nTotal fixture size: {total_size} bytes ({total_size/1024:.1f} KB)")
    print(f"Blocks: {len(blocks)}")
    print(f"Total tiles: {len(blocks) * 2}")  # 2 levels per block

if __name__ == "__main__":
    output_dir = "docs/acceptance/v1-convergence/fixtures/d1-medium/small-grid"
    generate_small_grid(output_dir)
    print(f"\nFixture generated at: {output_dir}")
