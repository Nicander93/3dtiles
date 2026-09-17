# Collect Windows x64 product runtime (converter from Release + product bins).
# Usage:
#   powershell -File apps/desktop/scripts/prepare-runtime.ps1 [-OutDir path] [-SkipBuild] [-ConverterZip path]
# Output layout:
#   <OutDir>/converter/_3dtile.exe + DLLs + osgPlugins-3.6.5 / gdal / proj
#   <OutDir>/bin/processor.exe, top_rebuild.exe
#   <OutDir>/manifest.json

param(
  [string]$OutDir = "",
  [switch]$SkipBuild,
  [string]$ConverterZip = ""
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..\..")
if (-not $OutDir) {
  $OutDir = Join-Path $RepoRoot "dist\runtime"
}
$OutDir = [System.IO.Path]::GetFullPath($OutDir)

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
$converterArgs = @(
  "-File", (Join-Path $PSScriptRoot "prepare-converter.ps1"),
  "-OutDir", $ConverterOut
)
$converterSourceNote = "Converter comes from third_party/3dtiles-converter.json Release; no local OSG/vcpkg build."
if ($ConverterZip) {
  $resolvedConverterZip = Resolve-Path -LiteralPath $ConverterZip -ErrorAction Stop
  if ((Get-Item -LiteralPath $resolvedConverterZip.Path).PSIsContainer) {
    throw "ConverterZip must be a file: $($resolvedConverterZip.Path)"
  }
  $converterArgs += @("-LocalZip", $resolvedConverterZip.Path)
  $converterSourceNote = "Converter comes from a caller-supplied local zip via -ConverterZip."
}
& pwsh -NoProfile @converterArgs
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$ProductBin = Join-Path $OutDir "bin"
New-Item -ItemType Directory -Force -Path $ProductBin | Out-Null
foreach ($name in @("processor.exe", "top_rebuild.exe")) {
  $src = Join-Path $RepoRoot "target\release\$name"
  if (-not (Test-Path $src)) {
    Write-Error "Missing $src - build product crates first"
  }
  Copy-Item -Force $src (Join-Path $ProductBin $name)
}

# processor.exe and top_rebuild.exe also use the dynamic MSVC CRT. Keep the
# same release DLLs beside the sidecars; Windows resolves native dependencies
# from the executable directory, not from the sibling converter directory.
$crtFiles = @(
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
foreach ($name in $crtFiles) {
  $src = Join-Path $ConverterOut $name
  if (Test-Path $src) {
    Copy-Item -Force $src (Join-Path $ProductBin $name)
  }
}
$missingProductCrt = @(
  "msvcp140.dll",
  "msvcp140_2.dll",
  "vcruntime140.dll",
  "vcruntime140_1.dll"
) |
  Where-Object { -not (Test-Path (Join-Path $ProductBin $_)) }
if ($missingProductCrt) {
  Write-Error "Product runtime missing MSVC DLLs: $($missingProductCrt -join ', ')"
}

$ManifestPath = Join-Path $OutDir "manifest.json"
$files = Get-ChildItem -Recurse $OutDir -File |
  Where-Object { $_.FullName -ne $ManifestPath } |
  ForEach-Object {
  @{ path = $_.FullName.Substring($OutDir.Length).TrimStart('\', '/'); size = $_.Length; sha256 = (Get-FileHash -Algorithm SHA256 -Path $_.FullName).Hash.ToLowerInvariant() }
}
$manifest = @{
  createdAt = (Get-Date).ToString("o")
  platform = "windows-x64"
  converterExe = "converter/_3dtile.exe"
  files = $files
  notes = $converterSourceNote
}
$manifest | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 $ManifestPath
Write-Host "Runtime staged at $OutDir"
