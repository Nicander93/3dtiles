#!/usr/bin/env python3
"""Generate synthetic NxN continuous Tile_+x_+y tileset for TopRebuild scale tests.

Usage:
  python3 scripts/generate_grid_fixture.py --n 16 --out tests/fixtures/top_rebuild/grid_16x16
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path


def box(cx: float, cy: float, hx: float, hy: float, hz: float = 10.0) -> list[float]:
    return [cx, cy, hz, hx, 0, 0, 0, hy, 0, 0, 0, hz]


def generate(n: int, out: Path, tile: float = 100.0) -> None:
    half = tile / 2.0
    data = out / "Data"
    data.mkdir(parents=True, exist_ok=True)
    children = []
    for y in range(n):
        for x in range(n):
            name = f"Tile_+{x:03}_+{y:03}"
            tdir = data / name
            tdir.mkdir(parents=True, exist_ok=True)
            (tdir / f"{name}.b3dm").write_bytes(b"")
            (tdir / f"{name}_L1.b3dm").write_bytes(b"")
            cx = half + x * tile
            cy = half + y * tile
            leaf = {
                "asset": {"version": "1.0", "gltfUpAxis": "Z"},
                "geometricError": 50.0,
                "root": {
                    "boundingVolume": {"box": box(cx, cy, half, half)},
                    "geometricError": 50.0,
                    "refine": "REPLACE",
                    "content": {"uri": f"./{name}.b3dm"},
                    "children": [
                        {
                            "boundingVolume": {"box": box(cx, cy, half, half)},
                            "geometricError": 10.0,
                            "content": {"uri": f"./{name}_L1.b3dm"},
                        }
                    ],
                },
            }
            (tdir / "tileset.json").write_text(json.dumps(leaf, indent=2) + "\n")
            children.append(
                {
                    "boundingVolume": {"box": box(cx, cy, half, half)},
                    "geometricError": 100.0,
                    "content": {"uri": f"./Data/{name}/tileset.json"},
                }
            )
    root_hx = n * tile / 2.0
    root = {
        "asset": {"version": "1.0", "gltfUpAxis": "Z"},
        "geometricError": float(n * tile),
        "root": {
            "boundingVolume": {"box": box(root_hx, root_hx, root_hx, root_hx)},
            "geometricError": float(n * tile),
            "refine": "ADD",
            "children": children,
        },
    }
    (out / "tileset.json").write_text(json.dumps(root, indent=2) + "\n")
    levels = []
    c = n * n
    lvl = 0
    while c >= 1:
        levels.append(f"L{lvl}={c}")
        if c == 1:
            break
        c //= 4
        lvl += 1
    (out / "README.md").write_text(
        f"""# grid_{n}x{n} fixture

Synthetic {n}×{n} continuous `Tile_+x_+y` tileset for TopRebuild scale ladder.
{n * n} leaf blocks, each with coarse + fine Representation stubs (empty .b3dm).
Expected quadtree: {' → '.join(levels)}.
Cell size {tile:g} m.
Regenerate: `python3 scripts/generate_grid_fixture.py --n {n} --out {out.as_posix()}`
"""
    )
    print(f"wrote {out} leaves={n * n} expected={' / '.join(levels)}")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=16)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--tile-meters", type=float, default=100.0)
    args = ap.parse_args()
    if args.n < 1 or (args.n & (args.n - 1)) != 0:
        raise SystemExit("--n must be a power of 2 >= 1")
    generate(args.n, args.out, args.tile_meters)


if __name__ == "__main__":
    main()
