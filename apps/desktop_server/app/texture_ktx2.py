"""Post-process 3D Tiles textures to KTX2 via basisu (KHR_texture_basisu).

Alternative to _3dtile --enable-texture-compress when the runtime binary lacks the flag.
Walks b3dm/glb/gltf, encodes JPEG/PNG textures with basisu (ETC1S or UASTC), rewrites
glTF to use KHR_texture_basisu. Cesium-loadable output.
"""

from __future__ import annotations

import json
import os
import shutil
import struct
import subprocess
import tempfile
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence, Tuple

BASISU_CANDIDATES = [
    Path(os.environ["GEOFORGE_BASISU"]) if os.environ.get("GEOFORGE_BASISU") else None,
    Path("/workspace/repos/3dtiles/vcpkg_installed/x64-linux/tools/basisu/basisu"),
    Path(__file__).resolve().parents[3] / "vcpkg_installed" / "x64-linux" / "tools" / "basisu" / "basisu",
]

IMAGE_MIMES = {"image/jpeg", "image/png", "image/jpg", "image/webp"}
KTX2_MIME = "image/ktx2"
EXT_NAME = "KHR_texture_basisu"


def find_basisu() -> Optional[Path]:
    for p in BASISU_CANDIDATES:
        if p and p.is_file() and os.access(p, os.X_OK):
            return p
    which = shutil.which("basisu")
    if which:
        return Path(which)
    return None


def basisu_available() -> bool:
    return find_basisu() is not None


def _pad4(n: int) -> int:
    return (4 - (n % 4)) % 4


def _align4(buf: bytearray) -> None:
    pad = _pad4(len(buf))
    if pad:
        buf.extend(b"\x00" * pad)


def parse_b3dm(path: Path) -> Tuple[bytes, bytes]:
    data = path.read_bytes()
    if data[:4] != b"b3dm":
        raise ValueError(f"not b3dm: {path}")
    ftj, ftb, btj, btb = struct.unpack_from("<IIII", data, 12)
    offset = 28 + ftj + ftb + btj + btb
    header = data[:offset]
    glb = data[offset:]
    if glb[:4] != b"glTF":
        for pad in range(0, 4):
            if data[offset + pad : offset + pad + 4] == b"glTF":
                header = data[: offset + pad]
                glb = data[offset + pad :]
                break
        else:
            raise ValueError(f"glb not found in {path}")
    return header, glb


def write_b3dm_preserving_header(path: Path, header: bytes, glb: bytes) -> None:
    out = bytearray(header) + glb
    struct.pack_into("<I", out, 8, len(out))
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(bytes(out))


def parse_glb(glb: bytes) -> Tuple[Dict[str, Any], bytes]:
    if glb[:4] != b"glTF":
        raise ValueError("not glb")
    _magic, version, length = struct.unpack_from("<4sII", glb, 0)
    if version != 2:
        raise ValueError(f"unsupported glTF version {version}")
    pos = 12
    json_data: Optional[Dict[str, Any]] = None
    bin_data = b""
    while pos + 8 <= len(glb) and pos < length:
        clen, ctype = struct.unpack_from("<I4s", glb, pos)
        pos += 8
        chunk = glb[pos : pos + clen]
        pos += clen
        if ctype == b"JSON":
            json_data = json.loads(chunk.decode("utf-8"))
        elif ctype.startswith(b"BIN"):
            bin_data = chunk
    if json_data is None:
        raise ValueError("GLB missing JSON chunk")
    return json_data, bin_data


def build_glb(gltf: Dict[str, Any], bin_data: bytes) -> bytes:
    json_bytes = json.dumps(gltf, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    json_pad = _pad4(len(json_bytes))
    json_bytes = json_bytes + (b" " * json_pad)
    bin_pad = _pad4(len(bin_data))
    bin_padded = bin_data + (b"\x00" * bin_pad)
    total = 12 + 8 + len(json_bytes) + 8 + len(bin_padded)
    out = bytearray()
    out.extend(struct.pack("<4sII", b"glTF", 2, total))
    out.extend(struct.pack("<I4s", len(json_bytes), b"JSON"))
    out.extend(json_bytes)
    out.extend(struct.pack("<I4s", len(bin_padded), b"BIN\x00"))
    out.extend(bin_padded)
    return bytes(out)


def _encode_image_basisu(
    basisu: Path,
    image_bytes: bytes,
    mime: str,
    mode: str,
    work: Path,
    idx: int,
    quality: int = 128,
) -> bytes:
    ext = ".png"
    m = (mime or "").lower()
    if "jpeg" in m or "jpg" in m:
        ext = ".jpg"
    elif "webp" in m:
        ext = ".webp"
    src = work / f"img_{idx}{ext}"
    dst = work / f"img_{idx}.ktx2"
    src.write_bytes(image_bytes)
    cmd = [str(basisu), "-ktx2", "-file", str(src), "-output_file", str(dst)]
    if mode == "ktx2-uastc":
        cmd.extend(["-uastc", "-uastc_level", "2"])
    else:
        cmd.extend(["-etc1s", "-q", str(quality)])
    proc = subprocess.run(cmd, capture_output=True, text=True, timeout=300)
    if proc.returncode != 0 or not dst.is_file():
        raise RuntimeError(
            f"basisu failed rc={proc.returncode}: {(proc.stderr or proc.stdout or '')[-500:]}"
        )
    return dst.read_bytes()


def _uri_to_path(base: Path, uri: str) -> Path:
    if uri.startswith("data:"):
        raise ValueError("data URI images not supported")
    return (base / uri).resolve()


def rewrite_gltf_textures(
    gltf: Dict[str, Any],
    bin_data: bytes,
    basisu: Path,
    mode: str,
    work: Path,
    external_base: Optional[Path] = None,
    quality: int = 128,
) -> Tuple[Dict[str, Any], bytes, int]:
    """Rewrite images/textures to KTX2 + KHR_texture_basisu. Returns (gltf, bin, converted_count)."""
    images = gltf.get("images") or []
    if not images:
        return gltf, bin_data, 0

    buffer_views: List[Dict[str, Any]] = list(gltf.get("bufferViews") or [])
    # Map image index -> new ktx2 bytes
    converted: Dict[int, bytes] = {}
    for i, img in enumerate(images):
        mime = (img.get("mimeType") or "").lower()
        if mime == KTX2_MIME or EXT_NAME in str(img):
            continue
        raw: Optional[bytes] = None
        if "bufferView" in img:
            bv = buffer_views[img["bufferView"]]
            off = int(bv.get("byteOffset") or 0)
            length = int(bv["byteLength"])
            raw = bin_data[off : off + length]
            if not mime:
                # sniff
                if raw[:3] == b"\xff\xd8\xff":
                    mime = "image/jpeg"
                elif raw[:8] == b"\x89PNG\r\n\x1a\n":
                    mime = "image/png"
                else:
                    mime = "image/jpeg"
        elif "uri" in img and external_base is not None:
            uri = img["uri"]
            if uri.startswith("data:"):
                continue
            p = _uri_to_path(external_base, uri)
            if not p.is_file():
                continue
            raw = p.read_bytes()
            if not mime:
                suf = p.suffix.lower()
                mime = {"jpg": "image/jpeg", ".jpeg": "image/jpeg", ".png": "image/png"}.get(suf, "image/jpeg")
                if suf == ".jpg" or suf == ".jpeg":
                    mime = "image/jpeg"
                elif suf == ".png":
                    mime = "image/png"
        if raw is None:
            continue
        if mime not in IMAGE_MIMES and not mime.startswith("image/"):
            continue
        if mime == KTX2_MIME:
            continue
        converted[i] = _encode_image_basisu(basisu, raw, mime, mode, work, i, quality=quality)

    if not converted:
        return gltf, bin_data, 0

    # Rebuild BIN: keep all bufferViews; replace image-owned ones; append for uri-based images
    image_bv_indices = {}
    for i, img in enumerate(images):
        if i in converted and "bufferView" in img:
            image_bv_indices[img["bufferView"]] = i

    new_bin = bytearray()
    new_bvs: List[Dict[str, Any]] = []
    for bi, bv in enumerate(buffer_views):
        nbv = dict(bv)
        if bi in image_bv_indices:
            ktx = converted[image_bv_indices[bi]]
            _align4(new_bin)
            nbv["byteOffset"] = len(new_bin)
            nbv["byteLength"] = len(ktx)
            new_bin.extend(ktx)
        else:
            off = int(bv.get("byteOffset") or 0)
            length = int(bv["byteLength"])
            chunk = bin_data[off : off + length]
            # preserve stride alignment: align start to 4
            _align4(new_bin)
            nbv["byteOffset"] = len(new_bin)
            nbv["byteLength"] = len(chunk)
            new_bin.extend(chunk)
        new_bvs.append(nbv)

    # External/uri images: add new bufferViews
    for i, ktx in converted.items():
        img = images[i]
        if "bufferView" in img:
            images[i] = {
                "mimeType": KTX2_MIME,
                "bufferView": img["bufferView"],
            }
        else:
            _align4(new_bin)
            bv_index = len(new_bvs)
            new_bvs.append({"buffer": 0, "byteOffset": len(new_bin), "byteLength": len(ktx)})
            new_bin.extend(ktx)
            images[i] = {"mimeType": KTX2_MIME, "bufferView": bv_index}

    gltf["images"] = images
    gltf["bufferViews"] = new_bvs
    buffers = gltf.get("buffers") or [{"byteLength": 0}]
    if not buffers:
        buffers = [{"byteLength": 0}]
    buffers[0] = dict(buffers[0])
    buffers[0]["byteLength"] = len(new_bin)
    # drop uri on buffer 0 if embedded
    buffers[0].pop("uri", None)
    gltf["buffers"] = buffers

    # textures: KHR_texture_basisu
    textures = gltf.get("textures") or []
    for ti, tex in enumerate(textures):
        src = tex.get("source")
        if src is None:
            continue
        if src not in converted and src not in {img_i for img_i in converted}:
            # still add extension if image is already ktx2
            img = images[src] if src < len(images) else {}
            if img.get("mimeType") != KTX2_MIME and src not in converted:
                continue
        ntex = {k: v for k, v in tex.items() if k != "source"}
        exts = dict(ntex.get("extensions") or {})
        exts[EXT_NAME] = {"source": src}
        ntex["extensions"] = exts
        # keep source as fallback omitted per spec recommendation when required
        textures[ti] = ntex
    gltf["textures"] = textures

    used = list(gltf.get("extensionsUsed") or [])
    req = list(gltf.get("extensionsRequired") or [])
    if EXT_NAME not in used:
        used.append(EXT_NAME)
    if EXT_NAME not in req:
        req.append(EXT_NAME)
    gltf["extensionsUsed"] = used
    gltf["extensionsRequired"] = req

    return gltf, bytes(new_bin), len(converted)


def process_glb_bytes(
    glb: bytes,
    basisu: Path,
    mode: str,
    work: Path,
    quality: int = 128,
) -> Tuple[bytes, int]:
    gltf, bin_data = parse_glb(glb)
    gltf2, bin2, n = rewrite_gltf_textures(gltf, bin_data, basisu, mode, work, quality=quality)
    if n == 0:
        return glb, 0
    return build_glb(gltf2, bin2), n


def process_b3dm_file(path: Path, basisu: Path, mode: str, work: Path, quality: int = 128) -> int:
    header, glb = parse_b3dm(path)
    new_glb, n = process_glb_bytes(glb, basisu, mode, work, quality=quality)
    if n == 0:
        return 0
    write_b3dm_preserving_header(path, header, new_glb)
    return n


def process_glb_file(path: Path, basisu: Path, mode: str, work: Path, quality: int = 128) -> int:
    glb = path.read_bytes()
    new_glb, n = process_glb_bytes(glb, basisu, mode, work, quality=quality)
    if n == 0:
        return 0
    path.write_bytes(new_glb)
    return n


def process_gltf_file(path: Path, basisu: Path, mode: str, work: Path, quality: int = 128) -> int:
    gltf = json.loads(path.read_text(encoding="utf-8"))
    # load bin if present
    bin_data = b""
    buffers = gltf.get("buffers") or []
    if buffers and buffers[0].get("uri") and not str(buffers[0]["uri"]).startswith("data:"):
        bin_path = path.parent / buffers[0]["uri"]
        if bin_path.is_file():
            bin_data = bin_path.read_bytes()
    gltf2, bin2, n = rewrite_gltf_textures(
        gltf, bin_data, basisu, mode, work, external_base=path.parent, quality=quality
    )
    if n == 0:
        return 0
    # write bin alongside
    bin_name = path.with_suffix(".bin").name
    if not buffers:
        gltf2["buffers"] = [{"byteLength": len(bin2), "uri": bin_name}]
    else:
        gltf2["buffers"][0]["uri"] = buffers[0].get("uri") or bin_name
        gltf2["buffers"][0]["byteLength"] = len(bin2)
        bin_name = gltf2["buffers"][0]["uri"]
    (path.parent / bin_name).write_bytes(bin2)
    path.write_text(json.dumps(gltf2, indent=2), encoding="utf-8")
    return n


def process_tileset_dir(
    root: Path,
    mode: str = "ktx2-etc1s",
    basisu: Optional[Path] = None,
    quality: int = 128,
    log: Optional[Any] = None,
) -> Dict[str, Any]:
    """In-place KTX2 post-process under a 3D Tiles directory."""
    root = Path(root)
    basisu = basisu or find_basisu()
    if not basisu:
        raise FileNotFoundError("basisu not found (set GEOFORGE_BASISU or install vcpkg basisu)")
    mode = (mode or "ktx2-etc1s").lower()
    if mode in ("ktx2", "etc1s"):
        mode = "ktx2-etc1s"
    def _log(msg: str) -> None:
        if log:
            log(msg)

    stats = {
        "root": str(root),
        "mode": mode,
        "basisu": str(basisu),
        "filesSeen": 0,
        "filesConverted": 0,
        "texturesConverted": 0,
        "errors": [],
    }
    patterns = ("*.b3dm", "*.glb", "*.gltf")
    files: List[Path] = []
    for pat in patterns:
        files.extend(root.rglob(pat))
    files = sorted(set(files))
    stats["filesSeen"] = len(files)
    with tempfile.TemporaryDirectory(prefix="geoforge_ktx2_") as td:
        work = Path(td)
        for fp in files:
            try:
                if fp.suffix.lower() == ".b3dm":
                    n = process_b3dm_file(fp, basisu, mode, work, quality=quality)
                elif fp.suffix.lower() == ".glb":
                    n = process_glb_file(fp, basisu, mode, work, quality=quality)
                else:
                    n = process_gltf_file(fp, basisu, mode, work, quality=quality)
                if n:
                    stats["filesConverted"] += 1
                    stats["texturesConverted"] += n
                    _log(f"[texture_ktx2] {fp.relative_to(root)} textures={n}")
            except Exception as e:  # noqa: BLE001
                stats["errors"].append(f"{fp}: {e}")
                _log(f"[texture_ktx2] ERROR {fp}: {e}")
    return stats


def copy_and_process(
    src: Path,
    dst: Path,
    mode: str = "ktx2-etc1s",
    quality: int = 128,
    log: Optional[Any] = None,
) -> Dict[str, Any]:
    src, dst = Path(src), Path(dst)
    if dst.exists():
        shutil.rmtree(dst)
    shutil.copytree(src, dst)
    return process_tileset_dir(dst, mode=mode, quality=quality, log=log)


if __name__ == "__main__":
    import argparse
    import sys

    ap = argparse.ArgumentParser(description="Post-process 3D Tiles textures to KTX2 (basisu)")
    ap.add_argument("-i", "--input", required=True, help="input tileset directory")
    ap.add_argument("-o", "--output", help="output directory (copy then process); default=in-place")
    ap.add_argument("--mode", default="ktx2-etc1s", choices=["ktx2", "ktx2-etc1s", "ktx2-uastc"])
    ap.add_argument("-q", "--quality", type=int, default=128)
    args = ap.parse_args()
    def _print(m: str) -> None:
        print(m, flush=True)
    if args.output:
        st = copy_and_process(Path(args.input), Path(args.output), mode=args.mode, quality=args.quality, log=_print)
    else:
        st = process_tileset_dir(Path(args.input), mode=args.mode, quality=args.quality, log=_print)
    print(json.dumps(st, indent=2))
    sys.exit(0 if st["texturesConverted"] > 0 or st["filesSeen"] == 0 else 1)
