# Download and stage prebuilt geoforge-converter Windows runtime.
# Usage:
#   powershell -File apps/desktop/scripts/prepare-converter.ps1 [-OutDir path]
# Reads third_party/3dtiles-converter.json

param(
  [string]$OutDir = "",
  [string]$LocalZip = ""
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..\..")
$ManifestPath = Join-Path $RepoRoot "third_party\3dtiles-converter.json"
if (-not (Test-Path $ManifestPath)) {
  Write-Error "Missing $ManifestPath"
}

$manifest = Get-Content -Raw -Encoding utf8 $ManifestPath | ConvertFrom-Json
$version = $manifest.version
$win = $manifest.windowsX64
if (-not $OutDir) {
  $OutDir = Join-Path $RepoRoot "dist\runtime\converter"
}

$CacheRoot = Join-Path $RepoRoot ".cache\converter\$version"
$ZipPath = Join-Path $CacheRoot "geoforge-converter-$version-windows-x64.zip"
$ExtractDir = Join-Path $CacheRoot "extracted"
New-Item -ItemType Directory -Force -Path $CacheRoot | Out-Null

function Get-FileSha256([string]$Path) {
  return (Get-FileHash -Algorithm SHA256 -Path $Path).Hash.ToLowerInvariant()
}

if ($LocalZip) {
  if (-not (Test-Path $LocalZip)) { Write-Error "LocalZip not found: $LocalZip" }
  Copy-Item -Force $LocalZip $ZipPath
  Write-Host "Using LocalZip $LocalZip"
} else {
  if (-not $win -or -not $win.url -or -not $win.sha256) {
    Write-Error "windowsX64.url / windowsX64.sha256 required in $ManifestPath (or pass -LocalZip)"
  }
  if ($win.sha256 -match '^(REPLACE|TODO|PENDING)') {
    Write-Error "windowsX64.sha256 is not finalized in $ManifestPath (Release CI not ready). Pass -LocalZip when you have a built zip."
  }
  $expected = $win.sha256.ToLowerInvariant()
  $needDownload = $true
  if (Test-Path $ZipPath) {
    $actual = Get-FileSha256 $ZipPath
    if ($actual -eq $expected) {
      Write-Host "Cache hit: $ZipPath"
      $needDownload = $false
    } else {
      Write-Warning "Cache SHA256 mismatch ($actual); re-downloading"
      Remove-Item -Force $ZipPath
    }
  }
  if ($needDownload) {
    Write-Host "Downloading $($win.url)"
    Invoke-WebRequest -Uri $win.url -OutFile $ZipPath -UseBasicParsing
    $actual = Get-FileSha256 $ZipPath
    if ($actual -ne $expected) {
      Write-Error "SHA256 mismatch for download: expected $expected got $actual"
    }
  }
}

if (Test-Path $ExtractDir) {
  Remove-Item -Recurse -Force $ExtractDir
}
New-Item -ItemType Directory -Force -Path $ExtractDir | Out-Null
Expand-Archive -Path $ZipPath -DestinationPath $ExtractDir -Force

# Zip may contain converter/* or flat files
$src = $ExtractDir
$nested = Join-Path $ExtractDir "converter"
if (Test-Path (Join-Path $nested "_3dtile.exe")) {
  $src = $nested
} elseif (-not (Test-Path (Join-Path $ExtractDir "_3dtile.exe"))) {
  $found = Get-ChildItem -Recurse $ExtractDir -Filter "_3dtile.exe" | Select-Object -First 1
  if (-not $found) {
    Write-Error "_3dtile.exe not found inside zip"
  }
  $src = $found.Directory.FullName
}

$exe = Join-Path $src "_3dtile.exe"
if (-not (Test-Path $exe)) {
  Write-Error "Missing _3dtile.exe at $exe"
}

# The converter is built with the dynamic MSVC CRT. Validate the DLLs before
# the soft launch so a package that only works on the build machine cannot be
# staged into the desktop installer.
$requiredCrt = @("msvcp140.dll", "msvcp140_2.dll", "vcruntime140.dll", "vcruntime140_1.dll")
$missingCrt = $requiredCrt | Where-Object { -not (Test-Path (Join-Path $src $_)) }
if ($missingCrt) {
  Write-Error "Converter runtime missing MSVC DLLs: $($missingCrt -join ', ')"
}

# Soft launch check
$p = Start-Process -FilePath $exe -ArgumentList "--help" -PassThru -WindowStyle Hidden -WorkingDirectory $src
if (-not $p.WaitForExit(8000)) {
  try { $p.Kill() } catch {}
  throw "_3dtile.exe --help timed out; refusing to stage an unverified converter"
} elseif ($p.ExitCode -ne 0) {
  throw "_3dtile.exe --help failed with exit=$($p.ExitCode); refusing to stage an unverified converter"
}

if (Test-Path $OutDir) {
  Remove-Item -Recurse -Force $OutDir
}
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
Copy-Item -Recurse -Force (Join-Path $src "*") $OutDir

$required = @(
  (Join-Path $OutDir "_3dtile.exe"),
  (Join-Path $OutDir "osgPlugins-3.6.5"),
  (Join-Path $OutDir "gdal"),
  (Join-Path $OutDir "proj")
)
foreach ($r in $required) {
  if (-not (Test-Path $r)) {
    Write-Error "Converter runtime incomplete: missing $r"
  }
}

Write-Host "Converter staged at $OutDir (version $version)"
