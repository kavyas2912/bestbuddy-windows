$ErrorActionPreference = 'Stop'
$sourceRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Push-Location -LiteralPath $sourceRoot
$previousArchive = $env:SHERPA_ONNX_ARCHIVE_DIR
try {
$archiveName = 'sherpa-onnx-v1.13.8-win-x64-static-MT-Release-lib.tar.bz2'
$relative = 'tools/cache/sherpa-onnx/' + $archiveName
$archivePath = Join-Path $sourceRoot $relative

# The small source ZIP deliberately excludes this 123 MB native archive.
# Reuse just that file before starting an offline Cargo build.
& (Join-Path $PSScriptRoot 'REUSE_LOCAL_RESOURCES.ps1') -TargetRoot $sourceRoot -OnlyAssets @($relative)
if (-not (Test-Path -LiteralPath $archivePath -PathType Leaf) -or (Get-Item -LiteralPath $archivePath).Length -lt 1000000) {
  throw "Required sherpa-onnx archive is missing: $archivePath. Run tools\SETUP_WINDOWS.cmd once to download it, or set BUDDY_RESOURCE_CACHE to a folder containing an older BestBuddy source tree."
}
$archiveStream = [IO.File]::OpenRead($archivePath)
$hasher = [Security.Cryptography.SHA256]::Create()
try { $archiveHash = [BitConverter]::ToString($hasher.ComputeHash($archiveStream)).Replace('-', '') }
finally { $hasher.Dispose(); $archiveStream.Dispose() }
if ($archiveHash -ne '56FFCF3C454C1F14F7BC9887286CC8143E7E542DC632804E1C447D5F8D534EAF') {
  throw "Cached sherpa-onnx archive is corrupt: $archivePath. Run tools\SETUP_WINDOWS.cmd to verify and replace it."
}
$env:SHERPA_ONNX_ARCHIVE_DIR = Split-Path -Path $archivePath -Parent
Write-Host "[OK] Using local sherpa-onnx archive: $archivePath"
& cargo test --locked --workspace --offline
if ($LASTEXITCODE -ne 0) { throw "Offline Cargo workspace tests failed with code $LASTEXITCODE." }
Write-Host '[OK] Offline Cargo workspace tests passed.'

} finally {
    $env:SHERPA_ONNX_ARCHIVE_DIR = $previousArchive
    Pop-Location
}
