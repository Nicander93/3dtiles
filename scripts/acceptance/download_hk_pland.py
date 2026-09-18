#!/usr/bin/env python3
"""
Download Hong Kong PlanD 3D Photo-realistic Model tiles (plan §9).

- Inventory from official GridIdx.geojson (NOT hard-coded grids)
- Contiguous region selection by MIN_X/MIN_Y/MAX_X/MAX_Y adjacency
- Downloads stay under GEOFORGE_ACCEPTANCE_ROOT / /workspace/data/acceptance
- Never commits datasets to git

Primary ZIP host (2026): pdmap.pland.gov.hk (ppgis.pland.gov.hk often NXDOMAIN).
"""
from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from acceptance_paths import acceptance_root  # noqa: E402

GRIDIDX_URL = (
    "https://www.pland.gov.hk/pland_en/info_serv/3D_models/Metadata/GridIdx.geojson"
)
REMARKS_URL = (
    "https://www.pland.gov.hk/pland_en/info_serv/3D_models/"
    "Remarks_for_the_3D_Photo-realistic_Model.pdf"
)
DOWNLOAD_PAGE = "https://www.pland.gov.hk/pland_en/info_serv/3D_models/download.htm"
PDMAP_ZIP = (
    "https://pdmap.pland.gov.hk/PLANDWEB/public/3d_photo_realistic_models/"
    "{fmt}/{grid}_{FMT}.zip"
)
PPGIS_ZIP = "https://ppgis.pland.gov.hk/3dpds/OpenData/{FMT}/{grid}_{FMT}.zip"
KLN_METADATA_URL = (
    "https://www.pland.gov.hk/pland_en/info_serv/3D_models/Metadata/KLN_metadata.zip"
)
UA = (
    "GeoForgeAcceptance/1.0 (+https://github.com/Nicander93/3dtiles; "
    "non-commercial internal verification)"
)


@dataclass
class Grid:
    name: str
    min_x: float
    min_y: float
    max_x: float
    max_y: float
    min_lng: float
    min_lat: float
    max_lng: float
    max_lat: float
    loc_en: str
    props: dict

    def region(self) -> str:
        if self.name.startswith("Tile_"):
            return "kowloon"
        if self.name.startswith("tile_"):
            return "hk_island"
        return "unknown"


def curl_head(url: str, timeout: int = 30) -> tuple[int | None, int | None]:
    cmd = [
        "curl", "-4", "-sI", "--max-time", str(timeout),
        "-A", UA, url,
    ]
    try:
        out = subprocess.check_output(cmd, text=True, stderr=subprocess.DEVNULL)
    except subprocess.CalledProcessError:
        return None, None
    codes = re.findall(r"HTTP/\S+\s+(\d+)", out)
    code = int(codes[-1]) if codes else None
    m = re.search(r"Content-Length:\s*(\d+)", out, re.I)
    cl = int(m.group(1)) if m else None
    return code, cl


def curl_download(url: str, dest: Path, timeout: int = 0) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    tmp = dest.with_suffix(dest.suffix + ".partial")
    cmd = [
        "curl", "-4", "-fL", "-C", "-",
        "-A", UA,
        "-o", str(tmp),
        url,
    ]
    if timeout > 0:
        cmd[1:1] = []  # keep -4
        cmd.extend(["--max-time", str(timeout)])
    print(f"[download] {url} -> {dest}")
    subprocess.check_call(cmd)
    tmp.rename(dest)


def zip_urls(grid: str, fmt: str) -> list[str]:
    fmt_u = fmt.upper()
    fmt_l = fmt.lower()
    # Encode '+' for safety; pdmap also accepts literal '+'
    enc = grid.replace("+", "%2B")
    urls = [
        PDMAP_ZIP.format(fmt=fmt_l, grid=enc, FMT=fmt_u),
        PDMAP_ZIP.format(fmt=fmt_l, grid=grid, FMT=fmt_u),
        PPGIS_ZIP.format(FMT=fmt_u, grid=enc),
        PPGIS_ZIP.format(FMT=fmt_u, grid=grid),
    ]
    # dedupe preserve order
    seen = set()
    out = []
    for u in urls:
        if u not in seen:
            seen.add(u)
            out.append(u)
    return out


def resolve_zip(grid: str, fmt: str) -> tuple[str, int]:
    last_err = None
    for url in zip_urls(grid, fmt):
        code, cl = curl_head(url)
        if code == 200 and cl and cl > 1000:
            return url, cl
        last_err = (url, code, cl)
    raise RuntimeError(f"No downloadable ZIP for {grid} {fmt}: last={last_err}")


def fetch_grididx(meta_dir: Path, refresh: bool = False) -> Path:
    dest = meta_dir / "GridIdx.geojson"
    if dest.is_file() and not refresh and dest.stat().st_size > 1000:
        return dest
    meta_dir.mkdir(parents=True, exist_ok=True)
    curl_download(GRIDIDX_URL, dest)
    # provenance
    (meta_dir / "source.json").write_text(
        json.dumps(
            {
                "downloadPage": DOWNLOAD_PAGE,
                "gridIdxUrl": GRIDIDX_URL,
                "remarksUrl": REMARKS_URL,
                "zipPattern": PDMAP_ZIP,
                "accessedAt": datetime.now(timezone.utc).isoformat(),
                "copyright": (
                    "Hong Kong Planning Department 3D Photo-realistic Model — "
                    "for reference / non-commercial internal verification only; "
                    "do not redistribute ZIPs; see official disclaimer on download page."
                ),
            },
            indent=2,
        )
        + "\n"
    )
    return dest


def load_grids(grididx: Path) -> list[Grid]:
    data = json.loads(grididx.read_text())
    grids: list[Grid] = []
    for feat in data["features"]:
        p = feat["properties"]
        grids.append(
            Grid(
                name=p["GRID_NAME"],
                min_x=float(p["MIN_X"]),
                min_y=float(p["MIN_Y"]),
                max_x=float(p["MAX_X"]),
                max_y=float(p["MAX_Y"]),
                min_lng=float(p["MIN_LNG"]),
                min_lat=float(p["MIN_LAT"]),
                max_lng=float(p["MAX_LNG"]),
                max_lat=float(p["MAX_LAT"]),
                loc_en=p.get("LOC_EN") or "",
                props=p,
            )
        )
    return grids


def select_contiguous(
    grids: list[Grid],
    *,
    width: int,
    height: int,
    region: str | None = None,
    prefer_small: bool = True,
) -> list[Grid]:
    """Select a contiguous WxH rectangle using quantized MIN_X/MIN_Y adjacency."""
    filtered = grids
    if region == "kowloon":
        filtered = [g for g in grids if g.region() == "kowloon"]
    elif region == "hk_island":
        filtered = [g for g in grids if g.region() == "hk_island"]
    if not filtered:
        raise RuntimeError(f"No grids for region={region}")

    widths = sorted(g.max_x - g.min_x for g in filtered)
    heights = sorted(g.max_y - g.min_y for g in filtered)
    cell_w = widths[len(widths) // 2]
    cell_h = heights[len(heights) // 2]
    minx = min(g.min_x for g in filtered)
    miny = min(g.min_y for g in filtered)

    cell: dict[tuple[int, int], Grid] = {}
    for g in filtered:
        ix = int(round((g.min_x - minx) / cell_w))
        iy = int(round((g.min_y - miny) / cell_h))
        cell.setdefault((ix, iy), g)

    candidates: list[list[Grid]] = []
    ixs = sorted({k[0] for k in cell})
    iys = sorted({k[1] for k in cell})
    for ix0 in ixs:
        for iy0 in iys:
            sel: list[Grid] = []
            ok = True
            for i in range(width):
                for j in range(height):
                    g = cell.get((ix0 + i, iy0 + j))
                    if not g:
                        ok = False
                        break
                    sel.append(g)
                if not ok:
                    break
            if ok:
                candidates.append(sel)
    if not candidates:
        raise RuntimeError(f"No contiguous {width}x{height} found (region={region})")

    if prefer_small:
        def score(sel: list[Grid]) -> float:
            # Prefer Kowloon tiles near Tile_+000_+013 (historically smaller ZIPs on waterfront)
            # without hard-coding a single grid: minimize |mean tile index| + mean easting.
            idxs = []
            for g in sel:
                # Tile_+012_+034 or Tile_-001_+022
                parts = g.name.replace("Tile_", "").split("_")
                try:
                    idxs.append(abs(int(parts[0])) + abs(int(parts[1])))
                except Exception:
                    idxs.append(10_000)
            return (sum(idxs) / len(idxs), sum(g.min_x for g in sel) / len(sel))

        candidates.sort(key=score)
    return candidates[0]


def disk_guard(need_bytes: int, root: Path) -> None:
    free = shutil.disk_usage(root).free
    # plan §0.1 / §9.2: stop if need >30GB new and disk insufficient; also 2.5x rule
    need_gib = need_bytes / (1024**3)
    free_gib = free / (1024**3)
    print(f"[disk] need≈{need_gib:.2f} GiB free={free_gib:.2f} GiB at {root}")
    if need_bytes > 30 * (1024**3) and free < need_bytes * 2.5:
        raise SystemExit(
            f"BLOCKER (plan §0.1): estimated new download {need_gib:.1f} GiB > 30 GiB "
            f"and free space {free_gib:.1f} GiB < 2.5× need. Stop."
        )
    if free < need_bytes * 2.5:
        raise SystemExit(
            f"BLOCKER: free space {free_gib:.1f} GiB < 2.5 × estimated download "
            f"{need_gib:.1f} GiB (plan §9.2)."
        )
    if need_bytes > 20 * (1024**3):
        print(
            f"[warn] estimated download {need_gib:.1f} GiB exceeds 20 GiB soft budget; "
            "caller should shrink region."
        )


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--width", type=int, default=4)
    ap.add_argument("--height", type=int, default=4)
    ap.add_argument(
        "--region",
        choices=["auto", "kowloon", "hk_island"],
        default="auto",
        help="auto prefers Kowloon (smaller typical OSGB ZIPs) when a WxH exists",
    )
    ap.add_argument("--format", choices=["OSGB", "CESIUM", "OBJ"], default="OSGB")
    ap.add_argument("--refresh-index", action="store_true")
    ap.add_argument(
        "--max-bytes",
        type=int,
        default=20 * 1024**3,
        help="soft budget; shrink region if estimate exceeds (default 20GiB)",
    )
    ap.add_argument("--grids", nargs="*", help="optional explicit GRID_NAME list")
    ap.add_argument("--skip-download", action="store_true")
    ap.add_argument("--probe-only", action="store_true")
    args = ap.parse_args()

    root = acceptance_root()
    hk = root / "hk_pland"
    meta = hk / "meta"
    dl = hk / "downloaded" / args.format.lower()
    meta.mkdir(parents=True, exist_ok=True)
    dl.mkdir(parents=True, exist_ok=True)

    grididx = fetch_grididx(meta, refresh=args.refresh_index)
    # remarks + kln metadata (tiny)
    remarks = meta / "Remarks_for_the_3D_Photo-realistic_Model.pdf"
    if not remarks.is_file():
        try:
            curl_download(REMARKS_URL, remarks)
        except Exception as e:  # noqa: BLE001
            print(f"[warn] remarks download failed: {e}")
    kln_meta_zip = meta / "KLN_metadata.zip"
    if not kln_meta_zip.is_file():
        try:
            curl_download(KLN_METADATA_URL, kln_meta_zip)
        except Exception as e:  # noqa: BLE001
            print(f"[warn] KLN metadata download failed: {e}")

    all_grids = load_grids(grididx)
    by_name = {g.name: g for g in all_grids}

    if args.grids:
        selected = [by_name[n] for n in args.grids]
    else:
        region = None if args.region == "auto" else args.region
        if args.region == "auto":
            try:
                selected = select_contiguous(
                    all_grids, width=args.width, height=args.height, region="kowloon"
                )
                region = "kowloon"
            except RuntimeError:
                selected = select_contiguous(
                    all_grids, width=args.width, height=args.height, region="hk_island"
                )
                region = "hk_island"
        else:
            selected = select_contiguous(
                all_grids, width=args.width, height=args.height, region=region
            )

    print(f"[select] region={selected[0].region()} n={len(selected)}")
    for g in selected:
        print(f"  {g.name}  HK1980=({g.min_x:.0f},{g.min_y:.0f})-({g.max_x:.0f},{g.max_y:.0f})  {g.loc_en[:50]}")

    # probe sizes
    sizes: dict[str, dict] = {}
    total = 0
    for g in selected:
        url, cl = resolve_zip(g.name, args.format)
        sizes[g.name] = {"url": url, "bytes": cl}
        total += cl
        print(f"  size {g.name}: {cl/1024/1024:.1f} MiB")

    if total > args.max_bytes and not args.grids:
        print(
            f"[budget] estimate {total/1024**3:.2f} GiB > max {args.max_bytes/1024**3:.2f} GiB — "
            "shrink (caller may lower --width/--height)."
        )
        return 2

    disk_guard(total, root)

    selection = {
        "source": "Hong Kong Planning Department",
        "downloadPage": DOWNLOAD_PAGE,
        "accessedAt": datetime.now(timezone.utc).isoformat(),
        "format": args.format,
        "width": args.width,
        "height": args.height,
        "region": selected[0].region(),
        "grids": [
            {
                "name": g.name,
                "bounds_hk1980": [g.min_x, g.min_y, g.max_x, g.max_y],
                "bounds_wgs84": [g.min_lng, g.min_lat, g.max_lng, g.max_lat],
                "loc_en": g.loc_en,
                "url": sizes[g.name]["url"],
                "bytes": sizes[g.name]["bytes"],
            }
            for g in selected
        ],
        "totalBytes": total,
        "copyrightNote": (
            "Internal non-commercial verification only. Do not commit ZIPs/OSGB/B3DM to git "
            "or re-publish. Acceptance reports may keep stats/hashes/grid names/screenshots."
        ),
    }
    sel_path = hk / f"selection_{args.format.lower()}_{args.width}x{args.height}.json"
    sel_path.write_text(json.dumps(selection, indent=2, ensure_ascii=False) + "\n")
    print(f"[wrote] {sel_path}")

    if args.probe_only or args.skip_download:
        return 0

    for g in selected:
        dest = dl / f"{g.name}_{args.format}.zip"
        if dest.is_file() and dest.stat().st_size == sizes[g.name]["bytes"]:
            print(f"[skip] {dest.name} already complete")
            continue
        curl_download(sizes[g.name]["url"], dest)

    print(f"[done] downloaded under {dl}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
