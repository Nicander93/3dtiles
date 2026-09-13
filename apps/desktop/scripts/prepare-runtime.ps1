# Collect Windows x64 Release converter runtime (T05).
# Usage:
#   powershell -File apps/desktop/scripts/prepare-runtime.ps1 [-OutDir path] [-SkipBuild]
# Output layout:
#   <OutDir>/converter/_3dtile.exe + DLLs/plugins/data
#   <OutDir>/manifest.json

param(
  [string]$OutDir = "",
  [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..\..")
$EngineRoot = Join-Path $RepoRoot "engines\3dtiles-converter"
if (-not $OutDir) {
  $OutDir = Join-Path $RepoRoot "dist\runtime"
}

$ConverterOut = Join-Path $OutDir "converter"
New-Item -ItemType Directory -Force -Path $ConverterOut | Out-Null

Write-Host "Repo: $RepoRoot"
Write-Host "Engine: $EngineRoot"
Write-Host "Out: $OutDir"

if (-not $SkipBuild) {
  Write-Host "Building product crates..."
  Push-Location $RepoRoot
  cargo build --release -p processor
  cargo build --release -p top_rebuild --bin top_rebuild
  Pop-Location

  Write-Host "Building converter (_3dtile) — requires MSVC/vcpkg..."
  Push-Location $EngineRoot
  cargo build --release
  Pop-Location
}

$ExeCandidates = @(
  (Join-Path $EngineRoot "target\release\_3dtile.exe"),
  (Join-Path $RepoRoot "target\release\_3dtile.exe")
)
$Exe = $ExeCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $Exe) {
  Write-Error "_3dtile.exe not found. Build engines/3dtiles-converter with vcpkg first."
}

Copy-Item -Force $Exe (Join-Path $ConverterOut "_3dtile.exe")

# Copy adjacent DLLs from the exe directory
$ExeDir = Split-Path $Exe -Parent
Get-ChildItem $ExeDir -Filter *.dll -ErrorAction SilentlyContinue | ForEach-Object {
  Copy-Item -Force $_.FullName $ConverterOut
}

# OSG plugins / GDAL data / geoids — best-effort from engine tree
$ExtraDirs = @(
  @{ Src = (Join-Path $EngineRoot "geoids"); Dst = (Join-Path $ConverterOut "geoids") },
  @{ Src = (Join-Path $EngineRoot "vcpkg_installed\x64-windows\plugins"); Dst = (Join-Path $ConverterOut "osgPlugins") },
  @{ Src = (Join-Path $EngineRoot "vcpkg_installed\x64-windows\share\gdal"); Dst = (Join-Path $ConverterOut "gdal-data") },
  @{ Src = (Join-Path $EngineRoot "vcpkg_installed\x64-windows\share\proj"); Dst = (Join-Path $ConverterOut "proj-data") }
)
foreach ($d in $ExtraDirs) {
  if (Test-Path $d.Src) {
    New-Item -ItemType Directory -Force -Path $d.Dst | Out-Null
    Copy-Item -Recurse -Force (Join-Path $d.Src "*") $d.Dst
    Write-Host "Copied $($d.Src) -> $($d.Dst)"
  } else {
    Write-Warning "Missing optional runtime dir: $($d.Src)"
  }
}

# Product sidecars next to runtime root
$ProductBin = Join-Path $OutDir "bin"
New-Item -ItemType Directory -Force -Path $ProductBin | Out-Null
foreach ($name in @("processor.exe", "top_rebuild.exe")) {
  $src = Join-Path $RepoRoot "target\release\$name"
  if (Test-Path $src) {
    Copy-Item -Force $src (Join-Path $ProductBin $name)
  }
}

$files = Get-ChildItem -Recurse $OutDir -File | ForEach-Object {
  @{ path = $_.FullName.Substring($OutDir.Length).TrimStart('\', '/'); size = $_.Length }
}
$manifest = @{
  createdAt = (Get-Date).ToString("o")
  platform = "windows-x64"
  converterExe = "converter/_3dtile.exe"
  files = $files
  notes = "Move this folder off the repo and clear GEOFORGE_* / PATH to verify standalone convert."
}
$manifest | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 (Join-Path $OutDir "manifest.json")
Write-Host "Runtime staged at $OutDir"
Write-Host "Verify: cd elsewhere; `$env:PATH=''; & '$ConverterOut\_3dtile.exe' --help"
