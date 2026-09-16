#requires -Version 5.1
<#
.SYNOPSIS
    Build the portable Windows release zip for Petsona.
.DESCRIPTION
    Stages dist\Petsona-windows-<arch>\ with the shell executable, the icon and a
    short readme, then packs dist\Petsona-windows-<arch>-<version>.zip.

    The executable is target\release\petsona-windows.exe, which already has
    packaging\windows\Petsona.ico embedded by crates\petsona-shell-windows\build.rs.
    Regenerate the icon with packaging\windows\generate-icon.ps1 after artwork changes.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\package-windows.ps1

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\package-windows.ps1 -Architecture arm64
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

$targetArgs = @()
if ($Architecture -eq "arm64") {
    $targetArgs = @("--target", "aarch64-pc-windows-msvc")
}

if (-not $SkipBuild) {
    Write-Host "=== cargo build (release, $Architecture) ===" -ForegroundColor Cyan
    Push-Location $root
    try {
        & cargo build -p petsona-shell-windows --release --locked @targetArgs
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build failed with exit code $LASTEXITCODE"
        }
    }
    finally {
        Pop-Location
    }
}

$releaseDirectory = Join-Path $root "target\release"
if ($targetArgs.Count -gt 0) {
    $releaseDirectory = Join-Path $root ("target\{0}\release" -f $targetArgs[1])
}
$binary = Join-Path $releaseDirectory "petsona-windows.exe"
if (-not (Test-Path $binary)) {
    throw "release binary was not found: $binary"
}

$staging = Join-Path $OutputDirectory ("Petsona-windows-{0}" -f $Architecture)
$zipPath = Join-Path $OutputDirectory ("Petsona-windows-{0}-{1}.zip" -f $Architecture, $Version)

if (Test-Path $staging) {
    Remove-Item -LiteralPath $staging -Recurse -Force
}
if (Test-Path $zipPath) {
    Remove-Item -LiteralPath $zipPath -Force
}
New-Item -ItemType Directory -Path $staging -Force | Out-Null

Copy-Item -LiteralPath $binary -Destination (Join-Path $staging "Petsona.exe")
Copy-Item -LiteralPath (Join-Path $root "packaging\windows\Petsona.ico") -Destination (Join-Path $staging "Petsona.ico")
[System.IO.File]::WriteAllText(
    (Join-Path $staging "VERSION.txt"),
    "$Version`r`n",
    [System.Text.UTF8Encoding]::new($false)
)

$readme = @"
Petsona $Version · Windows $Architecture

运行
  1. 解压到任意目录
  2. 双击 Petsona.exe，宠物出现在屏幕上，托盘图标同步出现
  3. 退出：托盘图标菜单 -> 退出，或宠物右键菜单 -> 退出

数据目录
  %APPDATA%\Petsona（可用环境变量 PETSONA_HOME 覆盖）
  pets\             本地宠物库
  logs\petsona.log  运行日志

说明
  - 未签名：SmartScreen 可能提示“未知发布者”，选择“仍要运行”
  - 单实例：重复启动会直接退出
  - Petsona.ico 可用于快捷方式或固定到任务栏
"@
[System.IO.File]::WriteAllText(
    (Join-Path $staging "README.txt"),
    ($readme -replace "`n", "`r`n"),
    [System.Text.UTF8Encoding]::new($true)
)

# .NET rather than Compress-Archive: PowerShell 5.1 writes backslashes as zip
# entry separators, which some extractors mishandle.
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory(
    $staging,
    $zipPath,
    [System.IO.Compression.CompressionLevel]::Optimal,
    $true
)

$entries = [System.IO.Compression.ZipFile]::OpenRead($zipPath)
try {
    $names = $entries.Entries | ForEach-Object { $_.FullName -replace '\\', '/' }
}
finally {
    $entries.Dispose()
}

foreach ($required in @("Petsona.exe", "Petsona.ico", "VERSION.txt", "README.txt")) {
    if (-not ($names | Where-Object { $_ -like "*/$required" })) {
        throw "zip is missing $required"
    }
}

$zip = Get-Item $zipPath
Write-Host ("Staged: {0}" -f $staging) -ForegroundColor DarkGray
Write-Host ("Wrote:  {0} ({1:N1} KB)" -f $zip.FullName, ($zip.Length / 1KB)) -ForegroundColor Green