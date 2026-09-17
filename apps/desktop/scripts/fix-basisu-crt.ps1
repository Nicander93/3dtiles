# Copy MSVC CRT and zstd.dll from runtime\bin (or converter) beside basisu.exe.
# Fixes direct basisu launch failing with 0xc0000135 (STATUS_DLL_NOT_FOUND) on machines
# without VC++ redist or when zstd.dll is missing.
# Usage:
#   powershell -File apps/desktop/scripts/fix-basisu-crt.ps1 [-RuntimeRoot "D:\sofware\GeoForge 3D\resources\runtime"]
param(
  [string]$RuntimeRoot = ""
)

$ErrorActionPreference = "Stop"
if (-not $RuntimeRoot) {
  $candidates = @(
    (Join-Path $PSScriptRoot "..\src-tauri\resources\runtime"),
    "D:\sofware\GeoForge 3D\resources\runtime"
  )
  $RuntimeRoot = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $RuntimeRoot -or -not (Test-Path $RuntimeRoot)) {
  throw "RuntimeRoot not found. Pass -RuntimeRoot explicitly."
}
$RuntimeRoot = (Resolve-Path -LiteralPath $RuntimeRoot).Path
$TextureDir = Join-Path $RuntimeRoot "texture"
$Basisu = Join-Path $TextureDir "basisu.exe"
if (-not (Test-Path -LiteralPath $Basisu -PathType Leaf)) {
  throw "basisu.exe missing: $Basisu"
}

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
  "vcruntime140_threads.dll",
  "zstd.dll"
)
$srcDirs = @(
  (Join-Path $RuntimeRoot "bin"),
  (Join-Path $RuntimeRoot "converter")
)
$copied = @()
foreach ($name in $crtNames) {
  $dest = Join-Path $TextureDir $name
  foreach ($srcDir in $srcDirs) {
    $src = Join-Path $srcDir $name
    if (Test-Path -LiteralPath $src -PathType Leaf) {
      Copy-Item -LiteralPath $src -Destination $dest -Force
      $copied += $name
      break
    }
  }
}
Write-Host "Copied CRT + zstd beside basisu: $($copied -join ', ')"
Write-Host "Probing: $Basisu -version"
& $Basisu -version
Write-Host "EXIT=$LASTEXITCODE"
