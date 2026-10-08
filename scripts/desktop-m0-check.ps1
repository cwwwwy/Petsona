# M0 checks for the new Petsona desktop shell (Windows, run on the real desktop).
# NOTE: this script moves the real cursor; do not touch the mouse while it runs.
#
# Checks:
#   1. start -> pet window visible timing (repeated runs)
#   2. pet window ex-styles: LAYERED / TOOLWINDOW / TOPMOST / NOACTIVATE
#   3. WS_EX_TRANSPARENT toggling between opaque and transparent pixels
#   4. the pet window never becomes the foreground window
#   5. drag: pressing an opaque pixel and moving carries the window
#   6. settings focus: --show-settings window becomes visible and foreground
#
# Usage (Windows PowerShell):
#   powershell -ExecutionPolicy Bypass -File scripts\desktop-m0-check.ps1 -Exe C:\path\petsona-desktop.exe

param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [int]$StartupRuns = 3
)

$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

public static class M0Native
{
    [DllImport("user32.dll", CharSet = CharSet.Unicode, ExactSpelling = true)]
    public static extern IntPtr FindWindowW(string lpClassName, string lpWindowName);

    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hWnd);

    [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")]
    public static extern IntPtr GetWindowLongPtrW64(IntPtr hWnd, int nIndex);

    [DllImport("user32.dll", EntryPoint = "GetWindowLongW")]
    public static extern int GetWindowLongW32(IntPtr hWnd, int nIndex);

    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);

    [DllImport("user32.dll")]
    public static extern bool SetCursorPos(int X, int Y);

    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();

    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);

    [DllImport("user32.dll")]
    public static extern void mouse_event(uint dwFlags, int dx, int dy, uint dwData, UIntPtr dwExtraInfo);

    public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }

    public static long GetExStyle(IntPtr hwnd)
    {
        if (IntPtr.Size == 8) { return GetWindowLongPtrW64(hwnd, -20).ToInt64(); }
        return GetWindowLongW32(hwnd, -20);
    }
}
'@

$WS_EX_TRANSPARENT = 0x00000020
$WS_EX_TOOLWINDOW  = 0x00000080
$WS_EX_TOPMOST     = 0x00000008
$WS_EX_LAYERED     = 0x00080000
$WS_EX_NOACTIVATE  = 0x08000000
$MOUSEEVENTF_LEFTDOWN = 0x0002
$MOUSEEVENTF_LEFTUP   = 0x0004

function Get-PetWindow {
    # PowerShell marshals $null as an empty string for string P/Invoke args;
    # pass a real NULL so the window title is not filtered.
    $hwnd = [M0Native]::FindWindowW('PetsonaPetWindow', [NullString]::Value)
    if ($hwnd -eq [IntPtr]::Zero) { return $null }
    return $hwnd
}

function Stop-Petsona {
    Get-Process petsona-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 400
}

Write-Host '== startup timing =='
for ($run = 1; $run -le $StartupRuns; $run++) {
    Stop-Petsona
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    $null = Start-Process -FilePath $Exe -PassThru
    $visible = $false
    while ($watch.ElapsedMilliseconds -lt 20000) {
        $hwnd = Get-PetWindow
        if ($hwnd -and [M0Native]::IsWindowVisible($hwnd)) { $visible = $true; break }
        Start-Sleep -Milliseconds 10
    }
    $watch.Stop()
    if ($visible) {
        Write-Host ("  run {0}: pet visible after {1} ms" -f $run, $watch.ElapsedMilliseconds)
    } else {
        Write-Host ("  run {0}: pet window NOT visible within 20s  [FAIL]" -f $run)
    }
    Stop-Petsona
}

Write-Host '== window styles / pass-through / focus =='
$null = Start-Process -FilePath $Exe -PassThru
$hwnd = $null
$deadline = (Get-Date).AddSeconds(20)
while ((Get-Date) -lt $deadline) {
    $hwnd = Get-PetWindow
    if ($hwnd -and [M0Native]::IsWindowVisible($hwnd)) { break }
    Start-Sleep -Milliseconds 50
}
if (-not $hwnd) { throw 'pet window not found within 20s' }

$style = [M0Native]::GetExStyle($hwnd)
Write-Host ("  WS_EX_LAYERED    : {0}" -f (($style -band $WS_EX_LAYERED) -ne 0))
Write-Host ("  WS_EX_TOOLWINDOW : {0}" -f (($style -band $WS_EX_TOOLWINDOW) -ne 0))
Write-Host ("  WS_EX_TOPMOST    : {0}" -f (($style -band $WS_EX_TOPMOST) -ne 0))
Write-Host ("  WS_EX_NOACTIVATE : {0}" -f (($style -band $WS_EX_NOACTIVATE) -ne 0))

$rect = New-Object M0Native+RECT
[void][M0Native]::GetWindowRect($hwnd, [ref]$rect)
$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top

$sawTransparent = $false
$sawOpaque = $false
$opaquePoint = $null
for ($ix = 1; $ix -le 7; $ix++) {
    for ($iy = 1; $iy -le 7; $iy++) {
        $x = [int]($rect.Left + $width * $ix / 8.0)
        $y = [int]($rect.Top + $height * $iy / 8.0)
        [void][M0Native]::SetCursorPos($x, $y)
        Start-Sleep -Milliseconds 100
        $s = [M0Native]::GetExStyle($hwnd)
        if (($s -band $WS_EX_TRANSPARENT) -ne 0) {
            $sawTransparent = $true
        } else {
            $sawOpaque = $true
            if (-not $opaquePoint) { $opaquePoint = @($x, $y) }
        }
    }
}
Write-Host ("  pass-through toggling: transparent={0} opaque={1}" -f $sawTransparent, $sawOpaque)

$foreground = [M0Native]::GetForegroundWindow()
Write-Host ("  foreground is pet window: {0}  (expected False)" -f ($foreground -eq $hwnd))

Write-Host '== drag test =='
if ($opaquePoint) {
    $before = New-Object M0Native+RECT
    [void][M0Native]::GetWindowRect($hwnd, [ref]$before)
    [void][M0Native]::SetCursorPos($opaquePoint[0], $opaquePoint[1])
    Start-Sleep -Milliseconds 80
    [M0Native]::mouse_event($MOUSEEVENTF_LEFTDOWN, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    for ($step = 1; $step -le 8; $step++) {
        [void][M0Native]::SetCursorPos([int]($opaquePoint[0] + 140 * $step / 8), [int]($opaquePoint[1] + 90 * $step / 8))
        Start-Sleep -Milliseconds 25
    }
    [M0Native]::mouse_event($MOUSEEVENTF_LEFTUP, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 200
    $after = New-Object M0Native+RECT
    [void][M0Native]::GetWindowRect($hwnd, [ref]$after)
    Write-Host ("  moved dx={0} dy={1}  (expected ~140/~90)" -f ($after.Left - $before.Left), ($after.Top - $before.Top))
} else {
    Write-Host '  no opaque point found to start the drag  [FAIL]'
}

Stop-Petsona

Write-Host '== settings focus (--show-settings) =='
$proc = Start-Process -FilePath $Exe -ArgumentList '--show-settings' -PassThru
$settings = $null
$deadline = (Get-Date).AddSeconds(20)
while ((Get-Date) -lt $deadline) {
    $candidate = [M0Native]::FindWindowW('Tauri Window', [NullString]::Value)
    if ($candidate -and [M0Native]::IsWindowVisible($candidate)) {
        [uint32]$candidatePid = 0
        [void][M0Native]::GetWindowThreadProcessId($candidate, [ref]$candidatePid)
        if ($candidatePid -eq $proc.Id) { $settings = $candidate; break }
    }
    Start-Sleep -Milliseconds 100
}
if ($settings) {
    Start-Sleep -Milliseconds 400
    $fg = [M0Native]::GetForegroundWindow()
    Write-Host ("  settings visible: True; foreground is settings: {0}" -f ($fg -eq $settings))
} else {
    Write-Host '  settings window not visible within 20s  [FAIL]'
}
Stop-Petsona

$log = Join-Path $env:TEMP 'petsona-desktop.log'
if (Test-Path $log) {
    Write-Host '== app log (last 6 lines) =='
    Get-Content -LiteralPath $log -Tail 6 | ForEach-Object { Write-Host ("  " + $_) }
}
