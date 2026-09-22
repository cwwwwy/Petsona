<#
.SYNOPSIS
    Screenshot the pet's speech bubble and the composer window.

.DESCRIPTION
    The bubble is a layered GDI+ window: screen capture via PrintWindow returns
    an empty frame, so the app itself dumps the rendered bitmap when
    `PETSONA_BUBBLE_SNAPSHOT` is set. The composer is a normal WinUI window and
    is captured with PrintWindow after `PETSONA_OPEN_COMPOSER=1` opens it.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\diagnostics\overlay-shots.ps1 -Theme dark
#>
[CmdletBinding()]
param(
    [string] $AppPath = "",
    [string] $OutputDirectory = "",
    [ValidateSet("light", "dark")]
    [string] $Theme = "light"
)

$ErrorActionPreference = "Stop"
if ($env:PATHEXT -notlike "*.EXE*") { $env:PATHEXT = ".COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC" }
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Add-Type -AssemblyName System.Drawing

if ([string]::IsNullOrWhiteSpace($AppPath)) {
    $AppPath = Join-Path $root "apps\windows\Petsona\bin\x64\Release\net10.0-windows10.0.26100.0\win-x64\Petsona.exe"
}
if (-not (Test-Path -LiteralPath $AppPath)) { throw "app not built: $AppPath" }
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $env:TEMP "petsona-overlay-shots"
}
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null

Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class OverlayWin {
  public delegate bool Proc(IntPtr h, IntPtr p);
  [DllImport("user32.dll")] public static extern bool EnumWindows(Proc cb, IntPtr p);
  [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "GetClassNameW")] public static extern int GetClassName(IntPtr h, StringBuilder sb, int max);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }

  public static IntPtr Composer(uint pid) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, p) => {
      uint winPid; GetWindowThreadProcessId(h, out winPid);
      if (winPid != pid || !IsWindowVisible(h)) return true;
      var sb = new StringBuilder(256); GetClassName(h, sb, 256);
      if (sb.ToString() != "WinUIDesktopWin32WindowClass") return true;
      RECT r; GetWindowRect(h, out r);
      int w = r.Right - r.Left, hh = r.Bottom - r.Top;
      // Plain comparisons: PowerShell 5.1 compiles this with an older C# than
      // the app project, which does not understand relational patterns.
      if (w > 320 && w < 460 && hh > 150 && hh < 260) { found = h; return false; }
      return true;
    }, IntPtr.Zero);
    return found;
  }

  public static bool Capture(IntPtr hwnd, IntPtr hdc) { return PrintWindow(hwnd, hdc, 2); }
}
"@

function New-PetHome([int] $Port) {
    $home2 = Join-Path $env:TEMP ("petsona-overlay-" + [Guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path (Join-Path $home2 "pets\v2-test-pet") -Force | Out-Null
    Copy-Item (Join-Path $root "crates\petsona-core\testdata\v2-test-pet\pet.json") (Join-Path $home2 "pets\v2-test-pet\pet.json") -Force
    Copy-Item (Join-Path $root "crates\petsona-core\testdata\v2-test-pet\spritesheet.png") (Join-Path $home2 "pets\v2-test-pet\spritesheet.png") -Force
    $config = '{"firstRun":false,"activePet":"test_fixture_v2","stateServer":{"enabled":true,"port":' + $Port + '}}'
    [System.IO.File]::WriteAllText((Join-Path $home2 "config.json"), $config, (New-Object System.Text.UTF8Encoding($false)))
    return $home2
}

# ---------------------------------------------------------------- bubble
$bubblePath = Join-Path $OutputDirectory ("bubble-" + $Theme + ".png")
$home2 = New-PetHome 18021
$env:PETSONA_HOME = $home2
$env:PETSONA_SETTINGS_THEME = $Theme
$env:PETSONA_BUBBLE_SNAPSHOT = $bubblePath
$env:PETSONA_OPEN_COMPOSER = $null
$process = $null
try {
    $process = Start-Process -FilePath $AppPath -PassThru
    # The app renders the sample bubble by itself; the protocol is not involved.
    $deadline = (Get-Date).AddSeconds(15)
    while ((Get-Date) -lt $deadline -and -not (Test-Path -LiteralPath $bubblePath)) { Start-Sleep -Milliseconds 200 }
    if (Test-Path -LiteralPath $bubblePath) { Write-Host ("saved " + $bubblePath) } else { Write-Host "bubble snapshot missing" }
}
finally {
    if ($null -ne $process) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item -LiteralPath $home2 -Recurse -Force -ErrorAction SilentlyContinue
}

# ---------------------------------------------------------------- composer
$composerPath = Join-Path $OutputDirectory ("composer-" + $Theme + ".png")
$home2 = New-PetHome 18022
$env:PETSONA_HOME = $home2
$env:PETSONA_SETTINGS_THEME = $Theme
$env:PETSONA_BUBBLE_SNAPSHOT = $null
$env:PETSONA_OPEN_COMPOSER = "1"
$process = $null
try {
    $process = Start-Process -FilePath $AppPath -PassThru
    $handle = [IntPtr]::Zero
    $deadline = (Get-Date).AddSeconds(25)
    while ((Get-Date) -lt $deadline -and $handle -eq [IntPtr]::Zero) {
        $handle = [OverlayWin]::Composer([uint32]$process.Id)
        if ($handle -eq [IntPtr]::Zero) { Start-Sleep -Milliseconds 250 }
    }
    if ($handle -eq [IntPtr]::Zero) { Write-Host "composer window not found" }
    else {
        Start-Sleep -Milliseconds 1500
        $rect = New-Object "OverlayWin+RECT"
        [void][OverlayWin]::GetWindowRect($handle, [ref]$rect)
        $width = $rect.Right - $rect.Left; $height = $rect.Bottom - $rect.Top
        $bitmap = New-Object System.Drawing.Bitmap $width, $height
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        $hdc = $graphics.GetHdc()
        $ok = [OverlayWin]::Capture($handle, $hdc)
        $graphics.ReleaseHdc($hdc); $graphics.Dispose()
        if ($ok) { $bitmap.Save($composerPath, [System.Drawing.Imaging.ImageFormat]::Png); Write-Host ("saved " + $composerPath) }
        $bitmap.Dispose()
    }
}
finally {
    if ($null -ne $process) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item -LiteralPath $home2 -Recurse -Force -ErrorAction SilentlyContinue
}
