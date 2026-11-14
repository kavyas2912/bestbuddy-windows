<#
SmartButler Microsoft Store MSIX build for Windows 11 x64.
Builds the current Smart Butler Tauri executable, stages the pinned OFFLINE voice models beside
it, and packages the result with Microsoft's SDK MakeAppx. No GitHub / Partner
Center write is performed. A Store package is unsigned intentionally: Microsoft
Store applies its own signature after certification.

Usage (repo root):
  BUILD_PREVIEW.cmd
  BUILD_STORE.cmd
  powershell -NoProfile -ExecutionPolicy Bypass -File tools\BUILD_STORE_MSIX_WINDOWS.ps1 -Mode Store

Identity comes from store\store-identity.json, which the user must populate
with their actual Microsoft Partner Center Package/Identity details.
#>
[CmdletBinding()]
param(
    [ValidateSet('Preview','Store')][string]$Mode = 'Preview',
    [string]$OutputDirectory = '',
    [switch]$OfflineModels,
    [switch]$KeepStaging,
    [string]$ReuseStagingDirectory = ''
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$source = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$packageDir = Join-Path $source 'store'
$frontend = Join-Path $source 'apps\vce-desktop'
$out = if ($OutputDirectory) { [IO.Path]::GetFullPath($OutputDirectory) } else { Join-Path $source ('release-artifacts\msix\' + $Mode.ToLowerInvariant()) }
$stage = ''
$oldTarget = $env:CARGO_TARGET_DIR
$oldSherpa = $env:SHERPA_ONNX_ARCHIVE_DIR
$oldNpmConfig = $env:NPM_CONFIG_USERCONFIG
$oldEncodedRustflags = $env:CARGO_ENCODED_RUSTFLAGS
$oldRustflags = $env:RUSTFLAGS
$oldReleaseStrip = $env:CARGO_PROFILE_RELEASE_STRIP
$successful = $false
function Need([string]$command) {
    if (-not (Get-Command $command -ErrorAction SilentlyContinue)) { throw "Missing $command. See store\README_STORE.md." }
}
function Run([string]$command, [string[]]$arguments, [string]$label) {
    Write-Host "[run] $label"
    & $command @arguments
    if ($LASTEXITCODE -ne 0) { throw "$label failed (exit $LASTEXITCODE)." }
}
function FileHash([string]$path) { return (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash.ToUpperInvariant() }

function Find-WindowsSdkTool([string]$toolName) {
    $found = Get-Command $toolName -ErrorAction SilentlyContinue
    if ($found -and $found.Source -and (Test-Path -LiteralPath $found.Source -PathType Leaf)) { return $found.Source }
    $roots = @()
    if (${env:ProgramFiles(x86)}) { $roots += (Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin') }
    if ($env:ProgramFiles) { $roots += (Join-Path $env:ProgramFiles 'Windows Kits\10\bin') }
    # WinApp CLI can install Microsoft.Windows.SDK.BuildTools in its cache.
    if ($env:USERPROFILE) { $roots += (Join-Path $env:USERPROFILE '.winapp') }
    if ($env:WINAPP_CLI_CACHE_DIRECTORY) { $roots += $env:WINAPP_CLI_CACHE_DIRECTORY }
    foreach ($dir in @($roots | Select-Object -Unique)) {
        if (-not (Test-Path -LiteralPath $dir -PathType Container)) { continue }
        $matches = @(Get-ChildItem -LiteralPath $dir -Filter $toolName -Recurse -File -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match '(?i)[\\/]x64[\\/]' } |
            Sort-Object -Property FullName -Descending)
        if ($matches.Count -gt 0) { return $matches[0].FullName }
    }
    throw "Cannot find the Microsoft Windows SDK x64 $toolName. Install the Windows SDK Build Tools through Visual Studio Installer (Windows 11 SDK) or run winapp update --setup-sdks stable, then retry."
}

function XmlEscape([string]$value) { return [System.Security.SecurityElement]::Escape($value) }

try {
    if ([Environment]::OSVersion.Platform -ne 'Win32NT') { throw 'MSIX build requires Windows 11 x64.' }
    foreach ($program in @('cargo.exe','rustc.exe','node.exe','npm.cmd','npx.cmd','python.exe','winapp.exe')) { Need $program }
    $rustTriple = (& rustc -vV | Out-String)
    if ($LASTEXITCODE -ne 0 -or $rustTriple -notmatch 'host:\s*x86_64-pc-windows-msvc') {
        throw 'Rust host must be x86_64-pc-windows-msvc. Install the stable MSVC Rust toolchain.'
    }
    $meta = Get-Content (Join-Path $frontend 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json
    $version = [string]$meta.version
    $releaseVersion = [regex]::Match($version, '^([1-9][0-9]*)\.([0-9]+)\.([0-9]+)$')
    if (-not $releaseVersion.Success) { throw "Unexpected SmartButler version $version; update the MSIX version mapping explicitly." }
    # Windows Store packages require a nonzero first version field and zero last field.
    $msixVersion = "$($releaseVersion.Groups[1].Value).$($releaseVersion.Groups[2].Value).$($releaseVersion.Groups[3].Value).0"
    $identity = $null
    if ($Mode -eq 'Store') {
        $identityFile = Join-Path $packageDir 'store-identity.json'
        if (-not (Test-Path -LiteralPath $identityFile -PathType Leaf)) {
            throw 'Copy store\store-identity.example.json to store\store-identity.json and insert the EXACT Partner Center package identity before building the Store MSIX.'
        }
        $identity = Get-Content -LiteralPath $identityFile -Raw | ConvertFrom-Json
        foreach ($field in @('IdentityName','Publisher','PublisherDisplayName')) {
            if ([string]::IsNullOrWhiteSpace([string]$identity.$field)) { throw "Missing $field in store-identity.json" }
        }
        if ([string]$identity.IdentityName -match 'REPLACE|EXAMPLE|PLACEHOLDER' -or [string]$identity.Publisher -match 'REPLACE|EXAMPLE|PLACEHOLDER') {
            throw 'Store identity placeholders are not allowed. Use Partner Center Product management > App identity.'
        }
        if ($identity.PSObject.Properties.Name -contains 'Version' -and -not [string]::IsNullOrWhiteSpace([string]$identity.Version)) {
            $msixVersion = [string]$identity.Version
        }
    } else {
        $identity = [pscustomobject]@{
            IdentityName = 'SmartButler.CleanPreviewR5'
            Publisher = 'CN=SmartButler Local Preview'
            PublisherDisplayName = 'SmartButler Preview'
        }
    }
    if ([string]$identity.IdentityName -notmatch '^[A-Za-z0-9][A-Za-z0-9._\-]{2,49}$') { throw 'Invalid MSIX IdentityName; use the exact valid Partner Center package name.' }
    if ([string]$identity.Publisher -notmatch '^CN=.+') { throw 'Publisher must be a full Partner Center publisher DN, commonly beginning CN=.' }
    $segments = $msixVersion.Split('.')
    if ($segments.Count -ne 4 -or $segments[3] -ne '0') { throw 'MSIX Version must have four components and end in .0' }
    foreach ($s in $segments) { $n = 0; if (-not [int]::TryParse($s,[ref]$n) -or $n -lt 0 -or $n -gt 65535) { throw "Invalid MSIX Version $msixVersion" } }
    if ($segments[0] -eq '0') { throw 'MSIX version first field must not be zero.' }
    Write-Host "[identity] $($identity.IdentityName) / $($identity.Publisher), version $msixVersion"

    if ($meta.productName -ne 'Smart Butler' -or $meta.identifier -ne 'com.vce.smartbutler.cleanr5') { throw 'Wrong source: this builder requires the Smart Butler CLEAN identity.' }
    Run 'python.exe' @((Join-Path $PSScriptRoot 'verify_butler_release.py'), '--branding-only') 'Verify Smart Butler branding and poses'
    # Always fail BEFORE downloads/build if a requested release identity is missing.
    Run 'powershell.exe' @('-NoProfile','-ExecutionPolicy','Bypass','-File',(Join-Path $PSScriptRoot 'RELEASE_PREFLIGHT_WINDOWS.ps1')) 'Rust/Tauri source preflight'
    [IO.Directory]::CreateDirectory($out) | Out-Null
    if ($ReuseStagingDirectory) {
        $stage = [IO.Path]::GetFullPath($ReuseStagingDirectory)
        $boundary = $out.TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
        if (-not $stage.StartsWith($boundary, [StringComparison]::OrdinalIgnoreCase) -or -not (Test-Path -LiteralPath $stage -PathType Container)) {
            throw 'Reusable staging must be an existing directory inside this output directory.'
        }
    } else {
        $stage = Join-Path $out ('stage-' + [guid]::NewGuid().ToString('N'))
    }
    [IO.Directory]::CreateDirectory($stage) | Out-Null

    # Build the verified voice pack DIRECTLY into this run's isolated staging
    # directory. Never access/overwrite $source\models: older extracted folders
    # can contain inherited restrictive ACLs (WinError 5). None of these host
    # paths are written into the app package; models/ lives beside the EXE.
    $modelCache = if ($env:BESTBUDDY_VOICE_BUILD_CACHE) { $env:BESTBUDDY_VOICE_BUILD_CACHE } else { Join-Path $env:LOCALAPPDATA 'SmartButler\build-cache\voice-models' }
    $modelArgs = @((Join-Path $PSScriptRoot 'prepare_voice_models.py'), '--target-root', $stage,
        '--cache', $modelCache)
    if ($OfflineModels) { $modelArgs += '--offline' }
    Run 'python.exe' $modelArgs 'Acquire and verify offline voice models into isolated MSIX staging'
    Run 'python.exe' @((Join-Path $PSScriptRoot 'prepare_voice_models.py'), '--target-root', $stage,
        '--cache', $modelCache, '--check-only') 'Verify staged offline voice models'
    $keywords = Join-Path $stage 'models\kws\keywords.txt'
    if (-not (Test-Path -LiteralPath $keywords) -or (Get-Item -LiteralPath $keywords).Length -lt 10) {
        throw 'The wake-word keyword file is incomplete.'
    }

    # Static native speech runtime (BUILD dependency; it does not download for users).
    $archiveName = 'sherpa-onnx-v1.13.8-win-x64-static-MT-Release-lib.tar.bz2'
    $archiveDir = Join-Path $source 'tools\cache\sherpa-onnx'
    $archive = Join-Path $archiveDir $archiveName
    $archiveSha = '56FFCF3C454C1F14F7BC9887286CC8143E7E542DC632804E1C447D5F8D534EAF'
    [IO.Directory]::CreateDirectory($archiveDir) | Out-Null
    if ((-not (Test-Path -LiteralPath $archive)) -and $OfflineModels) { throw "Offline build missing native speech archive: $archive" }
    if (-not (Test-Path -LiteralPath $archive)) {
        Need 'curl.exe'
        $part = "$archive.part"
        $url = "https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.8/$archiveName"
        Run 'curl.exe' @('-L','--fail','--retry','5','--retry-delay','3','-o',$part,$url) 'Download pinned native speech library'
        if ((FileHash $part) -ne $archiveSha) { throw 'Downloaded native speech library failed SHA-256 check.' }
        Move-Item -LiteralPath $part -Destination $archive
    }
    if ((FileHash $archive) -ne $archiveSha) { throw 'Pinned native speech library SHA-256 check failed.' }
    $env:SHERPA_ONNX_ARCHIVE_DIR = $archiveDir

    # Keep the build target short; previous MSVC/WiX builds had long-path issues.
    $target = if ($env:BUTLER_STORE_TARGET) { [IO.Path]::GetFullPath($env:BUTLER_STORE_TARGET) } else { Join-Path $env:LOCALAPPDATA 'SmartButler\cargo-targets\store-release' }
    $env:CARGO_TARGET_DIR = $target
    [IO.Directory]::CreateDirectory($target) | Out-Null
    # The app can store private data at runtime, but the published binary must
    # not include the developer's absolute source/build/home directory paths.
    # Rust accepts unit-separator encoded flags, including paths with spaces.
    # Refuse ambiguous user-supplied RUSTFLAGS to avoid silently overriding them.
    if ($oldRustflags -and (-not $oldEncodedRustflags)) {
        throw 'RUSTFLAGS is set. Clear it for this Store build or provide equivalent CARGO_ENCODED_RUSTFLAGS.'
    }
    $remapFlags = @(
        '--remap-path-prefix=' + $source + '=/smartbutler/source',
        '--remap-path-prefix=' + $target + '=/smartbutler/build'
    )
    if ($env:USERPROFILE) { $remapFlags += '--remap-path-prefix=' + $env:USERPROFILE + '=/smartbutler/user' }
    if ($oldEncodedRustflags) { $remapFlags = @($oldEncodedRustflags) + $remapFlags }
    $env:CARGO_ENCODED_RUSTFLAGS = [string]::Join([char]31, $remapFlags)
    # Strip link/debug information that can otherwise retain local absolute paths.
    # Runtime functionality and model inference are unchanged.
    $env:CARGO_PROFILE_RELEASE_STRIP = 'debuginfo'

    # Install only missing frontend dependencies. No redundant full unit-test pass.
    $vite = Join-Path $frontend 'node_modules\.bin\vite.cmd'
    if (-not (Test-Path -LiteralPath $vite)) {
        $npmConfig = Join-Path $frontend '.npmrc.vce-setup'
        [IO.File]::WriteAllText($npmConfig, "registry=https://registry.npmjs.org/`nfund=false`naudit=false`n")
        $env:NPM_CONFIG_USERCONFIG = $npmConfig
        try {
            Push-Location $frontend
            try { Run 'npm.cmd' @('ci','--ignore-scripts','--include=optional','--no-audit','--no-fund') 'Install locked Tauri frontend dependencies' }
            finally { Pop-Location }
        } finally {
            if ($null -eq $oldNpmConfig) { Remove-Item Env:NPM_CONFIG_USERCONFIG -ErrorAction SilentlyContinue } else { $env:NPM_CONFIG_USERCONFIG = $oldNpmConfig }
            Remove-Item -LiteralPath $npmConfig -Force -ErrorAction SilentlyContinue
        }
    }
    # Keep the pinned frontend tools visible to nested Tauri/npm commands.
    $env:PATH = (Join-Path $frontend 'node_modules/.bin') + [IO.Path]::PathSeparator + $env:PATH
    $esbuildSetup = Join-Path $frontend 'node_modules\esbuild\install.js'
    if (-not (Test-Path -LiteralPath $esbuildSetup)) { throw 'esbuild package missing. Check npm installation.' }
    $esbuildBin = Join-Path $frontend 'node_modules\@esbuild\win32-x64\esbuild.exe'
    if (-not (Test-Path -LiteralPath $esbuildBin)) {
        # Some Windows npm versions omit optional platform binaries when
        # lifecycle scripts are blocked. Pin the ABI-compatible lockfile build.
        Push-Location $frontend
        try {
            Run 'npm.cmd' @('install','--no-save','--ignore-scripts','--include=optional','--no-audit',
                '--no-fund','--package-lock=false','@esbuild/win32-x64@0.24.2') 'Repair pinned esbuild Windows binary'
        } finally { Pop-Location }
    }
    if (-not (Test-Path -LiteralPath $esbuildBin)) { throw "Required Windows esbuild binary missing: $esbuildBin" }
    Push-Location $frontend
    try {
        Run 'node.exe' @($esbuildSetup) 'Verify native esbuild binary'
        # Tauri config beforeBuildCommand already runs npm build, and release
        # build includes the current Smart Butler dashboard and compiled JS into the EXE.
        Run 'node.exe' @((Join-Path $frontend 'node_modules/typescript/bin/tsc'),'--noEmit') 'Check frontend types'
        Run 'node.exe' @((Join-Path $frontend 'node_modules/vite/bin/vite.js'),'build') 'Build Smart Butler frontend from this source folder'
        Run 'python.exe' @((Join-Path $PSScriptRoot 'verify_butler_release.py'), '--branding-only', '--dist') 'Reject old frontend and removed Music/Video panels'
        $buildConfig = Join-Path $out 'native-build.config.json'
        [IO.File]::WriteAllText($buildConfig, '{"build":{"beforeBuildCommand":""}}')
        Run 'node.exe' @((Join-Path $frontend 'node_modules/@tauri-apps/cli/tauri.js'),'build','--no-bundle','--config',$buildConfig) 'Compile Tauri x64 release executable'
    } finally { Pop-Location }
    $exe = Join-Path $target 'release\smart-butler-desktop.exe'
    if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw "Release executable not found: $exe" }
    Copy-Item -LiteralPath $exe -Destination (Join-Path $stage 'smart-butler-desktop.exe')
    # The SHA-256 verified model pack is already present at $stage\models.
    # Do not traverse legacy source-model directories or import user data.
    foreach ($dll in @(Get-ChildItem -LiteralPath (Join-Path $target 'release') -Filter '*.dll' -File)) {
        Copy-Item -LiteralPath $dll.FullName -Destination (Join-Path $stage $dll.Name)
    }
    foreach ($needPath in @(
        'models\asr\sherpa-onnx-whisper-base.en\base.en-encoder.int8.onnx',
        'models\asr\sherpa-onnx-whisper-base.en\base.en-decoder.int8.onnx',
        'models\tts\vits-piper-en_US-lessac-medium\en_US-lessac-medium.onnx',
        'models\tts\vits-piper-en_US-lessac-medium\espeak-ng-data\phontab',
        'models\vad\silero_vad.onnx',
        'models\kws\keywords.txt',
        'models\denoise\gtcrn_simple.onnx')) {
        if (-not (Test-Path -LiteralPath (Join-Path $stage $needPath) -PathType Leaf)) { throw "Packaged voice asset missing: $needPath" }
    }
    $assetsTarget = Join-Path $stage 'Assets'
    Copy-Item -LiteralPath (Join-Path $packageDir 'Assets') -Destination $assetsTarget -Recurse

    $n = XmlEscape ([string]$identity.IdentityName)
    $pub = XmlEscape ([string]$identity.Publisher)
    $pubDisplay = XmlEscape ([string]$identity.PublisherDisplayName)
    $v = XmlEscape $msixVersion
    $manifest = @"
<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
         xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
         xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
         IgnorableNamespaces="uap rescap">
  <Identity Name="$n" Publisher="$pub" Version="$v" ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>Smart Butler</DisplayName>
    <PublisherDisplayName>$pubDisplay</PublisherDisplayName>
    <Description>Smart Butler personal assistant</Description>
    <Logo>Assets\StoreLogo.png</Logo>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.19041.0" MaxVersionTested="10.0.26100.0" />
  </Dependencies>
  <Resources><Resource Language="en-US" /></Resources>
  <Applications>
    <Application Id="SmartButler" Executable="smart-butler-desktop.exe" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="Smart Butler" Description="Your local voice companion"
          Square150x150Logo="Assets\Square150x150Logo.png" Square44x44Logo="Assets\Square44x44Logo.png"
          BackgroundColor="transparent">
        <uap:DefaultTile Square71x71Logo="Assets\Square71x71Logo.png"
            Wide310x150Logo="Assets\Wide310x150Logo.png"
            Square310x310Logo="Assets\Square310x310Logo.png" ShortName="Smart Butler" />
        <uap:SplashScreen Image="Assets\SplashScreen.png" />
      </uap:VisualElements>
    </Application>
  </Applications>
  <Capabilities>
    <Capability Name="internetClient" />
    <rescap:Capability Name="runFullTrust" />
    <DeviceCapability Name="microphone" />
  </Capabilities>
</Package>
"@
    $manifestPath = Join-Path $stage 'AppxManifest.xml'
    [IO.File]::WriteAllText($manifestPath, $manifest, (New-Object System.Text.UTF8Encoding($false)))
    [xml]$validatedManifest = Get-Content -LiteralPath $manifestPath -Raw
    if ($validatedManifest.Package.Identity.Version -ne $msixVersion) { throw 'Generated MSIX version mismatch.' }
    # Windows schema: restricted Capability must precede DeviceCapability.
    $items = @($validatedManifest.Package.Capabilities.ChildNodes | Where-Object { $_.NodeType -eq [System.Xml.XmlNodeType]::Element })
    $seenDevice = $false
    foreach ($item in $items) {
        if ($item.LocalName -eq 'DeviceCapability') { $seenDevice = $true }
        elseif ($seenDevice -and $item.LocalName -eq 'Capability') { throw 'Manifest capability ordering is invalid.' }
    }
    if ($manifest -notmatch '<rescap:Capability Name="runFullTrust" />') { throw 'Missing full-trust capability.' }

    # Only an allowlisted executable, verified model assets and genuine Store
    # visuals may enter the MSIX; reject secrets, local data and embedded
    # developer-machine paths before packaging.
    $stageAudit = Join-Path $out 'privacy-audit-stage.json'
    Run 'python.exe' @((Join-Path $PSScriptRoot 'audit_store_payload.py'), '--stage', $stage,
        '--build-home', [string]$env:USERPROFILE, '--report', $stageAudit) 'Audit Store staging (see privacy-audit-stage.json for rejected files)'


    $filename = "SmartButler_CLEAN_${msixVersion}_x64.msix"
    $artifact = Join-Path $out $filename
    if (Test-Path -LiteralPath $artifact) { throw "Existing artifact will not be overwritten: $artifact. Specify -OutputDirectory." }
    # Invoke Microsoft's SDK MakeAppx.exe DIRECTLY.  Do not invoke `winapp tool`
    # with native arguments: that was the cause of the previous silent help/exit.
    # Do not invoke `winapp pack`: it can regenerate/move the corrected manifest.
    $makeappx = Find-WindowsSdkTool 'makeappx.exe'
    Run $makeappx @('pack','/o','/h','SHA256','/d',$stage,'/p',$artifact) 'MakeAppx: build MSIX from validated staged manifest'
    if (-not (Test-Path -LiteralPath $artifact -PathType Leaf)) { throw "MakeAppx produced no package: $artifact" }
    if ($Mode -eq 'Preview') {
        $cert = Join-Path $out 'smartbutler-preview-devcert.pfx'
        $passwordFile = Join-Path $out 'PREVIEW_CERT_PASSWORD.txt'
        # Generate a unique non-default certificate password. Never stage PFX/password.
        $random = New-Object byte[] 24
        $rng = [Security.Cryptography.RandomNumberGenerator]::Create()
        try { $rng.GetBytes($random) } finally { $rng.Dispose() }
        $password = [Convert]::ToBase64String($random)
        if (Test-Path -LiteralPath $cert) { throw "Preview PFX already exists: $cert; use a new output directory." }
        Run 'winapp.exe' @('cert','generate','--manifest',$manifestPath,'--output',$cert,'--password',$password,'--export-cer') 'Create a unique local test certificate'
        # Keep the secret OUTSIDE the MSIX. It exists only for the owner's preview.
        [IO.File]::WriteAllText($passwordFile, $password, (New-Object System.Text.UTF8Encoding($false)))
        # Use Windows SDK SignTool directly. WinApp CLI 'sign' argument forms
        # vary by version; SDK SignTool has stable, explicit switches.
        $signtool = Find-WindowsSdkTool 'signtool.exe'
        Run $signtool @('sign','/fd','SHA256','/f',$cert,'/p',$password,$artifact) 'Sign local-test preview with Windows SDK SignTool'
        $cer = [IO.Path]::ChangeExtension($cert, '.cer')
        if (-not (Test-Path -LiteralPath $cer -PathType Leaf)) { throw "Preview public certificate missing: $cer" }
    }
    if (-not (Test-Path -LiteralPath $artifact -PathType Leaf)) { throw "MSIX not found: $artifact" }
    $msixAudit = Join-Path $out 'privacy-audit-msix.json'
    Run 'python.exe' @((Join-Path $PSScriptRoot 'audit_store_payload.py'), '--msix', $artifact,
        '--build-home', [string]$env:USERPROFILE, '--report', $msixAudit) 'Audit completed MSIX package (see privacy-audit-msix.json)'


    # ZIP/MSIX manifest sanity checks. Full install, app launch, and WACK must
    # still be run on the real Windows 11 computer before Store submission.
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [System.IO.Compression.ZipFile]::OpenRead($artifact)
    try {
        $entries = @($zip.Entries | ForEach-Object { $_.FullName.Replace('\','/') })
        foreach ($need in @('smart-butler-desktop.exe','models/asr/sherpa-onnx-whisper-base.en/base.en-encoder.int8.onnx',
                 'models/tts/vits-piper-en_US-lessac-medium/en_US-lessac-medium.onnx',
                 'models/tts/vits-piper-en_US-lessac-medium/espeak-ng-data/phontab',
                 'models/vad/silero_vad.onnx','models/kws/keywords.txt','models/denoise/gtcrn_simple.onnx')) {
            if ($entries -notcontains $need) { throw "MSIX missing required payload: $need" }
        }
        $forbidden = @($entries | Where-Object { $_ -match '(?i)(^|/)(smartbutler-preview-devcert\.pfx|store-identity\.json|profile\.json|contacts\.json|butler\.log)$' -or $_ -match '(?i)\.gguf$' })
        if ($forbidden.Count) { throw "Personal data, signing keys or optional LLM files accidentally bundled: $($forbidden -join ', ')" }
    } finally { $zip.Dispose() }
    $hash = FileHash $artifact
    [IO.File]::WriteAllText("$artifact.sha256", "$hash  $filename`n")
    $summary = [ordered]@{
        mode = $Mode
        source_version = $version
        msix_version = $msixVersion
        identity_name = [string]$identity.IdentityName
        publisher = [string]$identity.Publisher
        artifact = $artifact
        sha256 = $hash
        contains_offline_voice_models = $true
        native_build_and_device_voice_test = 'Native build completed on Windows; real microphone/speaker acceptance remains required'
        store_submission = 'NOT SUBMITTED'
    }
    [IO.File]::WriteAllText((Join-Path $out "SmartButler_${msixVersion}_build-info.json"), ($summary | ConvertTo-Json -Depth 5))
    Write-Host "[OK] MSIX: $artifact"
    Write-Host "[OK] SHA-256: $hash"
    if ($Mode -eq 'Preview') { Write-Host '[NEXT] Trust the preview dev certificate only on your own PC and install the MSIX. See README_STORE.md.' }
    else { Write-Host '[NEXT] Run Store certification checks and submit the unsigned MSIX via Partner Center.' }
    $successful = $true
} finally {
    if ($null -eq $oldTarget) { Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue } else { $env:CARGO_TARGET_DIR = $oldTarget }
    if ($null -eq $oldSherpa) { Remove-Item Env:SHERPA_ONNX_ARCHIVE_DIR -ErrorAction SilentlyContinue } else { $env:SHERPA_ONNX_ARCHIVE_DIR = $oldSherpa }
    if ($null -eq $oldNpmConfig) { Remove-Item Env:NPM_CONFIG_USERCONFIG -ErrorAction SilentlyContinue } else { $env:NPM_CONFIG_USERCONFIG = $oldNpmConfig }
    if ($null -eq $oldEncodedRustflags) { Remove-Item Env:CARGO_ENCODED_RUSTFLAGS -ErrorAction SilentlyContinue } else { $env:CARGO_ENCODED_RUSTFLAGS = $oldEncodedRustflags }
    if ($null -eq $oldRustflags) { Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue } else { $env:RUSTFLAGS = $oldRustflags }
    if ($null -eq $oldReleaseStrip) { Remove-Item Env:CARGO_PROFILE_RELEASE_STRIP -ErrorAction SilentlyContinue } else { $env:CARGO_PROFILE_RELEASE_STRIP = $oldReleaseStrip }
    if ($successful -and (-not $KeepStaging) -and $stage -and (Test-Path -LiteralPath $stage)) {
        Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue
    } elseif ((-not $successful) -and $stage -and (Test-Path -LiteralPath $stage)) {
        Write-Warning "Build did not complete; staging files retained for diagnosis: $stage"
    }
}
