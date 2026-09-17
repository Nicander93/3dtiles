# Build the self-contained Windows texture sidecar and stage basisu beside it.
param(
  [string]$BasisuPath = "",
  [string]$PythonCommand = "python"
)

$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..")).Path
$TextureRoot = Join-Path $RepoRoot "tools\texture_ktx2"
$BuildRoot = Join-Path $TextureRoot "build"
$DistDir = Join-Path $TextureRoot "dist\geoforge-texture"
$Exe = Join-Path $DistDir "geoforge-texture.exe"

if (-not $BasisuPath -and $env:GEOFORGE_BASISU) {
  $BasisuPath = $env:GEOFORGE_BASISU
}
if (-not $BasisuPath) {
  $candidates = @(
    (Join-Path $RepoRoot "vcpkg_installed\x64-windows\tools\basisu\basisu.exe"),
    (Join-Path (Split-Path $RepoRoot -Parent) "geoforge-converter\vcpkg_installed\x64-windows\tools\basisu\basisu.exe")
  )
  $BasisuPath = $candidates | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
}
if (-not $BasisuPath -or -not (Test-Path -LiteralPath $BasisuPath -PathType Leaf)) {
  throw "basisu.exe not found. Pass -BasisuPath or set GEOFORGE_BASISU."
}
$BasisuPath = (Resolve-Path -LiteralPath $BasisuPath).Path

Push-Location $TextureRoot
try {
  & $PythonCommand -m PyInstaller `
    --noconfirm `
    --clean `
    --onedir `
    --name geoforge-texture `
    --distpath (Join-Path $TextureRoot "dist") `
    --workpath $BuildRoot `
    --specpath $BuildRoot `
    (Join-Path $TextureRoot "run.py")
  if ($LASTEXITCODE -ne 0) {
    throw "PyInstaller failed with exit code $LASTEXITCODE"
  }
} finally {
  Pop-Location
}

Copy-Item -LiteralPath $BasisuPath -Destination (Join-Path $DistDir "basisu.exe") -Force

# basisu.exe is a native MSVC binary. Windows loads CRT from the exe directory
# first; shipping DLLs beside basisu avoids 0xc0000135 when users run it directly
# or when PATH does not yet include runtime/bin.
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
$crtSearchRoots = @(
  (Join-Path $RepoRoot "dist\runtime\converter"),
  (Join-Path $RepoRoot "dist\runtime\bin"),
  (Join-Path $RepoRoot "apps\desktop\src-tauri\resources\runtime\converter"),
  (Join-Path $RepoRoot "apps\desktop\src-tauri\resources\runtime\bin"),
  (Split-Path -Parent $BasisuPath)
)
foreach ($name in $crtNames) {
  $src = $null
  foreach ($root in $crtSearchRoots) {
    $cand = Join-Path $root $name
    if (Test-Path -LiteralPath $cand -PathType Leaf) { $src = $cand; break }
  }
  if ($src) {
    Copy-Item -LiteralPath $src -Destination (Join-Path $DistDir $name) -Force
  }
}
$requiredBasisuCrt = @("msvcp140.dll", "msvcp140_2.dll", "vcruntime140.dll", "vcruntime140_1.dll")
$missingBasisuCrt = $requiredBasisuCrt | Where-Object { -not (Test-Path (Join-Path $DistDir $_)) }
if ($missingBasisuCrt) {
  Write-Warning ("basisu CRT not fully staged (will rely on PATH/runtime/bin): " + ($missingBasisuCrt -join ', '))
}

& $Exe --help | Out-Null
if ($LASTEXITCODE -ne 0) {
  throw "geoforge-texture.exe --help failed with exit code $LASTEXITCODE"
}
Write-Host "Texture component ready: $DistDir"
