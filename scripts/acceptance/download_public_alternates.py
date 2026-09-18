#!/usr/bin/env python3
"""
Alternate public datasets (plan §21) when HK PlanD is blocked.

- CesiumGS 3d-tiles-samples (git clone shallow) — structure / preflight only
- Utrecht Integrated Mesh tileset.json URL recorded (do not bulk-mirror city)
- Strasbourg: record lookup notes; require live official entry before download
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from acceptance_paths import acceptance_root  # noqa: E402

CESIUM_SAMPLES = "https://github.com/CesiumGS/3d-tiles-samples.git"
UTRECHT_TILESET = (
    "https://tiles.arcgis.com/tiles/V6ZHFr6zdgNZuVG0/arcgis/rest/services/"
    "Utrecht_3D_Tiles_Integrated_Mesh/3DTilesServer/tileset.json"
)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--clone-samples", action="store_true")
    ap.add_argument("--probe-utrecht", action="store_true")
    args = ap.parse_args()
    root = acceptance_root() / "alternates"
    root.mkdir(parents=True, exist_ok=True)
    report = {
        "accessedAt": datetime.now(timezone.utc).isoformat(),
        "items": [],
    }
    if args.clone_samples:
        dest = root / "3d-tiles-samples"
        if not (dest / ".git").exists():
            subprocess.check_call(
                ["git", "clone", "--depth", "1", CESIUM_SAMPLES, str(dest)]
            )
        report["items"].append(
            {
                "name": "CesiumGS/3d-tiles-samples",
                "path": str(dest),
                "purpose": "parser/validator/preflight compatibility — not TopRebuild production gate",
            }
        )
    if args.probe_utrecht:
        # HEAD/GET tileset.json only — do not mirror the city
        import urllib.request

        req = urllib.request.Request(
            UTRECHT_TILESET,
            headers={"User-Agent": "GeoForgeAcceptance/1.0"},
        )
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                body = r.read(200000)
            dest = root / "utrecht_tileset.json"
            dest.write_bytes(body)
            report["items"].append(
                {
                    "name": "Utrecht Integrated Mesh",
                    "url": UTRECHT_TILESET,
                    "local": str(dest),
                    "note": "Remote tileset.json only; do not bulk download without license check.",
                }
            )
        except Exception as e:  # noqa: BLE001
            report["items"].append({"name": "Utrecht", "error": str(e), "url": UTRECHT_TILESET})
    report["strasbourg"] = {
        "status": "lookup_required",
        "note": (
            "Plan §21.3: find current official public entry + confirm test access "
            "before any download; do not redistribute."
        ),
    }
    out = root / "alternates_manifest.json"
    out.write_text(json.dumps(report, indent=2) + "\n")
    print(f"wrote {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
