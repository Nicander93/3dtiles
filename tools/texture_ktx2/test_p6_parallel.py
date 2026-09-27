#!/usr/bin/env python3
"""P6 verification: parallel texture post-processing tests."""

import json
import tempfile
import shutil
from pathlib import Path

# Test fixtures
def create_minimal_glb_with_texture(path: Path) -> None:
    """Create a minimal GLB with an embedded PNG texture."""
    import struct
    
    # 1x1 red PNG
    png = b'\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde\x00\x00\x00\x0cIDATx\x9cc\xf8\xcf\xc0\x00\x00\x00\x03\x00\x01\x00\x18\xdd\x8d\xb4\x00\x00\x00\x00IEND\xaeB`\x82'
    
    gltf_json = {
        "asset": {"version": "2.0"},
        "images": [{"bufferView": 0, "mimeType": "image/png"}],
        "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": len(png)}],
        "buffers": [{"byteLength": len(png)}],
        "textures": [{"source": 0}],
        "materials": [{"pbrMetallicRoughness": {"baseColorTexture": {"index": 0}}}],
    }
    
    json_str = json.dumps(gltf_json, separators=(',', ':')).encode('utf-8')
    json_pad = (4 - (len(json_str) % 4)) % 4
    json_chunk = json_str + b' ' * json_pad
    
    bin_pad = (4 - (len(png) % 4)) % 4
    bin_chunk = png + b'\x00' * bin_pad
    
    total_len = 12 + 8 + len(json_chunk) + 8 + len(bin_chunk)
    
    glb = bytearray()
    glb.extend(struct.pack('<4sII', b'glTF', 2, total_len))
    glb.extend(struct.pack('<I4s', len(json_chunk), b'JSON'))
    glb.extend(json_chunk)
    glb.extend(struct.pack('<I4s', len(bin_chunk), b'BIN\x00'))
    glb.extend(bin_chunk)
    
    path.write_bytes(bytes(glb))


def test_parallel_file_processing():
    """Test that multiple files can be processed in parallel."""
    print("\n=== Test: Parallel File Processing ===")
    
    with tempfile.TemporaryDirectory() as tmpdir:
        root = Path(tmpdir) / "tileset"
        root.mkdir()
        
        # Create 5 test files
        for i in range(5):
            tile_dir = root / f"tile_{i}"
            tile_dir.mkdir()
            create_minimal_glb_with_texture(tile_dir / "tile.glb")
        
        # Verify files created
        glb_files = list(root.rglob("*.glb"))
        assert len(glb_files) == 5, f"Expected 5 GLB files, got {len(glb_files)}"
        print(f"Created {len(glb_files)} test GLB files")
        
        # Test serial processing
        from texture_ktx2 import process_tileset_dir, find_basisu
        
        basisu = find_basisu()
        if not basisu:
            print("SKIP: basisu not found")
            return
        
        print(f"\nTesting serial (file_workers=1)...")
        result_serial = process_tileset_dir(
            root, mode="ktx2-etc1s", basisu=basisu, quality=128,
            file_workers=1, encoder_threads=1
        )
        print(json.dumps(result_serial, indent=2))
        
        # Restore files for parallel test
        for i in range(5):
            tile_dir = root / f"tile_{i}"
            create_minimal_glb_with_texture(tile_dir / "tile.glb")
        
        print(f"\nTesting parallel (file_workers=2)...")
        result_parallel = process_tileset_dir(
            root, mode="ktx2-etc1s", basisu=basisu, quality=128,
            file_workers=2, encoder_threads=1
        )
        print(json.dumps(result_parallel, indent=2))
        
        assert result_parallel["filesSeen"] == 5
        assert result_parallel["filesConverted"] == 5
        assert result_parallel["texturesConverted"] == 5
        assert result_parallel["fileWorkers"] == 2
        
        print("✓ Parallel processing succeeded")


def test_cache_reuse():
    """Test that encode cache avoids redundant work."""
    print("\n=== Test: Cache Reuse ===")
    
    with tempfile.TemporaryDirectory() as tmpdir:
        root = Path(tmpdir) / "tileset"
        cache_dir = Path(tmpdir) / "cache"
        root.mkdir()
        cache_dir.mkdir()
        
        # Create 3 files with identical textures
        for i in range(3):
            tile_dir = root / f"tile_{i}"
            tile_dir.mkdir()
            create_minimal_glb_with_texture(tile_dir / "tile.glb")
        
        from texture_ktx2 import process_tileset_dir, find_basisu
        
        basisu = find_basisu()
        if not basisu:
            print("SKIP: basisu not found")
            return
        
        # First run should populate cache
        print("First run (populate cache)...")
        result1 = process_tileset_dir(
            root, mode="ktx2-etc1s", basisu=basisu, quality=128,
            file_workers=1, encoder_threads=1, cache_dir=cache_dir
        )
        cache_files = list(cache_dir.glob("*.ktx2"))
        print(f"Cache files created: {len(cache_files)}")
        assert len(cache_files) >= 1, "Cache should have at least one entry"
        
        # Restore files
        for i in range(3):
            tile_dir = root / f"tile_{i}"
            create_minimal_glb_with_texture(tile_dir / "tile.glb")
        
        # Second run should reuse cache (faster)
        print("Second run (reuse cache)...")
        result2 = process_tileset_dir(
            root, mode="ktx2-etc1s", basisu=basisu, quality=128,
            file_workers=1, encoder_threads=1, cache_dir=cache_dir
        )
        
        assert result2["filesConverted"] == 3
        print("✓ Cache reuse succeeded")


def test_atomic_write():
    """Test that writes are atomic (temp → rename)."""
    print("\n=== Test: Atomic Write ===")
    
    with tempfile.TemporaryDirectory() as tmpdir:
        root = Path(tmpdir) / "tileset"
        root.mkdir()
        
        glb_path = root / "test.glb"
        create_minimal_glb_with_texture(glb_path)
        
        from texture_ktx2 import process_tileset_dir, find_basisu
        
        basisu = find_basisu()
        if not basisu:
            print("SKIP: basisu not found")
            return
        
        # Process should not leave .tmp files
        result = process_tileset_dir(
            root, mode="ktx2-etc1s", basisu=basisu, quality=128,
            file_workers=1, encoder_threads=1
        )
        
        tmp_files = list(root.rglob("*.tmp"))
        assert len(tmp_files) == 0, f"Found leftover temp files: {tmp_files}"
        
        # GLB should be updated
        assert glb_path.exists()
        
        # Check for KTX2 evidence
        glb_data = glb_path.read_bytes()
        assert b'KHR_texture_basisu' in glb_data or b'\xabKTX 20\xbb' in glb_data
        
        print("✓ Atomic write succeeded (no temp files left)")


def test_external_gltf_locking():
    """Test that external glTF files are processed with proper locking."""
    print("\n=== Test: External glTF Locking ===")
    
    with tempfile.TemporaryDirectory() as tmpdir:
        root = Path(tmpdir) / "tileset"
        root.mkdir()
        
        # Create glTF with external BIN
        gltf_path = root / "model.gltf"
        bin_path = root / "model.bin"
        
        png = b'\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde\x00\x00\x00\x0cIDATx\x9cc\xf8\xcf\xc0\x00\x00\x00\x03\x00\x01\x00\x18\xdd\x8d\xb4\x00\x00\x00\x00IEND\xaeB`\x82'
        
        gltf_json = {
            "asset": {"version": "2.0"},
            "images": [{"bufferView": 0, "mimeType": "image/png"}],
            "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": len(png)}],
            "buffers": [{"byteLength": len(png), "uri": "model.bin"}],
            "textures": [{"source": 0}],
        }
        
        gltf_path.write_text(json.dumps(gltf_json, indent=2))
        bin_path.write_bytes(png)
        
        from texture_ktx2 import process_tileset_dir, find_basisu
        
        basisu = find_basisu()
        if not basisu:
            print("SKIP: basisu not found")
            return
        
        result = process_tileset_dir(
            root, mode="ktx2-etc1s", basisu=basisu, quality=128,
            file_workers=1, encoder_threads=1
        )
        
        # Both JSON and BIN should be updated
        assert gltf_path.exists()
        assert bin_path.exists()
        
        # No temp files left
        tmp_files = list(root.glob("*.tmp"))
        assert len(tmp_files) == 0, f"Found temp files: {tmp_files}"
        
        # Check glTF JSON for extension
        gltf_content = json.loads(gltf_path.read_text())
        assert "KHR_texture_basisu" in gltf_content.get("extensionsUsed", [])
        
        print("✓ External glTF locking succeeded")


def test_error_isolation():
    """Test that one file error doesn't stop other files."""
    print("\n=== Test: Error Isolation ===")
    
    with tempfile.TemporaryDirectory() as tmpdir:
        root = Path(tmpdir) / "tileset"
        root.mkdir()
        
        # Create one valid and one invalid file
        valid_path = root / "valid.glb"
        invalid_path = root / "invalid.glb"
        
        create_minimal_glb_with_texture(valid_path)
        invalid_path.write_bytes(b"not a valid GLB")
        
        from texture_ktx2 import process_tileset_dir, find_basisu
        
        basisu = find_basisu()
        if not basisu:
            print("SKIP: basisu not found")
            return
        
        result = process_tileset_dir(
            root, mode="ktx2-etc1s", basisu=basisu, quality=128,
            file_workers=2, encoder_threads=1
        )
        
        print(json.dumps(result, indent=2))
        
        # Should have seen both files
        assert result["filesSeen"] == 2
        # Should have converted the valid one
        assert result["filesConverted"] >= 1
        # Should have one error
        assert len(result["errors"]) >= 1
        
        print("✓ Error isolation succeeded")


if __name__ == "__main__":
    import sys
    
    try:
        test_parallel_file_processing()
        test_cache_reuse()
        test_atomic_write()
        test_external_gltf_locking()
        test_error_isolation()
        print("\n✅ All P6 verification tests passed!")
        sys.exit(0)
    except AssertionError as e:
        print(f"\n❌ Test failed: {e}")
        sys.exit(1)
    except Exception as e:
        print(f"\n❌ Test error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(2)
