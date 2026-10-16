param(
  [string]$Root = (Join-Path $PSScriptRoot '..'),
  [string]$LinkBase = ''
)
$ErrorActionPreference = 'Stop'
$sourceRoot = [IO.Path]::GetFullPath($Root)
$physicalRoot = if ($env:BUDDY_PHYSICAL_SOURCE_ROOT -and
  (Test-Path -LiteralPath (Join-Path $env:BUDDY_PHYSICAL_SOURCE_ROOT 'Cargo.toml') -PathType Leaf)) {
  [IO.Path]::GetFullPath($env:BUDDY_PHYSICAL_SOURCE_ROOT)
} else { $sourceRoot }
# The junction must outlive a temporary subst drive used only during setup.
$modelsRoot = [IO.Path]::GetFullPath((Join-Path $physicalRoot 'models'))
if (-not (Test-Path -LiteralPath (Join-Path $sourceRoot 'Cargo.toml') -PathType Leaf)) { throw "Not an BestBuddy source root: $sourceRoot" }
if (-not (Test-Path -LiteralPath $modelsRoot -PathType Container)) { throw "Model directory missing: $modelsRoot" }

# A junction exposes the existing model files through a short path. It does not copy them.
$sha = [Security.Cryptography.SHA256]::Create()
try {
  $bytes = [Text.Encoding]::UTF8.GetBytes($modelsRoot.ToLowerInvariant())
  $id = [BitConverter]::ToString($sha.ComputeHash($bytes)).Replace('-', '').Substring(0, 16).ToLowerInvariant()
} finally { $sha.Dispose() }
$encoderSuffix = 'kws/sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01/encoder-epoch-12-avg-2-chunk-16-left-64.int8.onnx'
if (-not $LinkBase) {
  # Prefer a nearby project folder, avoiding a system-wide drive or model copy.
  $parent = [IO.Directory]::GetParent($sourceRoot)
  $bases = @()
  if ($parent) {
    $bases += (Join-Path $parent.FullName 'buddy-model-links')
    if ($parent.Parent) { $bases += (Join-Path $parent.Parent.FullName 'buddy-model-links') }
  }
  $bases += (Join-Path $env:TEMP 'BestBuddy/model-links')
  foreach ($candidateBase in $bases) {
    $candidateAlias = Join-Path $candidateBase $id
    if ((Join-Path $candidateAlias $encoderSuffix).Length -ge 240) { continue }
    try {
      New-Item -ItemType Directory -Force -Path $candidateBase -ErrorAction Stop | Out-Null
      $LinkBase = $candidateBase
      break
    } catch { continue }
  }
  if (-not $LinkBase) { throw 'Could not create a short native model path near this source or in the user temp directory.' }
} else {
  New-Item -ItemType Directory -Force -Path $LinkBase | Out-Null
}
$alias = Join-Path ([IO.Path]::GetFullPath($LinkBase)) $id
if (Test-Path -LiteralPath $alias) {
  $existing = Get-Item -LiteralPath $alias -Force
  if ($existing.LinkType -ne 'Junction' -or [IO.Path]::GetFullPath($existing.Target) -ne $modelsRoot) {
    throw "Native model alias is occupied by a different target: $alias"
  }
} else {
  New-Item -ItemType Junction -Path $alias -Target $modelsRoot | Out-Null
}
$encoder = Join-Path $alias $encoderSuffix
if (-not (Test-Path -LiteralPath $encoder -PathType Leaf)) { throw "KWS encoder is not reachable via the short native path: $encoder" }
if ($encoder.Length -ge 240) { throw "Native KWS path is still too long ($($encoder.Length) characters): $encoder" }
[IO.File]::WriteAllText((Join-Path $modelsRoot '.native-link'), $alias, [Text.UTF8Encoding]::new($false))
Write-Host "[OK] Native model path alias ($($encoder.Length) characters): $alias"
