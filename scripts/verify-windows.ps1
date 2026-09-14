#requires -Version 5.1

[CmdletBinding()]
param(
    [switch] $Full
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root

try {
    foreach ($tool in @("rustc", "cargo")) {
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
            Write-Host "Missing required command: $tool" -ForegroundColor Red
            exit 1
        }
    }

    Write-Host "Repository: $root" -ForegroundColor DarkGray
    rustc --version
    cargo --version

    function Invoke-Step {
        param(
            [Parameter(Mandatory = $true)]
            [string] $Name,
            [Parameter(Mandatory = $true)]
            [scriptblock] $Command
        )

        Write-Host ""
        Write-Host "=== $Name ===" -ForegroundColor Cyan
        & $Command
        $exitCode = $LASTEXITCODE
        if ($exitCode -ne 0) {
            Write-Host "FAILED: $Name (exit code $exitCode)" -ForegroundColor Red
            exit $exitCode
        }
    }

    Invoke-Step "cargo fmt" { cargo fmt --all -- --check }
    Invoke-Step "cargo clippy" { cargo clippy --workspace --all-targets --locked -- -D warnings }
    Invoke-Step "cargo test" { cargo test --workspace --locked }
    Invoke-Step "cargo build (release linker check)" { cargo build --workspace --release --locked }

    if ($Full) {
        Invoke-Step "cargo clippy (test-hooks)" {
            cargo clippy -p petsona-app --features test-hooks --locked -- -D warnings
        }
        Invoke-Step "cargo test (test-hooks)" {
            cargo test -p petsona-app --features test-hooks --locked
        }
        Invoke-Step "cargo build (test-hooks release)" {
            cargo build -p petsona-app --release --features test-hooks --locked
        }

        $smoke = Join-Path $PSScriptRoot "windows-smoke.ps1"
        Invoke-Step "Windows smoke" {
            powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File $smoke -Configuration Release -SkipBuild -UseTestHooks
        }

        Invoke-Step "restore default release build" {
            cargo build --workspace --release --locked
        }
    }

    Write-Host ""
    Write-Host "All Windows verification gates passed." -ForegroundColor Green
}
finally {
    Pop-Location
}
