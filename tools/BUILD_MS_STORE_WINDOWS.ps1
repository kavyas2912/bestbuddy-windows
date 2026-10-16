param([string]$OutputDirectory = '')
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'SHA256_WINDOWS.ps1')
$source = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$output = if ($OutputDirectory) { [IO.Path]::GetFullPath($OutputDirectory) } else { Join-Path $source 'release-artifacts\microsoft-store' }
$previousTarget = $env:CARGO_TARGET_DIR
$previousArchive = $env:SHERPA_ONNX_ARCHIVE_DIR
$drive = $null
try {
    & (Join-Path $PSScriptRoot 'RELEASE_PREFLIGHT_WINDOWS.ps1')
    if (-not $?) { throw 'Release preflight failed.' }
    Write-Host '[publish] External and legacy resource reuse is disabled.'
    & (Join-Path $PSScriptRoot 'VERIFY_VOICE_MODELS_WINDOWS.ps1') -Root $source
    if (-not $?) { throw 'Publishing voice assets are incomplete. Stage the pinned basic voice pack in this source tree before building.' }
    # WiX 3 cannot open source paths longer than the legacy Windows limit. Keep
    # the installed layout unchanged while presenting the verified model trees
    # through short junction names inside the source root.
    $bundleModels = Join-Path $source '.bundle-models'
    [IO.Directory]::CreateDirectory($bundleModels) | Out-Null
    $modelAliases = @{
        'asr' = 'models\asr\sherpa-onnx-whisper-base.en'
        'tts' = 'models\tts\vits-piper-en_US-lessac-medium'
        'vad' = 'models\vad'
        'kws' = 'models\kws\sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01'
        'kws-keywords' = 'models\kws'
        'denoise' = 'models\denoise'
    }
    foreach ($name in $modelAliases.Keys) {
        $alias = Join-Path $bundleModels $name
        $modelTarget = [IO.Path]::GetFullPath((Join-Path $source $modelAliases[$name]))
        if (Test-Path -LiteralPath $alias) {
            $existing = Get-Item -LiteralPath $alias -Force
            if ($existing.LinkType -ne 'Junction' -or [IO.Path]::GetFullPath($existing.Target) -ne $modelTarget) {
                throw "Store model alias is occupied by a different target: $alias"
            }
        } else {
            New-Item -ItemType Junction -Path $alias -Target $modelTarget | Out-Null
        }
    }
    $longestBundleSource = Join-Path (Join-Path $bundleModels 'tts') 'espeak-ng-data\voices\!v\whisperf'
    if ($longestBundleSource.Length -ge 240) { throw "Store model alias path remains too long: $longestBundleSource" }
    Write-Host "[OK] WiX model aliases prepared under $bundleModels"
    $archive = Join-Path $source 'tools\cache\sherpa-onnx\sherpa-onnx-v1.13.8-win-x64-static-MT-Release-lib.tar.bz2'
    if (-not (Test-Path -LiteralPath $archive)) { throw 'Native sherpa archive missing. Run tools\SETUP_WINDOWS.cmd first.' }
    if ((Get-BuddySha256 $archive) -ne '56FFCF3C454C1F14F7BC9887286CC8143E7E542DC632804E1C447D5F8D534EAF') { throw 'Native sherpa archive checksum mismatch.' }
    if (-not (Test-Path -LiteralPath (Join-Path $source 'apps\vce-desktop\node_modules\.bin\tauri.cmd'))) { throw 'Frontend dependencies missing. Run tools\SETUP_WINDOWS.cmd first.' }

    $working = $source
    if ($source.Length -gt 100) {
        $drive = @('Z','Y','X','W','V','U','T') | Where-Object { -not (Test-Path "${_}:\") } | Select-Object -First 1
        if (-not $drive) { throw 'No free short drive for Store build. Use a shorter extraction folder.' }
        & subst.exe "${drive}:" $source
        if ($LASTEXITCODE -ne 0) { throw 'Could not map a short drive for Store build.' }
        $working = "${drive}:\"
    }
    $target = if ($env:BUDDY_STORE_TARGET) { [IO.Path]::GetFullPath($env:BUDDY_STORE_TARGET) } else { Join-Path $env:LOCALAPPDATA 'BestBuddy\cargo-targets\store-release' }
    if ($target.Length -ge 100) { throw 'Cargo build target is too long.' }
    [IO.Directory]::CreateDirectory($target) | Out-Null
    $env:CARGO_TARGET_DIR = $target
    $env:SHERPA_ONNX_ARCHIVE_DIR = Join-Path $working 'tools\cache\sherpa-onnx'
    Push-Location (Join-Path $working 'apps\vce-desktop')
    try {
        & npm.cmd run build
        if ($LASTEXITCODE -ne 0) { throw 'Frontend build failed.' }
        & npx.cmd tauri build --bundles msi,nsis --config src-tauri/tauri.microsoftstore.conf.json
        if ($LASTEXITCODE -ne 0) { throw 'Tauri publishing MSI/EXE build failed.' }
    } finally { Pop-Location }
    $msi = @(Get-ChildItem -LiteralPath (Join-Path $target 'release\bundle\msi') -Filter '*.msi' -File | Sort-Object LastWriteTime -Descending | Select-Object -First 1)
    if ($msi.Count -ne 1) { throw 'Tauri returned success but no MSI was found.' }
    & (Join-Path $PSScriptRoot 'VERIFY_MS_STORE_VOICE_WINDOWS.ps1') -MsiPath $msi[0].FullName
    if (-not $?) { throw 'MSI voice runtime verification failed.' }
    [IO.Directory]::CreateDirectory($output) | Out-Null
    $artifact = Join-Path $output $msi[0].Name
    if (Test-Path -LiteralPath $artifact) { throw "Artifact already exists: $artifact. Preserve it and choose a different OutputDirectory." }
    Copy-Item -LiteralPath $msi[0].FullName -Destination $artifact
    $hash = Get-BuddySha256 $artifact
    [IO.File]::WriteAllText("$artifact.sha256", "$hash  $($msi[0].Name)")
    $signature = Get-AuthenticodeSignature -LiteralPath $artifact
    Write-Host "Store MSI: $artifact"
    Write-Host "SHA256: $hash"
    Write-Host "Signature: $($signature.Status)"
    if ($signature.Status -ne 'Valid') { Write-Warning 'This MSI is a local build candidate. Microsoft Store submission requires valid signing of the installer and all bundled PE files.' }
    $installer = @(Get-ChildItem -LiteralPath (Join-Path $target 'release\bundle\nsis') -Filter '*.exe' -File | Sort-Object LastWriteTime -Descending | Select-Object -First 1)
    if ($installer.Count -ne 1) { throw 'Tauri returned success but no NSIS EXE was found.' }
    $exeArtifact = Join-Path $output $installer[0].Name
    if (Test-Path -LiteralPath $exeArtifact) { throw "Artifact already exists: $exeArtifact. Preserve it and choose a different OutputDirectory." }
    Copy-Item -LiteralPath $installer[0].FullName -Destination $exeArtifact
    $exeHash = Get-BuddySha256 $exeArtifact
    [IO.File]::WriteAllText("$exeArtifact.sha256", "$exeHash  $($installer[0].Name)")
    Write-Host "Publishing EXE: $exeArtifact"
    Write-Host "SHA256: $exeHash"
} finally {
    if ($drive) { & subst.exe "${drive}:" /D | Out-Null }
    if ($null -eq $previousTarget) { Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue } else { $env:CARGO_TARGET_DIR = $previousTarget }
    if ($null -eq $previousArchive) { Remove-Item Env:SHERPA_ONNX_ARCHIVE_DIR -ErrorAction SilentlyContinue } else { $env:SHERPA_ONNX_ARCHIVE_DIR = $previousArchive }
}
