"""CRS / origin helpers for GeoForge P5 (pragmatic subset)."""

from __future__ import annotations

import json
import re
from typing import Any, Dict, List, Optional, Tuple

_ENU_RE = re.compile(
    r"^\s*ENU\s*:\s*([+-]?\d+(?:\.\d+)?)\s*,\s*([+-]?\d+(?:\.\d+)?)\s*$",
    re.I,
)
_EPSG_RE = re.compile(r"^\s*EPSG\s*:\s*(\d+)\s*$", re.I)


def parse_origin_xyz(value: Any) -> Optional[Tuple[float, float, float]]:
    """Parse 'x,y,z' or [x,y,z] or {x,y,z} into floats."""
    if value is None:
        return None
    if isinstance(value, (list, tuple)) and len(value) >= 3:
        try:
            return float(value[0]), float(value[1]), float(value[2])
        except (TypeError, ValueError):
            return None
    if isinstance(value, dict):
        try:
            return float(value["x"]), float(value["y"]), float(value["z"])
        except (KeyError, TypeError, ValueError):
            return None
    text = str(value).strip()
    if not text:
        return None
    parts = re.split(r"[,;\s]+", text)
    if len(parts) < 3:
        return None
    try:
        return float(parts[0]), float(parts[1]), float(parts[2])
    except ValueError:
        return None


def parse_enu_lat_lon(srs: Optional[str]) -> Optional[Tuple[float, float]]:
    """ENU:lat,lon → (lat, lon)."""
    if not srs:
        return None
    m = _ENU_RE.match(srs.strip())
    if not m:
        return None
    return float(m.group(1)), float(m.group(2))


def parse_epsg_code(srs: Optional[str]) -> Optional[int]:
    if not srs:
        return None
    m = _EPSG_RE.match(srs.strip())
    if not m:
        return None
    return int(m.group(1))


def unit_hint(srs: Optional[str]) -> str:
    """Short Chinese unit / axis hint for UI."""
    if not srs:
        return "未知单位（缺 SRS）"
    s = srs.strip()
    if _ENU_RE.match(s):
        return "ENU 局部坐标：米（东/北/天）；地理原点为经纬度（度）"
    if _EPSG_RE.match(s):
        code = int(_EPSG_RE.match(s).group(1))
        if code in (4326, 4490, 4610, 4214):
            return f"EPSG:{code} 地理坐标：度（注意轴序）；高程另计"
        return f"EPSG:{code} 多为投影米制；请确认分带/假东移，高程另计"
    if "GEOGCS" in s.upper() or "PROJCS" in s.upper():
        return "WKT：按定义单位；请自行确认轴序与高程基准"
    return "自定义 CRS：请确认单位与轴序；高程基准独立"


def _geo_opts(options: Dict[str, Any]) -> Dict[str, Any]:
    geo = options.get("geo")
    if isinstance(geo, dict):
        return geo
    # flat fallbacks from older UI / convert block
    out: Dict[str, Any] = {}
    for k in ("crs", "crsOverride", "origin", "originOverride", "originX", "originY", "originZ"):
        if k in options and options[k] is not None:
            out[k] = options[k]
    if options.get("geographicExport") is not None:
        out["geographicExport"] = options.get("geographicExport")
    conv = options.get("convert") if isinstance(options.get("convert"), dict) else {}
    for k in ("crs", "origin", "x", "y", "offset", "geographicExport"):
        if k in conv and conv[k] is not None and k not in out:
            out[k] = conv[k]
    return out


def resolve_effective_geo(
    scan: Optional[Dict[str, Any]],
    options: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    """Merge scan metadata with task overrides into the CRS actually adopted."""
    options = options or {}
    geo = _geo_opts(options)
    meta = (scan or {}).get("metadata") or {}
    summary = (scan or {}).get("summary") or {}

    scan_srs = meta.get("srs") or summary.get("srs")
    scan_origin = meta.get("srsOrigin") or summary.get("srsOrigin")

    crs_override = (
        geo.get("crs")
        or geo.get("crsOverride")
        or options.get("crsOverride")
        or ""
    )
    crs_override = str(crs_override).strip() or None

    origin_override = None
    if geo.get("originX") is not None and geo.get("originY") is not None and geo.get("originZ") is not None:
        try:
            origin_override = (
                float(geo["originX"]),
                float(geo["originY"]),
                float(geo["originZ"]),
            )
        except (TypeError, ValueError):
            origin_override = None
    if origin_override is None:
        origin_override = parse_origin_xyz(
            geo.get("origin")
            or geo.get("originOverride")
            or options.get("originOverride")
        )

    effective_crs = crs_override or (str(scan_srs).strip() if scan_srs else None)
    effective_origin = None
    if origin_override is not None:
        effective_origin = {
            "x": origin_override[0],
            "y": origin_override[1],
            "z": origin_override[2],
            "source": "override",
            "text": f"{origin_override[0]},{origin_override[1]},{origin_override[2]}",
        }
    elif scan_origin:
        parsed = parse_origin_xyz(scan_origin)
        effective_origin = {
            "x": parsed[0] if parsed else None,
            "y": parsed[1] if parsed else None,
            "z": parsed[2] if parsed else None,
            "source": "metadata",
            "text": str(scan_origin),
        }

    geographic = bool(
        geo.get("geographicExport")
        or geo.get("requireCrs")
        or options.get("geographicExport")
    )

    enu = parse_enu_lat_lon(effective_crs)
    ep = parse_epsg_code(effective_crs)

    return {
        "scanSrs": scan_srs,
        "scanOrigin": scan_origin,
        "crsOverride": crs_override,
        "originOverride": (
            {"x": origin_override[0], "y": origin_override[1], "z": origin_override[2]}
            if origin_override
            else None
        ),
        "effectiveCrs": effective_crs,
        "effectiveOrigin": effective_origin,
        "unitHint": unit_hint(effective_crs),
        "geographicExport": geographic,
        "enuLatLon": {"lat": enu[0], "lon": enu[1]} if enu else None,
        "epsg": ep,
        "hasCrs": bool(effective_crs),
    }


def missing_crs_message(effective: Dict[str, Any]) -> Optional[str]:
    """If geographic export requested but no CRS → clear Chinese error."""
    if not effective.get("geographicExport"):
        return None
    if effective.get("hasCrs"):
        return None
    return (
        "缺参数阻止地理导出：扫描未得到 SRS，且未填写 CRS 覆盖。"
        "本地预览/非地理转换可关闭「地理导出」；"
        "地理定位请在 metadata.xml 提供 SRS（ENU:/EPSG:/WKT）或在转换页填写 CRS 覆盖。"
    )


def build_tile_config_json(effective: Dict[str, Any]) -> Tuple[Optional[str], List[str]]:
    """
    Build -c JSON for 3dtile when overrides map to CLI fields.
    CLI -c supports: x (lon), y (lat), offset (height), max_lvl, pbr.
    It does NOT accept EPSG/WKT/SRSOrigin directly (those come from metadata.xml).
    """
    notes: List[str] = []
    cfg: Dict[str, Any] = {}

    enu = effective.get("enuLatLon")
    # Pass -c x/y only when user overrode CRS to ENU:lat,lon
    if enu and effective.get("crsOverride"):
        cfg["x"] = float(enu["lon"])
        cfg["y"] = float(enu["lat"])
        notes.append(
            "CLI -c x/y from CRS override ENU lon=%s, lat=%s" % (enu["lon"], enu["lat"])
        )

    origin = effective.get("effectiveOrigin") or {}
    z = origin.get("z")
    if z is not None and effective.get("originOverride"):
        cfg["offset"] = float(z)
        notes.append("CLI -c offset from origin override z=%s" % (z,))

    if effective.get("crsOverride") and not enu:
        notes.append(
            "CRS override stored in task options; current 3dtile CLI has no EPSG/WKT flag; "
            "runtime still uses input metadata.xml SRS."
        )
    if effective.get("originOverride") and enu:
        ox = effective["originOverride"]
        notes.append(
            "origin override XYZ=(%s,%s,%s) recorded; CLI has no SRSOrigin flag; "
            "local E/N still from metadata.xml; z may map to -c offset."
            % (ox["x"], ox["y"], ox["z"])
        )

    if not cfg:
        return None, notes
    return json.dumps(cfg, separators=(",", ":")), notes
