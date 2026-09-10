#!/usr/bin/env python3
"""Capture acceptance environment.json (plan §8.1)."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path


def _run(cmd: list[str], timeout: int = 30) -> str:
    try:
        p = subprocess.run(
            cmd, capture_output=True, text=True, timeout=timeout, check=False
        )
        out = (p.stdout or "").strip() or (p.stderr or "").strip()
        return out
    except Exception as e:  # noqa: BLE001
        return f"<unavailable: {e}>"


def _file_sha256(path: Path | None) -> str | None:
    if not path or not path.is_file():
        return None
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def _which_or_env(name: str, env_keys: list[str]) -> Path | None:
    for k in env_keys:
        v = os.environ.get(k)
        if v and Path(v).exists():
            return Path(v)
    w = shutil.which(name)
    return Path(w) if w else None


def _cpu_model() -> str:
    if Path("/proc/cpuinfo").exists():
        for line in Path("/proc/cpuinfo").read_text(errors="replace").splitlines():
            if line.lower().startswith("model name"):
                return line.split(":", 1)[1].strip()
    return platform.processor() or "unknown"


def _ram_bytes() -> int | None:
    try:
        if Path("/proc/meminfo").exists():
            for line in Path("/proc/meminfo").read_text().splitlines():
                if line.startswith("MemTotal:"):
                    kb = int(line.split()[1])
                    return kb * 1024
    except Exception:  # noqa: BLE001
        return None
    return None


def _disk(path: Path) -> dict:
    usage = shutil.disk_usage(path)
    return {
        "path": str(path),
        "totalBytes": usage.total,
        "usedBytes": usage.used,
        "freeBytes": usage.free,
        "freeGiB": round(usage.free / (1024**3), 2),
    }


def _git_commit(repo: Path) -> str | None:
    out = _run(["git", "-C", str(repo), "rev-parse", "HEAD"])
    if out.startswith("<unavailable") or "fatal" in out.lower():
        return None
    return out.splitlines()[0].strip() or None


def capture(
    repo: Path,
    *,
    start_iso: str | None = None,
    end_iso: str | None = None,
    extra: dict | None = None,
) -> dict:
    processor = _which_or_env(
        "processor",
        ["GEOFORGE_PROCESSOR"],
    )
    # Prefer staged sidecars / release binaries relative to repo
    candidates = [
        repo / "apps/desktop/src-tauri/binaries/processor",
        repo / "target/release/processor",
        repo / "target/debug/processor",
    ]
    for c in candidates:
        if c.is_file():
            processor = c
            break

    top_rebuild = None
    for c in [
        os.environ.get("GEOFORGE_TOP_REBUILD"),
        str(repo / "apps/desktop/src-tauri/binaries/top_rebuild"),
        str(repo / "target/release/top_rebuild"),
        shutil.which("top_rebuild"),
    ]:
        if c and Path(c).is_file():
            top_rebuild = Path(c)
            break

    basisu = None
    for c in [
        os.environ.get("GEOFORGE_BASISU"),
        str(repo / "apps/desktop/src-tauri/binaries/basisu"),
        shutil.which("basisu"),
    ]:
        if c and Path(c).is_file():
            basisu = Path(c)
            break

    converter = None
    for c in [
        os.environ.get("GEOFORGE_3DTILE"),
        str(repo / "apps/desktop/src-tauri/resources/bin/_3dtile"),
        str(repo / "apps/desktop/src-tauri/resources/bin/run.sh"),
        shutil.which("_3dtile"),
    ]:
        if c and Path(c).is_file():
            converter = Path(c)
            break

    cesium_pkg = repo / "apps/desktop/package.json"
    cesium_ver = None
    if cesium_pkg.is_file():
        try:
            pkg = json.loads(cesium_pkg.read_text())
            deps = {**pkg.get("dependencies", {}), **pkg.get("devDependencies", {})}
            cesium_ver = deps.get("cesium")
        except Exception:  # noqa: BLE001
            cesium_ver = None

    now = datetime.now(timezone.utc).isoformat()
    env = {
        "schema": "geoforge.acceptance.environment.v1",
        "gitCommit": _git_commit(repo),
        "os": {
            "system": platform.system(),
            "release": platform.release(),
            "version": platform.version(),
            "machine": platform.machine(),
            "platform": platform.platform(),
        },
        "cpu": _cpu_model(),
        "ramBytes": _ram_bytes(),
        "gpu": _run(["bash", "-lc", "lspci 2>/dev/null | rg -i 'vga|3d|display' || true"]),
        "disk": _disk(Path("/workspace") if Path("/workspace").exists() else Path.cwd()),
        "rustVersion": _run(["rustc", "--version"]),
        "nodeVersion": _run(["node", "--version"]),
        "pythonVersion": sys.version.split()[0],
        "converter": {
            "path": str(converter) if converter else None,
            "sha256": _file_sha256(converter),
        },
        "basisu": {
            "path": str(basisu) if basisu else None,
            "sha256": _file_sha256(basisu),
            "version": _run([str(basisu), "-version"]) if basisu else None,
        },
        "processor": {
            "path": str(processor) if processor else None,
            "sha256": _file_sha256(processor),
        },
        "topRebuild": {
            "path": str(top_rebuild) if top_rebuild else None,
            "sha256": _file_sha256(top_rebuild),
        },
        "cesiumVersion": cesium_ver,
        "testStartTime": start_iso or now,
        "testEndTime": end_iso,
        "capturedAt": now,
        "hostname": platform.node(),
        "envOverrides": {
            k: os.environ.get(k)
            for k in [
                "GEOFORGE_ACCEPTANCE_ROOT",
                "GEOFORGE_PROCESSOR",
                "GEOFORGE_TOP_REBUILD",
                "GEOFORGE_3DTILE",
                "GEOFORGE_BASISU",
                "GEOFORGE_REBUILD_ENGINE",
            ]
            if os.environ.get(k)
        },
    }
    if extra:
        env["extra"] = extra
    return env


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--repo",
        type=Path,
        default=Path(__file__).resolve().parents[2],
    )
    ap.add_argument("-o", "--output", type=Path, required=True)
    ap.add_argument("--start", default=None)
    ap.add_argument("--end", default=None)
    args = ap.parse_args()
    data = capture(args.repo, start_iso=args.start, end_iso=args.end)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
    print(f"wrote {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
