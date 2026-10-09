# Petsona FA-1 settings window evidence (Windows, run on the real desktop).
#
# POLICY: this script never moves the cursor or injects mouse input. Window
# resize and WM_CLOSE are posted directly to the app windows, and screenshots
# use PrintWindow (the window's own content, independent of occlusion).
#
# Coverage:
#   1. --show-settings presents the settings window in the foreground
#      (the tray-menu path stays a manual item; see the checklist at the end)
#   2. default-width and narrow-width screenshots for the sidebar layout
#      (labels at 920 CSS px; icon-only below the 780 px breakpoint)
#   3. WM_CLOSE hides only: process, pet window and state server keep running
#   4. re-show proxy renders a non-blank WebView again (no white screen)
#
# Usage (from Windows PowerShell):
#   powershell -ExecutionPolicy Bypass -File scripts\desktop-settings-smoke.ps1

param(
  [string]$Exe = "$env:USERPROFILE\petsona-build\desktop-target\debug\petsona-desktop.exe",
  [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
  [int]$Port = 17899,
  [string]$OutDir = $env:TEMP,
  [int]$NarrowCssPx = 700
)

$ErrorActionPreference = 'Stop'

# WSL-launched PowerShell can have a polluted PATHEXT; restore it first.
$env:PATHEXT = '.COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC'

Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

public static class SettingsSmokeNative
{
    [DllImport("user32.dll", CharSet = CharSet.Unicode, ExactSpelling = true)]
    public static extern IntPtr FindWindowW(string c, string n);

    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr h);

    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr h, out RECT r);

    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);

    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();

    [DllImport("user32.dll")]
    public static extern bool PostMessageW(IntPtr h, uint m, UIntPtr w, IntPtr l);

    [DllImport("user32.dll")]
    public static extern uint GetDpiForWindow(IntPtr h);

    [DllImport("user32.dll")]
    public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);

    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr h, int cmd);

    [DllImport("user32.dll")]
    public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);

    public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
}
'@

$SWP_NOZORDER   = 0x0004
$SWP_NOACTIVATE = 0x0010
$SW_SHOW        = 5
$WM_CLOSE       = 0x0010

function Get-SettingsWindow([int]$ProcessId) {
    $candidate = [SettingsSmokeNative]::FindWindowW('Tauri Window', [NullString]::Value)
    if (-not $candidate) { return $null }
    [uint32]$candidatePid = 0
    [void][SettingsSmokeNative]::GetWindowThreadProcessId($candidate, [ref]$candidatePid)
    if ($candidatePid -ne $ProcessId) { return $null }
    return $candidate
}

function Get-PetWindow {
    $hwnd = [SettingsSmokeNative]::FindWindowW('PetsonaPetWindow', [NullString]::Value)
    if ($hwnd -eq [IntPtr]::Zero) { return $null }
    return $hwnd
}

function Stop-Petsona {
    Get-Process petsona-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 400
}

function Get-Health([int]$Port) {
    try {
        return Invoke-RestMethod -Uri ("http://127.0.0.1:{0}/health" -f $Port) -TimeoutSec 2
    } catch {
        return $null
    }
}

function Save-WindowShot([IntPtr]$Hwnd, [string]$Path, [int]$Padding = 12) {
    $rect = New-Object SettingsSmokeNative+RECT
    [void][SettingsSmokeNative]::GetWindowRect($Hwnd, [ref]$rect)
    $w = ($rect.Right - $rect.Left) + 2 * $Padding
    $h = ($rect.Bottom - $rect.Top) + 2 * $Padding
    $bitmap = New-Object System.Drawing.Bitmap $w, $h
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $hdc = $graphics.GetHdc()
    $printed = [SettingsSmokeNative]::PrintWindow($Hwnd, $hdc, 2)
    $graphics.ReleaseHdc($hdc)
    if (-not $printed) {
        $graphics.CopyFromScreen($rect.Left - $Padding, $rect.Top - $Padding, 0, 0, $bitmap.Size)
    }
    $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    $graphics.Dispose()
    $bitmap.Dispose()
    return $printed
}

function Get-DistinctColors([string]$Path, [int]$Step = 37) {
    $bitmap = New-Object System.Drawing.Bitmap $Path
    $colors = New-Object 'System.Collections.Generic.HashSet[int]'
    for ($y = 0; $y -lt $bitmap.Height; $y += $Step) {
        for ($x = 0; $x -lt $bitmap.Width; $x += $Step) {
            [void]$colors.Add($bitmap.GetPixel($x, $y).ToArgb())
        }
    }
    $bitmap.Dispose()
    return $colors.Count
}

$script:failures = @()

# ---------------- isolated data directory ----------------
Write-Host '== prepare isolated home =='
$homeDir = Join-Path $env:TEMP 'petsona-settings-smoke'
if (Test-Path $homeDir) { Remove-Item -LiteralPath $homeDir -Recurse -Force }
$petDir = Join-Path $homeDir 'pets\test_fixture_v2'
New-Item -ItemType Directory -Path $petDir -Force | Out-Null
Copy-Item (Join-Path $RepoRoot 'crates\petsona-core\testdata\v2-test-pet\pet.json') $petDir
Copy-Item (Join-Path $RepoRoot 'crates\petsona-core\testdata\v2-test-pet\spritesheet.png') $petDir
$config = '{"stateServer":{"port":' + $Port + '},"activePet":"test_fixture_v2","firstRun":false,"window":{"scale":1.0}}'
[IO.File]::WriteAllText((Join-Path $homeDir 'config.json'), $config)
Write-Host "  home: $homeDir"
Write-Host "  port: $Port"

try {
    # ---------------- 1. present + focus ----------------
    Write-Host '== settings present / focus =='
    Stop-Petsona
    $env:PETSONA_HOME = $homeDir
    $proc = Start-Process -FilePath $Exe -ArgumentList '--show-settings' -PassThru
    $settings = $null
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    while ($watch.ElapsedMilliseconds -lt 20000) {
        $settings = Get-SettingsWindow $proc.Id
        if ($settings -and [SettingsSmokeNative]::IsWindowVisible($settings)) { break }
        Start-Sleep -Milliseconds 100
    }
    $visible = [bool]$settings -and [SettingsSmokeNative]::IsWindowVisible($settings)
    if (-not $visible) { throw 'settings window not visible within 20s' }
    Start-Sleep -Milliseconds 500
    $fgOk = ([SettingsSmokeNative]::GetForegroundWindow() -eq $settings)
    if (-not $fgOk) { $script:failures += 'settings window is not foreground on --show-settings' }
    Write-Host ("  settings visible: True; foreground: {0}" -f $fgOk)

    # ---------------- 2. sidebar layout screenshots ----------------
    Write-Host '== sidebar layout =='
    $rect = New-Object SettingsSmokeNative+RECT
    [void][SettingsSmokeNative]::GetWindowRect($settings, [ref]$rect)
    $dpi = [SettingsSmokeNative]::GetDpiForWindow($settings)
    $dpiScale = [Math]::Max(1.0, $dpi / 96.0)
    $wideWidth = $rect.Right - $rect.Left
    Write-Host ("  default window: {0}x{1} physical (dpi {2}, scale {3})" -f $wideWidth, ($rect.Bottom - $rect.Top), $dpi, $dpiScale)

    $wideShot = Join-Path $OutDir 'petsona-fa1-wide.png'
    $printedWide = Save-WindowShot $settings $wideShot
    Write-Host "  wide shot:   $wideShot (PrintWindow=$printedWide)"

    $narrowPhysical = [int][Math]::Round($NarrowCssPx * $dpiScale)
    [void][SettingsSmokeNative]::SetWindowPos($settings, [IntPtr]::Zero, $rect.Left, $rect.Top, $narrowPhysical, ($rect.Bottom - $rect.Top), ($SWP_NOZORDER -bor $SWP_NOACTIVATE))
    Start-Sleep -Milliseconds 1000
    $narrowRect = New-Object SettingsSmokeNative+RECT
    [void][SettingsSmokeNative]::GetWindowRect($settings, [ref]$narrowRect)
    $actualNarrow = $narrowRect.Right - $narrowRect.Left
    if ($actualNarrow -ge $wideWidth) { $script:failures += 'settings window did not shrink' }
    $narrowShot = Join-Path $OutDir 'petsona-fa1-narrow.png'
    $printedNarrow = Save-WindowShot $settings $narrowShot
    Write-Host ("  narrow window: {0} physical (~{1} CSS px); shot: {2} (PrintWindow={3})" -f $actualNarrow, $NarrowCssPx, $narrowShot, $printedNarrow)

    # ---------------- 3. close hides only ----------------
    Write-Host '== close hides only =='
    [void][SettingsSmokeNative]::PostMessageW($settings, $WM_CLOSE, [UIntPtr]::Zero, [IntPtr]::Zero)
    $hidden = $false
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    while ($watch.ElapsedMilliseconds -lt 5000) {
        if (-not [SettingsSmokeNative]::IsWindowVisible($settings)) { $hidden = $true; break }
        Start-Sleep -Milliseconds 100
    }
    $alive = -not $proc.HasExited
    $pet = Get-PetWindow
    $petVisible = [bool]$pet -and [SettingsSmokeNative]::IsWindowVisible($pet)
    $health = Get-Health $Port
    $healthOk = [bool]$health -and $health.ok
    if (-not $hidden) { $script:failures += 'settings window still visible after WM_CLOSE' }
    if (-not $alive) { $script:failures += 'shell process exited with the settings window' }
    if (-not $petVisible) { $script:failures += 'pet window hidden with the settings window' }
    if (-not $healthOk) { $script:failures += 'state server unreachable after settings close' }
    Write-Host ("  hidden: {0}; process alive: {1}; pet visible: {2}; /health ok: {3}" -f $hidden, $alive, $petVisible, $healthOk)

    # ---------------- 4. re-show proxy renders again ----------------
    Write-Host '== re-show renders again (proxy for tray reopen) =='
    [void][SettingsSmokeNative]::ShowWindow($settings, $SW_SHOW)
    Start-Sleep -Milliseconds 900
    $reopened = [SettingsSmokeNative]::IsWindowVisible($settings)
    $reopenShot = Join-Path $OutDir 'petsona-fa1-reopen.png'
    $printedReopen = Save-WindowShot $settings $reopenShot
    $colors = Get-DistinctColors $reopenShot
    if (-not $reopened) { $script:failures += 'settings window did not re-show' }
    if ($colors -lt 20) { $script:failures += "re-shown settings webview looks blank (distinct sample colors=$colors)" }
    Write-Host ("  visible after re-show: {0}; distinct sample colors: {1} (expected >=20); shot: {2}" -f $reopened, $colors, $reopenShot)
} finally {
    Stop-Petsona
    Remove-Item Env:\PETSONA_HOME -ErrorAction SilentlyContinue
}

Write-Host '== manual checks (user, no automation) =='
Write-Host '  - FA-1-1: right-click the tray icon -> "设置…" presents and focuses the window'
Write-Host '  - FA-1-4: scroll every page; cards and bottom actions are reachable, not clipped'
Write-Host '  - FA-1-5: switch Windows app mode light/dark; content AND title bar follow without reopening'
Write-Host '  - FA-1-7: close with the X, then reopen from the tray; no white screen / error fallback'
Write-Host '  - FA-1-8: Tab through controls, Enter activates buttons, focus stays visible'

if ($script:failures.Count -gt 0) {
    Write-Host ('== FAIL: ' + ($script:failures -join '; '))
    exit 1
}
Write-Host '== PASS: settings window layout/lifecycle checks =='
exit 0
