"""Compatibility shim — implementation is tools/texture_ktx2/texture_ktx2.py."""
from __future__ import annotations

import sys
from pathlib import Path

_PKG = Path(__file__).resolve().parents[3] / "texture_ktx2"
sys.path.insert(0, str(_PKG))
from texture_ktx2 import *  # noqa: E402,F401,F403
from texture_ktx2 import (  # noqa: E402
    copy_and_process,
    find_basisu,
    process_tileset_dir,
)

if __name__ == "__main__":
    import argparse
    import json
    from pathlib import Path as P

    ap = argparse.ArgumentParser(description="Post-process 3D Tiles textures to KTX2 (basisu)")
    ap.add_argument("-i", "--input", required=True)
    ap.add_argument("-o", "--output")
    ap.add_argument("--mode", default="ktx2-etc1s", choices=["ktx2", "ktx2-etc1s", "ktx2-uastc"])
    ap.add_argument("-q", "--quality", type=int, default=128)
    args = ap.parse_args()

    def _print(m: str) -> None:
        print(m, flush=True)

    if args.output:
        st = copy_and_process(P(args.input), P(args.output), mode=args.mode, quality=args.quality, log=_print)
    else:
        st = process_tileset_dir(P(args.input), mode=args.mode, quality=args.quality, log=_print)
    print(json.dumps(st, indent=2))
    raise SystemExit(0 if st["texturesConverted"] > 0 or st["filesSeen"] == 0 else 1)
