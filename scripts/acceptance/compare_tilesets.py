#!/usr/bin/env python3
"""Compare baseline vs official Cesium tileset coverage (plan §11.3) — not byte-identical."""
from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path


def load_tileset(path: Path) -> dict:
    return json.loads(path.read_text())


def walk_boxes(node: dict, boxes: list, region: list) -> None:
    bv = node.get("boundingVolume") or {}
    if "box" in bv:
        boxes.append(bv["box"])
    if "region" in bv:
        region.append(bv["region"])
    for ch in node.get("children") or []:
        walk_boxes(ch, boxes, region)


def box_center_world(box: list[float]) -> tuple[float, float, float]:
    # 3D Tiles box: [cx,cy,cz, xx,xy,xz, yx,yy,yz, zx,zy,zz] in meters (EPSG cartesian-ish)
    return box[0], box[1], box[2]


def region_center(region: list[float]) -> tuple[float, float, float]:
    # west, south, east, north, minHeight, maxHeight (radians / meters)
    lon = (region[0] + region[2]) / 2
    lat = (region[1] + region[3]) / 2
    h = (region[4] + region[5]) / 2
    return lon, lat, h


def haversine_m(lon1: float, lat1: float, lon2: float, lat2: float) -> float:
    r = 6371000.0
    p1, p2 = math.radians(lat1), math.radians(lat2)
    dphi = math.radians(lat2 - lat1)
    dl = math.radians(lon2 - lon1)
    a = math.sin(dphi / 2) ** 2 + math.cos(p1) * math.cos(p2) * math.sin(dl / 2) ** 2
    return 2 * r * math.asin(min(1.0, math.sqrt(a)))


def summarize(tileset_path: Path) -> dict:
    ts = load_tileset(tileset_path)
    root = ts.get("root") or ts
    boxes: list = []
    regions: list = []
    walk_boxes(root, boxes, regions)
    summary: dict = {
        "path": str(tileset_path),
        "boxCount": len(boxes),
        "regionCount": len(regions),
    }
    if regions:
        # aggregate outer region
        west = min(r[0] for r in regions)
        south = min(r[1] for r in regions)
        east = max(r[2] for r in regions)
        north = max(r[3] for r in regions)
        minh = min(r[4] for r in regions)
        maxh = max(r[5] for r in regions)
        summary["coverageRegionRad"] = [west, south, east, north, minh, maxh]
        summary["centerLonLatDeg"] = [
            math.degrees((west + east) / 2),
            math.degrees((south + north) / 2),
        ]
        summary["heightRangeM"] = [minh, maxh]
    if boxes:
        cxs = [b[0] for b in boxes]
        cys = [b[1] for b in boxes]
        czs = [b[2] for b in boxes]
        summary["boxCenterMean"] = [
            sum(cxs) / len(cxs),
            sum(cys) / len(cys),
            sum(czs) / len(czs),
        ]
    return summary


def compare(a: Path, b: Path, max_horizontal_m: float = 1.0) -> dict:
    sa, sb = summarize(a), summarize(b)
    result = {
        "a": sa,
        "b": sb,
        "horizontalCenterDeviationM": None,
        "heightNote": None,
        "pass": False,
        "gate": f"horizontal center deviation <= {max_horizontal_m} m (same CRS)",
    }
    if sa.get("centerLonLatDeg") and sb.get("centerLonLatDeg"):
        lon1, lat1 = sa["centerLonLatDeg"]
        lon2, lat2 = sb["centerLonLatDeg"]
        d = haversine_m(lon1, lat1, lon2, lat2)
        result["horizontalCenterDeviationM"] = d
        result["pass"] = d <= max_horizontal_m
        ha, hb = sa.get("heightRangeM"), sb.get("heightRangeM")
        if ha and hb:
            # flag large systematic shift
            mid_a = (ha[0] + ha[1]) / 2
            mid_b = (hb[0] + hb[1]) / 2
            result["heightMidDeltaM"] = mid_b - mid_a
            if abs(mid_b - mid_a) > 50:
                result["heightNote"] = (
                    f"Vertical midpoints differ by {mid_b-mid_a:.1f} m — "
                    "possible datum mismatch; report only, do not silently correct."
                )
    elif sa.get("boxCenterMean") and sb.get("boxCenterMean"):
        ax, ay, az = sa["boxCenterMean"]
        bx, by, bz = sb["boxCenterMean"]
        d = math.dist((ax, ay), (bx, by))
        result["horizontalCenterDeviationM"] = d
        result["pass"] = d <= max_horizontal_m
        result["heightMidDeltaM"] = bz - az
        result["note"] = "Compared box centers in cartesian meters (same CRS assumed)."
    else:
        result["error"] = "Insufficient boundingVolume overlap types to compare centers"
        result["pass"] = False
    return result


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--a", type=Path, required=True, help="baseline tileset.json")
    ap.add_argument("--b", type=Path, required=True, help="reference tileset.json")
    ap.add_argument("-o", "--output", type=Path, required=True)
    ap.add_argument("--max-horizontal-m", type=float, default=1.0)
    args = ap.parse_args()
    result = compare(args.a, args.b, max_horizontal_m=args.max_horizontal_m)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({k: result[k] for k in ("pass", "horizontalCenterDeviationM", "heightNote", "error") if k in result or True}, indent=2))
    return 0 if result.get("pass") else 1


if __name__ == "__main__":
    raise SystemExit(main())
