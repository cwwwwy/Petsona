<#
.SYNOPSIS
    Capture one screenshot per Settings page (works in a background session).

.DESCRIPTION
    For each page it starts the app with a throwaway PETSONA_HOME (empty pet
    library, so Settings opens by itself) and `PETSONA_SETTINGS_PAGE=<page>`,
    then captures the window content with PrintWindow. Nothing depends on the
    window being in the foreground, so it also works while the user is using
    other applications (screenshots of the visible desktop would capture those).

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\diagnostics\settings-shots.ps1
#>
[CmdletBinding()]
param(
    [string] $AppPath = "",
    [string] $OutputDirectory = "",
    [ValidateSet("system", "light", "dark")]
    [string] $Theme = "system"
)

$ErrorActionPreference = "Stop"
if ($env:PATHEXT -notlike "*.EXE*") { $env:PATHEXT = ".COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC" }
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Add-Type -AssemblyName System.Drawing

if ([string]::IsNullOrWhiteSpace($AppPath)) {
    $AppPath = Join-Path $root "apps\windows\Petsona\bin\x64\Release\net10.0-windows10.0.26100.0\win-x64\Petsona.exe"
}
if (-not (Test-Path -LiteralPath $AppPath)) {
    throw "app not built: $AppPath (run scripts\verify-windows.ps1 first)"
}
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $env:TEMP "petsona-settings-shots"
}
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null

Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class ShotWin {
  public delegate bool Proc(IntPtr h, IntPtr p);
  [DllImport("user32.dll")] public static extern bool EnumWindows(Proc cb, IntPtr p);
  [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "GetClassNameW")] public static extern int GetClassName(IntPtr h, StringBuilder sb, int max);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }

  public static IntPtr VisibleByClass(uint pid, string className) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, p) => {
      uint winPid; GetWindowThreadProcessId(h, out winPid);
      if (winPid != pid || !IsWindowVisible(h)) return true;
      var sb = new StringBuilder(256); GetClassName(h, sb, 256);
      if (sb.ToString() == className) { found = h; return false; }
      return true;
    }, IntPtr.Zero);
    return found;
  }

  public static bool Capture(IntPtr hwnd, IntPtr hdc) {
    // PW_RENDERFULLCONTENT (2): required for WinUI/DirectComposition windows.
    return PrintWindow(hwnd, hdc, 2);
}
}
"@

$pages = @(
    @{ Tag = "pets"; Name = "01-pets" },
    @{ Tag = "appearance"; Name = "02-appearance" },
    @{ Tag = "persona"; Name = "03-persona" },
    @{ Tag = "memory"; Name = "04-memory" },
    @{ Tag = "deepseek"; Name = "05-model" },
    @{ Tag = "startup"; Name = "06-system" }
)

$memory = @'
{
  "version": 1,
  "personas": {
    "default": {
      "facts": [
        { "id": "f1", "key": "称呼", "value": "小明", "confidence": 0.9, "createdAt": 1758400000000, "updatedAt": 1758400000000, "source": "conversation" },
        { "id": "f2", "key": "咖啡", "value": "喜欢浅烘", "confidence": 0.8, "createdAt": 1758400000000, "updatedAt": 1758400000000, "source": "manual" },
        { "id": "f3", "key": "画像", "value": "喜欢安静的音乐；不喜欢早起", "confidence": 0.6, "createdAt": 1758400000000, "updatedAt": 1758400000000, "source": "compressed" }
      ],
      "events": [
        { "id": "e1", "kind": "user_message", "text": "帮我看看这段代码", "createdAt": 1758400000000 },
        { "id": "e2", "kind": "pet_greeting", "text": "早上好呀", "createdAt": 1758400000000 }
      ]
    }
  }
}
'@

foreach ($page in $pages) {
    $home2 = Join-Path $env:TEMP ("petsona-shots-" + [Guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $home2 -Force | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $home2 "config.json"),
        '{"firstRun":false,"stateServer":{"enabled":false},"memory":{"enabled":true,"recentEvents":5,"factLimit":20}}',
        (New-Object System.Text.UTF8Encoding($false)))
    [System.IO.File]::WriteAllText((Join-Path $home2 "memory.json"), $memory,
        (New-Object System.Text.UTF8Encoding($false)))

    $previousHome = $env:PETSONA_HOME
    $env:PETSONA_HOME = $home2
    $env:PETSONA_SETTINGS_PAGE = $page.Tag
    if ($Theme -ne "system") { $env:PETSONA_SETTINGS_THEME = $Theme } else { $env:PETSONA_SETTINGS_THEME = $null }
    $process = $null
    try {
        $process = Start-Process -FilePath $AppPath -PassThru
        $handle = [IntPtr]::Zero
        $deadline = (Get-Date).AddSeconds(25)
        while ((Get-Date) -lt $deadline -and $handle -eq [IntPtr]::Zero) {
            $handle = [ShotWin]::VisibleByClass([uint32]$process.Id, "WinUIDesktopWin32WindowClass")
            if ($handle -eq [IntPtr]::Zero) { Start-Sleep -Milliseconds 200 }
        }
        if ($handle -eq [IntPtr]::Zero) { throw "settings window did not open for page $($page.Tag)" }

        # WinUI re-lays out and re-themes asynchronously; give it time before capture.
        Start-Sleep -Milliseconds 1800
        $suffix = if ($Theme -eq "system") { "" } else { "-" + $Theme }
        $path = Join-Path $OutputDirectory ($page.Name + $suffix + ".png")
        $rect = New-Object "ShotWin+RECT"
        [void][ShotWin]::GetWindowRect($handle, [ref]$rect)
        $width = $rect.Right - $rect.Left
        $height = $rect.Bottom - $rect.Top
        $bitmap = New-Object System.Drawing.Bitmap $width, $height
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        $hdc = $graphics.GetHdc()
        $ok = [ShotWin]::Capture($handle, $hdc)
        $graphics.ReleaseHdc($hdc)
        $graphics.Dispose()
        if (-not $ok) { $bitmap.Dispose(); throw "PrintWindow failed for page $($page.Tag)" }
        $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
        $bitmap.Dispose()
        Write-Host ("saved " + $path + " (" + $width + "x" + $height + ")")
    }
    finally {
        if ($null -ne $process) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
        $env:PETSONA_HOME = $previousHome
        Remove-Item -LiteralPath $home2 -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Write-Host ("output: " + $OutputDirectory)
