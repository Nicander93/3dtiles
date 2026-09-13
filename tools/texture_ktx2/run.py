#!/usr/bin/env python3
"""Product CLI for KTX2 post-process (tools/texture_ktx2)."""
from __future__ import annotations

import json
import sys
from pathlib import Path

# Prefer local package module; fall back to experiments copy once.
_HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(_HERE))

from texture_ktx2 import (  # noqa: E402
    copy_and_process,
    find_basisu,
    process_tileset_dir,
)


def main() -> int:
    import argparse

    ap = argparse.ArgumentParser(description="GeoForge texture KTX2 post-process")
    ap.add_argument("-i", "--input", required=True, help="input tileset directory")
    ap.add_argument("-o", "--output", help="output directory (copy then process); default=in-place")
    ap.add_argument("--mode", default="ktx2-etc1s", choices=["ktx2", "ktx2-etc1s", "ktx2-uastc"])
    ap.add_argument("-q", "--quality", type=int, default=128)
    ap.add_argument("--basisu", help="absolute path to basisu")
    ap.add_argument("--report", help="write JSON report to this path")
    args = ap.parse_args()

    def _print(m: str) -> None:
        print(m, flush=True)

    basisu = Path(args.basisu) if args.basisu else find_basisu()
    if not basisu or not Path(basisu).is_file():
        print("basisu not found (pass --basisu or set GEOFORGE_BASISU)", file=sys.stderr)
        return 2

    if args.output:
        st = copy_and_process(
            Path(args.input),
            Path(args.output),
            mode=args.mode,
            quality=args.quality,
            log=_print,
        )
    else:
        st = process_tileset_dir(
            Path(args.input),
            mode=args.mode,
            basisu=Path(basisu),
            quality=args.quality,
            log=_print,
        )

    # Fail if any per-file errors
    if st.get("errors"):
        print(json.dumps(st, indent=2))
        if args.report:
            Path(args.report).write_text(json.dumps(st, indent=2), encoding="utf-8")
        return 1

    print(json.dumps(st, indent=2))
    if args.report:
        Path(args.report).write_text(json.dumps(st, indent=2), encoding="utf-8")

    if st["texturesConverted"] > 0 or st["filesSeen"] == 0:
        return 0
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
