#!/usr/bin/env python3
"""
Non-destructive staging of PlanD OSGB into Smart3D-like layout (plan §10).

HK Island ZIPs typically already contain metadata.xml + Data/.
Kowloon ZIPs are flat Tile_+xxx_+yyy/*.osgb — stage into Data/ + official KLN metadata.xml.

Never invents mesh/LOD; only directory rename / hardlink / copy + official SRS metadata.
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import zipfile
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from acceptance_paths import acceptance_root  # noqa: E402


def link_or_copy(src: Path, dst: Path) -> str:
    dst.parent.mkdir(parents=True, exist_ok=True)
    if dst.exists() or dst.is_symlink():
        dst.unlink()
    try:
        os.link(src, dst)
        return "hardlink"
    except OSError:
        try:
            os.symlink(src, dst)
            return "symlink"
        except OSError:
            shutil.copy2(src, dst)
            return "copy"


def extract_zip(zip_path: Path, dest_dir: Path) -> Path:
    dest_dir.mkdir(parents=True, exist_ok=True)
    # Skip if already extracted (marker)
    marker = dest_dir / ".extracted_from"
    if marker.is_file() and marker.read_text().strip() == str(zip_path.resolve()):
        return dest_dir
    with zipfile.ZipFile(zip_path) as zf:
        zf.extractall(dest_dir)
    marker.write_text(str(zip_path.resolve()) + "\n")
    return dest_dir


def find_tile_roots(extracted: Path) -> list[Path]:
    """Return directories that look like a single PlanD tile root."""
    # Case A: extracted/Tile_xxx/ with osgb files
    roots = []
    for child in sorted(extracted.iterdir()):
        if child.is_dir() and any(child.glob("*.osgb")):
            roots.append(child)
        elif child.is_dir() and (child / "Data").is_dir():
            roots.append(child)
        elif child.is_dir() and (child / "metadata.xml").is_file():
            roots.append(child)
    if not roots and any(extracted.glob("*.osgb")):
        roots.append(extracted)
    return roots


def ensure_kln_metadata(meta_dir: Path, staged_root: Path) -> Path:
    xml_src = meta_dir / "KLN_metadata.xml"
    if not xml_src.is_file():
        z = meta_dir / "KLN_metadata.zip"
        if z.is_file():
            with zipfile.ZipFile(z) as zf:
                zf.extract("metadata.xml", meta_dir)
            (meta_dir / "metadata.xml").replace(xml_src)
    if not xml_src.is_file():
        raise RuntimeError(
            "Missing official KLN_metadata.xml — download via download_hk_pland.py first"
        )
    dst = staged_root / "metadata.xml"
    shutil.copy2(xml_src, dst)
    return dst


def stage_selection(selection_path: Path, fmt: str = "OSGB") -> dict:
    root = acceptance_root()
    hk = root / "hk_pland"
    meta = hk / "meta"
    selection = json.loads(selection_path.read_text())
    grids = selection["grids"]
    region = selection.get("region") or "unknown"
    run_id = selection_path.stem.replace("selection_", "")
    downloaded = hk / "downloaded" / fmt.lower()
    extracted = hk / "extracted" / run_id
    staged = hk / "staged" / run_id
    staged.mkdir(parents=True, exist_ok=True)
    data_dir = staged / "Data"
    data_dir.mkdir(parents=True, exist_ok=True)

    mapping = []
    original_paths = []
    staged_paths = []
    total_bytes = 0

    for g in grids:
        name = g["name"]
        zip_path = downloaded / f"{name}_{fmt}.zip"
        if not zip_path.is_file():
            raise FileNotFoundError(f"Missing download: {zip_path}")
        total_bytes += zip_path.stat().st_size
        ex = extract_zip(zip_path, extracted / name)
        original_paths.append(str(zip_path))
        tile_roots = find_tile_roots(ex)
        if not tile_roots:
            raise RuntimeError(f"No tile content in {zip_path}")

        # Prefer directory matching grid name
        tile_root = None
        for tr in tile_roots:
            if tr.name == name or tr == ex:
                tile_root = tr
                break
        tile_root = tile_root or tile_roots[0]

        # HKI style: already has Data/ + metadata
        if (tile_root / "Data").is_dir() and (tile_root / "metadata.xml").is_file():
            # Merge Data/* into staged Data/, keep first metadata (or merge note)
            meta_dst = staged / "metadata.xml"
            if not meta_dst.exists():
                shutil.copy2(tile_root / "metadata.xml", meta_dst)
                mapping.append(
                    {
                        "grid": name,
                        "action": "copy_metadata",
                        "from": str(tile_root / "metadata.xml"),
                        "to": str(meta_dst),
                    }
                )
            for child in sorted((tile_root / "Data").iterdir()):
                if not child.is_dir():
                    continue
                dest = data_dir / child.name
                if dest.exists():
                    continue
                mode = "dir_hardlink_tree"
                # Prefer symlink of directory
                try:
                    os.symlink(child.resolve(), dest)
                    mode = "symlink_dir"
                except OSError:
                    shutil.copytree(child, dest, dirs_exist_ok=True)
                    mode = "copytree"
                mapping.append(
                    {
                        "grid": name,
                        "action": mode,
                        "from": str(child),
                        "to": str(dest),
                    }
                )
                staged_paths.append(str(dest))
            continue

        # KLN flat style: stage Tile folder under Data/
        dest_tile = data_dir / name
        dest_tile.mkdir(parents=True, exist_ok=True)
        for f in sorted(tile_root.glob("*.osgb")):
            mode = link_or_copy(f, dest_tile / f.name)
            mapping.append(
                {
                    "grid": name,
                    "action": mode,
                    "from": str(f),
                    "to": str(dest_tile / f.name),
                }
            )
        # also textures if any
        for pat in ("*.jpg", "*.jpeg", "*.png", "*.dds"):
            for f in sorted(tile_root.glob(pat)):
                mode = link_or_copy(f, dest_tile / f.name)
                mapping.append(
                    {
                        "grid": name,
                        "action": mode,
                        "from": str(f),
                        "to": str(dest_tile / f.name),
                    }
                )
        staged_paths.append(str(dest_tile))

    if region == "kowloon" or any(g["name"].startswith("Tile_") for g in grids):
        ensure_kln_metadata(meta, staged)
        mapping.append(
            {
                "grid": "*",
                "action": "official_kln_metadata",
                "from": str(meta / "KLN_metadata.xml"),
                "to": str(staged / "metadata.xml"),
            }
        )
    elif not (staged / "metadata.xml").is_file():
        raise RuntimeError(
            "Staged dataset missing metadata.xml and no official HKI metadata pack "
            "was available — refuse to invent CRS (plan §10)."
        )

    manifest = {
        "source": "Hong Kong Planning Department",
        "selection": str(selection_path),
        "region": region,
        "grids": [g["name"] for g in grids],
        "originalPaths": original_paths,
        "stagedPaths": staged_paths,
        "stagedRoot": str(staged),
        "mapping": mapping,
        "totalBytes": total_bytes,
        "preparedAt": datetime.now(timezone.utc).isoformat(),
        "notes": [
            "Non-destructive staging only: hardlink/symlink/copy + official metadata.",
            "No synthetic mesh / LOD / texture injection.",
        ],
    }
    man_path = staged / "dataset-manifest.json"
    man_path.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n")
    # also copy to hk root for convenience
    (hk / f"dataset-manifest_{run_id}.json").write_text(man_path.read_text())
    print(f"[staged] {staged}")
    print(f"[manifest] {man_path}")
    return manifest


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--selection",
        type=Path,
        help="selection_*.json from download_hk_pland.py",
    )
    ap.add_argument("--format", default="OSGB")
    args = ap.parse_args()
    root = acceptance_root() / "hk_pland"
    sel = args.selection
    if not sel:
        cands = sorted(root.glob("selection_osgb_*.json"))
        if not cands:
            print("No selection_*.json — run download_hk_pland.py first", file=sys.stderr)
            return 2
        sel = cands[-1]
    stage_selection(sel, fmt=args.format)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
