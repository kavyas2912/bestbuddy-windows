param(
  [switch]$Release,
  [switch]$Bundle,
  [string]$SdkRoot = $env:ANDROID_HOME,
  [string]$JavaHome = $env:JAVA_HOME,
  [string]$NdkRoot = $env:NDK_HOME,
  [string]$SpeechArchiveDirectory = $env:SHERPA_ONNX_ARCHIVE_DIR,
  [switch]$CheckJavaOnly,
  [switch]$NoJdkDownload
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Write-Warning 'Engineering build only. Android platform implementations still require device, audio and battery acceptance. See docs/ANDROID_PORT.md.'
& (Join-Path $PSScriptRoot 'CHECK_ANDROID_FOUNDATION.ps1')
if (-not $SdkRoot) { $SdkRoot = Join-Path $env:LOCALAPPDATA 'Android/Sdk' }
. (Join-Path $PSScriptRoot 'RESOLVE_ANDROID_JDK.ps1')
$jdkOptions = @{ InstallIfMissing = (-not $NoJdkDownload -and -not $CheckJavaOnly) }
if ($PSBoundParameters.ContainsKey('JavaHome')) { $jdkOptions.RequestedHome = $JavaHome }
$jdk = Resolve-AndroidJdk @jdkOptions
$JavaHome = $jdk.Home
Write-Host "Using JDK $($jdk.Version): $JavaHome"
if ($CheckJavaOnly) { Write-Host 'Java preflight passed. No downloads or APK compilation were started.'; return }
if (-not (Test-Path -LiteralPath (Join-Path $SdkRoot 'platform-tools/adb.exe'))) { throw 'Set ANDROID_HOME to the Android SDK directory.' }
if (-not $NdkRoot) {
  $installed = Get-ChildItem -LiteralPath (Join-Path $SdkRoot 'ndk') -Directory | Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
  if (-not $installed) { throw 'Install the Android NDK using Android Studio SDK Manager.' }
  $NdkRoot = $installed.FullName
}
if (-not (Test-Path -LiteralPath (Join-Path $NdkRoot 'toolchains/llvm'))) { throw 'NDK_HOME does not point to an installed Android NDK.' }
if (-not $SpeechArchiveDirectory) { $SpeechArchiveDirectory = Join-Path $root 'tools/cache/sherpa-onnx-android' }
$archive = Join-Path $SpeechArchiveDirectory 'sherpa-onnx-v1.13.8-android.tar.bz2'
if (-not (Test-Path -LiteralPath $archive)) {
  New-Item -ItemType Directory -Path $SpeechArchiveDirectory -Force | Out-Null
  Write-Host 'Downloading the official pinned Android speech runtime...'
  Invoke-WebRequest -Uri 'https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.8/sherpa-onnx-v1.13.8-android.tar.bz2' -OutFile $archive
}
$expected = '2FF63469A71CB6009AA2E3ED5F4A670F8ABDCBE4BB9FFD23776AFC792A6B4F44'
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $expected) { throw 'Android speech archive checksum does not match the verified engineering input.' }
$runtime = Join-Path $SpeechArchiveDirectory 'runtime-1.13.8'
New-Item -ItemType Directory -Path $runtime -Force | Out-Null
& tar.exe -xjf $archive -C $runtime
if ($LASTEXITCODE -ne 0) { throw 'Unable to extract the pinned Android speech runtime.' }
$libraries = Join-Path $runtime 'jniLibs/arm64-v8a'
$jni = Join-Path $root 'apps/vce-desktop/src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a'
New-Item -ItemType Directory -Path $jni -Force | Out-Null
foreach ($name in @('libsherpa-onnx-c-api.so', 'libonnxruntime.so')) {
  if (-not (Test-Path -LiteralPath (Join-Path $libraries $name))) { throw "The official Android runtime is missing $name." }
}
Get-ChildItem -LiteralPath $libraries -Filter '*.so' -File | ForEach-Object {
  Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $jni $_.Name) -Force
}
$env:JAVA_HOME = $JavaHome
$env:ANDROID_HOME = $SdkRoot
$env:NDK_HOME = $NdkRoot
$env:SHERPA_ONNX_ARCHIVE_DIR = $SpeechArchiveDirectory
$env:PATH = "$(Join-Path $JavaHome 'bin');$env:PATH"
$frontend = Join-Path $root 'apps/vce-desktop'
Push-Location $frontend
try {
  & npm.cmd ci --ignore-scripts --include=optional --no-audit --no-fund
  if ($LASTEXITCODE -ne 0) { throw 'Locked frontend dependency installation failed.' }
  $tauriArgs=@('android','build','--ci','--target','aarch64','--apk')
  if(-not $Release){$tauriArgs+='--debug'}
  if($Bundle){$tauriArgs+='--aab'}
  & node.exe 'node_modules/@tauri-apps/cli/tauri.js' @tauriArgs
  if ($LASTEXITCODE -ne 0) { throw 'Android ARM64 engineering build failed. No accepted beta was produced.' }
  Write-Host 'Engineering APK compilation completed. This is not release or device acceptance.'
} finally {
  Pop-Location
}
