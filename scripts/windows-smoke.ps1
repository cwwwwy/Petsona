#requires -Version 5.1
<#
.SYNOPSIS
    Native Windows frontend smoke tests (apps/windows, WinUI 3 + Win32).
.DESCRIPTION
    Starts the native app with an isolated PETSONA_HOME and checks the state
    protocol, TTL fallback, window traits, overlays, pixel pass-through
    switching, single-instance behaviour, the tray window, empty-library
    first run and shutdown cleanup.

    The previous egui-shell smoke lives in git history (commit c4fc413);
    it is not part of the frozen legacy line any more.
.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\windows-smoke.ps1
#>
[CmdletBinding()]
param(
    [string] $AppPath = "",
    [switch] $SkipBuild
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

$script:Passed = 0
$script:Failed = 0
$script:Skipped = 0

function Write-Section([string] $Title) {
    Write-Host ""
    Write-Host ("== {0} ==" -f $Title) -ForegroundColor Cyan
}

function Add-Result([string] $Id, [string] $Name, [bool] $Ok, [string] $Detail = "") {
    if ($Ok) {
        $script:Passed++
        Write-Host ("[PASS] {0} {1} {2}" -f $Id, $Name, $Detail) -ForegroundColor Green
    }
    else {
        $script:Failed++
        Write-Host ("[FAIL] {0} {1} {2}" -f $Id, $Name, $Detail) -ForegroundColor Red
    }
}

function Add-Skip([string] $Id, [string] $Name, [string] $Why) {
    $script:Skipped++
    Write-Host ("[SKIP] {0} {1} - {2}" -f $Id, $Name, $Why) -ForegroundColor Yellow
}

function Find-FreeTcpPort {
    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $listener.Start()
    $port = ([System.Net.IPEndPoint]$listener.LocalEndpoint).Port
    $listener.Stop()
    return $port
}

function Test-PortBindable([int] $Port) {
    try {
        $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, $Port)
        $listener.Start()
        $listener.Stop()
        return $true
    }
    catch {
        return $false
    }
}

function Write-Utf8NoBom([string] $Path, [string] $Content) {
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content, $encoding)
}

function Get-WindowHash([IntPtr] $Handle) {
    $rect = New-Object "PetsonaSmokeNative+RECT"
    [void][PetsonaSmokeNative]::GetWindowRect($Handle, [ref]$rect)
    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    $petRect = $rect
    if ($width -le 0 -or $height -le 0) { return "" }

    $bmp = New-Object System.Drawing.Bitmap $width, $height
    $graphics = [System.Drawing.Graphics]::FromImage($bmp)
    $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, (New-Object System.Drawing.Size $width, $height))
    $stream = New-Object System.IO.MemoryStream
    $bmp.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
    $graphics.Dispose()
    $bmp.Dispose()
    $bytes = $stream.ToArray()
    $stream.Dispose()
    $md5 = [System.Security.Cryptography.MD5]::Create()
    try { return [System.BitConverter]::ToString($md5.ComputeHash($bytes)) } finally { $md5.Dispose() }
}

function Wait-Health([int] $Port, [int] $TimeoutSeconds = 20) {
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        try {
            return Invoke-RestMethod -Uri ("http://127.0.0.1:{0}/health" -f $Port) -TimeoutSec 2
        }
        catch {
            Start-Sleep -Milliseconds 250
        }
    }

    return $null
}

function Wait-HealthState([int] $Port, [string] $State, [int] $TimeoutSeconds = 6) {
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        try {
            $health = Invoke-RestMethod -Uri ("http://127.0.0.1:{0}/health" -f $Port) -TimeoutSec 2
            if ($health.state -eq $State) {
                return $true
            }
        }
        catch {
        }

        Start-Sleep -Milliseconds 150
    }

    return $false
}

function Get-FocusOutcome([IntPtr] $Expected, [uint32] $AppPid) {
    $foreground = [PetsonaSmokeNative]::GetForegroundWindow()
    if ($foreground -eq $Expected) { return "PASS" }
    if ($foreground -eq [IntPtr]::Zero) { return "SKIP" }
    # A background automation session cannot legally grant foreground to the
    # app it started. Only a wrong window *inside* the app is a real failure.
    if ([PetsonaSmokeNative]::PidOf($foreground) -ne $AppPid) { return "SKIP" }
    return "FAIL"
}

function Find-OpaquePoint([IntPtr] $Handle, [int] $Left, [int] $Top, [int] $Width, [int] $Height) {
    # The 30% point is the known idle-row-union probe used by N9/N15, so it
    # stays clickable in every pose; the remaining candidates are fallbacks.
    $candidates = @(
        @(0.50, 0.30), @(0.50, 0.50), @(0.50, 0.70),
        @(0.35, 0.50), @(0.65, 0.50), @(0.50, 0.15),
        @(0.50, 0.85), @(0.20, 0.50), @(0.80, 0.50)
    )
    foreach ($candidate in $candidates) {
        $x = [int]($Left + $Width * $candidate[0])
        $y = [int]($Top + $Height * $candidate[1])
        [void][PetsonaSmokeNative]::SetCursorPos($x, $y)
        Start-Sleep -Milliseconds 250
        $style = [PetsonaSmokeNative]::GetWindowLongPtr($Handle, -20).ToInt64()
        if (($style -band 0x20) -eq 0) { return @($x, $y) }
    }
    return @()
}

function Get-HealthState([int] $Port) {
    try {
        return (Invoke-RestMethod -Uri ("http://127.0.0.1:{0}/health" -f $Port) -TimeoutSec 2).state
    }
    catch {
        return ""
    }
}

function Wait-HealthNotState([int] $Port, [string] $State, [int] $TimeoutSeconds = 8) {
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        try {
            $health = Invoke-RestMethod -Uri ("http://127.0.0.1:{0}/health" -f $Port) -TimeoutSec 2
            if ($health.state -ne $State) {
                return $true
            }
        }
        catch {
        }

        Start-Sleep -Milliseconds 200
    }

    return $false
}

if (-not ("PetsonaSmokeNative" -as [type])) {
    Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class PetsonaSmokeNative {
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr param);
    [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "GetClassNameW")] public static extern int GetClassName(IntPtr hWnd, StringBuilder text, int max);
    [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "GetWindowTextW")] public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int max);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtr(IntPtr hWnd, int index);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
    [DllImport("user32.dll", EntryPoint = "PostMessageW")] public static extern bool PostMessage(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
    public static IntPtr MakeLParam(int low, int high) {
        return new IntPtr((high << 16) | (low & 0xFFFF));
    }
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT point);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();

    /// <summary>
    /// Without this the probe sees virtualized (scaled) coordinates while the
    /// per-monitor-aware app sees physical ones, so cursor and window rects
    /// do not line up on scaled displays.
    /// </summary>
    public static void MakeDpiAware() {
        try {
            if (!SetProcessDpiAwarenessContext(new IntPtr(-4))) {
                SetProcessDPIAware();
            }
        }
        catch (Exception) {
            try { SetProcessDPIAware(); } catch (Exception) { }
        }
    }
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }

    public static List<IntPtr> ByClass(string className) {
        var found = new List<IntPtr>();
        EnumWindows((h, p) => {
            var sb = new StringBuilder(256);
            GetClassName(h, sb, 256);
            if (sb.ToString() == className) { found.Add(h); }
            return true;
        }, IntPtr.Zero);
        return found;
    }

    public static List<IntPtr> VisibleByClass(string className) {
        var found = new List<IntPtr>();
        EnumWindows((h, p) => {
            var sb = new StringBuilder(256);
            GetClassName(h, sb, 256);
            if (sb.ToString() == className && IsWindowVisible(h)) { found.Add(h); }
            return true;
        }, IntPtr.Zero);
        return found;
    }

    public static IntPtr VisibleByTitle(string titleLike) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, p) => {
            if (!IsWindowVisible(h)) { return true; }
            var sb = new StringBuilder(256);
            GetWindowText(h, sb, 256);
            if (sb.ToString().Contains(titleLike)) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }

    public static uint PidOf(IntPtr hWnd) {
        uint pid;
        GetWindowThreadProcessId(hWnd, out pid);
        return pid;
    }

    public static List<string> VisibleRows(uint pid) {
        var rows = new List<string>();
        EnumWindows((h, p) => {
            uint wpid;
            GetWindowThreadProcessId(h, out wpid);
            if (wpid != pid || !IsWindowVisible(h)) { return true; }
            var sb = new StringBuilder(256);
            GetClassName(h, sb, 256);
            rows.Add(sb.ToString());
            return true;
        }, IntPtr.Zero);
        return rows;
    }

    public static IntPtr ByClassForPid(string className, uint pid) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, p) => {
            uint wpid;
            GetWindowThreadProcessId(h, out wpid);
            if (wpid != pid) { return true; }
            var sb = new StringBuilder(256);
            GetClassName(h, sb, 256);
            if (sb.ToString() == className) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }

    public static List<IntPtr> VisibleByClassAndPid(string className, uint pid) {
        var found = new List<IntPtr>();
        EnumWindows((h, p) => {
            if (!IsWindowVisible(h)) { return true; }
            uint wpid;
            GetWindowThreadProcessId(h, out wpid);
            if (wpid != pid) { return true; }
            var sb = new StringBuilder(256);
            GetClassName(h, sb, 256);
            if (sb.ToString() == className) { found.Add(h); }
            return true;
        }, IntPtr.Zero);
        return found;
    }

    public static IntPtr VisibleByClassForPid(string className, uint pid) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, p) => {
            if (!IsWindowVisible(h)) { return true; }
            uint wpid;
            GetWindowThreadProcessId(h, out wpid);
            if (wpid != pid) { return true; }
            var sb = new StringBuilder(256);
            GetClassName(h, sb, 256);
            if (sb.ToString() == className) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
"@
}

Add-Type -AssemblyName System.Drawing
[PetsonaSmokeNative]::MakeDpiAware()

$app = $AppPath
if ([string]::IsNullOrWhiteSpace($app)) {
    $app = Join-Path $root "apps\windows\Petsona\bin\x64\Release\net10.0-windows10.0.26100.0\win-x64\Petsona.exe"
}

if (-not (Test-Path -LiteralPath $app)) {
    Write-Host "Missing app: $app (build apps\windows first)" -ForegroundColor Red
    exit 1
}

$fixture = Join-Path $root "crates\petsona-core\testdata\v2-test-pet"
$dataDir = Join-Path $env:TEMP ("petsona-native-smoke-" + [Guid]::NewGuid().ToString("N"))
$emptyHome = Join-Path $env:TEMP ("petsona-native-smoke-empty-" + [Guid]::NewGuid().ToString("N"))
$port = Find-FreeTcpPort
$emptyPort = Find-FreeTcpPort

$originalCursor = New-Object "PetsonaSmokeNative+POINT"
[void][PetsonaSmokeNative]::GetCursorPos([ref]$originalCursor)

$process = $null
$second = $null
$emptyProcess = $null
$exitCode = 0

try {
    Write-Section "setup"
    New-Item -ItemType Directory -Path (Join-Path $dataDir "pets\v2-test-pet") -Force | Out-Null
    Copy-Item (Join-Path $fixture "pet.json") (Join-Path $dataDir "pets\v2-test-pet\pet.json") -Force
    Copy-Item (Join-Path $fixture "spritesheet.png") (Join-Path $dataDir "pets\v2-test-pet\spritesheet.png") -Force
    Write-Utf8NoBom (Join-Path $dataDir "config.json") ('{"stateServer":{"enabled":true,"port":' + $port + '}}')

    New-Item -ItemType Directory -Path $emptyHome -Force | Out-Null
    Write-Utf8NoBom (Join-Path $emptyHome "config.json") ('{"stateServer":{"enabled":true,"port":' + $emptyPort + '}}')

    Write-Host ("home={0} port={1}" -f $dataDir, $port)

    Write-Section "launch"
    $env:PETSONA_HOME = $dataDir
    $process = Start-Process -FilePath $app -PassThru
    $health = Wait-Health $port 25
    Add-Result "N1" "process starts and reports health" ($null -ne $health) ("pid=" + $process.Id)
    if ($null -eq $health) {
        throw "state protocol did not come up"
    }

    # The first sprite frame can take a moment after the protocol is ready.
    $petWindows = @()
    $deadline = (Get-Date).AddSeconds(12)
    while ((Get-Date) -lt $deadline) {
        $petWindows = @([PetsonaSmokeNative]::VisibleByClassAndPid("PetsonaPetWindow", [uint32]$process.Id))
        if ($petWindows.Count -ge 1) { break }
        Start-Sleep -Milliseconds 250
    }

    Add-Result "N2" "pet window appears" ($petWindows.Count -ge 1) ("count=" + $petWindows.Count)
    Add-Result "N3" "fixture pet is loaded" ($health.pet -eq "TestPet") ("pet=" + $health.pet)
    if ($petWindows.Count -eq 0) {
        throw "pet window did not appear"
    }

    $rect = New-Object "PetsonaSmokeNative+RECT"
    [void][PetsonaSmokeNative]::GetWindowRect($petWindows[0], [ref]$rect)
    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    $style = [PetsonaSmokeNative]::GetWindowLongPtr($petWindows[0], -16).ToInt64()
    $exStyle = [PetsonaSmokeNative]::GetWindowLongPtr($petWindows[0], -20).ToInt64()
    $hasCaption = ($style -band 0x00C00000) -ne 0
    $layered = ($exStyle -band 0x80000) -ne 0
    $toolWindow = ($exStyle -band 0x80) -ne 0
    $noActivate = ($exStyle -band 0x08000000) -ne 0
    Add-Result "N4" "window traits" ((-not $hasCaption) -and $layered -and $toolWindow -and $noActivate -and $width -gt 0 -and $height -gt 0) `
        ("size={0}x{1} caption={2} layered={3} tool={4} noactivate={5}" -f $width, $height, $hasCaption, $layered, $toolWindow, $noActivate)

    $overlays = @()
    $deadline = (Get-Date).AddSeconds(6)
    while ((Get-Date) -lt $deadline) {
        $overlays = @([PetsonaSmokeNative]::VisibleByClassAndPid("PetsonaOverlayWindow", [uint32]$process.Id))
        if ($overlays.Count -ge 1) { break }
        Start-Sleep -Milliseconds 250
    }

    Add-Result "N5" "edit button overlay is visible" ($overlays.Count -ge 1) ("count=" + $overlays.Count)

    $trayWindow = [PetsonaSmokeNative]::ByClassForPid("PetsonaTrayWindow", [uint32]$process.Id)
    $trayWindows = if ($trayWindow -ne [IntPtr]::Zero) { @($trayWindow) } else { @() }
    Add-Result "N6" "tray window exists" ($trayWindows.Count -ge 1) ("count=" + $trayWindows.Count)

    Write-Section "tray context menu"
    if ($trayWindows.Count -ge 1) {
        # NOTIFYICON_VERSION_4 right-click: LOWORD(lParam)=icon id,
        # HIWORD(lParam)=WM_CONTEXTMENU, wParam=screen point. This is the
        # delivery shape Explorer uses (the callback goes through the window
        # procedure, not only through the message queue).
        $menuX = [int](($petRect.Left + $petRect.Right) / 2)
        $menuY = [int](($petRect.Top + $petRect.Bottom) / 2)
        [void][PetsonaSmokeNative]::PostMessage(
            $trayWindows[0],
            0x8002,
            [PetsonaSmokeNative]::MakeLParam($menuX, $menuY),
            [PetsonaSmokeNative]::MakeLParam(1, 0x007B))
        $menu = [IntPtr]::Zero
        $deadline = (Get-Date).AddSeconds(5)
        while ((Get-Date) -lt $deadline) {
            $menu = [PetsonaSmokeNative]::VisibleByClassForPid("#32768", [uint32]$process.Id)
            if ($menu -ne [IntPtr]::Zero) { break }
            Start-Sleep -Milliseconds 100
        }
        Add-Result "N17" "tray callback opens the context menu" ($menu -ne [IntPtr]::Zero) ("hwnd=" + $menu)
        if ($menu -ne [IntPtr]::Zero) {
            [PetsonaSmokeNative]::keybd_event(0x1B, 0, 0, [UIntPtr]::Zero)
            [PetsonaSmokeNative]::keybd_event(0x1B, 0, 2, [UIntPtr]::Zero)
            Start-Sleep -Milliseconds 300
        }
    }
    else {
        Add-Skip "N17" "tray callback opens the context menu" "tray window missing"
    }

    Write-Section "composer focus"
    $editButton = [IntPtr]::Zero
    $deadline = (Get-Date).AddSeconds(3)
    while ((Get-Date) -lt $deadline -and $editButton -eq [IntPtr]::Zero) {
        foreach ($overlay in @([PetsonaSmokeNative]::VisibleByClassAndPid("PetsonaOverlayWindow", [uint32]$process.Id))) {
            $overlayRect = New-Object "PetsonaSmokeNative+RECT"
            [void][PetsonaSmokeNative]::GetWindowRect($overlay, [ref]$overlayRect)
            $overlayWidth = $overlayRect.Right - $overlayRect.Left
            $overlayHeight = $overlayRect.Bottom - $overlayRect.Top
            if ($overlayWidth -ge 36 -and $overlayWidth -le 44 -and
                $overlayHeight -ge 36 -and $overlayHeight -le 44 -and
                $overlayRect.Top -ge ($petRect.Bottom - 4)) {
                $editButton = $overlay
                break
            }
        }
        if ($editButton -eq [IntPtr]::Zero) { Start-Sleep -Milliseconds 100 }
    }

    if ($editButton -ne [IntPtr]::Zero) {
        $buttonRect = New-Object "PetsonaSmokeNative+RECT"
        [void][PetsonaSmokeNative]::GetWindowRect($editButton, [ref]$buttonRect)
        $buttonX = [int](($buttonRect.Left + $buttonRect.Right) / 2)
        $buttonY = [int](($buttonRect.Top + $buttonRect.Bottom) / 2)
        [void][PetsonaSmokeNative]::SetCursorPos($buttonX, $buttonY)
        Start-Sleep -Milliseconds 250
        [PetsonaSmokeNative]::mouse_event(0x02, 0, 0, 0, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 80
        [PetsonaSmokeNative]::mouse_event(0x04, 0, 0, 0, [UIntPtr]::Zero)

        $composer = [IntPtr]::Zero
        $deadline = (Get-Date).AddSeconds(6)
        while ((Get-Date) -lt $deadline) {
            $composer = [PetsonaSmokeNative]::VisibleByClassForPid("WinUIDesktopWin32WindowClass", [uint32]$process.Id)
            if ($composer -ne [IntPtr]::Zero) { break }
            Start-Sleep -Milliseconds 150
        }

        if ($composer -ne [IntPtr]::Zero) {
            $deadline = (Get-Date).AddSeconds(3)
            while ((Get-Date) -lt $deadline -and [PetsonaSmokeNative]::GetForegroundWindow() -ne $composer) {
                Start-Sleep -Milliseconds 100
            }
        }

        $composerOutcome = if ($composer -eq [IntPtr]::Zero) { "MISSING" } else { Get-FocusOutcome $composer ([uint32]$process.Id) }
        if ($composerOutcome -eq "PASS") {
            Add-Result "N18" "composer opens with focus" $true ("hwnd=" + $composer)
        }
        elseif ($composerOutcome -eq "SKIP") {
            Add-Skip "N18" "composer opens with focus" "automated session holds the foreground; manual W13 covers real focus"
        }
        else {
            Add-Result "N18" "composer opens with focus" $false ("hwnd=" + $composer + " outcome=" + $composerOutcome)
        }

        # Always close the composer, even when the foreground assertion was
        # skipped: a stuck composer pauses gaze and would poison N21.
        if ($composer -ne [IntPtr]::Zero) {
            [PetsonaSmokeNative]::keybd_event(0x1B, 0, 0, [UIntPtr]::Zero)
            [PetsonaSmokeNative]::keybd_event(0x1B, 0, 2, [UIntPtr]::Zero)
            Start-Sleep -Milliseconds 300
            if ([PetsonaSmokeNative]::IsWindowVisible($composer)) {
                [void][PetsonaSmokeNative]::PostMessage($composer, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
                $deadline = (Get-Date).AddSeconds(3)
                while ((Get-Date) -lt $deadline -and [PetsonaSmokeNative]::IsWindowVisible($composer)) {
                    Start-Sleep -Milliseconds 100
                }
            }
        }
    }
    else {
        Add-Skip "N18" "composer opens with focus" "edit button not found"
    }

    Write-Section "idle animation"
    # Park the cursor away from the pet first: the doubled gaze range keeps
    # the pet in a held look pose while the cursor rests near the edit button,
    # and a held pose does not advance the idle row.
    $idleRect = New-Object "PetsonaSmokeNative+RECT"
    [void][PetsonaSmokeNative]::GetWindowRect($petWindows[0], [ref]$idleRect)
    [void][PetsonaSmokeNative]::SetCursorPos($idleRect.Left - 300, $idleRect.Top - 300)
    Start-Sleep -Milliseconds 600

    # Sample several frames: a single pair can land on the same animation
    # frame by chance because the idle cycle is shorter than the interval.
    $frameHashes = @()
    for ($sample = 0; $sample -lt 8; $sample++) {
        $frameHashes += Get-WindowHash $petWindows[0]
        Start-Sleep -Milliseconds 250
    }

    $distinctFrames = ($frameHashes | Select-Object -Unique).Count
    Add-Result "N16" "idle animation advances frames" ($distinctFrames -gt 1) ("distinctFrames=" + $distinctFrames)

    Write-Section "state protocol"
    $body = '{"source":"windows-smoke","state":"waiting","message":"smoke","ttlMs":1500}'
    Invoke-RestMethod -Method Post -Uri ("http://127.0.0.1:{0}/state" -f $port) -ContentType "application/json" -Body $body -TimeoutSec 5 | Out-Null
    Add-Result "N7" "POST /state switches the pet" (Wait-HealthState $port "waiting" 5)
    Add-Result "N8" "TTL falls back to the base state" (Wait-HealthNotState $port "waiting" 10)

    Write-Section "pixel pass-through"
    # Re-read the live rect: the initial placement is applied by the app and a
    # stale N4 rect would make the cursor probe the empty desktop next to it.
    $passRect = New-Object "PetsonaSmokeNative+RECT"
    [void][PetsonaSmokeNative]::GetWindowRect($petWindows[0], [ref]$passRect)
    $cx = [int](($passRect.Left + $passRect.Right) / 2)
    $opaqueY = [int]($passRect.Top + ($passRect.Bottom - $passRect.Top) * 0.30)
    # A human (or another tool) may move the physical mouse mid-check, so the
    # probe retries with an explicit cursor re-position each round.
    $passOk = $false
    $farTransparent = $false
    $nearTransparent = $true
    for ($attempt = 0; $attempt -lt 3 -and -not $passOk; $attempt++) {
        [void][PetsonaSmokeNative]::SetCursorPos($passRect.Left - 40, $passRect.Top - 40)
        # The pass-through poller is a WM_TIMER on the UI thread; under render
        # load a 50 ms tick can be delivered a few hundred ms late.
        Start-Sleep -Milliseconds 1200
        $farStyle = [PetsonaSmokeNative]::GetWindowLongPtr($petWindows[0], -20).ToInt64()
        $farTransparent = ($farStyle -band 0x20) -ne 0

        [void][PetsonaSmokeNative]::SetCursorPos($cx, $opaqueY)
        Start-Sleep -Milliseconds 1200
        $nearStyle = [PetsonaSmokeNative]::GetWindowLongPtr($petWindows[0], -20).ToInt64()
        $nearTransparent = ($nearStyle -band 0x20) -ne 0

        if ($farTransparent -and (-not $nearTransparent)) {
            $passOk = $true
        }
    }

    $cursorNow = New-Object "PetsonaSmokeNative+POINT"
    [void][PetsonaSmokeNative]::GetCursorPos([ref]$cursorNow)
    $cursorHeld = [Math]::Abs($cursorNow.X - $cx) -le 3 -and [Math]::Abs($cursorNow.Y - $opaqueY) -le 3
    if ($passOk) {
        Add-Result "N9" "pass-through follows the cursor" $true ("outside={0} inside={1} attempts={2}" -f $farTransparent, $nearTransparent, $attempt)
    }
    elseif (-not $cursorHeld) {
        Add-Skip "N9" "pass-through follows the cursor" "the physical cursor was moved by another input device during the check"
    }
    else {
        Add-Result "N9" "pass-through follows the cursor" $false ("outside={0} inside={1} attempts={2}" -f $farTransparent, $nearTransparent, $attempt)
    }

    Write-Section "click interaction"
    # Put the cursor back on an opaque pixel, wait for the pass-through
    # release, then click and expect the waving reaction within a second.
    $clickRect = New-Object "PetsonaSmokeNative+RECT"
    [void][PetsonaSmokeNative]::GetWindowRect($petWindows[0], [ref]$clickRect)
    $cx = [int](($clickRect.Left + $clickRect.Right) / 2)
    $opaqueY = [int]($clickRect.Top + ($clickRect.Bottom - $clickRect.Top) * 0.30)
    [void][PetsonaSmokeNative]::SetCursorPos($cx, $opaqueY)
    Start-Sleep -Milliseconds 1200
    $clickCursor = New-Object "PetsonaSmokeNative+POINT"
    [void][PetsonaSmokeNative]::GetCursorPos([ref]$clickCursor)
    $clickHeld = [Math]::Abs($clickCursor.X - $cx) -le 3 -and [Math]::Abs($clickCursor.Y - $opaqueY) -le 3
    if (-not $clickHeld) {
        Add-Skip "N15" "clicking the pet triggers waving" "the physical cursor was moved by another input device before the click"
    }
    else {
    [PetsonaSmokeNative]::mouse_event(0x02, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [PetsonaSmokeNative]::mouse_event(0x04, 0, 0, 0, [UIntPtr]::Zero)
    $waved = $false
    $deadline = (Get-Date).AddSeconds(5)
    while ((Get-Date) -lt $deadline) {
        try {
            $health = Invoke-RestMethod -Uri ("http://127.0.0.1:{0}/health" -f $port) -TimeoutSec 2
            if ($health.state -eq "waving") { $waved = $true; break }
        }
        catch {
        }

        Start-Sleep -Milliseconds 150
    }

    Add-Result "N15" "clicking the pet triggers waving" $waved
    }

    Write-Section "drag animation"
    # Keep the button down while moving: the running row must advance even
    # though the high-rate mouse stream keeps the dispatcher busy. Read the
    # engine's published sprite indices instead of screen hashes: the window
    # itself moves during a drag, so a screen hash would change even if the
    # sprite stayed on frame 0. Rows 1/2 are running-right/left (8..23); the
    # drag is retried once in case the first press lands while the pet is in a
    # transient pose.
    $dragLog = Join-Path $dataDir "logs\windows-native-drag.log"
    $runningSprites = @()
    $dragAttempts = 0
    for ($attempt = 1; $attempt -le 2 -and $runningSprites.Count -le 1; $attempt++) {
        $dragAttempts = $attempt
        $dragRect = New-Object "PetsonaSmokeNative+RECT"
        [void][PetsonaSmokeNative]::GetWindowRect($petWindows[0], [ref]$dragRect)
        # Let the N15 wave finish before picking the stable idle point.
        Start-Sleep -Milliseconds 700
        $dragPoint = Find-OpaquePoint $petWindows[0] $dragRect.Left $dragRect.Top `
            ($dragRect.Right - $dragRect.Left) ($dragRect.Bottom - $dragRect.Top)
        if ($dragPoint.Count -lt 2) { continue }
        $dragX = [int]$dragPoint[0]
        $dragY = [int]$dragPoint[1]
        [void][PetsonaSmokeNative]::SetCursorPos($dragX, $dragY)
        # Stay clear of the 320 ms double-click window left by the previous
        # interaction.
        Start-Sleep -Milliseconds 500
        [PetsonaSmokeNative]::mouse_event(0x02, 0, 0, 0, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 120
        for ($step = 0; $step -lt 14; $step++) {
            $dragX -= 7
            [void][PetsonaSmokeNative]::SetCursorPos($dragX, $dragY)
            Start-Sleep -Milliseconds 55
        }
        [PetsonaSmokeNative]::mouse_event(0x04, 0, 0, 0, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 400

        $dragSprites = @()
        if (Test-Path -LiteralPath $dragLog) {
            $dragSprites = Get-Content -LiteralPath $dragLog -ErrorAction SilentlyContinue |
                ForEach-Object { if ($_ -match 'sprite=(\d+)') { [int]$Matches[1] } }
        }
        $runningSprites = $dragSprites |
            Where-Object { $_ -ge 8 -and $_ -le 23 } |
            Select-Object -Unique
    }
    Add-Result "N20" "dragging advances the running animation" ($runningSprites.Count -gt 1) ("runningFrames=" + (($runningSprites | Sort-Object) -join ',') + " attempts=" + $dragAttempts)

    Write-Section "drag reversal"
    $reverseRect = New-Object "PetsonaSmokeNative+RECT"
    [void][PetsonaSmokeNative]::GetWindowRect($petWindows[0], [ref]$reverseRect)
    # N20 ended in a running pose; wait for idle before choosing the point so
    # the scan cannot pick a pixel that only exists in a running frame.
    Start-Sleep -Milliseconds 700
    $reversePoint = Find-OpaquePoint $petWindows[0] $reverseRect.Left $reverseRect.Top `
        ($reverseRect.Right - $reverseRect.Left) ($reverseRect.Bottom - $reverseRect.Top)
    if ($reversePoint.Count -lt 2) {
        Add-Skip "N22" "drag reverses direction while held" "no clickable pet point found"
    }
    else {
        $reverseX = [int]$reversePoint[0]
        $reverseY = [int]$reversePoint[1]
        [void][PetsonaSmokeNative]::SetCursorPos($reverseX, $reverseY)
        Start-Sleep -Milliseconds 500
        [PetsonaSmokeNative]::mouse_event(0x02, 0, 0, 0, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 120
        for ($step = 0; $step -lt 10; $step++) {
            $reverseX -= 6
            [void][PetsonaSmokeNative]::SetCursorPos($reverseX, $reverseY)
            Start-Sleep -Milliseconds 55
        }
        $cursorBeforeRight = New-Object "PetsonaSmokeNative+POINT"
        [void][PetsonaSmokeNative]::GetCursorPos([ref]$cursorBeforeRight)
        for ($step = 0; $step -lt 10; $step++) {
            $reverseX += 6
            [void][PetsonaSmokeNative]::SetCursorPos($reverseX, $reverseY)
            Start-Sleep -Milliseconds 55
        }
        $cursorAfterRight = New-Object "PetsonaSmokeNative+POINT"
        [void][PetsonaSmokeNative]::GetCursorPos([ref]$cursorAfterRight)
        [PetsonaSmokeNative]::mouse_event(0x04, 0, 0, 0, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 400

        # The drag state is a native override; the protocol /health copy is not
        # refreshed for every animation tick, so verify the engine frames that
        # the drag renderer actually published (row 1 = 8..15, row 2 = 16..23).
        $reverseSprites = @()
        if (Test-Path -LiteralPath $dragLog) {
            $reverseSprites = Get-Content -LiteralPath $dragLog -ErrorAction SilentlyContinue |
                ForEach-Object { if ($_ -match 'sprite=(\d+)') { [int]$Matches[1] } }
        }
        $reverseLeft = @($reverseSprites | Where-Object { $_ -ge 16 -and $_ -le 23 }).Count -gt 0
        $reverseRight = @($reverseSprites | Where-Object { $_ -ge 8 -and $_ -le 15 }).Count -gt 0
        $reverseTail = if (Test-Path -LiteralPath $dragLog) {
            (Get-Content -LiteralPath $dragLog -ErrorAction SilentlyContinue | Select-Object -Last 8) -join " | "
        } else { "no-drag-log" }
        $directionLog = Join-Path $dataDir "logs\windows-native-drag-direction.log"
        $directionTail = if (Test-Path -LiteralPath $directionLog) {
            (Get-Content -LiteralPath $directionLog -ErrorAction SilentlyContinue | Select-Object -Last 12) -join " | "
        } else { "no-direction-log" }
        Add-Result "N22" "drag reverses direction while held" ($reverseLeft -and $reverseRight) ("leftFrames=" + $reverseLeft + " rightFrames=" + $reverseRight + " cursor=" + $cursorBeforeRight.X + "->" + $cursorAfterRight.X + " dir=" + $directionTail + " drag=" + $reverseTail)
    }

    Write-Section "gaze directions"
    $leftoverComposer = [PetsonaSmokeNative]::VisibleByClassForPid("WinUIDesktopWin32WindowClass", [uint32]$process.Id)
    if ($leftoverComposer -ne [IntPtr]::Zero) {
        Add-Skip "N21" "gaze completes both look rows" "composer remained open; gaze is paused while typing"
    }
    else {
    # A cross-row gaze is a two-step transition. Once the cursor stops, the
    # app must keep re-issuing the target so the pet can finish the row switch
    # instead of freezing on the bridge pose.
    [void][PetsonaSmokeNative]::GetWindowRect($petWindows[0], [ref]$dragRect)
    $gazeY = [int](($dragRect.Top + $dragRect.Bottom) / 2)
    $gazeCenterX = [int](($dragRect.Left + $dragRect.Right) / 2)
    # 100 px from the centre: outside the widened 35% dead zone (67 px on the
    # 192x208 fixture), inside the doubled trigger ellipse.
    $gazeRightX = $gazeCenterX + 100
    $gazeLeftX = $gazeCenterX - 100
    [void][PetsonaSmokeNative]::SetCursorPos($gazeRightX, $gazeY)
    $gazeRight = Wait-HealthState $port "look-row-9" 3
    [void][PetsonaSmokeNative]::SetCursorPos($gazeLeftX, $gazeY)
    $gazeLeft = Wait-HealthState $port "look-row-10" 3
    Add-Result "N21" "gaze completes both look rows" ($gazeRight -and $gazeLeft) ("right={0} left={1}" -f $gazeRight, $gazeLeft)
    [void][PetsonaSmokeNative]::SetCursorPos($dragRect.Left - 200, $dragRect.Top - 200)
    Start-Sleep -Milliseconds 400
    }

    Write-Section "single instance"
    $second = Start-Process -FilePath $app -PassThru
    Start-Sleep -Seconds 6
    $secondAlive = -not $second.HasExited
    if ($secondAlive) {
        $secondPet = @([PetsonaSmokeNative]::VisibleByClassAndPid("PetsonaPetWindow", [uint32]$second.Id))
        Add-Result "N10" "second instance does not create a pet window" ($secondPet.Count -eq 0) ("secondPid=" + $second.Id)
    }
    else {
        Add-Result "N10" "second instance exits" $true ("exitCode=" + $second.ExitCode)
    }

    Add-Result "N11" "first instance still alive" (-not $process.HasExited)

    Write-Section "empty library first run"
    $env:PETSONA_HOME = $emptyHome
    $emptyProcess = Start-Process -FilePath $app -PassThru
    # Match the WinUI settings window by window class + process id; the title
    # is localized and PowerShell 5.1 console encoding is not trustworthy.
    $settings = [IntPtr]::Zero
    $deadline = (Get-Date).AddSeconds(25)
    while ((Get-Date) -lt $deadline) {
        $settings = [PetsonaSmokeNative]::VisibleByClassForPid("WinUIDesktopWin32WindowClass", [uint32]$emptyProcess.Id)
        if ($settings -ne [IntPtr]::Zero) { break }
        Start-Sleep -Milliseconds 300
    }

    Add-Result "N12" "empty library opens the settings window" ($settings -ne [IntPtr]::Zero) ("hwnd=" + $settings)

    if ($settings -ne [IntPtr]::Zero) {
        $deadline = (Get-Date).AddSeconds(3)
        while ((Get-Date) -lt $deadline -and [PetsonaSmokeNative]::GetForegroundWindow() -ne $settings) {
            Start-Sleep -Milliseconds 100
        }
    }
    $settingsOutcome = if ($settings -eq [IntPtr]::Zero) { "MISSING" } else { Get-FocusOutcome $settings ([uint32]$emptyProcess.Id) }
    if ($settingsOutcome -eq "PASS") {
        Add-Result "N19" "settings opens with focus" $true ("hwnd=" + $settings)
    }
    elseif ($settingsOutcome -eq "SKIP") {
        Add-Skip "N19" "settings opens with focus" "automated session holds the foreground; manual W13 covers real focus"
    }
    else {
        Add-Result "N19" "settings opens with focus" $false ("hwnd=" + $settings + " outcome=" + $settingsOutcome)
    }

    Write-Section "webp sprite sheet"
    if ($null -ne $emptyProcess -and -not $emptyProcess.HasExited) {
        Stop-Process -Id $emptyProcess.Id -Force
        Start-Sleep -Milliseconds 500
    }

    $webpHome = Join-Path $env:TEMP ("petsona-native-smoke-webp-" + [Guid]::NewGuid().ToString("N"))
    $webpPort = Find-FreeTcpPort
    $webpFixture = Join-Path $root "crates\petsona-core\testdata\v2-test-pet-webp"
    New-Item -ItemType Directory -Path (Join-Path $webpHome "pets\v2-test-pet-webp") -Force | Out-Null
    Copy-Item (Join-Path $webpFixture "pet.json") (Join-Path $webpHome "pets\v2-test-pet-webp\pet.json") -Force
    Copy-Item (Join-Path $webpFixture "spritesheet.webp") (Join-Path $webpHome "pets\v2-test-pet-webp\spritesheet.webp") -Force
    Write-Utf8NoBom (Join-Path $webpHome "config.json") ('{"stateServer":{"enabled":true,"port":' + $webpPort + '}}')

    $env:PETSONA_HOME = $webpHome
    $webpProcess = Start-Process -FilePath $app -PassThru
    $webpHealth = Wait-Health $webpPort 20
    Write-Output ("webp health: ok={0} pet={1} path={2}" -f $webpHealth.ok, $webpHealth.pet, $webpHealth.petPath)
    $webpPet = @()
    $deadline = (Get-Date).AddSeconds(18)
    $round = 0
    while ((Get-Date) -lt $deadline) {
        $webpPet = @([PetsonaSmokeNative]::VisibleByClassAndPid("PetsonaPetWindow", [uint32]$webpProcess.Id))
        $round++
        if ($webpPet.Count -ge 1) { break }
        if ($round % 10 -eq 0) {
            $visible = @([PetsonaSmokeNative]::VisibleRows([uint32]$webpProcess.Id)) -join ","
            Write-Output ("  webp round {0}: alive={1} windows=[{2}]" -f $round, (-not $webpProcess.HasExited), $visible)
        }

        Start-Sleep -Milliseconds 300
    }

    if ($webpPet.Count -lt 1) {
        Write-Output "--- webp decode error log ---"
        Get-Content (Join-Path $webpHome "logs\windows-native-error.log") -ErrorAction SilentlyContinue | Select-Object -Last 8
        Write-Output "--- webp frame log ---"
        Get-Content (Join-Path $webpHome "logs\windows-native-frame.log") -ErrorAction SilentlyContinue | Select-Object -Last 4
        Write-Output "--- webp home files ---"
        Get-ChildItem (Join-Path $webpHome "pets") -Recurse -File -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Name
    }

    Add-Result "N14" "webp sprite sheet renders (GDI+ fallback via WinRT)" ($webpPet.Count -ge 1) ("count=" + $webpPet.Count)
    if (-not $webpProcess.HasExited) { Stop-Process -Id $webpProcess.Id -Force }
    Start-Sleep -Milliseconds 500
    Remove-Item -LiteralPath $webpHome -Recurse -Force -ErrorAction SilentlyContinue

    Write-Section "shutdown"
    if ($null -ne $emptyProcess -and -not $emptyProcess.HasExited) { Stop-Process -Id $emptyProcess.Id -Force }
    if ($null -ne $second -and -not $second.HasExited) { Stop-Process -Id $second.Id -Force }
    if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }
    Start-Sleep -Seconds 1
    Add-Result "N13" "state-server port is released after shutdown" (Test-PortBindable $port)
}
catch {
    Write-Host ("smoke aborted: {0}" -f $_.Exception.Message) -ForegroundColor Red
    $exitCode = 1
}
finally {
    [void][PetsonaSmokeNative]::SetCursorPos($originalCursor.X, $originalCursor.Y)
    foreach ($p in @($emptyProcess, $second, $process)) {
        if ($null -ne $p -and -not $p.HasExited) {
            Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        }
    }

    Remove-Item Env:\PETSONA_HOME -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $dataDir -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $emptyHome -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host ("native smoke summary: passed={0} failed={1} skipped={2}" -f $script:Passed, $script:Failed, $script:Skipped) -ForegroundColor Cyan
if ($script:Failed -gt 0 -or $exitCode -ne 0) {
    exit 1
}
