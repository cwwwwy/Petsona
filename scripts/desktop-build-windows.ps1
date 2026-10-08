# Build apps/desktop/src-tauri on Windows with the pinned Rust MSVC toolchain.
# Usage (from Windows PowerShell):
#   powershell -ExecutionPolicy Bypass -File scripts\desktop-build-windows.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\desktop-build-windows.ps1 -Profile release
param(
  [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
  [ValidateSet('debug', 'release')][string]$Profile = 'debug'
)

$ErrorActionPreference = 'Stop'

# WSL-launched PowerShell can have a polluted PATHEXT; restore it first.
$env:PATHEXT = '.COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC'

$toolchain = Join-Path $env:USERPROFILE '.rustup\toolchains\1.98.0-x86_64-pc-windows-msvc\bin'
$cargo = Join-Path $toolchain 'cargo.exe'
$vcvars = 'C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat'
$targetDir = Join-Path $env:USERPROFILE 'petsona-build\desktop-target'

if (-not (Test-Path $vcvars)) { throw "vcvars64.bat not found at $vcvars" }
if (-not (Test-Path $cargo)) { throw "cargo.exe not found in $toolchain" }

$srcDir = Join-Path $RepoRoot 'apps\desktop\src-tauri'
if (-not (Test-Path (Join-Path $srcDir 'Cargo.toml'))) { throw "src-tauri not found under $RepoRoot" }

# cmd.exe cannot start on a UNC working directory; anchor to a local path first.
Set-Location -LiteralPath $env:USERPROFILE

# Import the MSVC environment (link.exe, rc.exe, SDK paths) into this process.
$envLines = cmd.exe /d /c "`"$vcvars`" >nul 2>&1 && set"
if ($LASTEXITCODE -ne 0) { throw "vcvars64.bat failed with exit code $LASTEXITCODE" }
foreach ($line in $envLines) {
  $index = $line.IndexOf('=')
  if ($index -gt 0) {
    $name = $line.Substring(0, $index)
    $value = $line.Substring($index + 1)
    Set-Item -Path ("env:" + $name) -Value $value
  }
}

$env:PATH = "$toolchain;$env:PATH"
$env:CARGO_TARGET_DIR = $targetDir

Push-Location -LiteralPath $srcDir
try {
  if ($Profile -eq 'release') {
    & $cargo build --release
  } else {
    & $cargo build
  }
  if ($LASTEXITCODE -ne 0) { throw "cargo build failed with exit code $LASTEXITCODE" }
} finally {
  Pop-Location
}

$exe = Join-Path $targetDir "$Profile\petsona-desktop.exe"
Write-Host "Built: $exe"
