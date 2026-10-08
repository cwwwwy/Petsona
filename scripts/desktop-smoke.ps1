# Petsona desktop smoke checks (Windows, run on the real desktop).
#
# M0 + M1 coverage:
#   1. startup timing (start -> pet window visible, runtime-driven)
#   2. pet window ex-styles / pass-through toggling / focus (mouse parts optional)
#   3. single instance: a second process exits, only one shell remains
#   4. state protocol: /health, /pets, /state TTL, invalid state, action:clear
#   5. graceful exit releases the state port
#   6. empty pet library opens the settings window and hides the pet
#
# NOTE: mouse sections move the real cursor; do not touch the mouse while they run.
# Usage (Windows PowerShell):
#   powershell -ExecutionPolicy Bypass -File scripts\desktop-smoke.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\desktop-smoke.ps1 -SkipMouseChecks

param(
  [string]$Exe = "$env:USERPROFILE\petsona-build\desktop-target\debug\petsona-desktop.exe",
  [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
  [int]$PetPort = 17897,
  [int]$EmptyPort = 17898,
  [int]$StartupRuns = 3,
  [switch]$SkipMouseChecks
)

$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

public static class SmokeNative
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
    public static extern uint GetDpiForWindow(IntPtr hWnd);

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

$homePet = Join-Path $env:TEMP 'petsona-smoke-pet'
$homeEmpty = Join-Path $env:TEMP 'petsona-smoke-empty'

function Get-PetWindow {
    # PowerShell marshals $null as an empty string for string P/Invoke args;
    # pass a real NULL so the window title is not filtered.
    $hwnd = [SmokeNative]::FindWindowW('PetsonaPetWindow', [NullString]::Value)
    if ($hwnd -eq [IntPtr]::Zero) { return $null }
    return $hwnd
}

function Get-SettingsWindow([int]$ProcessId) {
    $candidate = [SmokeNative]::FindWindowW('Tauri Window', [NullString]::Value)
    if (-not $candidate) { return $null }
    [uint32]$candidatePid = 0
    [void][SmokeNative]::GetWindowThreadProcessId($candidate, [ref]$candidatePid)
    if ($candidatePid -ne $ProcessId) { return $null }
    return $candidate
}

function Stop-Petsona {
    Get-Process petsona-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 400
}

function Wait-PetVisible([int]$TimeoutMs) {
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    while ($watch.ElapsedMilliseconds -lt $TimeoutMs) {
        $hwnd = Get-PetWindow
        if ($hwnd -and [SmokeNative]::IsWindowVisible($hwnd)) { return $watch.ElapsedMilliseconds }
        Start-Sleep -Milliseconds 10
    }
    return $null
}

function Test-PortOpen([int]$Port) {
    $client = New-Object System.Net.Sockets.TcpClient
    try {
        $client.Connect('127.0.0.1', $Port)
        return $true
    } catch {
        return $false
    } finally {
        $client.Close()
    }
}

function Get-Health([int]$Port) {
    try {
        return Invoke-RestMethod -Uri ("http://127.0.0.1:{0}/health" -f $Port) -TimeoutSec 2
    } catch {
        return $null
    }
}

function Wait-Health([int]$Port, [int]$TimeoutMs) {
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    while ($watch.ElapsedMilliseconds -lt $TimeoutMs) {
        $health = Get-Health $Port
        if ($health -and $health.ok) { return $health }
        Start-Sleep -Milliseconds 200
    }
    return $null
}

# ---------------- isolated data directories ----------------
Write-Host '== prepare isolated homes =='
if (Test-Path $homePet) { Remove-Item -LiteralPath $homePet -Recurse -Force }
if (Test-Path $homeEmpty) { Remove-Item -LiteralPath $homeEmpty -Recurse -Force }

$petDir = Join-Path $homePet 'pets\test_fixture_v2'
New-Item -ItemType Directory -Path $petDir -Force | Out-Null
Copy-Item (Join-Path $RepoRoot 'crates\petsona-core\testdata\v2-test-pet\pet.json') $petDir
Copy-Item (Join-Path $RepoRoot 'crates\petsona-core\testdata\v2-test-pet\spritesheet.png') $petDir
$configPet = '{"stateServer":{"port":' + $PetPort + '},"activePet":"test_fixture_v2","firstRun":false,"window":{"scale":1.5}}'
[IO.File]::WriteAllText((Join-Path $homePet 'config.json'), $configPet)

New-Item -ItemType Directory -Path $homeEmpty -Force | Out-Null
$configEmpty = '{"stateServer":{"port":' + $EmptyPort + '},"firstRun":true}'
[IO.File]::WriteAllText((Join-Path $homeEmpty 'config.json'), $configEmpty)
Write-Host "  pet home:   $homePet"
Write-Host "  empty home: $homeEmpty"

# ---------------- 1. startup timing (runtime-driven) ----------------
Write-Host '== startup timing =='
$env:PETSONA_HOME = $homePet
for ($run = 1; $run -le $StartupRuns; $run++) {
    Stop-Petsona
    $null = Start-Process -FilePath $Exe -PassThru
    $visibleMs = Wait-PetVisible 20000
    if ($visibleMs) {
        Write-Host ("  run {0}: pet visible after {1} ms" -f $run, $visibleMs)
    } else {
        Write-Host ("  run {0}: pet window NOT visible within 20s  [FAIL]" -f $run)
    }
    Stop-Petsona
}

# ---------------- 2. styles / pass-through / focus ----------------
Write-Host '== window styles / pass-through / focus =='
# Drop any remembered position so the drag test proves a fresh save.
$configPetNoPosition = '{"stateServer":{"port":' + $PetPort + '},"activePet":"test_fixture_v2","firstRun":false,"window":{"scale":1.5}}'
[IO.File]::WriteAllText((Join-Path $homePet 'config.json'), $configPetNoPosition)
$null = Start-Process -FilePath $Exe -PassThru
$visibleMs = Wait-PetVisible 20000
if (-not $visibleMs) { throw 'pet window not found within 20s' }
$hwnd = Get-PetWindow

$style = [SmokeNative]::GetExStyle($hwnd)
Write-Host ("  WS_EX_LAYERED    : {0}" -f (($style -band $WS_EX_LAYERED) -ne 0))
Write-Host ("  WS_EX_TOOLWINDOW : {0}" -f (($style -band $WS_EX_TOOLWINDOW) -ne 0))
Write-Host ("  WS_EX_TOPMOST    : {0}" -f (($style -band $WS_EX_TOPMOST) -ne 0))
Write-Host ("  WS_EX_NOACTIVATE : {0}" -f (($style -band $WS_EX_NOACTIVATE) -ne 0))

$rect = New-Object SmokeNative+RECT
[void][SmokeNative]::GetWindowRect($hwnd, [ref]$rect)
$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top

# geometry: the runtime reports the canonical V2 cell (192x208) for any V2
# pet, config scale is 1.5, and the shell clamps DPI to >= 1.0
$dpi = [SmokeNative]::GetDpiForWindow($hwnd)
$dpiScale = [Math]::Max(1.0, $dpi / 96.0)
$expectedW = [int][Math]::Round(192 * 1.5 * $dpiScale)
$expectedH = [int][Math]::Round(208 * 1.5 * $dpiScale)
Write-Host ("  geometry: dpi={0} expected={1}x{2} actual={3}x{4}" -f $dpi, $expectedW, $expectedH, $width, $height)

# animation: the runtime advances sprite_index; capture a few frames and
# require at least two distinct images
$hashes = @()
for ($frame = 1; $frame -le 14; $frame++) {
    $shot = Join-Path $env:TEMP ("petsona-smoke-anim{0}.png" -f $frame)
    & (Join-Path $PSScriptRoot 'desktop-shot.ps1') -Out $shot -Padding 20 | Out-Null
    $hashes += (Get-FileHash -LiteralPath $shot -Algorithm MD5).Hash
    Start-Sleep -Milliseconds 350
}
$distinct = ($hashes | Sort-Object -Unique | Measure-Object).Count
Write-Host ("  animation distinct frames over 14 samples: {0}  (expected >1)" -f $distinct)

$sawTransparent = $false
$sawOpaque = $false
$opaquePoint = $null
if (-not $SkipMouseChecks) {
    for ($ix = 1; $ix -le 7; $ix++) {
        for ($iy = 1; $iy -le 7; $iy++) {
            $x = [int]($rect.Left + $width * $ix / 8.0)
            $y = [int]($rect.Top + $height * $iy / 8.0)
            [void][SmokeNative]::SetCursorPos($x, $y)
            Start-Sleep -Milliseconds 100
            $s = [SmokeNative]::GetExStyle($hwnd)
            if (($s -band $WS_EX_TRANSPARENT) -ne 0) {
                $sawTransparent = $true
            } else {
                $sawOpaque = $true
                if (-not $opaquePoint) { $opaquePoint = @($x, $y) }
            }
        }
    }
    Write-Host ("  pass-through toggling: transparent={0} opaque={1}" -f $sawTransparent, $sawOpaque)

    Write-Host '== drag test =='
    # Real mouse activity can steal the cursor between our SetCursorPos and the
    # synthetic press; re-find a clickable point and retry a few times.
    $dragMoved = $false
    for ($attempt = 1; $attempt -le 3 -and -not $dragMoved; $attempt++) {
        $current = New-Object SmokeNative+RECT
        [void][SmokeNative]::GetWindowRect($hwnd, [ref]$current)
        $cw = $current.Right - $current.Left
        $ch = $current.Bottom - $current.Top
        # Sample a grid and press the clickable pixel nearest the window
        # centre: a few pixels of real-mouse drift around an edge pixel would
        # otherwise turn the press into a pass-through click.
        $candidates = @()
        for ($ix = 2; $ix -le 6; $ix++) {
            for ($iy = 2; $iy -le 6; $iy++) {
                $x = [int]($current.Left + $cw * $ix / 8.0)
                $y = [int]($current.Top + $ch * $iy / 8.0)
                [void][SmokeNative]::SetCursorPos($x, $y)
                Start-Sleep -Milliseconds 120
                if ((([SmokeNative]::GetExStyle($hwnd)) -band $WS_EX_TRANSPARENT) -eq 0) { $candidates += ,@($x, $y) }
            }
        }
        if ($candidates.Count -eq 0) { Write-Host ("  attempt {0}: no clickable point found" -f $attempt); continue }
        $centreX = $current.Left + $cw / 2.0
        $centreY = $current.Top + $ch / 2.0
        $point = $candidates | Sort-Object { [Math]::Pow($_[0] - $centreX, 2) + [Math]::Pow($_[1] - $centreY, 2) } | Select-Object -First 1

        [void][SmokeNative]::SetCursorPos($point[0], $point[1])
        Start-Sleep -Milliseconds 150
        if ((([SmokeNative]::GetExStyle($hwnd)) -band $WS_EX_TRANSPARENT) -ne 0) {
            Write-Host ("  attempt {0}: point went transparent before press (cursor stolen?)" -f $attempt)
            continue
        }

        $before = New-Object SmokeNative+RECT
        [void][SmokeNative]::GetWindowRect($hwnd, [ref]$before)
        [SmokeNative]::mouse_event($MOUSEEVENTF_LEFTDOWN, 0, 0, 0, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 80
        for ($step = 1; $step -le 5; $step++) {
            [void][SmokeNative]::SetCursorPos($point[0] + 20 * $step, $point[1] + 12 * $step)
            Start-Sleep -Milliseconds 40
        }
        [SmokeNative]::mouse_event($MOUSEEVENTF_LEFTUP, 0, 0, 0, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 250
        $after = New-Object SmokeNative+RECT
        [void][SmokeNative]::GetWindowRect($hwnd, [ref]$after)
        $dx = $after.Left - $before.Left
        $dy = $after.Top - $before.Top
        Write-Host ("  attempt {0}: moved dx={1} dy={2}  (expected ~100/~60)" -f $attempt, $dx, $dy)
        # Any real movement proves the capture/move path; a live mouse can steal
        # the cursor mid-drag so the full 100px is not a hard requirement.
        if (([Math]::Abs($dx) + [Math]::Abs($dy)) -ge 8) { $dragMoved = $true }
    }
    $dragResult = if ($dragMoved) { 'MOVED' } else { 'NO MOVEMENT [WARN: likely real-mouse interference]' }
    Write-Host ("  drag result: {0}" -f $dragResult)

    if ($dragMoved) {
        # the drag end sends SetPosition; the worker persists startPosition
        $configPath = Join-Path $homePet 'config.json'
        $saved = $null
        for ($wait = 1; $wait -le 10 -and -not $saved; $wait++) {
            Start-Sleep -Milliseconds 200
            try {
                $config = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json
                if ($config.window.startPosition) { $saved = $config.window.startPosition }
            } catch {}
        }
        if ($saved) {
            Write-Host ("  position saved: {0},{1}" -f $saved.x, $saved.y)
            Stop-Petsona
            $null = Start-Process -FilePath $Exe -PassThru
            if (Wait-PetVisible 20000) {
                $hwnd2 = Get-PetWindow
                $restored = New-Object SmokeNative+RECT
                [void][SmokeNative]::GetWindowRect($hwnd2, [ref]$restored)
                Write-Host ("  position restored dx={0} dy={1}  (expected ~0/~0)" -f `
                    ([Math]::Abs($restored.Left - [int]$saved.x)), ([Math]::Abs($restored.Top - [int]$saved.y)))
            } else {
                Write-Host '  restart for position check: pet not visible  [FAIL]'
            }
            Stop-Petsona
            $null = Start-Process -FilePath $Exe -PassThru
            $null = Wait-PetVisible 20000
            $hwnd = Get-PetWindow
        } else {
            Write-Host '  no startPosition saved after drag  [FAIL]'
        }
    }
} else {
    Write-Host '  (mouse checks skipped)'
}

$foreground = [SmokeNative]::GetForegroundWindow()
Write-Host ("  foreground is pet window: {0}  (expected False)" -f ($foreground -eq $hwnd))

Stop-Petsona

Write-Host '== settings focus (--show-settings) =='
$proc = Start-Process -FilePath $Exe -ArgumentList '--show-settings' -PassThru
$settings = $null
$watch = [System.Diagnostics.Stopwatch]::StartNew()
while ($watch.ElapsedMilliseconds -lt 20000) {
    $settings = Get-SettingsWindow $proc.Id
    if ($settings -and [SmokeNative]::IsWindowVisible($settings)) { break }
    Start-Sleep -Milliseconds 100
}
if ($settings -and [SmokeNative]::IsWindowVisible($settings)) {
    Start-Sleep -Milliseconds 400
    $fg = [SmokeNative]::GetForegroundWindow()
    Write-Host ("  settings visible: True; foreground is settings: {0}" -f ($fg -eq $settings))
} else {
    Write-Host '  settings window not visible within 20s  [FAIL]'
}
Stop-Petsona

# ---------------- 3. single instance ----------------
Write-Host '== single instance =='
Stop-Petsona
$procA = Start-Process -FilePath $Exe -PassThru
if (-not (Wait-PetVisible 20000)) { throw 'first instance pet window not visible' }

$procB = Start-Process -FilePath $Exe -PassThru
$exited = $false
$watch = [System.Diagnostics.Stopwatch]::StartNew()
while ($watch.ElapsedMilliseconds -lt 10000) {
    if ($procB.HasExited) { $exited = $true; break }
    Start-Sleep -Milliseconds 200
}
$aliveCount = (Get-Process petsona-desktop -ErrorAction SilentlyContinue | Measure-Object).Count
Write-Host ("  second process exited: {0}; surviving shells: {1}" -f $exited, $aliveCount)
$health = Get-Health $PetPort
Write-Host ("  first instance still healthy: {0}" -f [bool]$health)

# ---------------- 4. state protocol ----------------
Write-Host '== state protocol =='
$health = Wait-Health $PetPort 8000
if (-not $health) { throw "no /health on port $PetPort" }
Write-Host ("  /health ok: pet='{0}' pets={1}" -f $health.pet, ($health.pets -join ', '))
$listed = Invoke-RestMethod -Uri ("http://127.0.0.1:{0}/pets" -f $PetPort) -TimeoutSec 2
Write-Host ("  /pets contains fixture: {0}" -f ($listed -contains 'test_fixture_v2'))

$body = '{"source":"smoke","state":"waiting","message":"smoke","ttlMs":2000}'
Invoke-RestMethod -Method Post -Uri ("http://127.0.0.1:{0}/state" -f $PetPort) -ContentType 'application/json; charset=utf-8' -Body $body -TimeoutSec 2 | Out-Null
$stateNow = $null
$watch = [System.Diagnostics.Stopwatch]::StartNew()
while ($watch.ElapsedMilliseconds -lt 2000) {
    $h2 = Get-Health $PetPort
    if ($h2 -and $h2.state -eq 'waiting') { $stateNow = 'waiting'; break }
    Start-Sleep -Milliseconds 100
}
Write-Host ("  state after POST: {0}" -f $stateNow)
Start-Sleep -Milliseconds 2800
$h3 = Get-Health $PetPort
Write-Host ("  state after TTL:  {0}  (expected idle)" -f $h3.state)

$invalid = 0
try {
    Invoke-RestMethod -Method Post -Uri ("http://127.0.0.1:{0}/state" -f $PetPort) -ContentType 'application/json; charset=utf-8' -Body '{"state":"nope"}' -TimeoutSec 2 | Out-Null
} catch {
    $invalid = [int]$_.Exception.Response.StatusCode
}
Write-Host ("  invalid state returns: HTTP {0}  (expected 400)" -f $invalid)

$clearBody = '{"source":"smoke","action":"clear"}'
Invoke-RestMethod -Method Post -Uri ("http://127.0.0.1:{0}/state" -f $PetPort) -ContentType 'application/json; charset=utf-8' -Body $clearBody -TimeoutSec 2 | Out-Null
Write-Host '  action:clear accepted'

Stop-Petsona

# ---------------- 5. graceful exit releases the port ----------------
Write-Host '== exit releases the state port =='
$proc = Start-Process -FilePath $Exe -ArgumentList '--exit-after-ms', '3000' -PassThru
if (-not (Wait-Health $PetPort 10000)) { throw 'no /health before exit test' }
Write-Host '  /health ok before exit'
$watch = [System.Diagnostics.Stopwatch]::StartNew()
while ($watch.ElapsedMilliseconds -lt 10000) {
    if ($proc.HasExited) { break }
    Start-Sleep -Milliseconds 200
}
Start-Sleep -Milliseconds 500
$portOpen = Test-PortOpen $PetPort
Write-Host ("  process exited: {0}; port {1} open after exit: {2}  (expected False)" -f $proc.HasExited, $PetPort, $portOpen)

# ---------------- 6. empty library opens settings ----------------
Write-Host '== empty library opens settings =='
Stop-Petsona
$env:PETSONA_HOME = $homeEmpty
$procEmpty = Start-Process -FilePath $Exe -PassThru
$settingsSeen = $false
$watch = [System.Diagnostics.Stopwatch]::StartNew()
while ($watch.ElapsedMilliseconds -lt 20000) {
    $settings = Get-SettingsWindow $procEmpty.Id
    if ($settings -and [SmokeNative]::IsWindowVisible($settings)) { $settingsSeen = $true; break }
    Start-Sleep -Milliseconds 100
}
$petHwnd = Get-PetWindow
$petHidden = $true
if ($petHwnd) { $petHidden = -not [SmokeNative]::IsWindowVisible($petHwnd) }
$emptyHealth = Wait-Health $EmptyPort 8000
Write-Host ("  settings visible: {0}; pet hidden: {1}; /health ok: {2}; pet field empty: {3}" -f $settingsSeen, $petHidden, [bool]$emptyHealth, ($emptyHealth -and -not $emptyHealth.pet))

Stop-Petsona
Remove-Item Env:\PETSONA_HOME -ErrorAction SilentlyContinue

Write-Host '== smoke finished =='
$log = Join-Path $env:TEMP 'petsona-desktop.log'
if (Test-Path $log) {
    Write-Host '== shell log (last 8 lines) =='
    Get-Content -LiteralPath $log -Tail 8 | ForEach-Object { Write-Host ("  " + $_) }
}
