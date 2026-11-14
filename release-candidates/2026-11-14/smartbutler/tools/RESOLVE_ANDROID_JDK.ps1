function Get-AndroidJdkInfo {
  param([string]$JdkPath)
  if (-not $JdkPath) { return $null }
  $JdkPath = $JdkPath.Trim('"').TrimEnd('\','/')
  if (-not (Test-Path -LiteralPath (Join-Path $JdkPath 'bin/java.exe')) -or
      -not (Test-Path -LiteralPath (Join-Path $JdkPath 'bin/javac.exe')) -or
      -not (Test-Path -LiteralPath (Join-Path $JdkPath 'release'))) { return $null }
  $release = Get-Content -LiteralPath (Join-Path $JdkPath 'release') -Raw
  if ($release -notmatch '(?m)^JAVA_VERSION="([^"]+)"') { return $null }
  $versionText = $Matches[1]
  if ($versionText -notmatch '^(\d+)(?:[.+_-]|$)') { return $null }
  [pscustomobject]@{ Home = [IO.Path]::GetFullPath($JdkPath); Version = $versionText; Major = [int]$Matches[1] }
}

function Resolve-AndroidJdk {
  param(
    [string]$RequestedHome,
    [string]$CacheRoot = (Join-Path $env:LOCALAPPDATA 'SmartAppsBuild/Toolchains'),
    [string[]]$CandidateHomes,
    [switch]$InstallIfMissing
  )
  if ($RequestedHome) {
    $explicitJdk = Get-AndroidJdkInfo $RequestedHome
    if ($explicitJdk -and $explicitJdk.Major -in @(17,21)) { return $explicitJdk }
    $found = if ($explicitJdk) { "Java $($explicitJdk.Version)" } else { 'no complete JDK' }
    throw "The requested -JavaHome '$RequestedHome' contains $found. Choose a JDK 17 or 21, including javac.exe."
  }
  if (-not $PSBoundParameters.ContainsKey('CandidateHomes')) {
    $homes = @($env:JAVA_HOME)
    foreach ($base in @($env:ProgramFiles,${env:ProgramFiles(x86)})) {
      if (-not $base) { continue }
      $homes += Join-Path $base 'Android/Android Studio/jbr'
      foreach ($vendor in @('Java','Eclipse Adoptium','Microsoft','Amazon Corretto','BellSoft','Zulu')) {
        $parent = Join-Path $base $vendor
        if (Test-Path -LiteralPath $parent) { $homes += @(Get-ChildItem -LiteralPath $parent -Directory -ErrorAction SilentlyContinue | ForEach-Object FullName) }
      }
    }
    foreach ($parent in @((Join-Path $env:USERPROFILE '.jdks'),$CacheRoot)) {
      if (Test-Path -LiteralPath $parent) { $homes += @(Get-ChildItem -LiteralPath $parent -Directory -ErrorAction SilentlyContinue | ForEach-Object FullName) }
    }
    $java = Get-Command java.exe -ErrorAction SilentlyContinue
    if ($java) { $homes += Split-Path -Parent (Split-Path -Parent $java.Source) }
    $CandidateHomes = $homes
  }
  foreach ($homePath in @($CandidateHomes | Where-Object { $_ } | Select-Object -Unique)) {
    $jdk = Get-AndroidJdkInfo $homePath
    if (-not $jdk) { continue }
    if ($jdk.Major -in @(17,21)) { return $jdk }
    Write-Host "Skipping Java $($jdk.Version) at $($jdk.Home); this build uses JDK 17 or 21."
  }
  if (-not $InstallIfMissing) {
    throw 'No JDK 17 or 21 was found. Install Eclipse Temurin JDK 21 from https://adoptium.net/temurin/releases/?version=21 or run BUILD_APK.cmd to download it automatically. You can also pass -JavaHome "C:\path\to\jdk-21".'
  }
  New-Item -ItemType Directory -Path $CacheRoot -Force | Out-Null
  $cacheFull = [IO.Path]::GetFullPath($CacheRoot).TrimEnd('\','/')
  $api = 'https://api.adoptium.net/v3/assets/latest/21/hotspot?architecture=x64&image_type=jdk&os=windows&vendor=eclipse'
  Write-Host 'No compatible JDK found. Preparing shared Eclipse Temurin JDK 21 (about 200 MB).'
  Write-Host "Shared tools folder: $cacheFull"
  [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
  $oldProgress = $ProgressPreference
  $part = $null
  try {
    $ProgressPreference = 'SilentlyContinue'
    $assets = @(Invoke-RestMethod -Uri $api -TimeoutSec 60)
    $package = $assets[0].binary.package
    $sha = [string]$package.checksum
    $uri = [Uri]$package.link
    if ($sha -notmatch '^[a-fA-F0-9]{64}$' -or $uri.Scheme -ne 'https' -or $uri.Host -notin @('github.com','api.adoptium.net')) { throw 'Unexpected JDK download metadata.' }
    $target = Join-Path $cacheFull ('temurin-21-' + $sha.Substring(0,16).ToLowerInvariant())
    $existing = Get-AndroidJdkInfo $target
    if ($existing -and $existing.Major -eq 21) { return $existing }
    if (Test-Path -LiteralPath $target) { throw "Incomplete JDK folder: $target. Rename that folder and retry." }
    $zip = Join-Path $cacheFull ('jdk21-' + $sha.ToLowerInvariant() + '.zip')
    if (-not (Test-Path -LiteralPath $zip)) {
      $part = Join-Path $cacheFull ('jdk21-' + [Guid]::NewGuid().ToString('N') + '.download')
      Invoke-WebRequest -UseBasicParsing -Uri $uri.AbsoluteUri -OutFile $part -TimeoutSec 1800
      if ((Get-FileHash -LiteralPath $part -Algorithm SHA256).Hash -ne $sha) { throw 'JDK download checksum mismatch. No JDK was installed.' }
      if (-not (Test-Path -LiteralPath $zip)) { Move-Item -LiteralPath $part -Destination $zip; $part=$null }
    }
    if ((Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash -ne $sha) { throw "Cached JDK checksum mismatch: $zip. Rename that archive and retry." }
    $stage = Join-Path $cacheFull ('jdk-stage-' + [Guid]::NewGuid().ToString('N'))
    Expand-Archive -LiteralPath $zip -DestinationPath $stage
    $installed = @(Get-ChildItem -LiteralPath $stage -Directory | ForEach-Object { Get-AndroidJdkInfo $_.FullName } | Where-Object { $_ -and $_.Major -eq 21 })
    if ($installed.Count -ne 1) { throw "Downloaded archive did not contain one complete JDK 21. Retained for diagnosis: $stage" }
    $from = [IO.Path]::GetFullPath($installed[0].Home)
    if (-not $from.StartsWith($cacheFull + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) -or
        -not $target.StartsWith($cacheFull + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'JDK extraction path escaped the tools folder.' }
    Move-Item -LiteralPath $from -Destination $target
    # Remove only the now-empty staging directory, never recursively.
    if (@(Get-ChildItem -LiteralPath $stage -Force).Count -eq 0) { Remove-Item -LiteralPath $stage }
    return Get-AndroidJdkInfo $target
  } catch {
    throw "JDK setup failed: $($_.Exception.Message) Install JDK 21 from https://adoptium.net/temurin/releases/?version=21 and retry with -JavaHome, or retry with working internet access."
  } finally {
    $ProgressPreference = $oldProgress
    if ($part -and (Test-Path -LiteralPath $part)) { Remove-Item -LiteralPath $part -Force }
  }
}

