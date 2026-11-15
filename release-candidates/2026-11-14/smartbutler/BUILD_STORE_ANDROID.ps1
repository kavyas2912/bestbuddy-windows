param([Parameter(Mandatory=$true)][string]$SigningSettings)
$ErrorActionPreference='Stop'
$settings=Get-Content -LiteralPath $SigningSettings -Raw | ConvertFrom-Json
$env:BESTBUDDY_ANDROID_KEYSTORE=$settings.keystore
$env:BESTBUDDY_ANDROID_KEY_ALIAS=$settings.alias
$env:BESTBUDDY_ANDROID_STORE_PASSWORD=$settings.storePassword
$env:BESTBUDDY_ANDROID_KEY_PASSWORD=$settings.keyPassword
try {
  & (Join-Path $PSScriptRoot 'tools/BUILD_ANDROID_ENGINEERING.ps1') -Release -Bundle
  if($LASTEXITCODE){exit $LASTEXITCODE}
} finally {
  Remove-Item Env:BESTBUDDY_ANDROID_KEYSTORE,Env:BESTBUDDY_ANDROID_KEY_ALIAS,Env:BESTBUDDY_ANDROID_STORE_PASSWORD,Env:BESTBUDDY_ANDROID_KEY_PASSWORD -ErrorAction SilentlyContinue
}
