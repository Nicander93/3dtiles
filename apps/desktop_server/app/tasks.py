"""In-memory + SQLite task / artifact store for GeoForge 3D."""

from __future__ import annotations

import json
import os
import sqlite3
import threading
import time
import uuid
from pathlib import Path
from typing import Any, Dict, List, Optional


def _default_db_path() -> Path:
    env = os.environ.get("GEOFORGE_DB")
    if env:
        return Path(env).expanduser()
    candidates = [
        Path("/workspace/repos/3dtiles/.geoforge/tasks.db"),
        Path.home() / ".geoforge" / "tasks.db",
    ]
    for c in candidates:
        try:
            c.parent.mkdir(parents=True, exist_ok=True)
            return c
        except OSError:
            continue
    p = Path("/tmp/geoforge_tasks.db")
    p.parent.mkdir(parents=True, exist_ok=True)
    return p


def _default_log_dir(db_path: Path) -> Path:
    return db_path.parent / "logs"


class TaskStore:
    def __init__(self, db_path: Optional[Path] = None) -> None:
        self.db_path = Path(db_path) if db_path else _default_db_path()
        self.db_path.parent.mkdir(parents=True, exist_ok=True)
        self.log_dir = _default_log_dir(self.db_path)
        self.log_dir.mkdir(parents=True, exist_ok=True)
        self._lock = threading.RLock()
        self._mem: Dict[str, Dict[str, Any]] = {}
        self._init_db()
        self._load_from_db()

    def _connect(self) -> sqlite3.Connection:
        conn = sqlite3.connect(str(self.db_path), check_same_thread=False)
        conn.row_factory = sqlite3.Row
        return conn

    def log_path(self, task_id: str) -> Path:
        return self.log_dir / f"{task_id}.log"

    def _enrich(self, task: Dict[str, Any]) -> Dict[str, Any]:
        out = dict(task)
        out["logPath"] = str(self.log_path(out["id"]))
        return out

    def _init_db(self) -> None:
        with self._lock:
            conn = self._connect()
            try:
                conn.executescript(
                    """
                    CREATE TABLE IF NOT EXISTS tasks (
                        id TEXT PRIMARY KEY,
                        operation TEXT NOT NULL,
                        status TEXT NOT NULL,
                        task_name TEXT,
                        input_path TEXT,
                        output_path TEXT,
                        options_json TEXT,
                        stage TEXT,
                        progress_json TEXT,
                        log_text TEXT,
                        error TEXT,
                        created_at REAL,
                        updated_at REAL,
                        started_at REAL,
                        finished_at REAL,
                        pid INTEGER,
                        cancel_requested INTEGER DEFAULT 0
                    );
                    CREATE TABLE IF NOT EXISTS artifacts (
                        id TEXT PRIMARY KEY,
                        task_id TEXT,
                        path TEXT NOT NULL,
                        kind TEXT,
                        label TEXT,
                        created_at REAL,
                        available INTEGER DEFAULT 1
                    );
                    """
                )
                conn.commit()
            finally:
                conn.close()

    def _load_from_db(self) -> None:
        with self._lock:
            conn = self._connect()
            try:
                rows = conn.execute("SELECT * FROM tasks ORDER BY created_at DESC").fetchall()
                for row in rows:
                    task = self._row_to_task(row)
                    # On restart, running tasks become interrupted
                    if task["status"] in ("running", "cancelling", "queued"):
                        if task["status"] == "queued":
                            pass  # keep queued? safer mark interrupted for previous session
                        task["status"] = "interrupted"
                        task["stage"] = task.get("stage") or "interrupted"
                        task["updated_at"] = time.time()
                        self._mem[task["id"]] = task
                        self._upsert_db(task)
                    else:
                        self._mem[task["id"]] = task
            finally:
                conn.close()

    @staticmethod
    def _row_to_task(row: sqlite3.Row) -> Dict[str, Any]:
        d = dict(row)
        options = {}
        progress = {}
        try:
            options = json.loads(d.pop("options_json") or "{}")
        except json.JSONDecodeError:
            options = {}
        try:
            progress = json.loads(d.pop("progress_json") or "{}")
        except json.JSONDecodeError:
            progress = {}
        return {
            "id": d["id"],
            "operation": d["operation"],
            "status": d["status"],
            "taskName": d.get("task_name") or "",
            "input": {"path": d.get("input_path") or ""},
            "output": {"path": d.get("output_path") or ""},
            "options": options,
            "stage": d.get("stage") or "",
            "progress": progress,
            "log": d.get("log_text") or "",
            "error": d.get("error"),
            "createdAt": d.get("created_at"),
            "updatedAt": d.get("updated_at"),
            "startedAt": d.get("started_at"),
            "finishedAt": d.get("finished_at"),
            "pid": d.get("pid"),
            "cancelRequested": bool(d.get("cancel_requested")),
        }

    def _upsert_db(self, task: Dict[str, Any]) -> None:
        conn = self._connect()
        try:
            conn.execute(
                """
                INSERT INTO tasks (
                    id, operation, status, task_name, input_path, output_path,
                    options_json, stage, progress_json, log_text, error,
                    created_at, updated_at, started_at, finished_at, pid, cancel_requested
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(id) DO UPDATE SET
                    operation=excluded.operation,
                    status=excluded.status,
                    task_name=excluded.task_name,
                    input_path=excluded.input_path,
                    output_path=excluded.output_path,
                    options_json=excluded.options_json,
                    stage=excluded.stage,
                    progress_json=excluded.progress_json,
                    log_text=excluded.log_text,
                    error=excluded.error,
                    created_at=excluded.created_at,
                    updated_at=excluded.updated_at,
                    started_at=excluded.started_at,
                    finished_at=excluded.finished_at,
                    pid=excluded.pid,
                    cancel_requested=excluded.cancel_requested
                """,
                (
                    task["id"],
                    task["operation"],
                    task["status"],
                    task.get("taskName") or "",
                    (task.get("input") or {}).get("path") or "",
                    (task.get("output") or {}).get("path") or "",
                    json.dumps(task.get("options") or {}, ensure_ascii=False),
                    task.get("stage") or "",
                    json.dumps(task.get("progress") or {}, ensure_ascii=False),
                    task.get("log") or "",
                    task.get("error"),
                    task.get("createdAt"),
                    task.get("updatedAt"),
                    task.get("startedAt"),
                    task.get("finishedAt"),
                    task.get("pid"),
                    1 if task.get("cancelRequested") else 0,
                ),
            )
            conn.commit()
        finally:
            conn.close()

    def create_task(
        self,
        operation: str,
        input_path: str,
        output_path: str,
        options: Optional[Dict[str, Any]] = None,
        task_name: str = "",
    ) -> Dict[str, Any]:
        now = time.time()
        task_id = f"task-{uuid.uuid4().hex[:12]}"
        task: Dict[str, Any] = {
            "id": task_id,
            "operation": operation,
            "status": "queued",
            "taskName": task_name or task_id,
            "input": {"path": input_path},
            "output": {"path": output_path},
            "options": options or {},
            "stage": "queued",
            "progress": {},
            "log": "",
            "logPath": "",
            "error": None,
            "createdAt": now,
            "updatedAt": now,
            "startedAt": None,
            "finishedAt": None,
            "pid": None,
            "cancelRequested": False,
        }
        task["logPath"] = str(self.log_path(task_id))
        with self._lock:
            self._mem[task_id] = task
            self._upsert_db(task)
        return self._enrich(task)

    def get(self, task_id: str) -> Optional[Dict[str, Any]]:
        with self._lock:
            t = self._mem.get(task_id)
            return self._enrich(t) if t else None

    def list_tasks(self) -> List[Dict[str, Any]]:
        with self._lock:
            items = sorted(self._mem.values(), key=lambda x: x.get("createdAt") or 0, reverse=True)
            return [self._enrich(t) for t in items]

    def update(self, task_id: str, **fields: Any) -> Optional[Dict[str, Any]]:
        with self._lock:
            task = self._mem.get(task_id)
            if not task:
                return None
            task.update(fields)
            task["updatedAt"] = time.time()
            task["logPath"] = str(self.log_path(task_id))
            self._upsert_db(task)
            return self._enrich(task)

    def append_log(self, task_id: str, line: str) -> None:
        with self._lock:
            task = self._mem.get(task_id)
            if not task:
                return
            log = task.get("log") or ""
            if log and not log.endswith("\n"):
                log += "\n"
            entry = line if line.endswith("\n") else line + "\n"
            log += entry.rstrip("\n")
            # keep last ~200KB in DB/memory
            if len(log) > 200_000:
                log = log[-200_000:]
            task["log"] = log
            task["logPath"] = str(self.log_path(task_id))
            task["updatedAt"] = time.time()
            try:
                lp = self.log_path(task_id)
                lp.parent.mkdir(parents=True, exist_ok=True)
                with open(lp, "a", encoding="utf-8") as f:
                    f.write(entry if entry.endswith("\n") else entry + "\n")
            except OSError:
                pass
            self._upsert_db(task)

    def get_logs(self, task_id: str, tail: int = 500) -> Optional[Dict[str, Any]]:
        task = self.get(task_id)
        if not task:
            return None
        text = task.get("log") or ""
        lp = self.log_path(task_id)
        if lp.is_file():
            try:
                text = lp.read_text(encoding="utf-8", errors="replace")
            except OSError:
                pass
        lines = text.splitlines()
        if tail and len(lines) > tail:
            lines = lines[-tail:]
        return {
            "taskId": task_id,
            "logPath": str(lp),
            "lines": lines,
            "log": "\n".join(lines),
        }

    def set_stage(self, task_id: str, stage: str, progress: Optional[Dict[str, Any]] = None) -> None:
        fields: Dict[str, Any] = {"stage": stage}
        if progress is not None:
            with self._lock:
                cur = self._mem.get(task_id) or {}
                merged = dict(cur.get("progress") or {})
                merged.update(progress)
                fields["progress"] = merged
        self.update(task_id, **fields)

    def request_cancel(self, task_id: str) -> Optional[Dict[str, Any]]:
        with self._lock:
            task = self._mem.get(task_id)
            if not task:
                return None
            if task["status"] == "queued":
                task["status"] = "cancelled"
                task["stage"] = "cancelled"
                task["cancelRequested"] = True
                task["finishedAt"] = time.time()
                task["updatedAt"] = time.time()
                self._upsert_db(task)
                return self._enrich(task)
            if task["status"] == "running":
                task["status"] = "cancelling"
                task["cancelRequested"] = True
                task["updatedAt"] = time.time()
                self._upsert_db(task)
                return self._enrich(task)
            return self._enrich(task)

    def add_artifact(
        self,
        task_id: str,
        path: str,
        kind: str = "3dtiles",
        label: str = "",
    ) -> Dict[str, Any]:
        art_id = f"art-{uuid.uuid4().hex[:12]}"
        now = time.time()
        available = Path(path).exists()
        row = {
            "id": art_id,
            "taskId": task_id,
            "path": path,
            "kind": kind,
            "label": label or Path(path).name,
            "createdAt": now,
            "available": available,
        }
        with self._lock:
            conn = self._connect()
            try:
                conn.execute(
                    """
                    INSERT INTO artifacts (id, task_id, path, kind, label, created_at, available)
                    VALUES (?, ?, ?, ?, ?, ?, ?)
                    """,
                    (
                        art_id,
                        task_id,
                        path,
                        kind,
                        row["label"],
                        now,
                        1 if available else 0,
                    ),
                )
                conn.commit()
            finally:
                conn.close()
        return row

    def get_artifact(self, art_id: str) -> Optional[Dict[str, Any]]:
        with self._lock:
            conn = self._connect()
            try:
                row = conn.execute(
                    "SELECT * FROM artifacts WHERE id = ?", (art_id,)
                ).fetchone()
                if not row:
                    return None
                return self._artifact_row(row)
            finally:
                conn.close()

    @staticmethod
    def _artifact_row(r: sqlite3.Row) -> Dict[str, Any]:
        p = r["path"]
        root = Path(p) if p else None
        available = bool(root and root.exists())
        has_tileset = bool(root and (root / "tileset.json").is_file())
        created = r["created_at"]
        return {
            "id": r["id"],
            "taskId": r["task_id"],
            "path": p,
            "kind": r["kind"],
            "label": r["label"],
            "createdAt": created,
            "created_at": created,
            "available": available,
            "has_tileset": has_tileset,
            "hasTileset": has_tileset,
        }

    def list_artifacts(self) -> List[Dict[str, Any]]:
        with self._lock:
            conn = self._connect()
            try:
                rows = conn.execute(
                    "SELECT * FROM artifacts ORDER BY created_at DESC"
                ).fetchall()
                return [self._artifact_row(r) for r in rows]
            finally:
                conn.close()


# Singleton used by API + runner
store = TaskStore()
