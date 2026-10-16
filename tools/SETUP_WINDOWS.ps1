$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'SHA256_WINDOWS.ps1')
$sourceRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
# npm/esbuild still invokes a native EXE through Windows process APIs that can
# fail for deeply nested extraction paths. Run the entire setup through a
# temporary short drive alias, so every child process inherits short paths.
if ($sourceRoot.Length -gt 100 -and $env:BUDDY_SHORT_PATH_ACTIVE -ne '1') {
    # Cargo may canonicalize a subst-backed target directory to the original
    # long path before calling link.exe. Put all build output at a real short
    # physical path, while using subst only for source/native-tool commands.
    $sha = [Security.Cryptography.SHA256]::Create()
    try { $id = [BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($sourceRoot.ToLowerInvariant()))).Replace('-','').Substring(0,12).ToLowerInvariant() }
    finally { $sha.Dispose() }
    $targetBase = if ($env:BUDDY_BUILD_TARGET) { [IO.Path]::GetFullPath($env:BUDDY_BUILD_TARGET) } else { Join-Path $env:LOCALAPPDATA "BestBuddy\cargo-targets\$id" }
    if ($targetBase.Length -ge 100) { throw "Cargo build target is too long for link.exe: $targetBase. Set BUDDY_BUILD_TARGET to a short absolute folder." }
    New-Item -ItemType Directory -Force -Path $targetBase | Out-Null
    $shortDrive = @('Z','Y','X','W','V','U','T') | Where-Object { -not (Test-Path "${_}:\") } | Select-Object -First 1
    if (-not $shortDrive) { throw 'No free drive letter for short-path setup. Extract to C:\BestBuddy\AI_Buddy_Continued and rerun setup.' }
    & subst.exe "${shortDrive}:" $sourceRoot
    if ($LASTEXITCODE -ne 0) { throw 'Could not create a short-path alias. Extract to C:\BestBuddy\AI_Buddy_Continued and rerun setup.' }
    $previousPhysicalRoot = $env:BUDDY_PHYSICAL_SOURCE_ROOT
    $previousCargoTarget = $env:CARGO_TARGET_DIR
    $env:BUDDY_SHORT_PATH_ACTIVE = '1'
    $env:BUDDY_PHYSICAL_SOURCE_ROOT = $sourceRoot
    $env:CARGO_TARGET_DIR = $targetBase
    try {
        Write-Host "[path] Source: ${shortDrive}:; Cargo output: $targetBase"
        & powershell.exe -NoProfile -ExecutionPolicy Bypass -File "${shortDrive}:\tools\SETUP_WINDOWS.ps1"
        $setupExit = $LASTEXITCODE
    } finally {
        & subst.exe "${shortDrive}:" /D | Out-Null
        Remove-Item Env:BUDDY_SHORT_PATH_ACTIVE -ErrorAction SilentlyContinue
        if ($null -eq $previousPhysicalRoot) { Remove-Item Env:BUDDY_PHYSICAL_SOURCE_ROOT -ErrorAction SilentlyContinue }
        else { $env:BUDDY_PHYSICAL_SOURCE_ROOT = $previousPhysicalRoot }
        if ($null -eq $previousCargoTarget) { Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue }
        else { $env:CARGO_TARGET_DIR = $previousCargoTarget }
    }
    exit $setupExit
}
Set-Location (Join-Path $PSScriptRoot '..')
$logDir = Join-Path (Get-Location) 'logs'
New-Item -ItemType Directory -Force -Path $logDir | Out-Null
$buildTarget = if ($env:CARGO_TARGET_DIR) { [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR) } else { Join-Path (Get-Location) 'target' }
[IO.File]::WriteAllText((Join-Path $logDir 'BUILD_TARGET_PATH.txt'), $buildTarget)
$setupLog = Join-Path $logDir 'SETUP_WINDOWS.log'
try { Start-Transcript -Path $setupLog -Append -Force | Out-Null } catch {}
try {
$cargoHeader = Get-Content 'Cargo.toml' -Raw
$versionMatch = [regex]::Match($cargoHeader,'(?ms)\[workspace\.package\].*?^version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"')
$displayVersion = if($versionMatch.Success){$versionMatch.Groups[1].Value}else{'unknown'}
Write-Host ('='*74)
Write-Host "BESTBUDDY - RUST / TAURI 2 - VOICE + VISUAL ALPHA $displayVersion"
Write-Host ('='*74)

function Need-Cmd([string]$name){ if(-not (Get-Command $name -ErrorAction SilentlyContinue)){ throw "$name is required but was not found in PATH." }; Write-Host "[OK] $name" }
function Run-Checked([string]$exe,[string[]]$commandArgs,[string]$label){ & $exe @commandArgs; if($LASTEXITCODE -ne 0){ throw "$label failed with code $LASTEXITCODE." }; Write-Host "[OK] $label" }

# Avoid Cargo's legacy config/config.toml ambiguity.
$cargoDir=Join-Path (Get-Location) '.cargo'
$legacy=Join-Path $cargoDir 'config'
if(Test-Path $legacy){ try { [IO.File]::SetAttributes($legacy,[IO.FileAttributes]::Normal); Remove-Item $legacy -Force } catch { throw "Cannot remove legacy .cargo/config. Extract this release into a fresh folder: $legacy" } }
if(-not (Test-Path (Join-Path $cargoDir 'config.toml'))){ New-Item -ItemType Directory -Force -Path $cargoDir | Out-Null; Set-Content (Join-Path $cargoDir 'config.toml') "[net]`nretry = 5`n" }

Need-Cmd rustc; Need-Cmd cargo; Need-Cmd node; Need-Cmd npm; Need-Cmd curl.exe; Need-Cmd tar.exe

Write-Host "`n[1/12] Release preflight..."
Write-Host '[preflight] Starting manifest/assets/config checks...'
try { & "$PSScriptRoot\RELEASE_PREFLIGHT_WINDOWS.ps1" } catch { throw "Release preflight failed: $($_.Exception.Message)" }
Write-Host '[preflight] Completed.' 

Write-Host "`n[2/12] Checking Tauri assets..."
foreach($a in @('apps/vce-desktop/src-tauri/icons/icon.ico','apps/vce-desktop/src-tauri/icons/icon.png','apps/vce-desktop/src-tauri/icons/32x32.png','apps/vce-desktop/src-tauri/icons/128x128.png','apps/vce-desktop/public/favicon.ico','apps/vce-desktop/public/assets/macaw-buddy.png')) { if(-not (Test-Path $a)){ throw "Missing required asset: $a" }; Write-Host "[OK] $a" }

Write-Host "`n[3/12] Installing frontend dependencies..."
Push-Location apps/vce-desktop
try {
# Do not inherit a possibly broken user-level .npmrc. npm 12 has strict config parsing;
# this release supplies a clean project-specific config while retaining the normal public registry.
$npmConfig = Join-Path (Get-Location) '.npmrc.vce-setup'
@'
registry=https://registry.npmjs.org/
fund=false
audit=false
'@ | Set-Content -Encoding ASCII $npmConfig
$oldNpmUserConfig = $env:NPM_CONFIG_USERCONFIG
$env:NPM_CONFIG_USERCONFIG = $npmConfig
try {
    Write-Host '[npm] Using isolated release config; user .npmrc is not modified.'
    Write-Host "[npm] Version: $(& npm --version)"
    # Install the exact committed dependency graph; run only esbuild setup below.
    Run-Checked npm.cmd @('ci','--ignore-scripts','--include=optional','--no-audit','--no-fund') 'npm install (scripts disabled; native optional dependency included)'
    $esbuildInstaller = Join-Path (Get-Location) 'node_modules/esbuild/install.js'
    if(-not (Test-Path $esbuildInstaller)){ throw "esbuild installer was not installed: $esbuildInstaller" }
    $esbuildBinary = Join-Path (Get-Location) 'node_modules/@esbuild/win32-x64/esbuild.exe'
    if(-not (Test-Path $esbuildBinary)){
        Run-Checked npm.cmd @('install','--no-save','--ignore-scripts','--include=optional','--no-audit','--no-fund','--package-lock=false','@esbuild/win32-x64@0.24.2') 'esbuild Windows binary repair'
    }
    if(-not (Test-Path $esbuildBinary)){ throw "esbuild Windows binary is still absent: $esbuildBinary" }
    Run-Checked node @($esbuildInstaller) 'esbuild native binary setup'
    $viteCmd = Join-Path (Get-Location) 'node_modules/.bin/vite.cmd'
    if(-not (Test-Path $viteCmd)){ throw "Vite executable was not installed: $viteCmd" }
    Run-Checked $viteCmd @('--version') 'Vite verification'
} finally {
    if($null -eq $oldNpmUserConfig){ Remove-Item Env:NPM_CONFIG_USERCONFIG -ErrorAction SilentlyContinue } else { $env:NPM_CONFIG_USERCONFIG=$oldNpmUserConfig }
    Remove-Item $npmConfig -Force -ErrorAction SilentlyContinue
}
} finally { Pop-Location }

Write-Host "`n[4/12] Verifying VCE dependency declarations..."
foreach($dep in @(@{Path='plugins/news/Cargo.toml';Name='urlencoding'},@{Path='plugins/weather/Cargo.toml';Name='urlencoding'},@{Path='plugins/market/Cargo.toml';Name='urlencoding'},@{Path='crates/buddy-core/Cargo.toml';Name='urlencoding'})) { $txt=Get-Content $dep.Path -Raw; if($txt -notmatch ('(?m)^\s*'+[regex]::Escape($dep.Name)+'(?:\.workspace)?\s*=')){ throw "$($dep.Name) is not declared in $($dep.Path)" }; Write-Host "[OK] $($dep.Name) in $($dep.Path)" }

Write-Host "`n[5/12] Preparing sherpa-onnx native cache..."
& "$PSScriptRoot\REUSE_LOCAL_RESOURCES.ps1"
$cache=Join-Path (Get-Location) 'tools/cache/sherpa-onnx'; New-Item -ItemType Directory -Force -Path $cache | Out-Null
$archive='sherpa-onnx-v1.13.8-win-x64-static-MT-Release-lib.tar.bz2'; $archivePath=Join-Path $cache $archive
$nativeHash='56FFCF3C454C1F14F7BC9887286CC8143E7E542DC632804E1C447D5F8D534EAF'
if((Test-Path -LiteralPath $archivePath) -and (Get-BuddySha256 $archivePath) -ne $nativeHash){
    Move-Item -LiteralPath $archivePath -Destination ($archivePath + '.bad') -Force
}
if(-not (Test-Path -LiteralPath $archivePath)){
    $url="https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.8/$archive"
    $partial=$archivePath + '.part'
    Run-Checked curl.exe @('-L','--fail','--retry','8','--retry-delay','3','--retry-all-errors','--continue-at','-','-o',$partial,$url) 'sherpa-onnx native library download'
    if((Get-BuddySha256 $partial) -ne $nativeHash){
        Move-Item -LiteralPath $partial -Destination ($archivePath + '.bad') -Force
        throw 'Downloaded sherpa-onnx archive failed SHA-256 verification; invalid partial quarantined.'
    }
    Move-Item -LiteralPath $partial -Destination $archivePath
}
if((Get-BuddySha256 $archivePath) -ne $nativeHash){ throw 'sherpa-onnx native archive failed SHA-256 verification.' }
$env:SHERPA_ONNX_ARCHIVE_DIR=$cache
Write-Host "[OK] sherpa-onnx archive cache"

Write-Host "`n[6/12] Checking core ASR/TTS models..."
Run-Checked cmd.exe @('/c',"$PSScriptRoot\DOWNLOAD_MODELS_WINDOWS.cmd") 'Core voice models'
$env:BUDDY_VAD_MODEL=Join-Path (Get-Location) 'models/vad/silero_vad.onnx'

Write-Host "`n[7/12] Preparing wake word / denoise voice pack..."
Run-Checked powershell.exe @('-NoProfile','-ExecutionPolicy','Bypass','-File',"$PSScriptRoot\PREPARE_VOICE_PACK_WINDOWS.ps1") 'Voice enhancement pack preparation'
Run-Checked powershell.exe @('-NoProfile','-ExecutionPolicy','Bypass','-File',"$PSScriptRoot\VERIFY_VOICE_MODELS_WINDOWS.ps1") 'Voice model file validation'
Run-Checked powershell.exe @('-NoProfile','-ExecutionPolicy','Bypass','-File',"$PSScriptRoot\PREPARE_NATIVE_MODEL_PATH_WINDOWS.ps1") 'Short native model path'

Write-Host "`n[8/12] Cargo workspace check..."
Run-Checked cargo @('check','--locked','--workspace','--all-targets') 'Cargo workspace check'

Write-Host "`n[9/12] Cargo workspace tests..."
Run-Checked cargo @('test','--locked','--workspace','--all-targets') 'Cargo workspace tests'
Run-Checked cargo @('test','--locked','-p','buddy-voice','--offline','bundled_keyword_spotter_initializes','--','--ignored') 'Native KWS model initialization'
Run-Checked cargo @('test','--locked','-p','buddy-voice','--offline','piper_hey_buddy_replays_through_live_kws_path','--','--ignored') 'Piper wake phrase through live KWS chunks'
Run-Checked cargo @('test','--locked','-p','buddy-voice','--offline','selected_wav_replays_through_public_asr_path','--','--ignored') 'Whisper model WAV transcription'
Run-Checked cargo @('test','--locked','-p','buddy-voice','--offline','streaming_48khz_microphone_resampling_preserves_speech','--','--ignored') '48 kHz microphone resampling to Whisper'

Write-Host "`n[10/12] Frontend production build..."
Push-Location apps/vce-desktop
try {
Run-Checked npm.cmd @('run','build') 'Frontend production build'
} finally { Pop-Location }

Write-Host "`n[11/12] Tauri development compile..."
Push-Location apps/vce-desktop
try {
Run-Checked npx.cmd @('tauri','build','--debug','--no-bundle') 'Tauri debug compile'
} finally { Pop-Location }

Write-Host "`n[12/12] Final integrity audit..."
$cargoText=Get-Content Cargo.toml -Raw
$expected=[regex]::Match($cargoText,'(?ms)\[workspace\.package\].*?^version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"').Groups[1].Value
if([string]::IsNullOrWhiteSpace($expected)){throw 'Workspace version could not be determined.'}
foreach($f in @('plugins/news/Cargo.toml','plugins/weather/Cargo.toml','plugins/market/Cargo.toml','crates/buddy-core/Cargo.toml')){ if((Get-Content $f -Raw) -notmatch '(?m)^\s*urlencoding(?:\.workspace)?\s*='){throw "Missing urlencoding declaration: $f"} }
foreach($p in @('apps/vce-desktop/package.json','apps/vce-desktop/src-tauri/tauri.conf.json')) { $j=Get-Content $p -Raw | ConvertFrom-Json; if($j.version -ne $expected){ throw "$p version $($j.version) != workspace $expected" } }
Write-Host "[OK] Version consistency = $expected"
Write-Host '[OK] Dependencies, assets, Cargo, tests, Vite and Tauri debug build passed'
Write-Host "`nSETUP COMPLETE"
Write-Host "Debug executable: $(Join-Path $buildTarget 'debug/vce-buddy-desktop.exe')"
Write-Host 'Start: tools\RUN_WINDOWS.cmd'
Write-Host 'App log: %LOCALAPPDATA%\BestBuddy\logs\buddy.log'
Write-Host 'Setup log: logs\SETUP_WINDOWS.log'
} finally { try { Stop-Transcript | Out-Null } catch {} }
