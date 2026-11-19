# Build the private test installer with its offline conversation and speech assets.
[CmdletBinding()]
param([switch]$VerifyAssetsOnly)
$ErrorActionPreference='Stop'
$source=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$frontend=Join-Path $source 'apps/vce-desktop'
$oldTarget=$env:CARGO_TARGET_DIR
$oldFlags=$env:CARGO_ENCODED_RUSTFLAGS
$oldStrip=$env:CARGO_PROFILE_RELEASE_STRIP
$oldJobs=$env:CARGO_BUILD_JOBS
$oldArchive=$env:SHERPA_ONNX_ARCHIVE_DIR
function Check-Exit([string]$operation){if($LASTEXITCODE -ne 0){throw "$operation failed with exit $LASTEXITCODE"}}
Push-Location $source
try {
    if(Get-Command py -ErrorAction SilentlyContinue){
        if($VerifyAssetsOnly){& py -3 tools/restore_defaults.py --verify}else{& py -3 tools/restore_defaults.py}
    } else {
        if($VerifyAssetsOnly){& python tools/restore_defaults.py --verify}else{& python tools/restore_defaults.py}
    }
    Check-Exit 'Included resource verification'
    if($VerifyAssetsOnly){return}
    if(-not(Get-Command cl -ErrorAction SilentlyContinue)){
        $vswhere=Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
        if(-not(Test-Path -LiteralPath $vswhere)){throw 'Visual Studio Build Tools with Desktop development with C++ is required.'}
        $installation=(& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath | Select-Object -First 1)
        if(-not $installation){throw 'Install the Visual Studio Desktop development with C++ workload.'}
        & (Join-Path $installation 'Common7/Tools/Launch-VsDevShell.ps1') -Arch amd64 -HostArch amd64 -SkipAutomaticLocation
    }
    & './tools/RELEASE_PREFLIGHT_WINDOWS.ps1'
    $env:CARGO_TARGET_DIR=if($oldTarget){$oldTarget}else{Join-Path $env:LOCALAPPDATA 'BestBuddy/cargo-targets/private-release'}
    $env:CARGO_BUILD_JOBS='2'
    $env:SHERPA_ONNX_ARCHIVE_DIR=Join-Path $source 'tools/cache/sherpa-onnx'
    $flags=@('--remap-path-prefix='+$source+'=/bestbuddy/source','--remap-path-prefix='+$env:CARGO_TARGET_DIR+'=/bestbuddy/build')
    if($env:USERPROFILE){$flags+='--remap-path-prefix='+$env:USERPROFILE+'=/bestbuddy/user'}
    if($oldFlags){$flags=@($oldFlags)+$flags}
    $env:CARGO_ENCODED_RUSTFLAGS=[string]::Join([char]31,$flags)
    $env:CARGO_PROFILE_RELEASE_STRIP='debuginfo'
    Set-Location $frontend
    if(-not(Test-Path 'node_modules/.bin/tauri.cmd')){& npm ci --no-audit --no-fund;Check-Exit 'Locked frontend dependency installation'}
    & npm run tauri -- build --config src-tauri/tauri.private.conf.json --bundles nsis
    Check-Exit 'Private installer build'
    $version=(Get-Content 'src-tauri/tauri.conf.json' -Raw|ConvertFrom-Json).version
    $installer=Join-Path $env:CARGO_TARGET_DIR "release/bundle/nsis/BestBuddy_${version}_x64-setup.exe"
    if(-not(Test-Path -LiteralPath $installer)){throw 'The expected new installer is missing.'}
    Get-FileHash -LiteralPath $installer -Algorithm SHA256
    Write-Host "Private test installer: $installer"
}finally{
    Pop-Location
    foreach($entry in @(@('CARGO_TARGET_DIR',$oldTarget),@('CARGO_ENCODED_RUSTFLAGS',$oldFlags),@('CARGO_PROFILE_RELEASE_STRIP',$oldStrip),@('CARGO_BUILD_JOBS',$oldJobs),@('SHERPA_ONNX_ARCHIVE_DIR',$oldArchive))){
        if($null -eq $entry[1]){Remove-Item -LiteralPath ('Env:'+ $entry[0]) -ErrorAction SilentlyContinue}else{Set-Item -LiteralPath ('Env:'+ $entry[0]) -Value $entry[1]}
    }
}
