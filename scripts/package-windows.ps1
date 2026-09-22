#requires -Version 5.1
<#
.SYNOPSIS
    Build the portable Windows release zip for the native Petsona frontend.
.DESCRIPTION
    Publishes apps\windows\Petsona (WinUI 3, unpackaged, x64), stages the
    FFI dll (petsona_ffi.dll), the application icon, VERSION and README.txt,
    then packs dist\Petsona-windows-x64-<version>.zip with
    ZipFile::CreateFromDirectory (Compress-Archive backslash entries are
    avoided).

    Run scripts\verify-windows.ps1 first so petsona_ffi.dll exists.
.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\package-windows.ps1
#>
[CmdletBinding()]
param(
    [string] $OutputDirectory = "",
    [ValidateSet("x64", "arm64")]
    [string] $Architecture = "x64",
    [string] $Version = "",
    [switch] $SkipBuild
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

if ($Architecture -ne "x64") {
    throw "the native WinUI frontend currently ships x64 only (requested: $Architecture)"
}

if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $root "dist"
}

if ([string]::IsNullOrWhiteSpace($Version)) {
    if (-not [string]::IsNullOrWhiteSpace($env:PETSONA_VERSION)) {
        $Version = $env:PETSONA_VERSION
    }
    else {
        $match = Select-String -Path (Join-Path $root "Cargo.toml") -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
        if (-not $match) {
            throw "cannot read the workspace version from Cargo.toml"
        }

        $Version = $match.Matches[0].Groups[1].Value
    }
}

# Some launcher environments (WSL interop, automation hosts) break PATHEXT,
# which makes Get-Command blind to .exe files. Restore the Windows default.
if ($env:PATHEXT -notlike "*.EXE*") {
    $env:PATHEXT = ".COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC"
}

$dotnet = $null
$dotnetCommand = Get-Command dotnet -ErrorAction SilentlyContinue
if ($null -ne $dotnetCommand) {
    $dotnet = $dotnetCommand.Source
}
else {
    $candidate = Join-Path $env:ProgramFiles "dotnet\dotnet.exe"
    if (Test-Path -LiteralPath $candidate) {
        $dotnet = $candidate
    }
}

if ([string]::IsNullOrWhiteSpace($dotnet)) {
    throw "dotnet was not found"
}

New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$stageName = "Petsona-windows-$Architecture-$Version"
$stage = Join-Path $OutputDirectory $stageName
if (Test-Path -LiteralPath $stage) {
    Remove-Item -LiteralPath $stage -Recurse -Force
}

New-Item -ItemType Directory -Path $stage -Force | Out-Null

# The release payload is the RID build output. `dotnet publish -o <dir>` drops
# the WinUI XAML resources (App.xbf / Views\*.xbf / Petsona.pri), which makes
# SettingsWindow fail with XamlParseException at runtime, so the staging
# directory is assembled from the verified bin output instead.
$binDir = Join-Path $root "apps\windows\Petsona\bin\x64\Release\net10.0-windows10.0.26100.0\win-x64"

# Select the FFI DLL before building so the csproj copies the same file into
# the app output. An explicit PETSONA_FFI_DLL wins; otherwise use the newest
# candidate instead of a fixed toolchain order, which could ship a stale DLL.
$ffiCandidates = @()
if (-not [string]::IsNullOrWhiteSpace($env:PETSONA_FFI_DLL)) {
    $ffiCandidates += $env:PETSONA_FFI_DLL
}

if (-not [string]::IsNullOrWhiteSpace($env:CARGO_TARGET_DIR)) {
    $ffiCandidates += (Join-Path $env:CARGO_TARGET_DIR "x86_64-pc-windows-gnu\release\petsona_ffi.dll")
    $ffiCandidates += (Join-Path $env:CARGO_TARGET_DIR "release\petsona_ffi.dll")
}

$ffiCandidates += (Join-Path $root "target\x86_64-pc-windows-gnu\release\petsona_ffi.dll")
$ffiCandidates += (Join-Path $root "target\release\petsona_ffi.dll")
$ffi = $ffiCandidates |
    Where-Object { Test-Path -LiteralPath $_ } |
    Sort-Object { (Get-Item -LiteralPath $_).LastWriteTimeUtc } -Descending |
    Select-Object -First 1
if ($null -eq $ffi) {
    throw "petsona_ffi.dll not found; run scripts\verify-windows.ps1 first"
}

$env:PETSONA_FFI_DLL = $ffi

if (-not $SkipBuild) {
    Write-Host "Building native frontend (Release x64)..." -ForegroundColor Cyan
    $project = Join-Path $root "apps\windows\Petsona\Petsona.csproj"
    $selfContainedArgs = @()
    if ($root -match '^\\\\') {
        Write-Warning "UNC root: mt.exe cannot read UNC paths, so this build is framework-dependent. Run the packager from a local checkout (or CI) to get the self-contained release zip."
        $selfContainedArgs = @("-p:WindowsAppSDKSelfContained=false", "-p:SelfContained=false")
    }

    & $dotnet build $project -c Release -p:Platform=x64 --no-restore @selfContainedArgs
    if ($LASTEXITCODE -ne 0) {
        throw "dotnet build failed with exit $LASTEXITCODE"
    }
}

foreach ($required in @("Petsona.exe", "Petsona.pri", "App.xbf")) {
    if (-not (Test-Path -LiteralPath (Join-Path $binDir $required))) {
        throw "missing $required in the Release x64 output; build apps\windows first"
    }
}

Write-Host "Staging the release payload..." -ForegroundColor Cyan
Copy-Item -Path (Join-Path $binDir "*") -Destination $stage -Recurse -Force

Copy-Item -LiteralPath $ffi -Destination (Join-Path $stage "petsona_ffi.dll") -Force

$binFfi = Join-Path $binDir "petsona_ffi.dll"
if (Test-Path -LiteralPath $binFfi) {
    $binHash = (Get-FileHash -LiteralPath $binFfi -Algorithm SHA256).Hash
    $ffiHash = (Get-FileHash -LiteralPath $ffi -Algorithm SHA256).Hash
    if ($binHash -ne $ffiHash) {
        Write-Warning "the app output FFI DLL differs from the selected DLL; the package stages the selected one"
    }
}

$icon = Join-Path $root "packaging\windows\Petsona.ico"
if (Test-Path -LiteralPath $icon) {
    Copy-Item -LiteralPath $icon -Destination (Join-Path $stage "Petsona.ico") -Force
}

$encoding = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText((Join-Path $stage "VERSION"), $Version, $encoding)

$readme = @"
Petsona (Windows x64, native frontend)
Version: $Version

Run Petsona.exe to start the desktop pet. The app stores its data in
%APPDATA%\Petsona unless PETSONA_HOME overrides it.

Requirements:
- Windows 10 1809 or newer (Windows 11 recommended)
- Microsoft Windows App Runtime 1.8 or newer (installed with the system on
  most up-to-date machines; the app uses the unpackaged WinUI 3 bootstrapper)

The pet library starts empty: import a pet pack from a folder, a .zip or the
Codex library on first run. The local state protocol listens on
127.0.0.1:17872 (POST /state, GET /health, GET /pets).
"@
[System.IO.File]::WriteAllText((Join-Path $stage "README.txt"), $readme, $encoding)

$zipPath = Join-Path $OutputDirectory ("Petsona-windows-$Architecture-$Version.zip")
if (Test-Path -LiteralPath $zipPath) {
    Remove-Item -LiteralPath $zipPath -Force
}

# Build the archive entry by entry with '/' separators. Both Compress-Archive
# and ZipFile::CreateFromDirectory emit '\' separators on Windows PowerShell
# 5.1 (its .NET Framework), which breaks extraction on non-Windows tools.
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [System.IO.Compression.ZipFile]::Open($zipPath, [System.IO.Compression.ZipArchiveMode]::Create)
try {
    $baseName = Split-Path -Leaf $stage
    foreach ($file in Get-ChildItem -LiteralPath $stage -Recurse -File) {
        $relative = $file.FullName.Substring($stage.Length + 1).Replace('\', '/')
        $entryName = "$baseName/$relative"
        [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
            $archive,
            $file.FullName,
            $entryName,
            [System.IO.Compression.CompressionLevel]::Optimal) | Out-Null
    }
}
finally {
    $archive.Dispose()
}

Write-Host ("package: {0}" -f $zipPath) -ForegroundColor Green
Write-Host ("stage:   {0}" -f $stage) -ForegroundColor DarkGray
