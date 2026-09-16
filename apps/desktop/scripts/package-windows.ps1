# Single Windows packaging entry (T09).
# Fails if any required component is missing.
# Usage: powershell -File apps/desktop/scripts/package-windows.ps1

param(
  [switch]$SkipBuild,
  [switch]$SkipTextureBundle
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..\..")
$AppDir = Resolve-Path (Join-Path $PSScriptRoot "..")
$RuntimeDir = Join-Path $RepoRoot "dist\runtime"
$BundleDir = Join-Path $AppDir "src-tauri\resources\runtime"

Write-Host "=== GeoForge Windows package ==="

# 1) Converter Release + product runtime
$prepArgs = @("-File", (Join-Path $PSScriptRoot "prepare-runtime.ps1"), "-OutDir", $RuntimeDir)
if ($SkipBuild) { $prepArgs += "-SkipBuild" }
& pwsh @prepArgs
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# 2) Stage into Tauri resources. Clear the generated staging directory first
# so files from an older converter/runtime cannot survive into this package.
if (Test-Path -LiteralPath $BundleDir) {
  Remove-Item -LiteralPath $BundleDir -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $BundleDir | Out-Null
Copy-Item -Recurse -Force (Join-Path $RuntimeDir "*") $BundleDir

# 3) Texture tool (optional skip if not built yet)
$TextureSrc = Join-Path $RepoRoot "tools\texture_ktx2\dist\geoforge-texture"
$TextureDst = Join-Path $BundleDir "texture"
if (Test-Path $TextureSrc) {
  New-Item -ItemType Directory -Force -Path $TextureDst | Out-Null
  Copy-Item -Recurse -Force (Join-Path $TextureSrc "*") $TextureDst
} elseif (-not $SkipTextureBundle) {
  Write-Warning "geoforge-texture bundle missing at $TextureSrc - packaging will fail checklist"
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
  (Join-Path $BundleDir "converter\osgPlugins-3.6.5"),
  (Join-Path $BundleDir "converter\gdal"),
  (Join-Path $BundleDir "converter\proj"),
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
npm run tauri -- build --bundles nsis
$code = $LASTEXITCODE
Pop-Location
if ($code -ne 0) { exit $code }

$NsisDir = Join-Path $AppDir "src-tauri\target\release\bundle\nsis"
Write-Host "Package build finished."
if (Test-Path $NsisDir) {
  $exes = Get-ChildItem $NsisDir -Filter *.exe -File -ErrorAction SilentlyContinue
  if ($exes) {
    foreach ($exe in $exes) {
      Write-Host ("NSIS_INSTALLER=" + $exe.FullName)
      Write-Host ("NSIS_SIZE_BYTES=" + $exe.Length)
    }
  } else {
    Write-Warning "NSIS dir exists but no .exe found: $NsisDir"
  }
} else {
  Write-Warning "NSIS output dir missing: $NsisDir"
}
