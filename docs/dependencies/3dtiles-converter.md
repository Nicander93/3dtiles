# 3D Tiles Converter dependency

GeoForge does **not** compile `_3dtile` in the product workspace.

| Field | Value |
| --- | --- |
| Runtime source | https://github.com/Nicander93/geoforge-converter |
| Upstream lineage | https://github.com/fanvanzh/3dtiles |
| Pin file | [`third_party/3dtiles-converter.json`](../../third_party/3dtiles-converter.json) |
| Fetch script | `apps/desktop/scripts/prepare-converter.ps1` |

## CLI contract

```text
_3dtile.exe -f osgb -i <INPUT> -o <OUTPUT> [-c <CONFIG>] [-v]
```

## Dev

```powershell
powershell -File apps/desktop/scripts/prepare-converter.ps1
# or point at a custom binary:
$env:GEOFORGE_3DTILE = "D:\path\to\_3dtile.exe"
```

## Packaged layout

```text
resources/runtime/converter/
├─ _3dtile.exe
├─ *.dll                  # native dependencies, including MSVC release CRT
├─ osgPlugins-3.6.5/
├─ gdal/
├─ proj/
└─ geoids/   # optional
```

`prepare-converter.ps1` rejects a zip that omits `msvcp140.dll`,
`msvcp140_2.dll`, `vcruntime140.dll`, or `vcruntime140_1.dll`. The desktop
runtime also places the MSVC CRT beside `bin/processor.exe` and
`bin/top_rebuild.exe`; a developer machine's globally installed VC++ runtime
must not be required by the installer.


## basisu / KTX2 (runtime/texture)

Packaged layout:

```text
resources/runtime/
├─ bin/           # processor, top_rebuild, MSVC release CRT (vcruntime140*.dll, …)
└─ texture/       # basisu.exe (+ optional geoforge-texture)
```

`top_rebuild` prepends `runtime/texture` and sibling `runtime/bin` to `PATH` when
spawning `basisu` (encode + unpack), so CRT DLLs resolve without a system-wide
VC++ install. Prefer also shipping CRT **next to** `basisu.exe`, or ensure
`runtime/bin` remains the CRT home that `top_rebuild` adds to `PATH`.

If unpack still fails, rebuild keeps the original `image/ktx2` bytes as opaque
pass-through (no invented pixels) and logs a warning.

Do not use Docker as a product fallback. Do not pin fanvanzh `v0.4` zip as the runtime.
