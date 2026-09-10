"""GeoForge 3D local FastAPI server."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
from pathlib import Path
from typing import Any, Dict, List, Optional

from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import FileResponse
from fastapi.staticfiles import StaticFiles
from pydantic import BaseModel, field_validator

from . import __version__
from .osgb_scan import scan_osgb
from .convert_caps import capabilities_payload, probe_convert_bin
from .geo import missing_crs_message, resolve_effective_geo
from .runner import CONVERT_BIN, runner
from .tasks import store

PRODUCT = "GeoForge 3D"
PREVIEW_CACHE = Path(
    os.environ.get("GEOFORGE_PREVIEW_CACHE", "/workspace/data/geoforge_preview_cache")
)
REPO = Path("/workspace/repos/3dtiles")
WEB_DIST = REPO / "apps" / "desktop" / "dist"
PREVIEW_SITE = Path("/workspace/data/preview_site")

app = FastAPI(title=PRODUCT, version=__version__)
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


class ScanBody(BaseModel):
    path: str


class PathRef(BaseModel):
    path: str


class RebuildTopOpts(BaseModel):
    enabled: bool = False
    levels: int = 1  # V1: 1 or 2 (rebuild_top.py --levels)
    simplify: float = 0.5
    textureScale: float = 0.5

    @field_validator("levels", mode="before")
    @classmethod
    def clamp_rebuild_levels(cls, v: Any) -> int:
        try:
            n = int(v)
        except (TypeError, ValueError):
            return 1
        if n <= 1:
            return 1
        return 2


class TextureOpts(BaseModel):
    mode: str = "keep"


class GeoOpts(BaseModel):
    crs: Optional[str] = None
    crsOverride: Optional[str] = None
    origin: Optional[str] = None
    originOverride: Optional[str] = None
    originX: Optional[float] = None
    originY: Optional[float] = None
    originZ: Optional[float] = None
    geographicExport: bool = False


class TaskOptions(BaseModel):
    rebuildTop: Optional[RebuildTopOpts] = None
    texture: Optional[TextureOpts] = None
    taskName: Optional[str] = None
    convert: Optional[Dict[str, Any]] = None
    geo: Optional[GeoOpts] = None
    geographicExport: Optional[bool] = None


class CreateTaskBody(BaseModel):
    operation: str
    input: PathRef
    output: PathRef
    options: Optional[TaskOptions] = None
    taskName: Optional[str] = None


class PrepareOsgbBody(BaseModel):
    path: str


@app.get("/api/health")
def health() -> Dict[str, Any]:
    caps = probe_convert_bin()
    return {
        "ok": True,
        "product": PRODUCT,
        "version": __version__,
        "convertBin": str(CONVERT_BIN),
        "texture": {
            "enableTextureCompress": bool(caps.get("enableTextureCompress")),
            "ktx2Etc1s": bool(caps.get("ktx2Etc1s")),
            "ktx2Uastc": bool(caps.get("ktx2Uastc")),
            "processTilesetTexture": bool(caps.get("processTilesetTexture")),
            "postprocessBasisu": bool(caps.get("postprocessBasisu")),
            "basisuPath": caps.get("basisuPath"),
            "notes": caps.get("notes") or [],
        },
    }


@app.get("/api/capabilities")
def api_capabilities() -> Dict[str, Any]:
    return {"ok": True, **capabilities_payload()}


@app.post("/api/osgb/scan")
def api_osgb_scan(body: ScanBody) -> Dict[str, Any]:
    result = scan_osgb(body.path)
    effective = resolve_effective_geo(result, {})
    result["geo"] = effective
    result["unitHint"] = effective.get("unitHint")
    return result


@app.post("/api/tasks")
def create_task(body: CreateTaskBody) -> Dict[str, Any]:
    op = body.operation.strip()
    if op not in ("convert-osgb", "rebuild-top", "process-tileset"):
        raise HTTPException(400, f"Unsupported operation: {op}")
    opts: Dict[str, Any] = {}
    task_name = body.taskName or ""
    if body.options:
        if body.options.rebuildTop:
            opts["rebuildTop"] = body.options.rebuildTop.model_dump()
        if body.options.texture:
            opts["texture"] = body.options.texture.model_dump()
        if body.options.convert:
            opts["convert"] = body.options.convert
        if body.options.geo:
            opts["geo"] = body.options.geo.model_dump(exclude_none=True)
        if body.options.geographicExport is not None:
            opts["geographicExport"] = body.options.geographicExport
            geo = opts.setdefault("geo", {})
            if isinstance(geo, dict) and "geographicExport" not in geo:
                geo["geographicExport"] = body.options.geographicExport
        if body.options.taskName:
            task_name = body.options.taskName

    # P5 pre-submit: geographic export without SRS/override → 400
    if op == "convert-osgb":
        scan = scan_osgb(body.input.path)
        effective = resolve_effective_geo(scan if scan.get("ok") or scan.get("valid") else scan, opts)
        crs_err = missing_crs_message(effective)
        if crs_err:
            raise HTTPException(
                status_code=400,
                detail={"message": crs_err, "code": "MISSING_CRS", "geo": effective},
            )

    task = store.create_task(
        operation=op,
        input_path=body.input.path,
        output_path=body.output.path,
        options=opts,
        task_name=task_name,
    )
    runner.enqueue(task["id"])
    return {"ok": True, "task": task}


@app.get("/api/tasks")
def list_tasks() -> Dict[str, Any]:
    return {"ok": True, "tasks": store.list_tasks()}


@app.get("/api/tasks/{task_id}")
def get_task(task_id: str) -> Dict[str, Any]:
    task = store.get(task_id)
    if not task:
        raise HTTPException(404, "task not found")
    return {"ok": True, "task": task}


@app.post("/api/tasks/{task_id}/cancel")
def cancel_task(task_id: str) -> Dict[str, Any]:
    result = runner.cancel(task_id)
    if not result.get("ok"):
        raise HTTPException(404, result.get("error") or "task not found")
    return result


@app.get("/api/tasks/{task_id}/logs")
def get_task_logs(task_id: str, tail: int = 500) -> Dict[str, Any]:
    """Return recent log lines (and logPath) for a task."""
    logs = store.get_logs(task_id, tail=max(1, min(tail, 5000)))
    if not logs:
        raise HTTPException(404, "task not found")
    return {"ok": True, **logs}


@app.get("/api/artifacts")
def list_artifacts() -> Dict[str, Any]:
    return {"ok": True, "artifacts": store.list_artifacts()}


@app.get("/api/artifacts/{art_id}/preview-url")
def artifact_preview_url(art_id: str) -> Dict[str, Any]:
    art = store.get_artifact(art_id)
    if not art:
        raise HTTPException(404, "artifact not found")
    root = Path(art["path"])
    tileset = root / "tileset.json"
    if not tileset.is_file():
        raise HTTPException(404, f"tileset.json missing under {root}")
    url = f"/artifacts/{art_id}/tileset.json"
    return {
        "ok": True,
        "id": art_id,
        "url": url,
        "previewUrl": url,
        "path": str(tileset),
        "has_tileset": True,
    }


def _resolve_artifact_file(art_id: str, file_path: str) -> Path:
    art = store.get_artifact(art_id)
    if not art:
        raise HTTPException(404, "artifact not found")
    root = Path(art["path"]).resolve()
    if not root.is_dir():
        raise HTTPException(404, f"artifact path missing: {root}")
    # Prevent path traversal
    rel = file_path.strip("/") or "tileset.json"
    target = (root / rel).resolve()
    try:
        target.relative_to(root)
    except ValueError as exc:
        raise HTTPException(400, "invalid path") from exc
    if not target.is_file():
        raise HTTPException(404, f"file not found: {rel}")
    return target


@app.get("/artifacts/{art_id}/{file_path:path}")
def serve_artifact(art_id: str, file_path: str) -> FileResponse:
    target = _resolve_artifact_file(art_id, file_path)
    return FileResponse(target)


@app.post("/api/preview/osgb/native")
def preview_osgb_native(body: PathRef) -> Dict[str, Any]:
    """Removed in Phase 4 — OSGB native preview / Qt viewer deleted."""
    raise HTTPException(
        410,
        "OSGB native preview removed in Phase 4; see docs/product/historical/OSGB_NATIVE_PREVIEW.md",
    )



@app.post("/api/preview/osgb/prepare")
def preview_osgb_prepare(body: PrepareOsgbBody) -> Dict[str, Any]:
    """Alias: ensure GLB preview cache + return models for Three.js."""
    return prepare_osgb_preview(body)



def _cache_key(path: str) -> str:
    return hashlib.sha1(str(Path(path).resolve()).encode("utf-8")).hexdigest()[:16]


@app.post("/api/preview/prepare-osgb")
def prepare_osgb_preview(body: PrepareOsgbBody) -> Dict[str, Any]:
    """Convert root OSGB entry files to GLB under preview cache; return model list."""
    scan = scan_osgb(body.path)
    if not scan.get("valid"):
        raise HTTPException(400, detail={"message": "Invalid OSGB", "scan": scan})

    root = Path(scan["path"])
    cache_dir = PREVIEW_CACHE / _cache_key(str(root))
    glb_dir = cache_dir / "osgb_glb"
    glb_dir.mkdir(parents=True, exist_ok=True)

    if not CONVERT_BIN.is_file():
        raise HTTPException(500, f"Converter not found: {CONVERT_BIN}")

    models: List[Dict[str, Any]] = []
    errors: List[str] = []
    for tile in scan.get("tiles") or []:
        name = tile["name"]
        src = Path(tile["entryOsgb"])
        if not src.is_file():
            continue
        dst = glb_dir / f"{name}.glb"
        if not dst.is_file() or dst.stat().st_mtime < src.stat().st_mtime:
            cmd = [str(CONVERT_BIN), "-f", "gltf", "-i", str(src), "-o", str(dst)]
            try:
                proc = subprocess.run(cmd, capture_output=True, text=True, timeout=300)
                if proc.returncode != 0 or not dst.is_file():
                    err_tail = (proc.stderr or proc.stdout or "")[-500:]
                    errors.append(f"{name}: convert failed rc={proc.returncode} {err_tail}")
                    continue
            except Exception as e:  # noqa: BLE001
                errors.append(f"{name}: {e}")
                continue
        models.append(
            {
                "name": name,
                "url": f"/preview-cache/{cache_dir.name}/osgb_glb/{name}.glb",
                "path": str(dst),
                "bytes": dst.stat().st_size,
            }
        )

    meta_head = ""
    meta = root / "metadata.xml"
    if meta.is_file():
        meta_head = meta.read_text(encoding="utf-8", errors="replace")[:1500]

    manifest = {
        "source": str(root),
        "models": models,
        "metadata_xml_head": meta_head,
        "errors": errors,
    }
    (cache_dir / "osgb_models.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    return {"ok": True, "cacheDir": str(cache_dir), "models": models, "errors": errors, "scan": scan}


# Static mounts (after API routes)
PREVIEW_CACHE.mkdir(parents=True, exist_ok=True)
app.mount("/preview-cache", StaticFiles(directory=str(PREVIEW_CACHE)), name="preview-cache")

if PREVIEW_SITE.is_dir():
    app.mount(
        "/preview-site",
        StaticFiles(directory=str(PREVIEW_SITE), html=True),
        name="preview-site",
    )

if WEB_DIST.is_dir():
    app.mount("/assets", StaticFiles(directory=str(WEB_DIST / "assets")), name="web-assets")
    # favicon etc.
    for name in ("favicon.svg", "favicon.ico"):
        f = WEB_DIST / name
        if f.is_file():
            # bound via spa fallback below
            pass


@app.get("/")
def index() -> FileResponse:
    index_html = WEB_DIST / "index.html"
    if index_html.is_file():
        return FileResponse(index_html)
    raise HTTPException(404, "apps/desktop/dist/index.html not found")


@app.get("/{full_path:path}")
def spa_or_static(full_path: str) -> FileResponse:
    """Serve web dist files; SPA fallback to index.html for client routes."""
    if full_path.startswith(("api/", "artifacts/", "preview-cache/", "preview-site/")):
        raise HTTPException(404, "not found")
    candidate = (WEB_DIST / full_path).resolve()
    try:
        candidate.relative_to(WEB_DIST.resolve())
    except ValueError as exc:
        raise HTTPException(404, "not found") from exc
    if candidate.is_file():
        return FileResponse(candidate)
    index_html = WEB_DIST / "index.html"
    if index_html.is_file():
        return FileResponse(index_html)
    raise HTTPException(404, "apps/desktop/dist not found")
