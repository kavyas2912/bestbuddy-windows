<# SmartButler local test ONLY. Trusts exactly the cert that signed the preview MSIX.
   Requires elevation. Never imports cert to TrustedRoot; never uses PFX/private key.
   The Store MSIX must remain UNSIGNED for Partner Center. #>
[CmdletBinding()]
param([string]$OutputDirectory, [switch]$CheckOnly)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$preview = if ($OutputDirectory) { [IO.Path]::GetFullPath($OutputDirectory) } else { Join-Path $root 'release-artifacts\msix\preview' }
$meta = Get-Content -LiteralPath (Join-Path $root 'apps\vce-desktop\package.json') -Raw | ConvertFrom-Json
if ([string]$meta.version -notmatch '^\d+\.\d+\.\d+$') { throw 'Invalid current preview version.' }
$msixVersion = "$($meta.version).0"
$package = Join-Path $preview "SmartButler_CLEAN_${msixVersion}_x64.msix"
$certificate = Join-Path $preview 'smartbutler-preview-devcert.cer'
try {
    foreach ($file in @($package, $certificate)) {
        if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "Missing preview artifact: $file. Run BUILD_PREVIEW.cmd first." }
    }
    if ($CheckOnly) { Write-Host "[OK] Current preview package found: $package"; Write-Host '[OK] Certificate file found. No certificate trust or installation changes made.'; exit 0 }
    $expected = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2 -ArgumentList $certificate
    $signature = Get-AuthenticodeSignature -LiteralPath $package
    if ($null -eq $signature.SignerCertificate) { throw 'Preview MSIX lacks an Authenticode signer. Do not trust any certificate.' }
    if ($signature.SignerCertificate.Thumbprint -ne $expected.Thumbprint) {
        throw 'Preview MSIX signature does not match the generated public CER. Refusing to change certificate trust.'
    }
    if ($expected.Subject -ne 'CN=SmartButler Local Preview') {
        throw "Unexpected developer certificate subject: $($expected.Subject)"
    }
    if ($expected.NotAfter -lt (Get-Date) -or $expected.NotBefore -gt (Get-Date)) { throw 'Development certificate is expired/not yet valid.' }
    Write-Host "[VERIFIED] MSIX signer and public certificate match: $($expected.Thumbprint)"
    $principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        Write-Host '[NOTICE] Administrator approval is needed to trust the local TEST certificate.'
        $arg = '-NoProfile -ExecutionPolicy Bypass -File "' + $PSCommandPath + '" -OutputDirectory "' + $preview + '"'
        $child = Start-Process -FilePath 'powershell.exe' -ArgumentList $arg -Verb RunAs -WindowStyle Normal -Wait -PassThru
        if ($child.ExitCode -ne 0) { throw "Elevated preview install returned exit $($child.ExitCode)" }
        exit 0
    }
    $ok = Read-Host 'Install/test locally using this certificate? Type YES'
    if ($ok -ne 'YES') { throw 'Cancelled without changing certificate trust.' }
    Import-Certificate -FilePath $certificate -CertStoreLocation 'Cert:\LocalMachine\TrustedPeople' -ErrorAction Stop | Out-Null
    Write-Host '[OK] Imported development certificate into LocalMachine\TrustedPeople (NOT TrustedRoot).'
    $check = Get-AuthenticodeSignature -LiteralPath $package
    if ($check.Status -ne 'Valid') { throw "Signature still not trusted: $($check.Status). MSIX was not installed." }
    Add-AppxPackage -Path $package -ErrorAction Stop
    Write-Host '[SUCCESS] SmartButler LocalPreview installed. Test onboarding, consent, mic, ASR and audible TTS.'
    Write-Host '[IMPORTANT] Only upload BUILD_STORE.cmd output to Partner Center (not the signed preview).'
    exit 0
} catch {
    Write-Host ('[FAILED] ' + $_.Exception.Message) -ForegroundColor Red
    exit 1
}

