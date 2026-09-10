# desktop_server_py — legacy Python API (reference)

**Not the primary product runtime after Phase 3.**

This tree is the former `apps/desktop_server` FastAPI task/API server, retained as a reference implementation and optional legacy browser+HTTP path.

- Preferred desktop path: Tauri (`apps/desktop`) + Rust `processor` crate.
- KTX2 helper module `app/texture_ktx2.py` remains callable via `tools/texture_ktx2/run.py`.
- To run the legacy server explicitly:

```bash
bash scripts/run_geoforge.sh --legacy-server
# or: PYTHONPATH=tools/experiments/desktop_server_py uvicorn app.main:app --host 127.0.0.1 --port 8787
```

Do not treat this as the V1 install packaging backend.
