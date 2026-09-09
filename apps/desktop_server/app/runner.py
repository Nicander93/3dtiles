"""Serial task runner: convert-osgb / rebuild-top / process-tileset via subprocess."""

from __future__ import annotations

import os
import queue
import shutil
import signal
import subprocess
import threading
import time
from pathlib import Path
from typing import Any, Dict, List, Optional

import sys

from . import tasks as taskmod
from .convert_caps import (
    is_keep_mode,
    normalize_texture_mode,
    output_has_ktx2_evidence,
    process_tileset_texture_support,
    texture_mode_support,
)
from .texture_ktx2 import copy_and_process, process_tileset_dir
from .osgb_scan import scan_osgb
from .geo import (
    build_tile_config_json,
    missing_crs_message,
    resolve_effective_geo,
)

CONVERT_BIN = Path(os.environ.get("GEOFORGE_3DTILE", "/workspace/runtime/3dtile-bin/run.sh"))
REBUILD_PY = Path(
    os.environ.get(
        "GEOFORGE_REBUILD_TOP",
        "/workspace/repos/3dtiles/tools/rebuild_top/rebuild_top.py",
    )
)
PYTHON_BIN = Path(
    os.environ.get("GEOFORGE_PYTHON", "/workspace/venv-3dtiles/bin/python")
)


class SerialRunner:
    """Default serial queue: one running task at a time."""

    def __init__(self, store: Optional[taskmod.TaskStore] = None) -> None:
        self.store = store or taskmod.store
        self._cv = threading.Condition()
        self._queue: List[str] = []
        self._current: Optional[str] = None
        self._proc: Optional[subprocess.Popen] = None
        self._stop = False
        self._thread = threading.Thread(target=self._loop, name="geoforge-runner", daemon=True)
        self._thread.start()

    def enqueue(self, task_id: str) -> None:
        with self._cv:
            if task_id not in self._queue:
                self._queue.append(task_id)
                self._cv.notify()

    def cancel(self, task_id: str) -> Dict[str, Any]:
        task = self.store.request_cancel(task_id)
        if not task:
            return {"ok": False, "error": "task not found"}
        with self._cv:
            if task_id in self._queue and task["status"] == "cancelled":
                try:
                    self._queue.remove(task_id)
                except ValueError:
                    pass
            if self._current == task_id and self._proc and self._proc.poll() is None:
                self._terminate_proc(self._proc)
        return {"ok": True, "task": self.store.get(task_id)}

    def _terminate_proc(self, proc: subprocess.Popen) -> None:
        try:
            os.killpg(proc.pid, signal.SIGTERM)
        except (ProcessLookupError, PermissionError, OSError):
            try:
                proc.terminate()
            except Exception:  # noqa: BLE001
                pass
        deadline = time.time() + 8
        while time.time() < deadline and proc.poll() is None:
            time.sleep(0.2)
        if proc.poll() is None:
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except (ProcessLookupError, PermissionError, OSError):
                try:
                    proc.kill()
                except Exception:  # noqa: BLE001
                    pass

    def _loop(self) -> None:
        while not self._stop:
            with self._cv:
                while not self._queue and not self._stop:
                    self._cv.wait(timeout=1.0)
                if self._stop:
                    return
                task_id = self._queue.pop(0)
                self._current = task_id
            try:
                self._run_task(task_id)
            except Exception as e:  # noqa: BLE001
                self.store.update(
                    task_id,
                    status="failed",
                    stage="failed",
                    error=str(e),
                    finishedAt=time.time(),
                )
                self.store.append_log(task_id, f"[runner] fatal: {e}")
            finally:
                with self._cv:
                    self._current = None
                    self._proc = None

    def _run_task(self, task_id: str) -> None:
        task = self.store.get(task_id)
        if not task:
            return
        if task.get("cancelRequested") or task["status"] == "cancelled":
            self.store.update(task_id, status="cancelled", stage="cancelled", finishedAt=time.time())
            return

        self.store.update(
            task_id,
            status="running",
            stage="scan",
            startedAt=time.time(),
            error=None,
        )
        op = task["operation"]
        input_path = (task.get("input") or {}).get("path") or ""
        output_path = (task.get("output") or {}).get("path") or ""
        options = task.get("options") or {}

        if op == "convert-osgb":
            self._run_convert_osgb(task_id, input_path, output_path, options)
        elif op == "rebuild-top":
            self._run_rebuild_top(task_id, input_path, output_path, options)
        elif op == "process-tileset":
            self._run_process_tileset(task_id, input_path, output_path, options)
        else:
            self.store.update(
                task_id,
                status="failed",
                stage="failed",
                error=f"Unknown operation: {op}",
                finishedAt=time.time(),
            )

    def _check_cancel(self, task_id: str) -> bool:
        t = self.store.get(task_id)
        return bool(t and t.get("cancelRequested"))

    def _run_subprocess(
        self,
        task_id: str,
        cmd: List[str],
        cwd: Optional[str] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> int:
        self.store.append_log(task_id, f"$ {' '.join(cmd)}")
        run_env = os.environ.copy()
        if env:
            run_env.update(env)
        proc = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            cwd=cwd,
            env=run_env,
            start_new_session=True,
            bufsize=1,
        )
        with self._cv:
            self._proc = proc
        self.store.update(task_id, pid=proc.pid)

        assert proc.stdout is not None
        for line in proc.stdout:
            line = line.rstrip("\n")
            self.store.append_log(task_id, line)
            if self._check_cancel(task_id):
                self._terminate_proc(proc)
                break
        rc = proc.wait()
        with self._cv:
            if self._proc is proc:
                self._proc = None
        return rc

    def _run_convert_osgb(
        self,
        task_id: str,
        input_path: str,
        output_path: str,
        options: Dict[str, Any],
    ) -> None:
        self.store.set_stage(task_id, "scan", {"message": "Validating OSGB root"})
        scan = scan_osgb(input_path)
        self.store.append_log(task_id, f"[scan] valid={scan.get('valid')} tiles={scan.get('summary', {}).get('tileCount')}")
        if not scan.get("valid"):
            self.store.update(
                task_id,
                status="failed",
                stage="scan",
                error="; ".join(scan.get("errors") or ["invalid OSGB"]),
                finishedAt=time.time(),
            )
            return
        if self._check_cancel(task_id):
            self.store.update(task_id, status="cancelled", stage="cancelled", finishedAt=time.time())
            return

        root = scan["path"]
        out = Path(output_path)
        out.mkdir(parents=True, exist_ok=True)

        # P5: resolve effective CRS / origin; block geographic export when missing
        effective = resolve_effective_geo(scan, options)
        self.store.append_log(
            task_id,
            f"[geo] effectiveCrs={effective.get('effectiveCrs')!r} "
            f"origin={(effective.get('effectiveOrigin') or {}).get('text')!r} "
            f"unitHint={effective.get('unitHint')!r} "
            f"geographicExport={effective.get('geographicExport')}",
        )
        crs_err = missing_crs_message(effective)
        if crs_err:
            self.store.set_stage(
                task_id,
                "scan",
                {
                    "message": "缺 CRS，阻止地理导出",
                    "geo": effective,
                    "failed": True,
                },
            )
            self.store.append_log(task_id, f"[geo] FAIL: {crs_err}")
            self.store.update(
                task_id,
                status="failed",
                stage="scan",
                error=crs_err,
                finishedAt=time.time(),
                progress={"geo": effective},
            )
            return
        self.store.set_stage(
            task_id,
            "scan",
            {"message": "OSGB validated", "geo": effective},
        )

        self.store.set_stage(task_id, "convert", {"message": "OSGB → 3D Tiles"})
        if not CONVERT_BIN.is_file():
            self.store.update(
                task_id,
                status="failed",
                stage="convert",
                error=f"Converter not found: {CONVERT_BIN}",
                finishedAt=time.time(),
            )
            return

        texture = options.get("texture") or {}
        tex_mode = normalize_texture_mode(texture.get("mode"))
        tex_info = texture_mode_support(tex_mode)
        want_ktx2 = not is_keep_mode(tex_mode)
        use_postprocess = bool(want_ktx2 and tex_info.get("postprocess"))
        use_native = bool(want_ktx2 and (tex_info.get("cliFlags") or []) and not use_postprocess)

        # Fail early when KTX2 requested but neither native flag nor basisu postprocess
        if want_ktx2 and not tex_info.get("supported"):
            self.store.set_stage(
                task_id,
                "texture",
                {
                    "message": f"mode={tex_mode} unsupported",
                    "textureMode": tex_mode,
                    "failed": True,
                },
            )
            reason = tex_info.get("reason") or "KTX2 not supported by convert binary"
            self.store.append_log(task_id, f"[texture] FAIL: {reason}")
            self.store.update(
                task_id,
                status="failed",
                stage="texture",
                error=reason,
                finishedAt=time.time(),
            )
            return

        if use_postprocess:
            self.store.append_log(
                task_id,
                f"[texture] will post-process with basisu after convert (mode={tex_mode})",
            )

        cmd = [str(CONVERT_BIN), "-f", "osgb", "-i", root, "-o", str(out)]
        # optional flags from options.convert
        conv = options.get("convert") or {}
        if conv.get("verbose"):
            cmd.append("-v")
        # Explicit convert.config wins; else build -c from geo overrides / ENU
        if conv.get("config"):
            cmd.extend(["-c", str(conv["config"])])
            self.store.append_log(task_id, f"[convert] using options.convert.config")
        else:
            cfg_json, geo_notes = build_tile_config_json(effective)
            for n in geo_notes:
                self.store.append_log(task_id, f"[geo] {n}")
            if cfg_json:
                cmd.extend(["-c", cfg_json])
                self.store.append_log(task_id, f"[convert] -c {cfg_json}")
        # Native KTX2 CLI flags only when binary supports them (not postprocess path)
        if use_native:
            for fl in tex_info.get("cliFlags") or []:
                cmd.append(str(fl))
                self.store.append_log(task_id, f"[convert] texture flag: {fl} (mode={tex_mode})")

        rc = self._run_subprocess(task_id, cmd)
        if self._check_cancel(task_id):
            self.store.update(task_id, status="cancelled", stage="cancelled", finishedAt=time.time())
            return
        if rc != 0:
            self.store.update(
                task_id,
                status="failed",
                stage="convert",
                error=f"convert exited {rc}",
                finishedAt=time.time(),
            )
            return

        tileset = out / "tileset.json"
        if not tileset.is_file():
            self.store.update(
                task_id,
                status="failed",
                stage="check",
                error=f"tileset.json missing under {out}",
                finishedAt=time.time(),
            )
            return

        # optional rebuild after convert
        rebuild = options.get("rebuildTop") or {}
        final_out = out
        if rebuild.get("enabled"):
            rebuild_out = Path(str(out) + "_rebuild")
            if rebuild_out.exists():
                shutil.rmtree(rebuild_out)
            self.store.set_stage(task_id, "rebuild", {"message": "Top-level rebuild"})
            ok = self._invoke_rebuild(task_id, str(out), str(rebuild_out), rebuild)
            if self._check_cancel(task_id):
                self.store.update(task_id, status="cancelled", stage="cancelled", finishedAt=time.time())
                return
            if not ok:
                # _invoke_rebuild already recorded failure (unless cancel — handled above)
                return
            final_out = rebuild_out

        if self._check_cancel(task_id):
            self.store.update(task_id, status="cancelled", stage="cancelled", finishedAt=time.time())
            return

        # Texture stage: keep = skip; native KTX2 verify; or basisu post-process
        if not self._finish_texture_stage(
            task_id,
            final_out,
            tex_mode,
            convert_applied=use_native,
            postprocess=use_postprocess,
        ):
            return

        if self._check_cancel(task_id):
            self.store.update(task_id, status="cancelled", stage="cancelled", finishedAt=time.time())
            return

        self.store.set_stage(task_id, "check", {"message": "Verifying output"})
        if not (final_out / "tileset.json").is_file():
            self.store.update(
                task_id,
                status="failed",
                stage="check",
                error="final tileset.json missing",
                finishedAt=time.time(),
            )
            return

        tcur = self.store.get(task_id) or {}
        art = self.store.add_artifact(
            task_id,
            str(final_out),
            kind="3dtiles",
            label=tcur.get("taskName") or final_out.name,
        )
        self.store.append_log(task_id, f"[done] artifact={art['path']}")
        self.store.update(
            task_id,
            status="succeeded",
            stage="done",
            finishedAt=time.time(),
            progress={
                "artifactId": art["id"],
                "path": str(final_out),
                "textureMode": tex_mode,
                "geo": effective,
            },
        )

    def _invoke_rebuild(
        self,
        task_id: str,
        input_path: str,
        output_path: str,
        rebuild: Dict[str, Any],
    ) -> bool:
        if not REBUILD_PY.is_file():
            self.store.update(
                task_id,
                status="failed",
                stage="rebuild",
                error=f"rebuild_top.py not found: {REBUILD_PY}",
                finishedAt=time.time(),
            )
            return False
        py = str(PYTHON_BIN) if PYTHON_BIN.is_file() else sys.executable
        try:
            levels = int(rebuild.get("levels") or 1)
        except (TypeError, ValueError):
            levels = 1
        # V1: expose only 1|2 matching rebuild_top.py --levels
        levels = 1 if levels <= 1 else 2
        simplify = float(rebuild.get("simplify") or 0.5)
        texture_scale = float(rebuild.get("textureScale") or rebuild.get("texture_scale") or 0.5)
        cmd = [
            py,
            str(REBUILD_PY),
            "-i",
            input_path,
            "-o",
            output_path,
            "--levels",
            str(levels),
            "--simplify",
            str(simplify),
            "--texture-scale",
            str(texture_scale),
        ]
        rc = self._run_subprocess(task_id, cmd)
        if self._check_cancel(task_id):
            # Do not classify SIGTERM/SIGKILL from cancel as a hard rebuild failure
            return False
        if rc != 0:
            self.store.update(
                task_id,
                status="failed",
                stage="rebuild",
                error=f"rebuild-top exited {rc}",
                finishedAt=time.time(),
            )
            return False
        if not (Path(output_path) / "tileset.json").is_file():
            if self._check_cancel(task_id):
                return False
            self.store.update(
                task_id,
                status="failed",
                stage="rebuild",
                error="rebuild output missing tileset.json",
                finishedAt=time.time(),
            )
            return False
        return True

    def _run_rebuild_top(
        self,
        task_id: str,
        input_path: str,
        output_path: str,
        options: Dict[str, Any],
    ) -> None:
        self.store.set_stage(task_id, "scan", {"message": "Checking tileset input"})
        inp = Path(input_path)
        tileset = inp / "tileset.json" if inp.is_dir() else inp
        if not tileset.is_file():
            self.store.update(
                task_id,
                status="failed",
                stage="scan",
                error=f"tileset.json not found at {tileset}",
                finishedAt=time.time(),
            )
            return
        in_dir = str(tileset.parent)
        out = Path(output_path)
        if out.exists():
            # rebuild_top expects new dir
            shutil.rmtree(out)
        rebuild = options.get("rebuildTop") or options
        self.store.set_stage(task_id, "rebuild", {"message": "Top-level rebuild"})
        if not self._invoke_rebuild(task_id, in_dir, str(out), rebuild):
            if self._check_cancel(task_id):
                self.store.update(task_id, status="cancelled", stage="cancelled", finishedAt=time.time())
            return
        if self._check_cancel(task_id):
            self.store.update(task_id, status="cancelled", stage="cancelled", finishedAt=time.time())
            return
        tex = options.get("texture") or {}
        tex_mode = normalize_texture_mode(tex.get("mode"))
        # keep=skip; KTX2 via basisu post-process when available
        if is_keep_mode(tex_mode):
            self.store.set_stage(
                task_id,
                "texture",
                {"message": "skipped (keep)", "skipped": True, "textureMode": "keep"},
            )
            self.store.append_log(task_id, "[texture] mode=keep — skipped")
        else:
            info = process_tileset_texture_support(tex_mode)
            if not info.get("supported"):
                reason = info.get("reason") or f"KTX2 mode={tex_mode} unsupported on rebuild-top"
                self.store.set_stage(
                    task_id,
                    "texture",
                    {"message": f"mode={tex_mode} unsupported", "textureMode": tex_mode, "failed": True},
                )
                self.store.append_log(task_id, f"[texture] FAIL: {reason}")
                self.store.update(
                    task_id,
                    status="failed",
                    stage="texture",
                    error=reason,
                    finishedAt=time.time(),
                )
                return
            if not self._finish_texture_stage(
                task_id, out, tex_mode, convert_applied=False, postprocess=True
            ):
                return
        self.store.set_stage(task_id, "check", {"message": "Verifying output"})
        art = self.store.add_artifact(task_id, str(out), kind="3dtiles", label=out.name)
        self.store.update(
            task_id,
            status="succeeded",
            stage="done",
            finishedAt=time.time(),
            progress={"artifactId": art["id"], "path": str(out)},
        )

    def _finish_texture_stage(
        self,
        task_id: str,
        out_dir: Path,
        tex_mode: str,
        convert_applied: bool = False,
        postprocess: bool = False,
    ) -> bool:
        """Mark texture stage. keep=skipped; native verify or basisu post-process. Returns False on fail."""
        if is_keep_mode(tex_mode):
            self.store.set_stage(
                task_id,
                "texture",
                {"message": "skipped (keep)", "skipped": True, "textureMode": "keep"},
            )
            self.store.append_log(task_id, "[texture] mode=keep — skipped")
            return True

        self.store.set_stage(
            task_id,
            "texture",
            {
                "message": (
                    f"post-process basisu mode={tex_mode}"
                    if postprocess
                    else f"verifying mode={tex_mode}"
                ),
                "textureMode": tex_mode,
                "postprocess": bool(postprocess),
            },
        )

        if postprocess:
            try:
                stats = process_tileset_dir(
                    out_dir,
                    mode=tex_mode,
                    log=lambda m: self.store.append_log(task_id, m),
                )
            except Exception as e:  # noqa: BLE001
                reason = f"basisu post-process failed: {e}"
                self.store.append_log(task_id, f"[texture] FAIL: {reason}")
                self.store.update(
                    task_id,
                    status="failed",
                    stage="texture",
                    error=reason,
                    finishedAt=time.time(),
                )
                return False
            self.store.append_log(
                task_id,
                f"[texture] postprocess filesConverted={stats.get('filesConverted')} "
                f"texturesConverted={stats.get('texturesConverted')} errors={len(stats.get('errors') or [])}",
            )
            if stats.get("errors"):
                for err in (stats.get("errors") or [])[:10]:
                    self.store.append_log(task_id, f"[texture] postprocess error: {err}")
            if not stats.get("texturesConverted"):
                reason = (
                    f"KTX2 mode={tex_mode} post-process ran but converted 0 textures under {out_dir}"
                )
                self.store.append_log(task_id, f"[texture] FAIL: {reason}")
                self.store.update(
                    task_id,
                    status="failed",
                    stage="texture",
                    error=reason,
                    finishedAt=time.time(),
                )
                return False
        elif not convert_applied:
            info = texture_mode_support(tex_mode)
            reason = info.get("reason") or f"KTX2 mode={tex_mode} was not applied"
            self.store.append_log(task_id, f"[texture] FAIL: {reason}")
            self.store.update(
                task_id,
                status="failed",
                stage="texture",
                error=reason,
                finishedAt=time.time(),
            )
            return False

        evidence = output_has_ktx2_evidence(out_dir)
        self.store.append_log(
            task_id,
            f"[texture] evidence found={evidence.get('found')} "
            f"ktx2Files={evidence.get('ktx2Files')} basisuMentions={evidence.get('basisuMentions')} "
            f"samples={evidence.get('samples')}",
        )
        if not evidence.get("found"):
            reason = (
                f"KTX2 mode={tex_mode} was requested, "
                f"but no KTX2 / KHR_texture_basisu evidence found under {out_dir}. "
                "Refusing to mark succeeded."
            )
            self.store.append_log(task_id, f"[texture] FAIL: {reason}")
            self.store.update(
                task_id,
                status="failed",
                stage="texture",
                error=reason,
                finishedAt=time.time(),
            )
            return False

        self.store.set_stage(
            task_id,
            "texture",
            {
                "message": f"KTX2 applied ({tex_mode}"
                + (" via basisu postprocess)" if postprocess else ")"),
                "textureMode": tex_mode,
                "evidence": evidence,
                "postprocess": bool(postprocess),
            },
        )
        self.store.append_log(task_id, f"[texture] OK mode={tex_mode}")
        return True

    def _run_process_tileset(
        self,
        task_id: str,
        input_path: str,
        output_path: str,
        options: Dict[str, Any],
    ) -> None:
        rebuild = options.get("rebuildTop") or {}
        texture = options.get("texture") or {}
        tex_mode = normalize_texture_mode(texture.get("mode"))
        want_rebuild = bool(rebuild.get("enabled"))
        want_texture = not is_keep_mode(tex_mode)

        if not want_rebuild and not want_texture:
            self.store.update(
                task_id,
                status="failed",
                stage="scan",
                error="process-tileset requires rebuildTop.enabled and/or texture.mode != keep",
                finishedAt=time.time(),
            )
            return

        self.store.set_stage(task_id, "scan", {"message": "Checking tileset input"})
        inp = Path(input_path)
        tileset = inp / "tileset.json" if inp.is_dir() else inp
        if not tileset.is_file():
            self.store.update(
                task_id,
                status="failed",
                stage="scan",
                error=f"tileset.json not found at {tileset}",
                finishedAt=time.time(),
            )
            return

        if want_texture:
            info = process_tileset_texture_support(tex_mode)
            if not info.get("supported"):
                reason = info.get("reason") or f"KTX2 mode={tex_mode} unsupported on process-tileset"
                self.store.set_stage(
                    task_id,
                    "texture",
                    {"message": f"mode={tex_mode} unsupported", "textureMode": tex_mode, "failed": True},
                )
                self.store.append_log(task_id, f"[texture] FAIL: {reason}")
                if want_rebuild:
                    self.store.append_log(
                        task_id,
                        "[texture] Note: rebuildTop was also requested but task aborted before rebuild "
                        "because texture.mode cannot be satisfied (no silent partial success).",
                    )
                self.store.update(
                    task_id,
                    status="failed",
                    stage="texture",
                    error=reason,
                    finishedAt=time.time(),
                )
                return

        out = Path(output_path)
        if want_rebuild:
            # rebuild first (with keep), then optional KTX2 post-process on rebuild output
            opts = dict(options)
            opts["texture"] = {"mode": "keep"}
            self._run_rebuild_top(task_id, input_path, output_path, opts)
            # If rebuild path already finished task as succeeded/failed, re-open for texture
            cur = self.store.get(task_id) or {}
            if cur.get("status") == "failed" or cur.get("status") == "cancelled":
                return
            if want_texture:
                # clear succeeded so we can continue texture stage
                self.store.update(task_id, status="running", finishedAt=None)
                if not self._finish_texture_stage(
                    task_id, out, tex_mode, convert_applied=False, postprocess=True
                ):
                    return
                if self._check_cancel(task_id):
                    self.store.update(task_id, status="cancelled", stage="cancelled", finishedAt=time.time())
                    return
                art = self.store.add_artifact(task_id, str(out), kind="3dtiles", label=out.name)
                self.store.update(
                    task_id,
                    status="succeeded",
                    stage="done",
                    finishedAt=time.time(),
                    progress={"artifactId": art["id"], "path": str(out), "textureMode": tex_mode},
                )
            return

        # texture-only process-tileset: copy then post-process
        if out.exists():
            shutil.rmtree(out)
        self.store.append_log(task_id, f"[texture] copy {tileset.parent} -> {out}")
        shutil.copytree(tileset.parent, out)
        if not self._finish_texture_stage(
            task_id, out, tex_mode, convert_applied=False, postprocess=True
        ):
            return
        if self._check_cancel(task_id):
            self.store.update(task_id, status="cancelled", stage="cancelled", finishedAt=time.time())
            return
        art = self.store.add_artifact(task_id, str(out), kind="3dtiles", label=out.name)
        self.store.update(
            task_id,
            status="succeeded",
            stage="done",
            finishedAt=time.time(),
            progress={"artifactId": art["id"], "path": str(out), "textureMode": tex_mode},
        )


runner = SerialRunner()
