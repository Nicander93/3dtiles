# 3D Tiles Converter Engine

Isolated upstream OSGB → 3D Tiles converter (`_3dtile`).

This directory is a **separate Cargo workspace** and is excluded from the
product workspace default build so OSG/GDAL/vcpkg are not pulled into
ordinary `cargo build -p processor` runs.

## Build (Windows x64 Release)

Requires MSVC, CMake, and vcpkg as documented historically in the root README.

```bash
cd engines/3dtiles-converter
cargo build --release
```

Binary name remains `_3dtile` / `_3dtile.exe`.

## Origin

Inherited from the Nicander93/3dtiles / CesiumLab-related converter lineage.
Preserve existing LICENSE and copyright notices in this tree.
Product UI and Processor invoke this engine as an external process.
