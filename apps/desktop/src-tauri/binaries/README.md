# Sidecar binaries (Phase 14)

Tauri `bundle.externalBin` expects target-triple–suffixed names at build time, e.g.:

```text
processor-x86_64-unknown-linux-gnu
top_rebuild-x86_64-unknown-linux-gnu
basisu-x86_64-unknown-linux-gnu
```

Do **not** commit large binaries. Stage them before `tauri build`:

```bash
./scripts/release/stage_sidecars.sh
```

At runtime, Tauri strips the triple suffix so the app sees `processor`, `top_rebuild`, `basisu` next to the main executable.

Windows packaging is Phase 18 (`*.exe` + DLL set).
