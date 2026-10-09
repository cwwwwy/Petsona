# Petsona desktop smoke checks (Windows, run on the real desktop).
#
# POLICY: this script never moves the cursor or injects mouse input. Pass-through
# clicks, dragging, clamping and cursor-shape checks are MANUAL items for the
# user (see docs/DESKTOP_VERIFICATION.md section A and the checklist printed at
# the end of this script).
#
# Coverage:
#   1. startup timing (start -> pet window visible, runtime-driven)
#   2. window ex-styles + position restore + geometry (scale x DPI) + animation
#   3. focus: the pet never becomes the foreground window; --show-settings does
#   4. single instance: a second process exits, only one shell remains
#   5. state protocol: /health, /pets, /state TTL, invalid state, action:clear
#   6. graceful exit releases the state port
#   7. empty pet library opens the settings window and hides the pet
#
# Usage (Windows PowerShell):
#   powershell -ExecutionPolicy Bypass -File scripts\desktop-smoke.ps1

param(
  [string]$Exe = "$env:USERPROFILE\petsona-build\desktop-target\debug\petsona-desktop.exe",
  [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
  [int]$PetPort = 17897,
  [int]$EmptyPort = 17898,
  [int]$StartupRuns = 3
)

$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System;
using System.Text;
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
    public static extern IntPtr GetForegroundWindow();

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern IntPtr FindWindowExW(IntPtr parent, IntPtr after, string className, string windowName);

    [DllImport("user32.dll")]
    public static extern bool PostMessageW(IntPtr hWnd, uint msg, UIntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern int GetWindowTextW(IntPtr hWnd, StringBuilder text, int count);

    // WM_GETTEXT is marshalled across processes, so it can read a control in
    // another process (unlike GetWindowText).
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern IntPtr SendMessageW(IntPtr hWnd, uint msg, IntPtr wParam, StringBuilder lParam);

    [DllImport("user32.dll")]
    public static extern bool AllowSetForegroundWindow(uint processId);

    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);

    public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }

    public delegate bool EnumProc(IntPtr h, IntPtr l);

    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumProc cb, IntPtr l);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);

    /// First *visible* top-level window of the given class (a hidden window of
    /// the same class may exist in another instance).
    public static IntPtr FindVisibleByClass(string cls)
    {
        IntPtr result = IntPtr.Zero;
        EnumWindows((h, l) => {
            if (!IsWindowVisible(h)) { return true; }
            var sb = new StringBuilder(128);
            GetClassNameW(h, sb, 128);
            if (sb.ToString() == cls) { result = h; return false; }
            return true;
        }, IntPtr.Zero);
        return result;
    }

    public static long GetExStyle(IntPtr hwnd)
    {
        if (IntPtr.Size == 8) { return GetWindowLongPtrW64(hwnd, -20).ToInt64(); }
        return GetWindowLongW32(hwnd, -20);
    }
}
'@

$WS_EX_TOOLWINDOW  = 0x00000080
$WS_EX_TOPMOST     = 0x00000008
$WS_EX_LAYERED     = 0x00080000
$WS_EX_NOACTIVATE  = 0x08000000

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

function Get-StripWindow {
    $hwnd = [SmokeNative]::FindWindowW('PetsonaStripWindow', [NullString]::Value)
    if ($hwnd -eq [IntPtr]::Zero) { return $null }
    return $hwnd
}

function Get-BubbleWindow {
    # Only visible bubbles count: the other instance may own a hidden one.
    $hwnd = [SmokeNative]::FindVisibleByClass('PetsonaOverlayWindow')
    if ($hwnd -eq [IntPtr]::Zero) { return $null }
    return $hwnd
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

# ---------------- 2. styles / position restore / geometry / animation ----------------
Write-Host '== window styles / position restore / geometry / animation =='
# Known remembered position: the pet must appear exactly there before we assert
# anything else (clamped only if it would fall outside the work area).
$configPetPositioned = '{"stateServer":{"port":' + $PetPort + '},"activePet":"test_fixture_v2","firstRun":false,"window":{"scale":1.5,"startPosition":{"x":200.0,"y":200.0,"displayId":null,"backingScale":1.0}}}'
[IO.File]::WriteAllText((Join-Path $homePet 'config.json'), $configPetPositioned)

$null = Start-Process -FilePath $Exe -PassThru
if (-not (Wait-PetVisible 20000)) { throw 'pet window not found within 20s' }
$hwnd = Get-PetWindow

$rect = New-Object SmokeNative+RECT
[void][SmokeNative]::GetWindowRect($hwnd, [ref]$rect)
$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top
Write-Host ("  position restore: expected=(200,200) actual=({0},{1})" -f $rect.Left, $rect.Top)

$style = [SmokeNative]::GetExStyle($hwnd)
Write-Host ("  WS_EX_LAYERED    : {0}" -f (($style -band $WS_EX_LAYERED) -ne 0))
Write-Host ("  WS_EX_TOOLWINDOW : {0}" -f (($style -band $WS_EX_TOOLWINDOW) -ne 0))
Write-Host ("  WS_EX_TOPMOST    : {0}" -f (($style -band $WS_EX_TOPMOST) -ne 0))
Write-Host ("  WS_EX_NOACTIVATE : {0}" -f (($style -band $WS_EX_NOACTIVATE) -ne 0))

# geometry: the runtime reports the canonical V2 cell (192x208) for any V2
# pet, config scale is 1.5, and the shell clamps DPI to >= 1.0
$dpi = [SmokeNative]::GetDpiForWindow($hwnd)
$dpiScale = [Math]::Max(1.0, $dpi / 96.0)
$expectedW = [int][Math]::Round(192 * 1.5 * $dpiScale)
$expectedH = [int][Math]::Round(208 * 1.5 * $dpiScale)
Write-Host ("  geometry: dpi={0} expected={1}x{2} actual={3}x{4}" -f $dpi, $expectedW, $expectedH, $width, $height)

# animation: the runtime advances sprite_index; screenshots (no cursor input)
$hashes = @()
for ($frame = 1; $frame -le 14; $frame++) {
    $shot = Join-Path $env:TEMP ("petsona-smoke-anim{0}.png" -f $frame)
    & (Join-Path $PSScriptRoot 'desktop-shot.ps1') -Out $shot -Padding 20 | Out-Null
    $hashes += (Get-FileHash -LiteralPath $shot -Algorithm MD5).Hash
    Start-Sleep -Milliseconds 350
}
$distinct = ($hashes | Sort-Object -Unique | Measure-Object).Count
Write-Host ("  animation distinct frames over 14 samples: {0}  (expected >1)" -f $distinct)

$foreground = [SmokeNative]::GetForegroundWindow()
Write-Host ("  foreground is pet window: {0}  (expected False)" -f ($foreground -eq $hwnd))

# the compact edit strip rides under the pet while the composer is closed
$strip = Get-StripWindow
$stripShown = [bool]$strip -and [SmokeNative]::IsWindowVisible($strip)
$stripSize = ''
if ($stripShown) {
    $stripRect = New-Object SmokeNative+RECT
    [void][SmokeNative]::GetWindowRect($strip, [ref]$stripRect)
    $stripSize = "{0}x{1}" -f ($stripRect.Right - $stripRect.Left), ($stripRect.Bottom - $stripRect.Top)
}
Write-Host ("  edit strip visible: {0} size={1}  (expected 36x6)" -f $stripShown, $stripSize)

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
$watchB = [System.Diagnostics.Stopwatch]::StartNew()
$noticeSeen = $false
while ($watchB.ElapsedMilliseconds -lt 2500) {
    $faultBubble = Get-BubbleWindow
    if ($faultBubble -and [SmokeNative]::IsWindowVisible($faultBubble)) { $noticeSeen = $true; break }
    Start-Sleep -Milliseconds 100
}
Write-Host ("  second instance fault notice visible: {0}" -f $noticeSeen)
$exited = $false
while ($watchB.ElapsedMilliseconds -lt 12000) {
    if ($procB.HasExited) { $exited = $true; break }
    Start-Sleep -Milliseconds 200
}
# the exited process can linger briefly in the process table
$aliveCount = 2
$watchC = [System.Diagnostics.Stopwatch]::StartNew()
while ($watchC.ElapsedMilliseconds -lt 4000) {
    $aliveCount = (Get-Process petsona-desktop -ErrorAction SilentlyContinue | Measure-Object).Count
    if ($aliveCount -le 1) { break }
    Start-Sleep -Milliseconds 200
}
Write-Host ("  second process exited: {0} after {1} ms (~3s notice); surviving shells: {2}" -f $exited, $watchB.ElapsedMilliseconds, $aliveCount)
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

# the speech bubble window must be visible while the message is alive
# (the runtime keeps protocol bubbles for a fixed 8 s, independent of ttlMs)
$bubbleSeen = $false
$watch = [System.Diagnostics.Stopwatch]::StartNew()
while ($watch.ElapsedMilliseconds -lt 2500) {
    $bubble = Get-BubbleWindow
    if ($bubble -and [SmokeNative]::IsWindowVisible($bubble)) { $bubbleSeen = $true; break }
    Start-Sleep -Milliseconds 100
}
Write-Host ("  bubble visible after POST: {0}" -f $bubbleSeen)

Start-Sleep -Milliseconds 2800
$h3 = Get-Health $PetPort
Write-Host ("  state after TTL:  {0}  (expected idle)" -f $h3.state)

$bubbleGone = $false
$watch = [System.Diagnostics.Stopwatch]::StartNew()
while ($watch.ElapsedMilliseconds -lt 12000) {
    $bubble = Get-BubbleWindow
    if (-not $bubble -or -not [SmokeNative]::IsWindowVisible($bubble)) { $bubbleGone = $true; break }
    Start-Sleep -Milliseconds 200
}
Write-Host ("  bubble hidden after its ~8s lifetime: {0}" -f $bubbleGone)

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

# ---------------- composer (keyboard only, no cursor input) ----------------
Write-Host '== composer (posted key events, no mouse) =='
Stop-Petsona
# start from an empty shell log so the composer assertions are unambiguous
Remove-Item -LiteralPath (Join-Path $env:TEMP 'petsona-desktop.log') -ErrorAction SilentlyContinue
$proc = Start-Process -FilePath $Exe -PassThru
if (-not (Wait-PetVisible 20000)) { throw 'pet window not visible for the composer test' }
$petHwnd = Get-PetWindow

function Get-ComposerWindow {
    $hwnd = [SmokeNative]::FindWindowW('PetsonaComposerWindow', [NullString]::Value)
    if ($hwnd -eq [IntPtr]::Zero) { return $null }
    return $hwnd
}
function Get-ComposerText([IntPtr]$Edit) {
    $buffer = New-Object System.Text.StringBuilder 512
    [void][SmokeNative]::SendMessageW($Edit, 0x000D, [IntPtr]512, $buffer)
    return $buffer.ToString()
}
function Wait-ComposerVisible([int]$TimeoutMs) {
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    while ($watch.ElapsedMilliseconds -lt $TimeoutMs) {
        $c = Get-ComposerWindow
        if ($c -and [SmokeNative]::IsWindowVisible($c)) { return $c }
        Start-Sleep -Milliseconds 100
    }
    return $null
}

# open via the pet window message (WM_APP + 1); grant this process foreground
# rights first so the composer can take focus without a real click
[void][SmokeNative]::AllowSetForegroundWindow([uint32]$proc.Id)
[void][SmokeNative]::PostMessageW($petHwnd, 0x8001, [UIntPtr]::Zero, [IntPtr]::Zero)
$composer = Wait-ComposerVisible 8000
Write-Host ("  composer visible: {0}" -f [bool]$composer)
if (-not $composer) { throw 'composer did not open' }
Start-Sleep -Milliseconds 300
$stripWhileOpen = Get-StripWindow
Write-Host ("  edit strip hidden while composer open: {0}" -f (-not $stripWhileOpen -or -not [SmokeNative]::IsWindowVisible($stripWhileOpen)))
$fg = [SmokeNative]::GetForegroundWindow()
Write-Host ("  foreground is composer: {0}" -f ($fg -eq $composer))
$edit = [SmokeNative]::FindWindowExW($composer, [IntPtr]::Zero, 'Edit', [NullString]::Value)
Write-Host ("  edit control found: {0}" -f [bool]$edit)

# type "hi" by posting WM_CHAR to the edit control
foreach ($char in @([char]'h', [char]'i')) {
    [void][SmokeNative]::PostMessageW($edit, 0x0102, [UIntPtr][int]$char, [IntPtr]::Zero)
}
Start-Sleep -Milliseconds 400
$typed = Get-ComposerText $edit
Write-Host ("  typed text: '{0}'  (expected 'hi')" -f $typed)

# Esc closes and keeps the draft
[void][SmokeNative]::PostMessageW($edit, 0x0100, [UIntPtr]0x1B, [IntPtr]::Zero)
Start-Sleep -Milliseconds 500
$hidden = -not [SmokeNative]::IsWindowVisible($composer)
Write-Host ("  composer hidden after Esc: {0}" -f $hidden)
Start-Sleep -Milliseconds 300
$stripBack = Get-StripWindow
Write-Host ("  edit strip back after Esc: {0}" -f ([bool]$stripBack -and [SmokeNative]::IsWindowVisible($stripBack)))

# reopen: the draft must still be there
[void][SmokeNative]::PostMessageW($petHwnd, 0x8001, [UIntPtr]::Zero, [IntPtr]::Zero)
$composer = Wait-ComposerVisible 8000
$draft = ''
if ($composer) { $draft = Get-ComposerText $edit }
Write-Host ("  draft restored after reopen: '{0}'  (expected 'hi')" -f $draft)

# Enter sends and clears the input. Cross-process WM_GETTEXT reads of a
# control can be stale, so the app log is the authoritative check here.
[void][SmokeNative]::PostMessageW($edit, 0x0100, [UIntPtr]0x0D, [IntPtr]::Zero)
$clearedInApp = $false
$watch = [System.Diagnostics.Stopwatch]::StartNew()
while ($watch.ElapsedMilliseconds -lt 3000) {
    $tail = Get-Content -LiteralPath (Join-Path $env:TEMP 'petsona-desktop.log') -Tail 8 -ErrorAction SilentlyContinue
    if ($tail -match "composer: cleared result=1 remaining=''") { $clearedInApp = $true; break }
    Start-Sleep -Milliseconds 150
}
Write-Host ("  input cleared after Enter: {0}  (app log)" -f $clearedInApp)

# Esc again for cleanliness
[void][SmokeNative]::PostMessageW($edit, 0x0100, [UIntPtr]0x1B, [IntPtr]::Zero)
Start-Sleep -Milliseconds 400
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

Write-Host '== manual checks (user, do not automate) =='
Write-Host '  - pass-through: click a transparent pixel -> desktop; click the pet body -> pet'
Write-Host '  - drag: follows the cursor; release at screen/taskbar edges stays inside the work area'
Write-Host '  - position memory: drag, quit, relaunch -> pet returns to the last position'
Write-Host '  - cursor over the pet stays the normal arrow (no busy ring)'
Write-Host '  - gaze: look follows the cursor in all 16 directions (including above the pet),'
Write-Host '    stays neutral very close to the centre, and does not jitter near the boundary'
Write-Host '  - bubble: hover pauses the progress bar and resume continues from the remaining time;'
Write-Host '    near the top of the screen the bubble flips below the pet; fade-in is visible'
Write-Host '  - tray: settings / show-hide / quit'

Write-Host '== smoke finished =='
$log = Join-Path $env:TEMP 'petsona-desktop.log'
if (Test-Path $log) {
    Write-Host '== shell log (last 8 lines) =='
    Get-Content -LiteralPath $log -Tail 8 | ForEach-Object { Write-Host ("  " + $_) }
}
