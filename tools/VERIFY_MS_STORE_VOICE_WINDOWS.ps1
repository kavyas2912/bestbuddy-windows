param(
  [Parameter(Mandatory = $true)][string]$MsiPath,
  [string]$ReportPath = ''
)
$ErrorActionPreference = 'Stop'
$msi = [IO.Path]::GetFullPath($MsiPath)
if (-not (Test-Path -LiteralPath $msi -PathType Leaf)) { throw "MSI missing: $msi" }
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$audit = Join-Path $tempRoot ('buddy-msi-voice-verify-' + [guid]::NewGuid().ToString('N'))
$image = Join-Path $audit 'image'
$userData = Join-Path $audit 'user-data'
$process = $null
$oldLocal = $env:LOCALAPPDATA
try {
  [IO.Directory]::CreateDirectory($image) | Out-Null
  [IO.Directory]::CreateDirectory((Join-Path $userData 'BestBuddy\data')) | Out-Null
  $args = @('/a', ('"' + $msi + '"'), '/qn', ('TARGETDIR="' + $image + '"'))
  $extract = Start-Process msiexec.exe -ArgumentList $args -PassThru -Wait -WindowStyle Hidden
  if ($extract.ExitCode -ne 0) { throw "MSI administrative extraction failed: $($extract.ExitCode)" }
  $install = Join-Path $image 'PFiles\BestBuddy'
  $exe = Join-Path $install 'vce-buddy-desktop.exe'
  $tts = Join-Path $install 'models\tts\vits-piper-en_US-lessac-medium'
  $required = @(
    $exe,
    (Join-Path $install 'models\asr\sherpa-onnx-whisper-base.en\base.en-encoder.int8.onnx'),
    (Join-Path $tts 'en_US-lessac-medium.onnx'),
    (Join-Path $tts 'tokens.txt'),
    (Join-Path $tts 'espeak-ng-data\phontab'),
    (Join-Path $tts 'espeak-ng-data\phonindex'),
    (Join-Path $tts 'espeak-ng-data\phondata'),
    (Join-Path $tts 'espeak-ng-data\intonations')
  )
  foreach ($file in $required) {
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "MSI voice asset missing: $file" }
  }
  $forbiddenNames = @(
    'profile.json','contacts.json','reminders.json','registration-draft.json',
    'assistant-settings.json','assistant-keys.vault','buddy.log','desktop.log',
    'voice.log','voice-sidecar.log','llama-server.log'
  )
  $forbidden = @(Get-ChildItem -LiteralPath $install -Recurse -Force -File | Where-Object {
    $_.Name -in $forbiddenNames -or $_.Extension -eq '.gguf' -or $_.Name -like 'llama-server*'
  })
  if ($forbidden.Count) {
    throw "MSI contains user state or optional local-LLM assets: $($forbidden.FullName -join ', ')"
  }
  $env:LOCALAPPDATA = $userData
  $process = Start-Process -FilePath $exe -WorkingDirectory $install -PassThru -WindowStyle Hidden
  $log = Join-Path $userData 'BestBuddy\logs\buddy.log'
  $deadline = [DateTime]::UtcNow.AddSeconds(75)
  $ready = $false
  $lastStatus = $null
  while ([DateTime]::UtcNow -lt $deadline) {
    Start-Sleep -Milliseconds 500
    if (Test-Path -LiteralPath $log) {
      $lines = @(Get-Content -LiteralPath $log | Where-Object { $_ -match '\| voice: \{' })
      if ($lines.Count) {
        $json = ($lines[-1] -split '\| voice: ', 2)[1]
        $lastStatus = $json | ConvertFrom-Json
        if ($lastStatus.tts_ready -and $lastStatus.asr_ready -and $lastStatus.vad_ready -and $lastStatus.kws_ready -and $lastStatus.always_on) {
          $ready = $true
          break
        }
      }
    }
    $process.Refresh()
    if ($process.HasExited) { break }
  }
  if (-not $ready) {
    $detail = if (Test-Path -LiteralPath $log) { (Get-Content -LiteralPath $log -Tail 10) -join "`n" } else { 'No app log was created.' }
    throw "Packaged voice did not become ready.`n$detail"
  }
  $result = "PASS: MSI-extracted app initialized bundled TTS, ASR, VAD and KWS; always-on voice entered ready state."
  Write-Host $result
  if ($ReportPath) { [IO.File]::WriteAllText([IO.Path]::GetFullPath($ReportPath), "$result`n$($lastStatus | ConvertTo-Json -Compress)`n") }
} finally {
  if ($process) {
    $process.Refresh()
    if (-not $process.HasExited) { Stop-Process -Id $process.Id -ErrorAction SilentlyContinue }
  }
  if ($null -eq $oldLocal) { Remove-Item Env:LOCALAPPDATA -ErrorAction SilentlyContinue } else { $env:LOCALAPPDATA = $oldLocal }
  $resolved = [IO.Path]::GetFullPath($audit)
  if ($resolved.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -and (Split-Path $resolved -Leaf) -like 'buddy-msi-voice-verify-*') {
    Remove-Item -LiteralPath $resolved -Recurse -Force -ErrorAction SilentlyContinue
  }
}
