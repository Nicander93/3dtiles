# Single Windows packaging entry (T09).
# Fails if any required component is missing.
# Usage: powershell -File apps/desktop/scripts/package-windows.ps1 [-SkipBuild] [-SkipTextureBundle] [-ConverterZip path]

param(
  [switch]$SkipBuild,
  [switch]$SkipTextureBundle,
  [string]$ConverterZip = ""
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
if ($ConverterZip) { $prepArgs += @("-ConverterZip", $ConverterZip) }
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
if (-not $SkipTextureBundle -and -not (Test-Path (Join-Path $TextureSrc "geoforge-texture.exe"))) {
  & (Join-Path $PSScriptRoot "prepare-texture.ps1")
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
if (Test-Path $TextureSrc) {
  New-Item -ItemType Directory -Force -Path $TextureDst | Out-Null
  Copy-Item -Recurse -Force (Join-Path $TextureSrc "*") $TextureDst
  # Ensure MSVC CRT sits beside basisu.exe (direct launch / 0xc0000135 packaging fix).
  $crtNames = @(
    "concrt140.dll",
    "msvcp140.dll",
    "msvcp140_1.dll",
    "msvcp140_2.dll",
    "msvcp140_atomic_wait.dll",
    "msvcp140_codecvt_ids.dll",
    "vccorlib140.dll",
    "vcruntime140.dll",
    "vcruntime140_1.dll",
    "vcruntime140_threads.dll"
  )
  $crtSrcDirs = @(
    (Join-Path $BundleDir "bin"),
    (Join-Path $BundleDir "converter")
  )
  foreach ($name in $crtNames) {
    $dest = Join-Path $TextureDst $name
    if (Test-Path -LiteralPath $dest -PathType Leaf) { continue }
    foreach ($srcDir in $crtSrcDirs) {
      $src = Join-Path $srcDir $name
      if (Test-Path -LiteralPath $src -PathType Leaf) {
        Copy-Item -LiteralPath $src -Destination $dest -Force
        break
      }
    }
  }
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
  (Join-Path $BundleDir "converter\msvcp140.dll"),
  (Join-Path $BundleDir "converter\msvcp140_2.dll"),
  (Join-Path $BundleDir "converter\vcruntime140.dll"),
  (Join-Path $BundleDir "converter\vcruntime140_1.dll"),
  (Join-Path $BundleDir "bin\processor.exe"),
  (Join-Path $BundleDir "bin\top_rebuild.exe"),
  (Join-Path $BundleDir "bin\msvcp140.dll"),
  (Join-Path $BundleDir "bin\msvcp140_2.dll"),
  (Join-Path $BundleDir "bin\vcruntime140.dll"),
  (Join-Path $BundleDir "bin\vcruntime140_1.dll")
)
$missing = @()
foreach ($r in $required) {
  if (-not (Test-Path $r)) { $missing += $r }
}
if (-not $SkipTextureBundle) {
  $tex = Get-ChildItem -Recurse $TextureDst -Filter "geoforge-texture.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
  if (-not $tex) { $missing += "resources/runtime/texture/geoforge-texture.exe" }
  if (-not (Test-Path (Join-Path $TextureDst "basisu.exe"))) { $missing += "resources/runtime/texture/basisu.exe" }
  foreach ($dll in @("msvcp140.dll", "msvcp140_2.dll", "vcruntime140.dll", "vcruntime140_1.dll")) {
    if (-not (Test-Path (Join-Path $TextureDst $dll))) {
      $missing += ("resources/runtime/texture/" + $dll)
    }
  }
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
