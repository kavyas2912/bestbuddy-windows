$ErrorActionPreference='Stop'
Set-Location (Join-Path $PSScriptRoot '..')
function Run-Checked([string]$exe,[string[]]$commandArgs,[string]$label){ & $exe @commandArgs; if($LASTEXITCODE -ne 0){ throw "$label failed with code $LASTEXITCODE." } }
foreach($d in @("models/kws","models/denoise")){ New-Item -ItemType Directory -Force -Path $d | Out-Null }

# Official English streaming KWS model. Low-latency always-on stage.
$kwsRoot='models/kws/sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01'
$kwsFiles=@('tokens.txt','bpe.model','encoder-epoch-12-avg-2-chunk-16-left-64.int8.onnx','decoder-epoch-12-avg-2-chunk-16-left-64.onnx','joiner-epoch-12-avg-2-chunk-16-left-64.int8.onnx')
function Test-KwsPack {
  foreach($name in $kwsFiles){
    $file=Join-Path $kwsRoot $name
    if(-not (Test-Path -LiteralPath $file -PathType Leaf)){ return $false }
    if((Get-Item -LiteralPath $file).Length -eq 0){ return $false }
  }
  return $true
}
if(-not (Test-KwsPack)){
  Write-Host '[KWS] Model is absent or incomplete; retrieving the complete pack.'
  $tmp="$env:TEMP\buddy-kws.tar.bz2"
  Run-Checked curl.exe @('-L','--fail','--retry','8','--retry-delay','3','--retry-all-errors','-o',$tmp,'https://github.com/k2-fsa/sherpa-onnx/releases/download/kws-models/sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01.tar.bz2') 'KWS model download'
  Run-Checked tar.exe @('-xjf',$tmp,'-C','models/kws') 'KWS model extraction'
  Remove-Item $tmp -Force -ErrorAction SilentlyContinue
}
if(-not (Test-KwsPack)){ throw "KWS model remains incomplete after preparation: $kwsRoot" }

# Custom default wake phrase. The application remains usable in VAD always-on mode if token generation is unavailable.
$raw='HEY BUDDY @HEY_BUDDY'
$keywordFile='models/kws/keywords.txt'
if((Test-Path $keywordFile) -and ((Get-Item $keywordFile).Length -gt 10)) {
  Write-Host '[reuse] Existing KWS keyword file; generation and pip install skipped.'
} else {
Set-Content models/kws/keywords_raw.txt $raw -Encoding UTF8
$python = Get-Command py -ErrorAction SilentlyContinue
if(-not $python){ $python = Get-Command python -ErrorAction SilentlyContinue }
if($python){
  & $python.Source -c 'import sentencepiece' 2>$null
  if($LASTEXITCODE -ne 0) {
    & $python.Source -m pip install --disable-pip-version-check --quiet sentencepiece 2>$null
    if($LASTEXITCODE -ne 0){ Write-Host '[WARN] Python sentencepiece install unavailable; KWS custom keyword generation may be skipped.' }
  }
  $code=@'
import sys, pathlib
try:
    import sentencepiece as spm
except Exception:
    print("sentencepiece package unavailable; leaving KWS in fallback mode")
    raise SystemExit(2)
root=pathlib.Path("models/kws/sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01")
sp=spm.SentencePieceProcessor(model_file=str(root/"bpe.model"))
parts=sp.encode("HEY BUDDY", out_type=str)
pathlib.Path("models/kws/keywords.txt").write_text(" ".join(parts)+" @HEY_BUDDY\n",encoding="utf-8")
print("Generated KWS keyword file:", " ".join(parts)+" @HEY_BUDDY")
'@
  $tmpPy=Join-Path $env:TEMP 'buddy_make_kws.py'; Set-Content $tmpPy $code -Encoding UTF8
  & $python.Source $tmpPy
  if($LASTEXITCODE -ne 0){ Write-Host '[WARN] Could not generate custom Hey Buddy KWS file; VAD always-on fallback remains enabled.' }
  Remove-Item $tmpPy -Force -ErrorAction SilentlyContinue
} else { Write-Host '[WARN] Python not found; VAD always-on voice remains enabled. Install Python later to generate custom KWS tokens.' }
}

# Optional low-latency speech enhancement. Failure here does NOT disable voice; the engine falls back to clean input.
if(-not (Test-Path 'models/denoise/gtcrn_simple.onnx')){
  try { Run-Checked curl.exe @('-L','--fail','--retry','5','--retry-delay','2','--retry-all-errors','-o','models/denoise/gtcrn_simple.onnx','https://github.com/k2-fsa/sherpa-onnx/releases/download/speech-enhancement-models/gtcrn_simple.onnx') 'GTCRN speech enhancement download' } catch { Write-Host "[WARN] GTCRN not downloaded: $($_.Exception.Message)" }
}

Write-Host '[OK] Voice pack preparation finished'
