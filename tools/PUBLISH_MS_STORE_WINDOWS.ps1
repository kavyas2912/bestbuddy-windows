param(
    [Parameter(Mandatory=$true)][string]$InstallerPath,
    [Parameter(Mandatory=$true)][string]$InstallerUrl,
    [Parameter(Mandatory=$true)][string]$ProductId,
    [string]$PackageId,
    [switch]$Submit
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'SHA256_WINDOWS.ps1')

# Microsoft Store's MSI/EXE submission API fetches an HTTPS installer URL; it
# does not accept a source ZIP or upload bytes from this script. Use only after
# a signed installer has passed Windows device acceptance and Partner Center
# has an existing app plus its first manually-created submission.
$installer = (Resolve-Path -LiteralPath $InstallerPath -ErrorAction Stop).Path
if ([IO.Path]::GetExtension($installer) -ine '.msi') { throw 'Expected a signed Windows MSI installer, not a source ZIP or debug executable.' }
$uri = [uri]$InstallerUrl
if ($uri.Scheme -ne 'https' -or $uri.Host -eq '') { throw 'InstallerUrl must be a public HTTPS URL for the exact MSI.' }
if ($ProductId -notmatch '^[A-Za-z0-9-]+$') { throw 'Invalid Partner Center product ID.' }
$file = [IO.File]::OpenRead($installer)
try {
    $magic = New-Object byte[] 8
    if ($file.Read($magic,0,8) -ne 8 -or ([BitConverter]::ToString($magic) -ne 'D0-CF-11-E0-A1-B1-1A-E1')) {
        throw 'The installer does not have an MSI file signature.'
    }
} finally { $file.Dispose() }
$size = (Get-Item -LiteralPath $installer).Length
$hash = Get-BuddySha256 $installer
Write-Host "MSI: $installer"
Write-Host "Bytes: $size"
Write-Host "SHA256: $hash"
Write-Host "Store download URL: $InstallerUrl"
Write-Host "Partner Center product: $ProductId"
if (-not $Submit) {
    Write-Host 'Review only: no Store draft was changed or submitted. Add -Submit after Windows acceptance, signing, hosting, and Partner Center setup.'
    return
}
$signature = Get-AuthenticodeSignature -LiteralPath $installer
if ($signature.Status -ne 'Valid') { throw "MSI Authenticode signature is not valid: $($signature.Status). Sign and verify the release installer before Store submission." }

foreach ($name in @('BUDDY_STORE_TENANT_ID','BUDDY_STORE_CLIENT_ID','BUDDY_STORE_CLIENT_SECRET','BUDDY_STORE_SELLER_ID')) {
    if ([string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($name))) { throw "Missing $name. Set Partner Center Entra credentials in the process environment; never save secrets in this source tree." }
}
$remote = Invoke-WebRequest -Uri $InstallerUrl -Method Head -MaximumRedirection 5
if ($remote.StatusCode -ne 200) { throw "Installer URL returned HTTP $($remote.StatusCode)." }
$remoteLength = [string]$remote.Headers['Content-Length']
if ($remoteLength -and [long]$remoteLength -ne $size) { throw "Hosted MSI size $remoteLength differs from local MSI size $size." }

$tenant = [Environment]::GetEnvironmentVariable('BUDDY_STORE_TENANT_ID')
$token = Invoke-RestMethod -Method Post -Uri "https://login.microsoftonline.com/$tenant/oauth2/v2.0/token" -Body @{
    grant_type='client_credentials'
    client_id=[Environment]::GetEnvironmentVariable('BUDDY_STORE_CLIENT_ID')
    client_secret=[Environment]::GetEnvironmentVariable('BUDDY_STORE_CLIENT_SECRET')
    scope='https://api.store.microsoft.com/.default'
}
$headers = @{
    Authorization = "Bearer $($token.access_token)"
    'X-Seller-Account-Id' = [Environment]::GetEnvironmentVariable('BUDDY_STORE_SELLER_ID')
}
$base = 'https://api.store.microsoft.com/submission/v1/product/' + $ProductId
function StoreCall([string]$method,[string]$path,[object]$body=$null) {
    $args = @{ Method=$method; Uri=($base+$path); Headers=$headers; ContentType='application/json' }
    if ($null -ne $body) { $args.Body = ConvertTo-Json -InputObject $body -Depth 12 -Compress }
    $response = Invoke-RestMethod @args
    if (-not $response.isSuccess) {
        $errors = @($response.errors | ForEach-Object { $_.message }) -join '; '
        throw "Microsoft Store API rejected $method $path : $errors"
    }
    return $response.responseData
}

$packages = @( (StoreCall 'Get' '/packages').packages )
if (-not $PackageId) {
    if ($packages.Count -ne 1) { throw "Expected exactly one existing Store package; found $($packages.Count). Pass -PackageId for an existing package." }
    $PackageId = [string]$packages[0].packageId
}
$existing = @($packages | Where-Object { $_.packageId -eq $PackageId })
if ($existing.Count -ne 1) { throw "Store package $PackageId was not found in the draft." }
if ($existing[0].packageType -ne 'msi') { throw "Store package $PackageId is not an MSI entry." }
if (@($existing[0].architectures) -notcontains 'X64') { throw "Store package $PackageId is not configured for X64." }
$encodedId = [uri]::EscapeDataString($PackageId)
[void](StoreCall 'Patch' "/packages/$encodedId" @{ packageUrl=$InstallerUrl })
[void](StoreCall 'Post' '/packages/commit')
Write-Host 'Package URL updated and package commit requested. Waiting for Partner Center validation.'
$ready = $false
for ($attempt=0; $attempt -lt 60; $attempt++) {
    Start-Sleep -Seconds 10
    $status = StoreCall 'Get' '/status'
    if ($status.ongoingSubmissionId) { throw "Another Store submission is in progress: $($status.ongoingSubmissionId)" }
    if ($status.isReady) { $ready = $true; break }
}
if (-not $ready) { throw 'Store draft was not ready after 10 minutes; inspect Partner Center before retrying. Package URL was committed, but no submission was created.' }
$submission = StoreCall 'Post' '/submit'
Write-Host "Submitted for Microsoft Store certification: $($submission.submissionId)"
Write-Host 'Submission is asynchronous; check its status in Partner Center. This message does not mean certification or publication passed.'
