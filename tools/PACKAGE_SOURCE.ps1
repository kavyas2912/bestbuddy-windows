param([Parameter(Mandatory=$true)][string]$ArchivePath)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$output = [IO.Path]::GetFullPath($ArchivePath)
if (Test-Path -LiteralPath $output) { throw "Archive already exists: $output" }
Push-Location $root
try {
    $dirty = & git status --porcelain --untracked-files=all
    if ($LASTEXITCODE -ne 0) { throw 'Not a Git repository.' }
    if ($dirty) { throw 'Commit the intended source changes before packaging.' }
    & git archive --format=zip --prefix=BestBuddy_Source/ --output=$output HEAD
    if ($LASTEXITCODE -ne 0) { throw 'Source archive creation failed.' }
    $hash = (Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash
    [IO.File]::WriteAllText($output + '.sha256', "$hash  $([IO.Path]::GetFileName($output))")
    Write-Host "Source ZIP: $output"
    Write-Host "SHA256: $hash"
} finally { Pop-Location }
