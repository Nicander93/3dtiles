"""Probe 3dtile binary for KTX2 / texture-compress support."""

from __future__ import annotations

import functools
import os
import subprocess
from pathlib import Path
from typing import Any, Dict, List

from .texture_ktx2 import find_basisu

CONVERT_BIN = Path(os.environ.get("GEOFORGE_3DTILE", "/workspace/runtime/3dtile-bin/run.sh"))

# Modes the GeoForge UI / API accept
TEXTURE_MODES = ("keep", "ktx2", "ktx2-etc1s", "ktx2-uastc")

# Upstream fanvanzh/3dtiles only exposes one CLI flag and encodes ETC1S (see mesh_processor.cpp).
KTX2_CLI_FLAG = "--enable-texture-compress"

KEEP_MODES = frozenset({"keep", "none", "", "passthrough"})
ETC1S_MODES = frozenset({"ktx2", "ktx2-etc1s"})
UASTC_MODES = frozenset({"ktx2-uastc"})


def is_keep_mode(mode: str | None) -> bool:
    return (mode or "keep").lower() in KEEP_MODES


def normalize_texture_mode(mode: str | None) -> str:
    m = (mode or "keep").lower().strip()
    if m in KEEP_MODES:
        return "keep"
    if m in ("ktx2", "ktx2-etc1s", "etc1s"):
        return "ktx2-etc1s"
    if m in ("ktx2-uastc", "uastc"):
        return "ktx2-uastc"
    return m



@functools.lru_cache(maxsize=1)
def probe_basisu() -> Dict[str, Any]:
    path = find_basisu()
    return {
        "available": bool(path),
        "path": str(path) if path else None,
        "postprocessBasisu": bool(path),
        "ktx2Etc1s": bool(path),
        "ktx2Uastc": bool(path),
        "processTilesetTexture": bool(path),
    }



def _attach_basisu(result: Dict[str, Any]) -> Dict[str, Any]:
    """Merge basisu post-process capabilities into probe result."""
    bp = probe_basisu()
    result["postprocessBasisu"] = bool(bp.get("available"))
    result["basisuPath"] = bp.get("path")
    if bp.get("available"):
        result["ktx2Etc1s"] = True
        result["ktx2Uastc"] = True
        result["processTilesetTexture"] = True
        result["notes"].append(
            f"Post-process basisu available at {bp.get('path')} "
            "(KTX2 ETC1S/UASTC after convert / on process-tileset)."
        )
        if result.get("probeError") == "flag_unsupported":
            result["probeError"] = None
    else:
        result["notes"].append(
            "basisu post-process not found (expected vcpkg_installed/.../tools/basisu/basisu)."
        )
        if not result["ktx2Etc1s"]:
            result["notes"].append(
                "KTX2 ETC1S/UASTC unavailable until supporting binary or basisu is found."
            )
    return result


@functools.lru_cache(maxsize=1)
def probe_convert_bin(bin_path: str | None = None) -> Dict[str, Any]:
    """Return capability dict for the convert binary (cached)."""
    path = Path(bin_path or CONVERT_BIN)
    result: Dict[str, Any] = {
        "bin": str(path),
        "exists": path.is_file(),
        "enableTextureCompress": False,
        "textureFlag": KTX2_CLI_FLAG,
        "ktx2Etc1s": False,
        "ktx2Uastc": False,
        "processTilesetTexture": False,
        "postprocessBasisu": False,
        "basisuPath": None,
        "notes": [],
        "probeError": None,
    }
    if not path.is_file():
        result["notes"].append(f"Converter not found: {path}")
        result["probeError"] = "missing_binary"
        return _attach_basisu(result)

    try:
        proc = subprocess.run(
            [str(path), "--help"],
            capture_output=True,
            text=True,
            timeout=15,
            env={**os.environ},
        )
        help_text = (proc.stdout or "") + (proc.stderr or "")
    except Exception as e:  # noqa: BLE001
        result["probeError"] = str(e)
        result["notes"].append(f"help probe failed: {e}")
        return _attach_basisu(result)

    has_flag = KTX2_CLI_FLAG in help_text
    if not has_flag:
        # Older clap builds may omit long help; try a dry reject probe.
        try:
            proc2 = subprocess.run(
                [str(path), "-f", "osgb", "-i", "/__no_such_input__", "-o", "/tmp", KTX2_CLI_FLAG],
                capture_output=True,
                text=True,
                timeout=15,
            )
            err = (proc2.stdout or "") + (proc2.stderr or "")
            # clap unknown-arg message
            unknown = (
                "wasn't expected" in err
                or "unrecognized" in err.lower()
                or "Found argument" in err
                and "wasn't expected" in err
            )
            if "wasn't expected" in err or "unrecognized" in err.lower():
                has_flag = False
            elif "Found argument" in err and KTX2_CLI_FLAG in err and "wasn't expected" in err:
                has_flag = False
            else:
                # Flag accepted enough to proceed to I/O error → treat as supported
                has_flag = KTX2_CLI_FLAG not in err or "wasn't expected" not in err
                # Safer: if the binary complains about missing input rather than unknown arg
                if "wasn't expected" in err or "unexpected argument" in err.lower():
                    has_flag = False
                elif "No such file" in err or "not found" in err.lower() or "input" in err.lower():
                    has_flag = True
                elif proc2.returncode != 0 and "enable-texture-compress" in err and "expected" in err:
                    has_flag = False
        except Exception as e:  # noqa: BLE001
            result["notes"].append(f"flag probe failed: {e}")
            has_flag = False

    result["enableTextureCompress"] = bool(has_flag)
    result["ktx2Etc1s"] = bool(has_flag)
    # Upstream encoder hard-codes ETC1S; no UASTC CLI switch.
    result["ktx2Uastc"] = False
    # README format matrix: texture-compress applies to OSGB/FBX convert only, not B3DM/gltf post-process.
    result["processTilesetTexture"] = False

    if has_flag:
        result["notes"].append(
            f"Binary accepts {KTX2_CLI_FLAG} (KTX2 ETC1S during OSGB convert)."
        )
        result["notes"].append(
            "UASTC not exposed by fanvanzh/3dtiles CLI (encoder uses ETC1S only)."
        )
        result["notes"].append(
            "process-tileset KTX2 not supported by binary (flag is convert-time only)."
        )
    else:
        result["notes"].append(
            f"Runtime binary does not accept {KTX2_CLI_FLAG} "
            "(exported winner1/3dtiles:1.0 / Aug-2023 build)."
        )
        result["probeError"] = "flag_unsupported"

    return _attach_basisu(result)


def texture_mode_support(mode: str | None, caps: Dict[str, Any] | None = None) -> Dict[str, Any]:
    """Describe whether a texture mode can run for convert-osgb."""
    caps = caps or probe_convert_bin()
    norm = normalize_texture_mode(mode)
    post = bool(caps.get("postprocessBasisu"))
    native = bool(caps.get("enableTextureCompress"))
    if norm == "keep":
        return {
            "mode": "keep",
            "supported": True,
            "cliFlags": [],
            "postprocess": False,
            "reason": "passthrough — no compress",
        }
    if norm == "ktx2-etc1s":
        ok = bool(caps.get("ktx2Etc1s") or native or post)
        flags = [KTX2_CLI_FLAG] if native else []
        if native:
            reason = f"Will pass {KTX2_CLI_FLAG} to OSGB convert (ETC1S / KHR_texture_basisu)."
        elif post:
            reason = (
                "Will convert with keep textures, then post-process with basisu "
                f"({caps.get('basisuPath')}) to KTX2 ETC1S / KHR_texture_basisu."
            )
        else:
            reason = (
                f"Binary at {caps.get('bin')} does not support {KTX2_CLI_FLAG} "
                "and basisu post-process is unavailable."
            )
        return {
            "mode": "ktx2-etc1s",
            "supported": ok,
            "cliFlags": flags,
            "postprocess": bool(post and not native),
            "reason": reason,
        }
    if norm == "ktx2-uastc":
        ok = bool(caps.get("ktx2Uastc") and post)
        return {
            "mode": "ktx2-uastc",
            "supported": ok,
            "cliFlags": [],
            "postprocess": ok,
            "reason": (
                "Will post-process with basisu -uastc after convert (KHR_texture_basisu)."
                if ok
                else "UASTC requires basisu post-process (fanvanzh CLI encodes ETC1S only)."
            ),
        }
    return {
        "mode": norm,
        "supported": False,
        "cliFlags": [],
        "postprocess": False,
        "reason": f"Unknown texture mode: {norm}",
    }


def process_tileset_texture_support(mode: str | None, caps: Dict[str, Any] | None = None) -> Dict[str, Any]:
    """process-tileset KTX2 via basisu post-process when available."""
    caps = caps or probe_convert_bin()
    norm = normalize_texture_mode(mode)
    if norm == "keep":
        return {
            "mode": "keep",
            "supported": True,
            "reason": "passthrough — no compress",
        }
    post = bool(caps.get("postprocessBasisu") or caps.get("processTilesetTexture"))
    if norm in ("ktx2-etc1s", "ktx2-uastc") and post:
        return {
            "mode": norm,
            "supported": True,
            "reason": (
                f"Will post-process existing B3DM/GLB with basisu ({caps.get('basisuPath')}) "
                f"to {norm} / KHR_texture_basisu."
            ),
        }
    return {
        "mode": norm,
        "supported": False,
        "reason": (
            f"KTX2 mode={norm} is not supported on process-tileset: "
            f"upstream {KTX2_CLI_FLAG} applies only during OSGB/FBX convert, "
            "and basisu post-process is not configured on this box."
        ),
    }


def capabilities_payload() -> Dict[str, Any]:
    caps = probe_convert_bin()
    modes: List[Dict[str, Any]] = []
    for m in ("keep", "ktx2-etc1s", "ktx2-uastc"):
        info = texture_mode_support(m, caps)
        info["processTileset"] = process_tileset_texture_support(m, caps)
        modes.append(info)
    return {
        "convert": caps,
        "textureModes": modes,
        "aliases": {"ktx2": "ktx2-etc1s"},
        "postprocessBasisu": {
            "available": bool(caps.get("postprocessBasisu")),
            "path": caps.get("basisuPath"),
        },
    }


def output_has_ktx2_evidence(root: Path) -> Dict[str, Any]:
    """Scan tileset output for KTX2 / KHR_texture_basisu evidence (honest verify)."""
    root = Path(root)
    evidence: Dict[str, Any] = {
        "found": False,
        "ktx2Files": 0,
        "basisuMentions": 0,
        "samples": [],
    }
    if not root.exists():
        return evidence
    markers = (b"KHR_texture_basisu", b"image/ktx2", b".ktx2")
    for p in root.rglob("*"):
        if not p.is_file():
            continue
        name = p.name.lower()
        if name.endswith(".ktx2"):
            evidence["ktx2Files"] += 1
            evidence["found"] = True
            if len(evidence["samples"]) < 5:
                evidence["samples"].append(str(p))
            continue
        if name.endswith((".b3dm", ".glb", ".gltf", ".i3dm", ".json")):
            try:
                # Cap read for large b3dm
                data = p.read_bytes()[: min(p.stat().st_size, 2_000_000)]
            except OSError:
                continue
            hits = sum(1 for m in markers if m in data)
            if hits:
                evidence["basisuMentions"] += 1
                evidence["found"] = True
                if len(evidence["samples"]) < 5:
                    evidence["samples"].append(str(p))
    return evidence


if __name__ == "__main__":
    import json

    print(json.dumps(capabilities_payload(), indent=2))
