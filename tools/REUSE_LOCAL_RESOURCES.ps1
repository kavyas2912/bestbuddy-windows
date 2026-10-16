param(
  [string]$TargetRoot = (Join-Path $PSScriptRoot '..'),
  [string[]]$SearchRoots = @(),
  [string[]]$OnlyAssets = @()
)
$ErrorActionPreference = 'Stop'
$target = [IO.Path]::GetFullPath($TargetRoot)
if (-not (Test-Path -LiteralPath (Join-Path $target 'Cargo.toml'))) { throw "Not an BestBuddy source root: $target" }

if ($SearchRoots.Count -eq 0) {
  $SearchRoots = @()
  if ($env:BUDDY_RESOURCE_CACHE) { $SearchRoots += $env:BUDDY_RESOURCE_CACHE }
  $codexHome = if ($env:CODEX_HOME) { $env:CODEX_HOME } else { Join-Path $env:USERPROFILE '.codex' }
  $SearchRoots += (Join-Path $codexHome 'buddy-resources')
  # A subst-mapped source appears as Z:\ and has no .NET parent. The setup
  # launcher passes its physical source so sibling release caches remain usable.
  $physical = if ($env:BUDDY_PHYSICAL_SOURCE_ROOT -and
    (Test-Path -LiteralPath (Join-Path $env:BUDDY_PHYSICAL_SOURCE_ROOT 'Cargo.toml') -PathType Leaf)) {
    [IO.Path]::GetFullPath($env:BUDDY_PHYSICAL_SOURCE_ROOT)
  } else { $target }
  $parent = [IO.Directory]::GetParent($physical)
  if ($parent) {
    $SearchRoots += (Join-Path $parent.FullName 'buddy-resources')
    $SearchRoots += $parent.FullName
  }
  $projectRoot = Join-Path $codexHome '.chatgpt-projects'
  if (Test-Path -LiteralPath $projectRoot) { $SearchRoots += $projectRoot }
}

# Search only known cache locations and direct BestBuddy release folders. No broad home-directory scan.
$candidates = [System.Collections.Generic.List[string]]::new()
foreach ($raw in $SearchRoots) {
  if (-not $raw -or -not (Test-Path -LiteralPath $raw -PathType Container)) { continue }
  $root = [IO.Path]::GetFullPath($raw)
  if ($root -ne $target) { $candidates.Add($root) }
  foreach ($child in @(Get-ChildItem -LiteralPath $root -Directory -ErrorAction SilentlyContinue | Where-Object { $_.Name -like 'AI_Buddy*' -or $_.Name -like 'VCE_Buddy*' -or $_.Name -like 'BestBuddy*' })) {
    if ($child.FullName -ne $target) { $candidates.Add($child.FullName) }
    # Expand-Archive commonly creates <release-name>/AI_Buddy_Continued/.
    foreach ($nested in @(Get-ChildItem -LiteralPath $child.FullName -Directory -ErrorAction SilentlyContinue | Where-Object { $_.Name -like 'AI_Buddy*' -or $_.Name -like 'VCE_Buddy*' -or $_.Name -like 'BestBuddy*' })) {
      if ($nested.FullName -ne $target) { $candidates.Add($nested.FullName) }
    }
  }
  # .codex/.chatgpt-projects/<project>/<release>
  if ((Split-Path $root -Leaf) -eq '.chatgpt-projects') {
    foreach ($project in @(Get-ChildItem -LiteralPath $root -Directory -ErrorAction SilentlyContinue)) {
      foreach ($child in @(Get-ChildItem -LiteralPath $project.FullName -Directory -ErrorAction SilentlyContinue | Where-Object { $_.Name -like 'AI_Buddy*' -or $_.Name -like 'VCE_Buddy*' -or $_.Name -like 'BestBuddy*' })) {
        if ($child.FullName -ne $target) { $candidates.Add($child.FullName) }
        foreach ($nested in @(Get-ChildItem -LiteralPath $child.FullName -Directory -ErrorAction SilentlyContinue | Where-Object { $_.Name -like 'AI_Buddy*' -or $_.Name -like 'VCE_Buddy*' -or $_.Name -like 'BestBuddy*' })) {
          if ($nested.FullName -ne $target) { $candidates.Add($nested.FullName) }
        }
      }
    }
  }
}
$candidates = @($candidates | Select-Object -Unique)
$nativeArchive = 'sherpa-onnx-v1.13.8-win-x64-static-MT-Release-lib.tar.bz2'
$nativeHash = '56FFCF3C454C1F14F7BC9887286CC8143E7E542DC632804E1C447D5F8D534EAF'
$assets = @(
  @{ Path='models/asr/sherpa-onnx-whisper-base.en'; Marker='base.en-tokens.txt'; Minimum=500000; Required=@('base.en-encoder.int8.onnx','base.en-decoder.int8.onnx') },
  @{ Path='models/tts/vits-piper-en_US-lessac-medium'; Marker='tokens.txt'; Minimum=100; Required=@('en_US-lessac-medium.onnx','espeak-ng-data') },
  @{ Path='models/vad/silero_vad.onnx'; Marker=''; Minimum=100000 },
  @{ Path='models/kws/sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01'; Marker='tokens.txt'; Minimum=100; Required=@('bpe.model','encoder-epoch-12-avg-2-chunk-16-left-64.int8.onnx','decoder-epoch-12-avg-2-chunk-16-left-64.onnx','joiner-epoch-12-avg-2-chunk-16-left-64.int8.onnx') },
  @{ Path='models/kws/keywords.txt'; Marker=''; Minimum=10 },
  @{ Path='models/denoise/gtcrn_simple.onnx'; Marker=''; Minimum=100000 },
  @{ Path="tools/cache/sherpa-onnx/$nativeArchive"; Marker=''; Minimum=1000000 }
)
if ($OnlyAssets.Count -gt 0) {
  $assets = @($assets | Where-Object { $OnlyAssets -contains $_.Path })
  if ($assets.Count -ne $OnlyAssets.Count) { throw 'Unknown or duplicate resource path requested for reuse.' }
}
function Test-Asset([string]$path, [string]$marker, [long]$minimum, [string[]]$required) {
  $base = $path
  if ($marker) { $path = Join-Path $path $marker }
  if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { return $false }
  if ((Get-Item -LiteralPath $path).Length -lt $minimum) { return $false }
  if ($path.EndsWith($nativeArchive, [StringComparison]::OrdinalIgnoreCase)) {
    $stream = [IO.File]::OpenRead($path)
    $hasher = [Security.Cryptography.SHA256]::Create()
    try { $actual = [BitConverter]::ToString($hasher.ComputeHash($stream)).Replace('-', '') }
    finally { $hasher.Dispose(); $stream.Dispose() }
    if ($actual -ne $nativeHash) { return $false }
  }
  foreach ($name in $required) {
    $item = Join-Path $base $name
    if (-not (Test-Path -LiteralPath $item)) { return $false }
    if ((Test-Path -LiteralPath $item -PathType Leaf) -and (Get-Item -LiteralPath $item).Length -eq 0) { return $false }
  }
  return $true
}
$reused = 0
foreach ($asset in $assets) {
  $destination = Join-Path $target $asset.Path
  if (Test-Asset $destination $asset.Marker $asset.Minimum $asset.Required) { continue }
  foreach ($root in $candidates) {
    $source = $null
    $sourcePaths = @($asset.Path)
    if ($asset.Path -eq "tools/cache/sherpa-onnx/$nativeArchive") {
      # The user's persistent buddy-resources cache predates the source layout.
      # Read its existing archive in place; do not create another cache copy.
      $sourcePaths += "sherpa-onnx/$nativeArchive"
      $sourcePaths += "sherpa-onnx-prebuilt/$nativeArchive"
    }
    foreach ($relative in $sourcePaths) {
      $candidate = Join-Path $root $relative
      if (Test-Asset $candidate $asset.Marker $asset.Minimum $asset.Required) { $source = $candidate; break }
    }
    if (-not $source) { continue }
    $parent = Split-Path $destination -Parent
    New-Item -ItemType Directory -Force -Path $parent | Out-Null
    if ($asset.Path -eq 'models/asr/sherpa-onnx-whisper-base.en') {
      # The upstream archive also contains large unquantized variants. Reuse
      # only the quantized files that this one-ASR build actually loads.
      New-Item -ItemType Directory -Force -Path $destination | Out-Null
      foreach ($name in @($asset.Marker) + $asset.Required) {
        Copy-Item -LiteralPath (Join-Path $source $name) -Destination (Join-Path $destination $name) -Force
      }
      $sample = Join-Path $source 'test_wavs/0.wav'
      if (Test-Path -LiteralPath $sample -PathType Leaf) {
        $sampleDir = Join-Path $destination 'test_wavs'
        New-Item -ItemType Directory -Force -Path $sampleDir | Out-Null
        Copy-Item -LiteralPath $sample -Destination (Join-Path $sampleDir '0.wav') -Force
      }
    } elseif (Test-Path -LiteralPath $destination) {
      if (Test-Path -LiteralPath $destination -PathType Container) {
        Get-ChildItem -LiteralPath $source -Force | Copy-Item -Destination $destination -Recurse -Force
      } else {
        Copy-Item -LiteralPath $source -Destination $destination -Force
      }
    } else {
      Copy-Item -LiteralPath $source -Destination $destination -Recurse
    }
    if (-not (Test-Asset $destination $asset.Marker $asset.Minimum $asset.Required)) { throw "Copied asset failed validation: $($asset.Path)" }
    Write-Host "[reuse] $($asset.Path) from $source"
    $reused++
    break
  }
}
Write-Host "[reuse] Reused $reused asset(s); setup will download any still missing."
