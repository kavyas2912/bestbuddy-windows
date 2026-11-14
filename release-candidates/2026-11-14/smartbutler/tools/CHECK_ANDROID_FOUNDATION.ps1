param([switch]$RequireReleaseReady)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$native = Join-Path $root 'apps/vce-desktop/src-tauri'
$config = Get-Content -LiteralPath (Join-Path $native 'tauri.android.conf.json') -Raw | ConvertFrom-Json
if ($config.identifier -ne 'com.smartbutler.app') { throw 'Unexpected Android application identity.' }
if ($config.bundle.android.minSdkVersion -lt 26) { throw 'The current audio backend requires Android API 26 or newer.' }
if ($config.bundle.android.versionCode -lt 1) { throw 'Android version code is missing.' }
$project = Join-Path $native 'gen/android'
$gradle = Get-Content -LiteralPath (Join-Path $project 'app/build.gradle.kts') -Raw
if (-not $gradle.Contains('applicationId = "com.smartbutler.app"')) { throw 'Gradle application identity differs from Tauri.' }
$manifest = Get-Content -LiteralPath (Join-Path $project 'app/src/main/AndroidManifest.xml') -Raw
if (-not $manifest.Contains('android:allowBackup="false"')) { throw 'App-private backup policy is missing.' }
foreach ($density in @('mdpi','hdpi','xhdpi','xxhdpi','xxxhdpi')) {
  if (-not (Test-Path -LiteralPath (Join-Path $project "app/src/main/res/mipmap-$density/ic_launcher.png"))) {
    throw "Missing Android launcher icon for $density."
  }
}
foreach ($permission in @('android.permission.RECORD_AUDIO','android.permission.FOREGROUND_SERVICE','android.permission.FOREGROUND_SERVICE_MICROPHONE','android.permission.POST_NOTIFICATIONS')) {
  if (-not $manifest.Contains($permission)) { throw "Missing Android permission declaration: $permission" }
}
if (-not $manifest.Contains('android:foregroundServiceType="microphone"') -or -not $manifest.Contains('android:stopWithTask="true"')) { throw 'Microphone service lifecycle declaration is incomplete.' }
if ($manifest.Contains('BOOT_COMPLETED')) { throw 'This build must not start listening from a boot receiver.' }
Write-Host 'Android foundation configuration checks passed.'
Write-Host 'Release readiness: NOT READY. See docs/ANDROID_PORT.md for storage, voice, lifecycle and device-test requirements.'
if ($RequireReleaseReady) { throw 'This foundation is not an accepted Android release.' }
