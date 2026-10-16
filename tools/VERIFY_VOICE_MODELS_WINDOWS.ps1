param([string]$Root = (Join-Path $PSScriptRoot '..'))
$ErrorActionPreference = 'Stop'
$resolved = [IO.Path]::GetFullPath($Root)
& python (Join-Path $PSScriptRoot 'prepare_voice_models.py') --target-root $resolved --check-only
if ($LASTEXITCODE -ne 0) { throw 'Voice model pack failed SHA-256 or completeness verification.' }
Write-Host '[OK] ASR, TTS, VAD, KWS and denoise models match the pinned manifest.'
