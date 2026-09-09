#!/usr/bin/env python3
"""Post-process top-level rebuild for 3D Tiles (v0).

Groups root external tilesets by Tile_+row_+col 2x2 grid, merges each group's
root .b3dm GLB payloads into a coarser parent tile, and rewrites tileset.json
with refine=REPLACE parents.
"""
from __future__ import annotations

import argparse
import json
import math
import re
import shutil
import struct
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

import numpy as np

try:
    import trimesh
    from PIL import Image
except ImportError as e:
    print("Missing dependency:", e, file=sys.stderr)
    print("Install with: python -m pip install numpy pillow trimesh pygltflib", file=sys.stderr)
    sys.exit(1)

TILE_RE = re.compile(r"Tile_\+?(-?\d+)_\+?(-?\d+)", re.I)


@dataclass
class RootChild:
    name: str
    row: int
    col: int
    node: Dict[str, Any]
    tileset_rel: str  # relative uri to external tileset.json
    tile_dir: Path
    root_b3dm: Path
    geometric_error: float
    bounding_volume: Dict[str, Any]


def parse_args(argv: Optional[List[str]] = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(prog="rebuild-top", description="3D Tiles top-level rebuild (post-process)")
    p.add_argument("-i", "--input", required=True, help="Input tileset directory")
    p.add_argument("-o", "--output", required=True, help="Output directory (new)")
    p.add_argument("--levels", type=int, default=1, help="Pyramid merge levels (default 1)")
    p.add_argument("--simplify", type=float, default=0.5, help="Mesh keep-ratio (0,1]")
    p.add_argument("--texture-scale", type=float, default=0.5, help="Texture scale (0,1]")
    p.add_argument("-v", "--verbose", action="store_true")
    return p.parse_args(argv)


def read_json(path: Path) -> Dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, data: Dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")


def normalize_uri(uri: str) -> str:
    u = uri.replace("\\", "/")
    while u.startswith("./"):
        u = u[2:]
    while u.startswith(".//"):
        u = u[3:]
    while u.startswith("/"):
        u = u[1:]
    return u


def parse_b3dm(path: Path) -> Tuple[bytes, bytes]:
    data = path.read_bytes()
    if data[:4] != b"b3dm":
        raise ValueError(f"not b3dm: {path}")
    ftj, ftb, btj, btb = struct.unpack_from("<IIII", data, 12)
    offset = 28 + ftj + ftb + btj + btb
    header = data[:offset]
    glb = data[offset:]
    if glb[:4] not in (b"glTF", b"gltf"):
        # try align
        for pad in range(0, 4):
            if data[offset + pad : offset + pad + 4] == b"glTF":
                header = data[: offset + pad]
                glb = data[offset + pad :]
                break
        else:
            raise ValueError(f"glb not found in {path}")
    return header, glb


def write_b3dm(path: Path, glb: bytes) -> None:
    # minimal feature/batch tables matching fanvanzh style
    ftj = b'{"BATCH_LENGTH":1}  '
    btj = b'{"batchId":{"byteOffset":0}} '
    # pad tables to 4-byte
    def pad4(b: bytes) -> bytes:
        return b + (b" " * ((4 - (len(b) % 4)) % 4))

    ftj = pad4(ftj)
    btj = pad4(btj)
    header = bytearray(28)
    header[0:4] = b"b3dm"
    struct.pack_into("<I", header, 4, 1)
    body = bytes(header) + ftj + btj + glb
    # fix lengths
    out = bytearray(body)
    struct.pack_into("<I", out, 8, len(out))
    struct.pack_into("<I", out, 12, len(ftj))
    struct.pack_into("<I", out, 16, 0)
    struct.pack_into("<I", out, 20, len(btj))
    struct.pack_into("<I", out, 24, 0)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(bytes(out))


def box_union(boxes: List[List[float]]) -> List[float]:
    """Union axis-aligned boxes stored as Cesium box: center(3)+halfAxes(9).
    We only handle diagonal AABB half-axes (common in this converter output).
    """
    mins = []
    maxs = []
    for b in boxes:
        cx, cy, cz = b[0], b[1], b[2]
        hx, hy, hz = abs(b[3]), abs(b[7]), abs(b[11])
        mins.append((cx - hx, cy - hy, cz - hz))
        maxs.append((cx + hx, cy + hy, cz + hz))
    min_x = min(v[0] for v in mins)
    min_y = min(v[1] for v in mins)
    min_z = min(v[2] for v in mins)
    max_x = max(v[0] for v in maxs)
    max_y = max(v[1] for v in maxs)
    max_z = max(v[2] for v in maxs)
    cx = (min_x + max_x) / 2
    cy = (min_y + max_y) / 2
    cz = (min_z + max_z) / 2
    hx = (max_x - min_x) / 2
    hy = (max_y - min_y) / 2
    hz = (max_z - min_z) / 2
    return [cx, cy, cz, hx, 0, 0, 0, hy, 0, 0, 0, hz]


def collect_root_children(input_dir: Path, root: Dict[str, Any], verbose: bool = False) -> List[RootChild]:
    children = []
    for node in root.get("children") or []:
        content = (node.get("content") or {}).get("uri")
        if not content:
            continue
        rel = normalize_uri(content)
        ts_path = input_dir / rel
        if not ts_path.exists():
            if verbose:
                print(f"skip missing tileset {ts_path}", file=sys.stderr)
            continue
        m = TILE_RE.search(rel)
        if not m:
            if verbose:
                print(f"skip non-tile uri {rel}", file=sys.stderr)
            continue
        row, col = int(m.group(1)), int(m.group(2))
        tile_dir = ts_path.parent
        name = tile_dir.name
        # root b3dm usually Tile_xxx.b3dm
        cand = tile_dir / f"{name}.b3dm"
        if not cand.exists():
            # fallback: first b3dm without _L
            b3s = sorted(tile_dir.glob("*.b3dm"))
            b3s = [p for p in b3s if "_L" not in p.name] or b3s
            if not b3s:
                continue
            cand = b3s[0]
        ge = float(node.get("geometricError") or root.get("geometricError") or 1000)
        bv = node.get("boundingVolume") or {}
        children.append(
            RootChild(
                name=name,
                row=row,
                col=col,
                node=node,
                tileset_rel=rel,
                tile_dir=tile_dir,
                root_b3dm=cand,
                geometric_error=ge,
                bounding_volume=bv,
            )
        )
    return children


def group_2x2(children: List[RootChild], cell: int = 2) -> Dict[Tuple[int, int], List[RootChild]]:
    groups: Dict[Tuple[int, int], List[RootChild]] = {}
    for ch in children:
        key = (ch.row // cell, ch.col // cell)
        groups.setdefault(key, []).append(ch)
    return groups


def simplify_mesh(mesh: trimesh.Trimesh, keep_ratio: float) -> trimesh.Trimesh:
    keep_ratio = min(max(keep_ratio, 0.05), 1.0)
    target = max(int(len(mesh.faces) * keep_ratio), 4)
    if len(mesh.faces) <= target or keep_ratio >= 0.999:
        return mesh
    try:
        return mesh.simplify_quadric_decimation(face_count=target)
    except Exception:
        try:
            return mesh.simplify_quadratic_decimation(target)
        except Exception:
            return mesh


def downsample_scene_textures(scene: trimesh.Scene, scale: float) -> None:
    scale = min(max(scale, 0.05), 1.0)
    if scale >= 0.999:
        return
    # trimesh Geometry may carry visual
    for geom in scene.geometry.values():
        vis = getattr(geom, "visual", None)
        if vis is None:
            continue
        mat = getattr(vis, "material", None)
        img = getattr(mat, "image", None) if mat is not None else None
        if img is None:
            img = getattr(vis, "image", None)
        if img is None:
            continue
        try:
            w, h = img.size
            nw, nh = max(1, int(w * scale)), max(1, int(h * scale))
            if (nw, nh) != (w, h):
                img2 = img.resize((nw, nh), Image.BILINEAR)
                if mat is not None and hasattr(mat, "image"):
                    mat.image = img2
                elif hasattr(vis, "image"):
                    vis.image = img2
        except Exception:
            pass


def merge_b3dms(paths: List[Path], simplify: float, texture_scale: float, verbose: bool = False) -> bytes:
    meshes = []
    for p in paths:
        _, glb = parse_b3dm(p)
        # trimesh load from bytes
        loaded = trimesh.load(file_obj=trimesh.util.wrap_as_stream(glb), file_type="glb", force="scene")
        if isinstance(loaded, trimesh.Scene):
            for g in loaded.geometry.values():
                if isinstance(g, trimesh.Trimesh) and len(g.faces):
                    meshes.append(g.copy())
        elif isinstance(loaded, trimesh.Trimesh):
            meshes.append(loaded.copy())
    if not meshes:
        raise ValueError("no meshes to merge")
    # apply simplify per mesh then concatenate
    simplified = [simplify_mesh(m, simplify) for m in meshes]
    merged = trimesh.util.concatenate(simplified)
    if not isinstance(merged, trimesh.Trimesh):
        merged = simplified[0]
    scene = trimesh.Scene(merged)
    downsample_scene_textures(scene, texture_scale)
    glb_out = scene.export(file_type="glb")
    if isinstance(glb_out, str):
        glb_out = glb_out.encode("utf-8")
    if verbose:
        print(f"  merged {len(paths)} b3dm -> glb {len(glb_out)} bytes, faces~{len(merged.faces)}")
    return glb_out


def rebuild(input_dir: Path, output_dir: Path, levels: int, simplify: float, texture_scale: float, verbose: bool) -> None:
    input_dir = input_dir.resolve()
    output_dir = output_dir.resolve()
    if output_dir.exists():
        shutil.rmtree(output_dir)
    shutil.copytree(input_dir, output_dir)

    tileset_path = output_dir / "tileset.json"
    tileset = read_json(tileset_path)
    root = tileset["root"]
    children = collect_root_children(output_dir, root, verbose=verbose)
    if verbose:
        print(f"root children: {len(children)}")
    if not children:
        raise SystemExit("no Tile_* root children found")

    cell = 2
    working = children
    for level in range(max(levels, 1)):
        groups = group_2x2(working, cell=cell)
        if verbose:
            print(f"level {level+1}: {len(working)} tiles -> {len(groups)} groups")
        new_children_nodes: List[Dict[str, Any]] = []
        new_working: List[RootChild] = []
        for (gr, gc), members in sorted(groups.items()):
            members = sorted(members, key=lambda m: (m.row, m.col))
            if len(members) == 1:
                # pass-through
                new_children_nodes.append(members[0].node)
                new_working.append(members[0])
                continue
            merge_name = f"Merge_L{level+1}_{gr}_{gc}"
            merge_dir = output_dir / "Data" / merge_name
            merge_dir.mkdir(parents=True, exist_ok=True)
            glb = merge_b3dms([m.root_b3dm for m in members], simplify, texture_scale, verbose=verbose)
            parent_b3dm = merge_dir / f"{merge_name}.b3dm"
            write_b3dm(parent_b3dm, glb)
            boxes = []
            for m in members:
                box = (m.bounding_volume or {}).get("box")
                if box and len(box) >= 12:
                    boxes.append(box)
            bv = {"box": box_union(boxes)} if boxes else (members[0].bounding_volume or {})
            parent_ge = max(m.geometric_error for m in members) * 2.0
            parent_node = {
                "boundingVolume": bv,
                "geometricError": parent_ge,
                "refine": "REPLACE",
                "content": {"uri": f"./Data/{merge_name}/{merge_name}.b3dm"},
                "children": [m.node for m in members],
            }
            new_children_nodes.append(parent_node)
            # For multi-level, treat parent as a synthetic child for next pass
            new_working.append(
                RootChild(
                    name=merge_name,
                    row=gr,
                    col=gc,
                    node=parent_node,
                    tileset_rel=f"Data/{merge_name}/{merge_name}.b3dm",
                    tile_dir=merge_dir,
                    root_b3dm=parent_b3dm,
                    geometric_error=parent_ge,
                    bounding_volume=bv,
                )
            )
        root["children"] = new_children_nodes
        root["geometricError"] = float(root.get("geometricError") or 2000) * 2
        working = new_working
        # next level uses coarser cell on already-merged indices (gr,gc)
        # members now keyed by previous group coords; keep cell=2

    write_json(tileset_path, tileset)
    if verbose:
        print(f"wrote {tileset_path}")
        print(f"root children now: {len(root.get('children') or [])}")


def main(argv: Optional[List[str]] = None) -> int:
    args = parse_args(argv)
    if not (0 < args.simplify <= 1):
        print("--simplify must be in (0,1]", file=sys.stderr)
        return 2
    if not (0 < args.texture_scale <= 1):
        print("--texture-scale must be in (0,1]", file=sys.stderr)
        return 2
    if args.levels < 1:
        print("--levels must be >= 1", file=sys.stderr)
        return 2
    try:
        rebuild(
            Path(args.input),
            Path(args.output),
            levels=args.levels,
            simplify=args.simplify,
            texture_scale=args.texture_scale,
            verbose=args.verbose,
        )
    except Exception as e:
        print(f"rebuild-top failed: {e}", file=sys.stderr)
        if args.verbose:
            raise
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
