$ErrorActionPreference = 'Stop'
$sandbox = Join-Path ([IO.Path]::GetTempPath()) ('buddy-reuse-test-' + [guid]::NewGuid().ToString('N'))
$source = Join-Path $sandbox 'AI_Buddy_Old\AI_Buddy_Continued'
$target = Join-Path $sandbox 'AI_Buddy_New'
$mappedDrive = $null
$nativeAlias = $null
$oldPhysicalRoot = $env:BUDDY_PHYSICAL_SOURCE_ROOT
$oldCodexHome = $env:CODEX_HOME
try {
  New-Item -ItemType Directory -Force -Path (Join-Path $source 'models/asr/sherpa-onnx-whisper-base.en'), $target | Out-Null
  Set-Content -LiteralPath (Join-Path $target 'Cargo.toml') -Value '[workspace]' -Encoding ASCII
  $tokens = Join-Path $source 'models/asr/sherpa-onnx-whisper-base.en/base.en-tokens.txt'
  Set-Content -LiteralPath $tokens -Value ('token' * 100001) -Encoding ASCII
  foreach ($model in @('base.en-encoder.int8.onnx','base.en-decoder.int8.onnx')) {
    Set-Content -LiteralPath (Join-Path (Split-Path $tokens -Parent) $model) -Value 'fixture' -Encoding ASCII
  }
  & "$PSScriptRoot\REUSE_LOCAL_RESOURCES.ps1" -TargetRoot $target -SearchRoots @($sandbox)
  $copied = Join-Path $target 'models/asr/sherpa-onnx-whisper-base.en/base.en-tokens.txt'
  if (-not (Test-Path -LiteralPath $copied)) { throw 'Model was not copied.' }
  Set-Content -LiteralPath $copied -Value ('local' * 100001) -Encoding ASCII
  & "$PSScriptRoot\REUSE_LOCAL_RESOURCES.ps1" -TargetRoot $target -SearchRoots @($sandbox)
  if ((Get-Content -LiteralPath $copied -Raw) -notmatch '^local') { throw 'Existing model was overwritten.' }
  $decoder = Join-Path (Split-Path $copied -Parent) 'base.en-decoder.int8.onnx'
  Remove-Item -LiteralPath $decoder -Force
  & "$PSScriptRoot\REUSE_LOCAL_RESOURCES.ps1" -TargetRoot $target -SearchRoots @($sandbox)
  if (-not (Test-Path -LiteralPath $decoder)) { throw 'Incomplete model was not repaired.' }
  # Setup maps long source paths to a drive root. A drive root has no parent,
  # and any model junction must point to the physical path after subst ends.
  $mappedDrive = @('Y','X','W','V','U','T') | Where-Object { -not (Test-Path "${_}:\") } | Select-Object -First 1
  if (-not $mappedDrive) { throw 'No free drive letter for mapped-root regression.' }
  & subst.exe "${mappedDrive}:" $target
  if ($LASTEXITCODE -ne 0) { throw 'Could not create mapped-root regression drive.' }
  $env:BUDDY_PHYSICAL_SOURCE_ROOT = $target
  $env:CODEX_HOME = Join-Path $sandbox 'empty-codex'
  Remove-Item -LiteralPath $decoder -Force
  & "$PSScriptRoot\REUSE_LOCAL_RESOURCES.ps1" -TargetRoot "${mappedDrive}:\"
  if (-not (Test-Path -LiteralPath $decoder)) { throw 'Mapped-root resource reuse failed.' }
  $encoder = Join-Path $target 'models/kws/sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01/encoder-epoch-12-avg-2-chunk-16-left-64.int8.onnx'
  New-Item -ItemType Directory -Force -Path (Split-Path $encoder -Parent) | Out-Null
  Set-Content -LiteralPath $encoder -Value 'fixture' -Encoding ASCII
  & "$PSScriptRoot\PREPARE_NATIVE_MODEL_PATH_WINDOWS.ps1" -Root "${mappedDrive}:\" -LinkBase (Join-Path $sandbox 'links')
  $nativeAlias = Get-Content -LiteralPath (Join-Path $target 'models/.native-link') -Raw
  $nativeAlias = $nativeAlias.Trim()
  if (-not $nativeAlias.StartsWith((Join-Path $sandbox 'links'), [StringComparison]::OrdinalIgnoreCase)) { throw 'Native alias escaped the test directory.' }
  & subst.exe "${mappedDrive}:" /D | Out-Null
  $mappedDrive = $null
  if (-not (Test-Path -LiteralPath (Join-Path $nativeAlias 'kws/sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01/encoder-epoch-12-avg-2-chunk-16-left-64.int8.onnx'))) { throw 'Native model alias did not survive removal of the temporary drive.' }
  Write-Host '[OK] Nested resource copied, preserved on rerun, and repaired when incomplete.'
  Write-Host '[OK] Mapped-root reuse and persistent native model alias.'
} finally {
  if ($mappedDrive) { & subst.exe "${mappedDrive}:" /D | Out-Null }
  if ($null -eq $oldPhysicalRoot) { Remove-Item Env:BUDDY_PHYSICAL_SOURCE_ROOT -ErrorAction SilentlyContinue } else { $env:BUDDY_PHYSICAL_SOURCE_ROOT = $oldPhysicalRoot }
  if ($null -eq $oldCodexHome) { Remove-Item Env:CODEX_HOME -ErrorAction SilentlyContinue } else { $env:CODEX_HOME = $oldCodexHome }
  if ($nativeAlias -and $nativeAlias.StartsWith((Join-Path $sandbox 'links'), [StringComparison]::OrdinalIgnoreCase) -and (Test-Path -LiteralPath $nativeAlias)) { [IO.Directory]::Delete($nativeAlias) }
  $resolved = [IO.Path]::GetFullPath($sandbox)
  if ($resolved.StartsWith([IO.Path]::GetFullPath([IO.Path]::GetTempPath()), [StringComparison]::OrdinalIgnoreCase) -and (Split-Path $resolved -Leaf) -like 'buddy-reuse-test-*') {
    Remove-Item -LiteralPath $resolved -Recurse -Force -ErrorAction SilentlyContinue
  }
}
