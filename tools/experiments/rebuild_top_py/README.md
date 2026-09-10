# rebuild_top_py — algorithm baseline only

**Not release runtime.** This is the Python Proxy / merge prototype kept as a
Phase 3–8 algorithm baseline for experiments and regression comparison.

- Formal V1 top rebuild is the Rust TopRebuild core (`crates/top_rebuild`,
  binary `top_rebuild`).
- Processor default engine is Rust (`top_rebuild`). Python is **not** on the
  formal user path.
- Explicit regression only: set `GEOFORGE_REBUILD_ENGINE=python` (and optionally
  `GEOFORGE_REBUILD_TOP` to this script). Do not ship a user install that
  requires this package as the production rebuild kernel.

```bash
# from repo root — baseline / regression only
python tools/experiments/rebuild_top_py/rebuild_top.py -i <tileset_dir> -o <out> --levels 1 -v

# Processor formal path (default):
#   cargo build -p top_rebuild --bin top_rebuild -p processor
#   ./target/debug/processor process-tileset -i <tileset> -o <out> --rebuild-top
```

Env:
- `GEOFORGE_TOP_REBUILD` — path to Rust `top_rebuild` binary (Processor default)
- `GEOFORGE_REBUILD_ENGINE=python` — force this baseline
- `GEOFORGE_REBUILD_TOP` — path override for this Python script when engine=python

Requirements: see `requirements.txt`.
