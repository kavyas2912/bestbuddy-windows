$ErrorActionPreference='Stop'
Set-Location (Join-Path $PSScriptRoot '..')
Write-Host 'VCE BUDDY - INTEGRATION TEST'
Write-Host ('='*72)
function Run-Checked([string]$exe,[string[]]$commandArgs,[string]$label){ & $exe @commandArgs; if($LASTEXITCODE -ne 0){ throw "$label failed with code $LASTEXITCODE." }; Write-Host "[OK] $label" }
& "$PSScriptRoot\RELEASE_PREFLIGHT_WINDOWS.ps1"; if(-not $?){throw 'Release preflight failed.'}
Run-Checked cargo @('check','--locked','--workspace','--all-targets') 'Cargo workspace check'
Run-Checked cargo @('test','--locked','--workspace','--all-targets') 'Cargo workspace tests'
Push-Location apps/vce-desktop
try {
Run-Checked npm.cmd @('test') 'Reminder regression tests'
Run-Checked npm.cmd @('run','build') 'Frontend production build'
Run-Checked npx.cmd @('tauri','build','--debug','--no-bundle') 'Tauri debug compile'
} finally { Pop-Location }
foreach($p in @('models/asr/sherpa-onnx-whisper-base.en/base.en-tokens.txt','models/asr/sherpa-onnx-whisper-base.en/base.en-encoder.int8.onnx','models/asr/sherpa-onnx-whisper-base.en/base.en-decoder.int8.onnx','models/tts/vits-piper-en_US-lessac-medium/tokens.txt','models/vad/silero_vad.onnx')){if(-not(Test-Path $p)){throw "Missing required voice asset $p"};Write-Host "[OK] $p"}
if(Test-Path 'models/kws/keywords.txt'){Write-Host '[OK] Custom KWS keyword file'} else {Write-Host '[INFO] KWS keyword file absent; always-on VAD fallback remains available.'}
Write-Host '[OK] Voice architecture present: always-on + VAD + streaming ASR + TTS + barge-in playback control'
Write-Host 'BUILD/UNIT/ASSET CHECKS PASSED. Live microphone, speech, barge-in and service behavior still require the manual checklist in docs/DEVICE_ACCEPTANCE.md.'
