$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$marker = Join-Path $root 'logs/BUILD_TARGET_PATH.txt'
if (-not (Test-Path -LiteralPath $marker -PathType Leaf)) { throw 'Build location is unknown. Run tools\SETUP_WINDOWS.cmd first.' }
$target = (Get-Content -LiteralPath $marker -Raw).Trim()
if (-not [IO.Path]::IsPathRooted($target)) { throw 'Build location is invalid. Run setup again.' }
$exe = Join-Path $target 'debug/vce-buddy-desktop.exe'
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw "Desktop executable not found: $exe. Complete setup first." }
Set-Location -LiteralPath $root
& $exe
exit $LASTEXITCODE
