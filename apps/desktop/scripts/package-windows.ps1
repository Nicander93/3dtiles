# Single Windows packaging entry (T09).
# Fails if any required component is missing.
# Usage: powershell -File apps/desktop/scripts/package-windows.ps1

param(
  [switch]$SkipConverterBuild,
  [switch]$SkipTextureBundle
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..\..")
$AppDir = Resolve-Path (Join-Path $PSScriptRoot "..")
$RuntimeDir = Join-Path $RepoRoot "dist\runtime"
$BundleDir = Join-Path $AppDir "src-tauri\resources\runtime"

Write-Host "=== GeoForge Windows package ==="

# 1) Converter + product runtime
$prepArgs = @("-File", (Join-Path $PSScriptRoot "prepare-runtime.ps1"), "-OutDir", $RuntimeDir)
if ($SkipConverterBuild) { $prepArgs += "-SkipBuild" }
& powershell @prepArgs
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# 2) Stage into Tauri resources
New-Item -ItemType Directory -Force -Path $BundleDir | Out-Null
Copy-Item -Recurse -Force (Join-Path $RuntimeDir "*") $BundleDir

# 3) Texture tool (optional skip if not built yet)
$TextureSrc = Join-Path $RepoRoot "tools\texture_ktx2\dist\geoforge-texture"
$TextureDst = Join-Path $BundleDir "texture"
if (Test-Path $TextureSrc) {
  New-Item -ItemType Directory -Force -Path $TextureDst | Out-Null
  Copy-Item -Recurse -Force (Join-Path $TextureSrc "*") $TextureDst
} elseif (-not $SkipTextureBundle) {
  Write-Warning "geoforge-texture bundle missing at $TextureSrc — packaging will fail checklist"
}

# 4) Sidecars
Push-Location $AppDir
npm run prepare:sidecars
if ($LASTEXITCODE -ne 0) { Pop-Location; exit $LASTEXITCODE }
npm run prepare:cesium
Pop-Location

# 5) Manifest checklist
$required = @(
  (Join-Path $BundleDir "converter\_3dtile.exe"),
  (Join-Path $BundleDir "bin\processor.exe"),
  (Join-Path $BundleDir "bin\top_rebuild.exe")
)
$missing = @()
foreach ($r in $required) {
  if (-not (Test-Path $r)) { $missing += $r }
}
if (-not $SkipTextureBundle) {
  $tex = Get-ChildItem -Recurse $TextureDst -Filter "geoforge-texture.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
  if (-not $tex) { $missing += "resources/runtime/texture/geoforge-texture.exe" }
}
if ($missing.Count -gt 0) {
  Write-Error ("Missing required package files:`n - " + ($missing -join "`n - "))
}

# 6) Tauri NSIS
Push-Location $AppDir
npm run tauri build
$code = $LASTEXITCODE
Pop-Location
if ($code -ne 0) { exit $code }

Write-Host "Package build finished. Check apps/desktop/src-tauri/target/release/bundle/nsis/"
