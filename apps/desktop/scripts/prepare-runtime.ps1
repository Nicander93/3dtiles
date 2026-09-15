# Collect Windows x64 product runtime (converter from Release + product bins).
# Usage:
#   powershell -File apps/desktop/scripts/prepare-runtime.ps1 [-OutDir path] [-SkipBuild]
# Output layout:
#   <OutDir>/converter/_3dtile.exe + DLLs + osgPlugins-3.6.5 / gdal / proj
#   <OutDir>/bin/processor.exe, top_rebuild.exe
#   <OutDir>/manifest.json

param(
  [string]$OutDir = "",
  [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..\..")
if (-not $OutDir) {
  $OutDir = Join-Path $RepoRoot "dist\runtime"
}

Write-Host "Repo: $RepoRoot"
Write-Host "Out: $OutDir"

if (-not $SkipBuild) {
  Write-Host "Building product crates..."
  Push-Location $RepoRoot
  cargo build --release -p processor
  if ($LASTEXITCODE -ne 0) { Pop-Location; exit $LASTEXITCODE }
  cargo build --release -p top_rebuild --bin top_rebuild
  if ($LASTEXITCODE -ne 0) { Pop-Location; exit $LASTEXITCODE }
  Pop-Location
}

$ConverterOut = Join-Path $OutDir "converter"
& powershell -File (Join-Path $PSScriptRoot "prepare-converter.ps1") -OutDir $ConverterOut
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$ProductBin = Join-Path $OutDir "bin"
New-Item -ItemType Directory -Force -Path $ProductBin | Out-Null
foreach ($name in @("processor.exe", "top_rebuild.exe")) {
  $src = Join-Path $RepoRoot "target\release\$name"
  if (-not (Test-Path $src)) {
    Write-Error "Missing $src — build product crates first"
  }
  Copy-Item -Force $src (Join-Path $ProductBin $name)
}

$files = Get-ChildItem -Recurse $OutDir -File | ForEach-Object {
  @{ path = $_.FullName.Substring($OutDir.Length).TrimStart('\', '/'); size = $_.Length }
}
$manifest = @{
  createdAt = (Get-Date).ToString("o")
  platform = "windows-x64"
  converterExe = "converter/_3dtile.exe"
  files = $files
  notes = "Converter comes from third_party/3dtiles-converter.json Release; no local OSG/vcpkg build."
}
$manifest | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 (Join-Path $OutDir "manifest.json")
Write-Host "Runtime staged at $OutDir"
