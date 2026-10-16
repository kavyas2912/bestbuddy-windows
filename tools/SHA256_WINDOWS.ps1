function Get-BuddySha256([string]$Path) {
  $stream = [IO.File]::OpenRead([IO.Path]::GetFullPath($Path))
  $hasher = [Security.Cryptography.SHA256]::Create()
  try { return [BitConverter]::ToString($hasher.ComputeHash($stream)).Replace('-', '') }
  finally { $hasher.Dispose(); $stream.Dispose() }
}
