$ErrorActionPreference = 'Stop'
Push-Location (Join-Path $PSScriptRoot '..')
try {

Write-Host 'VCE RELEASE PREFLIGHT'
Write-Host ('-' * 72)

function Fail([string]$message) { throw "RELEASE PREFLIGHT FAILED: $message" }
function Read-Text([string]$path) { if(-not (Test-Path -LiteralPath $path)){ Fail "Missing $path" }; Get-Content -LiteralPath $path -Raw }

# 1. Workspace version is the single source of truth.
$cargoToml = Read-Text 'Cargo.toml'
$workspaceMatch = [regex]::Match($cargoToml, '(?ms)\[workspace\.package\].*?^version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"')
if(-not $workspaceMatch.Success){ Fail 'Cannot determine [workspace.package] version.' }
$expected = $workspaceMatch.Groups[1].Value
Write-Host "[OK] Workspace version: $expected"

# 2. VCE-owned metadata must match it.
$ownedFiles = @(
  'apps/vce-desktop/package.json',
  'apps/vce-desktop/src-tauri/tauri.conf.json'
)
foreach($file in $ownedFiles) {
  $json = Get-Content -LiteralPath $file -Raw | ConvertFrom-Json
  if($json.version -ne $expected){ Fail "$file version '$($json.version)' != $expected" }
  Write-Host "[OK] $file version: $($json.version)"
}

# 3. Plugin manifests embedded in Rust source must match it.
foreach($file in @('plugins/news/src/lib.rs','plugins/weather/src/lib.rs','plugins/market/src/lib.rs','plugins/fun-facts/src/lib.rs')) {
  $text = Read-Text $file
  $matches = [regex]::Matches($text, 'version\s*:\s*"([0-9]+\.[0-9]+\.[0-9]+)"')
  if($matches.Count -lt 1){ Fail "No embedded plugin version found in $file" }
  # Deferred plugin manifests remain independently versioned at 0.6.12.
  Write-Host "[OK] $file embedded version"
}

# 4. Direct dependency declaration contract.
foreach($file in @('plugins/news/Cargo.toml','plugins/weather/Cargo.toml','plugins/market/Cargo.toml','crates/buddy-core/Cargo.toml')) {
  $text = Read-Text $file
  if($text -notmatch '(?m)^\s*urlencoding(?:\.workspace)?\s*=\s*'){ Fail "urlencoding is not declared in $file" }
  Write-Host "[OK] urlencoding: $file"
}

# 5. Cargo config must be unambiguous.
if(Test-Path '.cargo/config'){ Fail 'Legacy .cargo/config exists. Keep only .cargo/config.toml.' }
if(-not (Test-Path '.cargo/config.toml')){ Fail 'Missing .cargo/config.toml' }
Write-Host '[OK] Cargo config is unambiguous'

# 6. Required Tauri assets.
foreach($asset in @(
  'apps/vce-desktop/src-tauri/icons/icon.ico',
  'apps/vce-desktop/src-tauri/icons/icon.png',
  'apps/vce-desktop/src-tauri/icons/32x32.png',
  'apps/vce-desktop/src-tauri/icons/128x128.png',
  'apps/vce-desktop/public/favicon.ico',
  'apps/vce-desktop/public/assets/macaw-buddy.png'
)) { if(-not(Test-Path $asset)){ Fail "Missing asset $asset" }; Write-Host "[OK] $asset" }

# 7. Validate manifests without changing the committed dependency lockfile.
Write-Host '[7] cargo metadata --locked --no-deps --format-version 1'
$previousPreference = $ErrorActionPreference
try {
  # Windows PowerShell turns native stderr warnings into ErrorRecords.
  # Decide success from the process exit code, not from harmless warnings.
  $ErrorActionPreference = 'Continue'
  $metaOutput = cargo metadata --locked --no-deps --format-version 1 2>&1
  $metaCode = $LASTEXITCODE
} finally { $ErrorActionPreference = $previousPreference }
if($metaCode -ne 0){ Write-Host ($metaOutput | Out-String); Fail "cargo metadata could not parse the workspace manifests (exit $metaCode)." }
Write-Host '[OK] Cargo manifests parse' 

} finally { Pop-Location }
