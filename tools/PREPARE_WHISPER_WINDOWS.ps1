$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'SHA256_WINDOWS.ps1')
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$model = Join-Path $root 'models/asr/sherpa-onnx-whisper-base.en'
$cache = Join-Path $root 'tools/cache/voice'
$archive = Join-Path $cache 'sherpa-onnx-whisper-base.en.tar.bz2'
$archiveHash = '475BC7052CE299C007F6D5D5407BA8601F819A2867F6EECEE510ED17DF581542'
$files = @(
  @{Name='base.en-encoder.int8.onnx'; Hash='EF6B936F4C9B1D90A3B68634B60C4ED8576B26172B33C2535EC0E933C9EDB823'},
  @{Name='base.en-decoder.int8.onnx'; Hash='F7162AD6DB2DBEF16CFAEAA7F945B9D7DD9C1B8D472F6ACA82F2273D185E4D41'},
  @{Name='base.en-tokens.txt'; Hash='306CD27F03C1A714ECA7108E03D66B7DC042ABE8C258B44C199A7ED9838DD930'}
)
function Test-Model {
  foreach ($item in $files) {
    $path = Join-Path $model $item.Name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { return $false }
    if ((Get-BuddySha256 $path) -ne $item.Hash) { return $false }
  }
  return $true
}
if (Test-Model) { Write-Host '[reuse] Verified Rust-bound Whisper base.en ASR model'; return }
New-Item -ItemType Directory -Force -Path $cache,(Join-Path $root 'models/asr') | Out-Null
if ((Test-Path -LiteralPath $archive -PathType Leaf) -and
    (Get-BuddySha256 $archive) -ne $archiveHash) {
  Move-Item -LiteralPath $archive -Destination ($archive + '.bad') -Force
}
if (-not (Test-Path -LiteralPath $archive -PathType Leaf)) {
  $partial = $archive + '.part'
  Write-Host 'Downloading the single Whisper ASR model (about 199 MB); interrupted downloads resume.'
  & curl.exe -L --fail --retry 8 --retry-delay 3 --retry-all-errors --continue-at - -o $partial 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-base.en.tar.bz2'
  if ($LASTEXITCODE -ne 0) { throw "Whisper ASR download failed; partial cache kept at $partial" }
  if ((Get-BuddySha256 $partial) -ne $archiveHash) {
    Move-Item -LiteralPath $partial -Destination ($archive + '.bad') -Force
    throw 'Whisper ASR archive hash mismatch; the invalid partial was quarantined for a clean retry.'
  }
  Move-Item -LiteralPath $partial -Destination $archive
}
if ((Get-BuddySha256 $archive) -ne $archiveHash) { throw 'Whisper ASR archive hash mismatch.' }
$entries = @($files | ForEach-Object { 'sherpa-onnx-whisper-base.en/' + $_.Name })
$entries += 'sherpa-onnx-whisper-base.en/test_wavs/0.wav'
& tar.exe -xf $archive -C (Join-Path $root 'models/asr') @entries
if ($LASTEXITCODE -ne 0) { throw 'Whisper ASR archive extraction failed.' }
if (-not (Test-Model)) { throw 'Extracted Whisper ASR files failed SHA-256 verification.' }
Write-Host '[OK] Rust-bound Whisper base.en ASR model verified'
