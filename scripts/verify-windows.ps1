#requires -Version 5.1
<#
.SYNOPSIS
    Windows gate for the native frontend (apps/windows) and the shared Rust workspace.
.DESCRIPTION
    Runs the Rust workspace gates, builds petsona_ffi.dll (MSVC when Visual
    Studio is present, GNU fallback otherwise), then the .NET restore/build/
    format/test chain for apps\windows. With -Full it also runs the native
    smoke (scripts\windows-smoke.ps1) and a package structure check.

    The workspace root may be a UNC path (\\wsl.localhost\...): the script maps
    a temporary drive letter first because Windows PowerShell 5.1 cannot use
    UNC paths as the current location.
.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1
.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1 -Full
#>
[CmdletBinding()]
param(
    [switch] $Full
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$mappedDrive = $null
$isUncRoot = $root.StartsWith("\\")

if ($isUncRoot) {
    foreach ($code in 90..80) {
        $letter = [char]$code
        if (-not (Get-PSDrive -Name $letter -ErrorAction SilentlyContinue)) {
            New-PSDrive -Name $letter -PSProvider FileSystem -Root $root -Scope Script | Out-Null
            $mappedDrive = $letter
            $root = ($letter.ToString() + ":")
            break
        }
    }

    if ($null -eq $mappedDrive) {
        throw "no free drive letter available to map $root"
    }
}

# Cargo cannot create incremental-compilation lock files on some network
# filesystems (for example \\wsl.localhost paths). When the repository is
# reached through UNC, build on the local disk instead and copy the FFI dll
# back into the repository target directory.
if ($isUncRoot) {
    $env:CARGO_INCREMENTAL = "0"
    $env:CARGO_TARGET_DIR = Join-Path $env:TEMP "petsona-cargo-target"
    New-Item -ItemType Directory -Path $env:CARGO_TARGET_DIR -Force | Out-Null
}

# Some launcher environments (WSL interop, automation hosts) break PATHEXT,
# which makes Get-Command blind to .exe files. Restore the Windows default.
if ($env:PATHEXT -notlike "*.EXE*") {
    $env:PATHEXT = ".COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC"
}

$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (Test-Path -LiteralPath $cargoBin) {
    $env:PATH = "$cargoBin;$env:PATH"
}

foreach ($tool in @("rustc", "cargo")) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        Write-Host "Missing required command: $tool" -ForegroundColor Red
        exit 1
    }
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
    Write-Host "Missing required command: dotnet" -ForegroundColor Red
    exit 1
}

Push-Location $root
try {
    Write-Host "Repository: $root" -ForegroundColor DarkGray
    rustc --version
    cargo --version

    function Invoke-Step {
        param(
            [Parameter(Mandatory = $true)][string] $Name,
            [Parameter(Mandatory = $true)][scriptblock] $Command
        )

        Write-Host ""
        Write-Host "== $Name ==" -ForegroundColor Cyan
        & $Command
        if ($LASTEXITCODE -ne 0) {
            throw "step failed: $Name (exit $LASTEXITCODE)"
        }
    }

    Invoke-Step "cargo fmt" { cargo fmt --all -- --check }
    Invoke-Step "cargo clippy" { cargo clippy --workspace --all-targets --locked -- -D warnings }
    Invoke-Step "cargo test" { cargo test --workspace --locked }
    Invoke-Step "cargo build (release)" { cargo build --workspace --release --locked }

    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    $hasVisualStudio = $false
    if (Test-Path -LiteralPath $vswhere) {
        $vcInstall = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
        $hasVisualStudio = -not [string]::IsNullOrWhiteSpace($vcInstall)
    }

    $targetRoot = if ([string]::IsNullOrWhiteSpace($env:CARGO_TARGET_DIR)) { Join-Path $root "target" } else { $env:CARGO_TARGET_DIR }
    $ffiDll = $null

    if ($hasVisualStudio) {
        Invoke-Step "petsona-ffi dll (msvc)" { cargo build -p petsona-ffi --release --locked }
        $ffiDll = Join-Path $targetRoot "release\petsona_ffi.dll"
    }
    else {
        $gnuToolchain = "stable-x86_64-pc-windows-gnu"
        $gnuLinker = Join-Path $env:USERPROFILE ".rustup\toolchains\$gnuToolchain\lib\rustlib\x86_64-pc-windows-gnu\bin\rust-lld.exe"
        if (-not (Test-Path -LiteralPath $gnuLinker)) {
            throw "VS not found and the GNU fallback linker is missing: $gnuLinker"
        }

        Invoke-Step "petsona-ffi dll (gnu fallback)" {
            $env:RUSTUP_TOOLCHAIN = $gnuToolchain
            $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = $gnuLinker
            $env:RUSTFLAGS = "-C link-self-contained=yes"
            cargo build -p petsona-ffi --release --locked --target x86_64-pc-windows-gnu
        }
        $ffiDll = Join-Path $targetRoot "x86_64-pc-windows-gnu\release\petsona_ffi.dll"
    }

    if (-not (Test-Path -LiteralPath $ffiDll)) {
        throw "petsona_ffi.dll was not produced by the FFI build step: $ffiDll"
    }

    if ($isUncRoot) {
        $mirror = if ($ffiDll -like "*x86_64-pc-windows-gnu*") {
            Join-Path $root "target\x86_64-pc-windows-gnu\release"
        }
        else {
            Join-Path $root "target\release"
        }

        New-Item -ItemType Directory -Path $mirror -Force | Out-Null
        Copy-Item -LiteralPath $ffiDll -Destination (Join-Path $mirror "petsona_ffi.dll") -Force
    }

    $env:PETSONA_FFI_DLL = $ffiDll

    Invoke-Step "dotnet restore (locked)" { & $dotnet restore apps/windows/Petsona.sln --locked-mode }
    Invoke-Step "dotnet build (debug)" { & $dotnet build apps/windows/Petsona.sln -c Debug --no-restore }
    Invoke-Step "dotnet format" { & $dotnet format apps/windows/Petsona.sln --verify-no-changes --no-restore }
    Invoke-Step "dotnet build (release)" { & $dotnet build apps/windows/Petsona.sln -c Release -p:Platform=x64 --no-restore }

    # Guard against the class of bug where the gate builds one toolchain's DLL
    # but the app output silently keeps a stale DLL from the other toolchain.
    $appFfi = Get-ChildItem -LiteralPath (Join-Path $root "apps\windows\Petsona\bin\x64\Release") -Recurse -Filter "petsona_ffi.dll" -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -like "*\win-x64\*" } |
        Select-Object -First 1
    if ($null -eq $appFfi) {
        throw "the app Release output has no petsona_ffi.dll"
    }

    $builtHash = (Get-FileHash -LiteralPath $ffiDll -Algorithm SHA256).Hash
    $appHash = (Get-FileHash -LiteralPath $appFfi.FullName -Algorithm SHA256).Hash
    if ($builtHash -ne $appHash) {
        throw "the app output petsona_ffi.dll does not match the freshly built DLL: $($appFfi.FullName)"
    }

    Invoke-Step "dotnet test (release)" { & $dotnet test apps/windows/Petsona.sln -c Release -p:Platform=x64 --no-build }

    if ($Full) {
        $smoke = Join-Path $PSScriptRoot "windows-smoke.ps1"
        Invoke-Step "Windows native smoke" {
            & powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File $smoke
        }

        $packageOutput = Join-Path ([System.IO.Path]::GetTempPath()) ("petsona-package-smoke-" + [Guid]::NewGuid().ToString("N"))
        try {
            $packageScript = Join-Path $PSScriptRoot "package-windows.ps1"
            Invoke-Step "Windows package" {
                & $packageScript -SkipBuild -OutputDirectory $packageOutput
            }

            Invoke-Step "package structure" {
                $zip = Get-ChildItem -LiteralPath $packageOutput -Filter "Petsona-windows-*.zip" | Select-Object -First 1
                if ($null -eq $zip) {
                    throw "package-windows.ps1 did not produce a zip"
                }

                $extract = Join-Path $packageOutput "extract"
                Expand-Archive -LiteralPath $zip.FullName -DestinationPath $extract -Force
                foreach ($required in @("Petsona.exe", "Petsona.dll", "Petsona.Core.dll", "petsona_ffi.dll", "VERSION", "README.txt", "Petsona.ico")) {
                    if (-not (Get-ChildItem -LiteralPath $extract -Recurse -Filter $required -ErrorAction SilentlyContinue)) {
                        throw "package is missing $required"
                    }
                }

                Write-Host ("package structure ok: {0}" -f $zip.Name) -ForegroundColor Green
            }

            # The packaged exe must actually run: a publish that drops the XAML
            # resources passes a file listing but fails at runtime.
            Invoke-Step "package smoke (unpacked)" {
                $extract = Join-Path $packageOutput "extract"
                $exe = Get-ChildItem -LiteralPath $extract -Recurse -Filter "Petsona.exe" | Select-Object -First 1
                if ($null -eq $exe) {
                    throw "no Petsona.exe inside the package"
                }

                & powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "windows-smoke.ps1") -AppPath $exe.FullName
                if ($LASTEXITCODE -ne 0) {
                    throw "the packaged app failed the smoke run (exit $LASTEXITCODE)"
                }
            }
        }
        finally {
            if (Test-Path -LiteralPath $packageOutput) {
                Remove-Item -LiteralPath $packageOutput -Recurse -Force -ErrorAction SilentlyContinue
            }
        }
    }

    Write-Host ""
    Write-Host "Windows gates passed." -ForegroundColor Green
}
finally {
    Pop-Location
    if ($null -ne $mappedDrive) {
        Remove-PSDrive -Name $mappedDrive -ErrorAction SilentlyContinue
    }
}
