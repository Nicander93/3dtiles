"""Validate OSGB dataset roots and return a summary JSON."""

from __future__ import annotations

import re
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

TILE_DIR_RE = re.compile(r"^Tile_[+\-]?\d+_[+\-]?\d+$", re.I)


def _normalize_root(path: Path) -> Tuple[Path, Optional[str]]:
    """If user selected Data/, walk up to parent with metadata.xml."""
    note = None
    p = path.resolve()
    if p.name == "Data" and (p.parent / "metadata.xml").is_file():
        note = "Selected Data/ directory; normalized to dataset root."
        return p.parent, note
    if not (p / "metadata.xml").is_file() and (p / "Data").is_dir():
        # maybe already root but missing metadata — leave as-is for error reporting
        pass
    return p, note


def _parse_metadata(meta_path: Path) -> Dict[str, Any]:
    info: Dict[str, Any] = {"path": str(meta_path), "srs": None, "srsOrigin": None, "rawHead": ""}
    try:
        text = meta_path.read_text(encoding="utf-8", errors="replace")
        info["rawHead"] = text[:1500]
        root = ET.fromstring(text)
        srs = root.findtext("SRS")
        origin = root.findtext("SRSOrigin")
        info["srs"] = (srs or "").strip() or None
        info["srsOrigin"] = (origin or "").strip() or None
    except Exception as e:  # noqa: BLE001
        info["parseError"] = str(e)
    return info


def scan_osgb(path: str) -> Dict[str, Any]:
    """Validate OSGB root (metadata.xml + Data/Tile_*/same-name.osgb)."""
    raw = Path(path).expanduser()
    if not raw.exists():
        return {
            "ok": False,
            "valid": False,
            "path": str(raw),
            "errors": [f"Path does not exist: {raw}"],
            "warnings": [],
            "summary": {},
        }

    root, note = _normalize_root(raw)
    errors: List[str] = []
    warnings: List[str] = []
    if note:
        warnings.append(note)

    meta_path = root / "metadata.xml"
    data_dir = root / "Data"
    if not meta_path.is_file():
        errors.append(f"Missing metadata.xml under {root}")
    if not data_dir.is_dir():
        errors.append(f"Missing Data/ directory under {root}")

    metadata: Dict[str, Any] = {}
    if meta_path.is_file():
        metadata = _parse_metadata(meta_path)
        if metadata.get("parseError"):
            warnings.append(f"metadata.xml parse issue: {metadata['parseError']}")
        if not metadata.get("srs"):
            warnings.append("metadata.xml has no SRS; local preview ok, geographic export needs CRS.")

    tiles: List[Dict[str, Any]] = []
    missing_entry: List[str] = []
    total_osgb = 0
    total_bytes = 0

    if data_dir.is_dir():
        for child in sorted(data_dir.iterdir()):
            if not child.is_dir():
                continue
            name = child.name
            if not TILE_DIR_RE.match(name):
                warnings.append(f"Non-standard directory under Data/: {name}")
                continue
            entry = child / f"{name}.osgb"
            entry_ok = entry.is_file()
            osgb_files = list(child.glob("*.osgb"))
            file_count = len(osgb_files)
            size = sum(f.stat().st_size for f in osgb_files if f.is_file())
            total_osgb += file_count
            total_bytes += size
            if not entry_ok:
                missing_entry.append(str(entry))
            tiles.append(
                {
                    "name": name,
                    "path": str(child),
                    "entryOsgb": str(entry),
                    "entryExists": entry_ok,
                    "osgbCount": file_count,
                    "bytes": size,
                }
            )

    if missing_entry:
        for m in missing_entry[:20]:
            errors.append(f"Missing entry OSGB (must match folder name): {m}")
        if len(missing_entry) > 20:
            errors.append(f"... and {len(missing_entry) - 20} more missing entry files")

    if data_dir.is_dir() and not tiles:
        errors.append("No Tile_* directories found under Data/")

    valid = len(errors) == 0
    summary = {
        "root": str(root),
        "tileCount": len(tiles),
        "osgbFileCount": total_osgb,
        "totalBytes": total_bytes,
        "srs": metadata.get("srs"),
        "srsOrigin": metadata.get("srsOrigin"),
        "tiles": [
            {"name": t["name"], "osgbCount": t["osgbCount"], "entryExists": t["entryExists"]}
            for t in tiles
        ],
    }

    return {
        "ok": valid,
        "valid": valid,
        "path": str(root),
        "requestedPath": str(raw),
        "errors": errors,
        "warnings": warnings,
        "metadata": metadata,
        "summary": summary,
        "tiles": tiles,
    }
