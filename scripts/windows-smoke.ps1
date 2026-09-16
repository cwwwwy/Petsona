#requires -Version 5.1

[CmdletBinding()]
param(
    [ValidateSet("Debug", "Release")]
    [string] $Configuration = "Release",
    [switch] $SkipBuild,
    [switch] $KeepArtifacts,
    [switch] $UseTestHooks,
    [string] $Executable = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$script:ChecksPassed = 0
$script:ChecksFailed = 0
$script:SmokeHome = $null
$script:Processes = New-Object System.Collections.Generic.List[object]
$script:ExitCode = 0
$script:TestHookPort = $null
$script:TestHookToken = $null
$script:NotepadProcess = $null
$script:NotepadWindowPid = $null
$script:NotepadFile = $null

function Write-Section {
    param([string] $Text)
    Write-Host ""
    Write-Host "=== $Text ===" -ForegroundColor Cyan
}

function Add-CheckResult {
    param(
        [string] $Id,
        [string] $Name,
        [bool] $Passed,
        [string] $Detail = ""
    )

    if ($Passed) {
        $script:ChecksPassed++
        Write-Host ("[PASS] {0} {1}" -f $Id, $Name) -ForegroundColor Green
    }
    else {
        $script:ChecksFailed++
        Write-Host ("[FAIL] {0} {1}" -f $Id, $Name) -ForegroundColor Red
        if ($Detail) {
            Write-Host ("       {0}" -f $Detail) -ForegroundColor DarkRed
        }
    }
}

function Invoke-SmokeCheck {
    param(
        [string] $Id,
        [string] $Name,
        [scriptblock] $Action
    )

    try {
        & $Action
        Add-CheckResult -Id $Id -Name $Name -Passed $true
    }
    catch {
        Add-CheckResult -Id $Id -Name $Name -Passed $false -Detail $_.Exception.Message
        throw
    }
}

function Assert-True {
    param(
        [bool] $Condition,
        [string] $Message
    )
    if (-not $Condition) {
        throw $Message
    }
}

function Find-FreeTcpPort {
    $listener = New-Object System.Net.Sockets.TcpListener([System.Net.IPAddress]::Loopback, 0)
    $listener.Start()
    try {
        return ([System.Net.IPEndPoint] $listener.LocalEndpoint).Port
    }
    finally {
        $listener.Stop()
    }
}

function Write-Utf8NoBom {
    param(
        [string] $Path,
        [string] $Text
    )
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Text, $encoding)
}

function Write-SmokeConfig {
    param(
        [string] $DataHome,
        [int] $Port
    )

    $config = [ordered] @{
        schemaVersion = 1
        activePet = $null
        activePersona = $null
        firstRun = $false
        bundledPetRemoved = $false
        window = [ordered] @{
            scale = 1.0
            opacity = 1.0
            alwaysOnTop = $true
            clickThrough = $true
            autoWalk = [ordered] @{
                enabled = $false
                intervalMinutes = 45
                walkSeconds = 8.0
                speedPxS = 18.0
                rangePx = 120.0
                userGraceSeconds = 30.0
            }
            startPosition = $null
        }
        deepseek = [ordered] @{
            baseUrl = "http://127.0.0.1:9/v1"
            model = "deepseek-v4-flash"
            apiKeyEnv = "DEEPSEEK_API_KEY"
            timeoutSeconds = 1
            maxTokens = 8
            temperature = 0.9
            thinkingDisabled = $true
        }
        greeting = [ordered] @{
            enabled = $false
            idleMinutes = 30
            cooldownMinutes = 120
            maxChars = 40
        }
        memory = [ordered] @{
            enabled = $false
            recentEvents = 5
            retentionDays = 90
            factLimit = 20
        }
        stateServer = [ordered] @{
            enabled = $true
            port = $Port
        }
    }

    $json = $config | ConvertTo-Json -Depth 8
    Write-Utf8NoBom -Path (Join-Path $DataHome "config.json") -Text $json
}

function Start-PetsonaProcess {
    param(
        [string] $DataHome,
        [string] $Tag,
        [string] $Executable
    )

    $stdout = Join-Path $DataHome ("stdout-{0}.log" -f $Tag)
    $stderr = Join-Path $DataHome ("stderr-{0}.log" -f $Tag)
    $oldHome = [Environment]::GetEnvironmentVariable("PETSONA_HOME", "Process")
    $oldRustLog = [Environment]::GetEnvironmentVariable("RUST_LOG", "Process")
    $oldHooksEnabled = [Environment]::GetEnvironmentVariable("PETSONA_TEST_HOOKS", "Process")
    $oldHooksPort = [Environment]::GetEnvironmentVariable("PETSONA_TEST_HOOKS_PORT", "Process")
    $oldHooksToken = [Environment]::GetEnvironmentVariable("PETSONA_TEST_HOOKS_TOKEN", "Process")
    $hadKey = Test-Path Env:\DEEPSEEK_API_KEY
    $oldKey = if ($hadKey) { $env:DEEPSEEK_API_KEY } else { $null }

    [Environment]::SetEnvironmentVariable("PETSONA_HOME", $DataHome, "Process")
    [Environment]::SetEnvironmentVariable("RUST_LOG", "info", "Process")
    if ($UseTestHooks) {
        [Environment]::SetEnvironmentVariable("PETSONA_TEST_HOOKS", "1", "Process")
        [Environment]::SetEnvironmentVariable("PETSONA_TEST_HOOKS_PORT", [string] $script:TestHookPort, "Process")
        [Environment]::SetEnvironmentVariable("PETSONA_TEST_HOOKS_TOKEN", $script:TestHookToken, "Process")
    }
    else {
        Remove-Item Env:\PETSONA_TEST_HOOKS -ErrorAction SilentlyContinue
        Remove-Item Env:\PETSONA_TEST_HOOKS_PORT -ErrorAction SilentlyContinue
        Remove-Item Env:\PETSONA_TEST_HOOKS_TOKEN -ErrorAction SilentlyContinue
    }
    Remove-Item Env:\DEEPSEEK_API_KEY -ErrorAction SilentlyContinue
    try {
        $process = Start-Process -FilePath $Executable `
            -WorkingDirectory $root `
            -RedirectStandardOutput $stdout `
            -RedirectStandardError $stderr `
            -PassThru
    }
    finally {
        [Environment]::SetEnvironmentVariable("PETSONA_HOME", $oldHome, "Process")
        [Environment]::SetEnvironmentVariable("RUST_LOG", $oldRustLog, "Process")
        [Environment]::SetEnvironmentVariable("PETSONA_TEST_HOOKS", $oldHooksEnabled, "Process")
        [Environment]::SetEnvironmentVariable("PETSONA_TEST_HOOKS_PORT", $oldHooksPort, "Process")
        [Environment]::SetEnvironmentVariable("PETSONA_TEST_HOOKS_TOKEN", $oldHooksToken, "Process")
        if ($hadKey) {
            $env:DEEPSEEK_API_KEY = $oldKey
        }
        else {
            Remove-Item Env:\DEEPSEEK_API_KEY -ErrorAction SilentlyContinue
        }
    }

    $entry = [pscustomobject] @{
        Process = $process
        Stdout = $stdout
        Stderr = $stderr
        Tag = $Tag
    }
    $script:Processes.Add($entry)
    return $entry
}

function Stop-PetsonaProcess {
    param($Entry)

    if ($null -eq $Entry -or $null -eq $Entry.Process) {
        return
    }
    try {
        $Entry.Process.Refresh()
        if (-not $Entry.Process.HasExited) {
            Stop-Process -Id $Entry.Process.Id -Force -ErrorAction SilentlyContinue
            [void] $Entry.Process.WaitForExit(5000)
        }
    }
    catch {
    }
}

function Get-Health {
    Invoke-RestMethod -Uri ("http://127.0.0.1:{0}/health" -f $script:Port) `
        -Method Get -TimeoutSec 2 -ErrorAction Stop
}

function Wait-ForHealth {
    param(
        [int] $TimeoutSeconds = 20,
        $Entry = $null
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $lastError = $null
    while ((Get-Date) -lt $deadline) {
        if ($null -ne $Entry) {
            $Entry.Process.Refresh()
            if ($Entry.Process.HasExited) {
                throw "Petsona exited before the state server became ready (exit $($Entry.Process.ExitCode))."
            }
        }
        try {
            return Get-Health
        }
        catch {
            $lastError = $_
            Start-Sleep -Milliseconds 200
        }
    }

    throw "state server did not become ready within $TimeoutSeconds seconds: $($lastError.Exception.Message)"
}

function Wait-ForHealthState {
    param(
        [string] $Expected,
        [int] $TimeoutSeconds = 4
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $lastState = ""
    while ((Get-Date) -lt $deadline) {
        $health = Get-Health
        $lastState = [string] $health.state
        if ($lastState -eq $Expected) {
            return $health
        }
        Start-Sleep -Milliseconds 100
    }

    throw "state did not become '$Expected' within $TimeoutSeconds seconds (last '$lastState')."
}

function Wait-ForHealthNotState {
    param(
        [string] $Unexpected,
        [int] $TimeoutSeconds = 5
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $lastState = ""
    while ((Get-Date) -lt $deadline) {
        $health = Get-Health
        $lastState = [string] $health.state
        if ($lastState -ne $Unexpected) {
            return $health
        }
        Start-Sleep -Milliseconds 100
    }

    throw "state stayed '$Unexpected' for $TimeoutSeconds seconds."
}

function Invoke-StatePost {
    param(
        [string] $State,
        [object] $Message = $null,
        [object] $TtlMs = $null
    )

    $body = [ordered] @{
        source = "windows-smoke"
        state = $State
    }
    if ($null -ne $Message) {
        $body.message = [string] $Message
    }
    if ($null -ne $TtlMs) {
        $body.ttlMs = [int] $TtlMs
    }

    $json = $body | ConvertTo-Json -Compress
    Invoke-RestMethod -Uri ("http://127.0.0.1:{0}/state" -f $script:Port) `
        -Method Post `
        -ContentType "application/json" `
        -Body $json `
        -TimeoutSec 2 `
        -ErrorAction Stop | Out-Null
}

function Get-TestStatus {
    Invoke-RestMethod -Uri ("http://127.0.0.1:{0}/test/status" -f $script:TestHookPort) `
        -Method Get `
        -Headers @{ "x-petsona-token" = $script:TestHookToken } `
        -TimeoutSec 2 `
        -ErrorAction Stop
}

function Wait-ForTestStatus {
    param(
        [scriptblock] $Predicate,
        [string] $Description,
        [int] $TimeoutSeconds = 5
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $lastStatus = $null
    while ((Get-Date) -lt $deadline) {
        try {
            $lastStatus = Get-TestStatus
            if (& $Predicate $lastStatus) {
                return $lastStatus
            }
        }
        catch {
        }
        Start-Sleep -Milliseconds 100
    }

    $detail = if ($null -ne $lastStatus) { $lastStatus | ConvertTo-Json -Compress } else { "no response" }
    throw "test status did not satisfy '$Description' within $TimeoutSeconds seconds ($detail)."
}

function Invoke-TestAction {
    param(
        [string] $Action,
        [object] $Enabled = $null,
        [object] $Value = $null,
        [string] $Text = $null,
        [object] $TtlMs = $null
    )

    $body = [ordered] @{ action = $Action }
    if ($null -ne $Enabled) {
        $body.enabled = [bool] $Enabled
    }
    if ($null -ne $Value) {
        $body.value = [double] $Value
    }
    if ($null -ne $Text) {
        $body.text = $Text
    }
    if ($null -ne $TtlMs) {
        $body.ttlMs = [int] $TtlMs
    }

    $json = $body | ConvertTo-Json -Compress
    Invoke-RestMethod -Uri ("http://127.0.0.1:{0}/test/action" -f $script:TestHookPort) `
        -Method Post `
        -Headers @{ "x-petsona-token" = $script:TestHookToken } `
        -ContentType "application/json" `
        -Body $json `
        -TimeoutSec 2 `
        -ErrorAction Stop | Out-Null
}

function Wait-WindowGone {
    param(
        [int] $ProcessId,
        [string] $Title,
        [int] $TimeoutSeconds = 5
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        $windows = [PetsonaSmoke.Native]::GetTopLevelWindows($ProcessId)
        $match = @($windows | Where-Object { $_.Visible -and $_.Title -eq $Title })
        if ($match.Count -eq 0) {
            return
        }
        Start-Sleep -Milliseconds 100
    }

    throw "window '$Title' did not disappear within $TimeoutSeconds seconds."
}
function Get-PetsonaWindow {
    param(
        [int] $ProcessId,
        [string] $Title,
        [int] $TimeoutSeconds = 10
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $seenTitles = New-Object System.Collections.Generic.List[string]
    while ((Get-Date) -lt $deadline) {
        $windows = [PetsonaSmoke.Native]::GetTopLevelWindows($ProcessId)
        foreach ($window in $windows) {
            if (-not [string]::IsNullOrEmpty($window.Title)) {
                $seenTitles.Add($window.Title)
            }
            if ($window.Visible -and $window.Title -eq $Title) {
                return $window
            }
        }
        Start-Sleep -Milliseconds 100
    }

    throw "window '$Title' was not visible within $TimeoutSeconds seconds (seen: $([string]::Join(', ', $seenTitles.ToArray())))."
}

function Wait-ProcessExit {
    param(
        $Entry,
        [int] $TimeoutMilliseconds = 8000
    )

    [void] $Entry.Process.WaitForExit($TimeoutMilliseconds)
    $Entry.Process.Refresh()
    if (-not $Entry.Process.HasExited) {
        throw "process $($Entry.Process.Id) did not exit within $TimeoutMilliseconds ms."
    }
    return $Entry.Process.ExitCode
}

function Remove-SmokeHome {
    param([string] $Path)

    if ([string]::IsNullOrWhiteSpace($Path) -or -not (Test-Path -LiteralPath $Path)) {
        return
    }

    $tempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    $fullPath = [System.IO.Path]::GetFullPath($Path)
    $leaf = Split-Path -Leaf $fullPath
    if (-not $fullPath.StartsWith($tempRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "refusing to remove a smoke directory outside the temp root: $fullPath"
    }
    if (-not $leaf.StartsWith("petsona-windows-smoke-", [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "refusing to remove an unexpected smoke directory: $fullPath"
    }

    Remove-Item -LiteralPath $fullPath -Recurse -Force
}

# The native helpers (SendInput, window enumeration, foreground tracking) are
# embedded so this smoke test stays a single file. They are only compiled once
# per PowerShell session.
$nativeHelpers = @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

namespace PetsonaSmoke
{
    public sealed class WindowInfo
    {
        public IntPtr Hwnd;
        public string Title;
        public string ClassName;
        public int ProcessId;
        public bool Visible;
        public uint Style;
        public uint ExStyle;
        public int Left;
        public int Top;
        public int Width;
        public int Height;
    }

    public static class Native
    {
        public const uint WS_POPUP = 0x80000000u;
        public const uint WS_CAPTION = 0x00C00000u;
        public const uint WS_BORDER = 0x00800000u;
        public const uint WS_DLGFRAME = 0x00400000u;
        public const uint WS_FRAME = WS_CAPTION | WS_BORDER | WS_DLGFRAME;
        public const uint WS_EX_NOACTIVATE = 0x08000000u;
        public const uint WS_EX_TOPMOST = 0x00000008u;
        public const uint WS_EX_TOOLWINDOW = 0x00000080u;
        public const uint WS_EX_APPWINDOW = 0x00040000u;

        private const int GWL_STYLE = -16;
        private const int GWL_EXSTYLE = -20;

        [StructLayout(LayoutKind.Sequential)]
        private struct RECT
        {
            public int Left;
            public int Top;
            public int Right;
            public int Bottom;
        }

        private struct POINT
        {
            public int X;
            public int Y;
        }

        private delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

        [DllImport("user32.dll")]
        private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern int GetWindowTextLengthW(IntPtr hWnd);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern int GetWindowTextW(IntPtr hWnd, StringBuilder text, int maxCount);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern int GetClassNameW(IntPtr hWnd, StringBuilder className, int maxCount);

        [DllImport("user32.dll")]
        private static extern bool IsWindowVisible(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);

        [DllImport("user32.dll")]
        private static extern bool AttachThreadInput(uint attachThread, uint attachToThread, bool attach);

        [DllImport("user32.dll")]
        private static extern bool BringWindowToTop(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern IntPtr SetActiveWindow(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern IntPtr SetFocus(IntPtr hWnd);

        [DllImport("kernel32.dll")]
        private static extern uint GetCurrentThreadId();

        [DllImport("user32.dll")]
        private static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);

        [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")]
        private static extern IntPtr GetWindowLongPtr64(IntPtr hWnd, int index);

        [DllImport("user32.dll", EntryPoint = "GetWindowLongW")]
        private static extern int GetWindowLong32(IntPtr hWnd, int index);

        private static IntPtr GetWindowLongPtr(IntPtr hWnd, int index)
        {
            if (IntPtr.Size == 8)
            {
                return GetWindowLongPtr64(hWnd, index);
            }
            return new IntPtr(GetWindowLong32(hWnd, index));
        }

        public static bool HasAll(uint value, uint bits)
        {
            return (value & bits) == bits;
        }

        public static bool HasAny(uint value, uint bits)
        {
            return (value & bits) != 0;
        }

        [DllImport("user32.dll")]
        private static extern IntPtr GetForegroundWindow();

        [DllImport("user32.dll")]
        private static extern bool SetForegroundWindow(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern void keybd_event(
            byte virtualKey,
            byte scanCode,
            uint flags,
            UIntPtr extraInfo);

        [DllImport("user32.dll")]
        private static extern bool ShowWindow(IntPtr hWnd, int command);

        [DllImport("user32.dll")]
        private static extern bool SetCursorPos(int x, int y);

        [DllImport("user32.dll")]
        private static extern bool GetCursorPos(out POINT point);

        [DllImport("user32.dll")]
        private static extern int GetSystemMetrics(int index);

        [DllImport("user32.dll")]
        private static extern void mouse_event(
            uint flags,
            uint dx,
            uint dy,
            uint data,
            UIntPtr extraInfo);

        [DllImport("user32.dll", SetLastError = true)]
        private static extern uint SendInput(
            uint inputCount,
            INPUT[] inputs,
            int inputSize);

        [DllImport("user32.dll")]
        private static extern short GetAsyncKeyState(int virtualKey);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern IntPtr SendMessageW(
            IntPtr hWnd,
            uint message,
            UIntPtr wParam,
            IntPtr lParam);

        [StructLayout(LayoutKind.Sequential)]
        private struct INPUT
        {
            public uint Type;
            public INPUTUNION Data;
        }

        [StructLayout(LayoutKind.Explicit)]
        private struct INPUTUNION
        {
            [FieldOffset(0)]
            public MOUSEINPUT Mouse;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct MOUSEINPUT
        {
            public int Dx;
            public int Dy;
            public uint MouseData;
            public uint Flags;
            public uint Time;
            public UIntPtr ExtraInfo;
        }

        public static IntPtr ForegroundWindow()
        {
            return GetForegroundWindow();
        }

        public static bool ActivateWindow(IntPtr hWnd)
        {
            uint ignored;
            uint currentThread = GetCurrentThreadId();
            uint foregroundThread = GetWindowThreadProcessId(GetForegroundWindow(), out ignored);
            uint targetThread = GetWindowThreadProcessId(hWnd, out ignored);
            bool attachedForeground = foregroundThread != currentThread &&
                AttachThreadInput(currentThread, foregroundThread, true);
            bool attachedTarget = targetThread != currentThread &&
                AttachThreadInput(currentThread, targetThread, true);

            ShowWindow(hWnd, 9);
            BringWindowToTop(hWnd);
            keybd_event(0x12, 0, 0, UIntPtr.Zero);
            bool activated = SetForegroundWindow(hWnd);
            SetActiveWindow(hWnd);
            SetFocus(hWnd);
            keybd_event(0x12, 0, 0x0002u, UIntPtr.Zero);

            if (attachedTarget)
            {
                AttachThreadInput(currentThread, targetThread, false);
            }
            if (attachedForeground)
            {
                AttachThreadInput(currentThread, foregroundThread, false);
            }
            return activated;
        }

        public static void SetCursorPosition(int x, int y)
        {
            SetCursorPos(x, y);
        }

        public static int[] CursorPosition()
        {
            POINT point;
            GetCursorPos(out point);
            return new int[] { point.X, point.Y };
        }

        // SetCursorPos bypasses WH_MOUSE_LL; send real input so the app updates
        // its cached pointer position the same way as a physical mouse.
        public static bool MoveCursorWithInput(int x, int y)
        {
            int virtualX = GetSystemMetrics(76);
            int virtualY = GetSystemMetrics(77);
            int virtualWidth = Math.Max(1, GetSystemMetrics(78) - 1);
            int virtualHeight = Math.Max(1, GetSystemMetrics(79) - 1);
            int normalizedX = (int)Math.Round((x - virtualX) * 65535.0 / virtualWidth);
            int normalizedY = (int)Math.Round((y - virtualY) * 65535.0 / virtualHeight);

            INPUT input = new INPUT();
            input.Type = 0;
            input.Data.Mouse.Flags = 0x0001u | 0x8000u | 0x4000u;
            input.Data.Mouse.Dx = normalizedX;
            input.Data.Mouse.Dy = normalizedY;
            INPUT[] inputs = new INPUT[] { input };
            return SendInput(1, inputs, Marshal.SizeOf(typeof(INPUT))) == 1;
        }

        private static void SendMouseInput(uint flags)
        {
            INPUT input = new INPUT();
            input.Type = 0;
            input.Data.Mouse.Flags = flags;
            INPUT[] inputs = new INPUT[] { input };
            SendInput(1, inputs, Marshal.SizeOf(typeof(INPUT)));
        }

        public static void LeftMouseDown()
        {
            SendMouseInput(0x0002u);
        }

        public static void LeftMouseUp()
        {
            SendMouseInput(0x0004u);
        }

        public static bool LeftMouseIsDown()
        {
            return (GetAsyncKeyState(0x01) & 0x8000) != 0;
        }

        public static int MouseActivateResult(IntPtr hWnd)
        {
            return (int)SendMessageW(hWnd, 0x0021u, UIntPtr.Zero, IntPtr.Zero);
        }

        public static List<WindowInfo> GetTopLevelWindows(int processId)
        {
            return GetWindows(processId);
        }

        public static List<WindowInfo> GetAllTopLevelWindows()
        {
            return GetWindows(-1);
        }

        private static List<WindowInfo> GetWindows(int processId)
        {
            List<WindowInfo> windows = new List<WindowInfo>();
            EnumWindows(delegate(IntPtr hWnd, IntPtr lParam)
            {
                uint owner;
                GetWindowThreadProcessId(hWnd, out owner);
                if (processId >= 0 && (int)owner != processId)
                {
                    return true;
                }

                int length = GetWindowTextLengthW(hWnd);
                StringBuilder title = new StringBuilder(Math.Max(length + 1, 256));
                GetWindowTextW(hWnd, title, title.Capacity);
                StringBuilder className = new StringBuilder(256);
                GetClassNameW(hWnd, className, className.Capacity);

                RECT rect;
                GetWindowRect(hWnd, out rect);
                windows.Add(new WindowInfo
                {
                    Hwnd = hWnd,
                    Title = title.ToString(),
                    ClassName = className.ToString(),
                    ProcessId = (int)owner,
                    Visible = IsWindowVisible(hWnd),
                    Style = unchecked((uint)GetWindowLongPtr(hWnd, GWL_STYLE).ToInt64()),
                    ExStyle = unchecked((uint)GetWindowLongPtr(hWnd, GWL_EXSTYLE).ToInt64()),
                    Left = rect.Left,
                    Top = rect.Top,
                    Width = rect.Right - rect.Left,
                    Height = rect.Bottom - rect.Top
                });
                return true;
            }, IntPtr.Zero);
            return windows;
        }
    }
}
'@

if (-not ("PetsonaSmoke.Native" -as [type])) {
    Add-Type -TypeDefinition $nativeHelpers -ErrorAction Stop
}

if ([string]::IsNullOrWhiteSpace($Executable)) {
    $executable = Join-Path $root ("target\{0}\petsona-windows.exe" -f $Configuration.ToLowerInvariant())
}
else {
    $executable = $Executable
}
if (-not $SkipBuild) {
    Write-Section "Build ($Configuration)"
    Push-Location $root
    try {
        if ($Configuration -eq "Release") {
            cargo build -p petsona-shell-windows --release --locked
        }
        else {
            cargo build -p petsona-shell-windows --locked
        }
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build failed with exit code $LASTEXITCODE."
        }
    }
    finally {
        Pop-Location
    }
}

if (-not (Test-Path -LiteralPath $executable)) {
    throw "missing Petsona executable: $executable"
}

$tempRoot = [System.IO.Path]::GetTempPath()
$stamp = Get-Date -Format "yyyyMMdd-HHmmssfff"
$script:SmokeHome = Join-Path $tempRoot ("petsona-windows-smoke-{0}-{1}" -f $PID, $stamp)
[void] (New-Item -ItemType Directory -Path $script:SmokeHome -Force)
$script:Port = Find-FreeTcpPort
if ($UseTestHooks) {
    $script:TestHookPort = Find-FreeTcpPort
    $script:TestHookToken = [Guid]::NewGuid().ToString("N")
}
Write-SmokeConfig -DataHome $script:SmokeHome -Port $script:Port

Write-Host "Repository: $root" -ForegroundColor DarkGray
Write-Host "Executable: $executable" -ForegroundColor DarkGray
Write-Host "Smoke home: $script:SmokeHome" -ForegroundColor DarkGray
Write-Host "State port: $script:Port" -ForegroundColor DarkGray
if ($UseTestHooks) {
    Write-Host "Test hook port: $script:TestHookPort" -ForegroundColor DarkGray
}

$first = $null
$restart = $null

try {
    Write-Section "Start Petsona"
    $first = Start-PetsonaProcess -DataHome $script:SmokeHome -Tag "first" -Executable $executable
    $health = Wait-ForHealth -TimeoutSeconds 25 -Entry $first

    if ($UseTestHooks) {
        Invoke-SmokeCheck -Id "T1" -Name "test hook channel" {
            $status = Wait-ForTestStatus -Predicate { param($candidate) $candidate.ok -and $candidate.hooksPort -eq $script:TestHookPort } -Description "hooks port $script:TestHookPort" -TimeoutSeconds 8
            Assert-True ($status.processId -eq $first.Process.Id) "hook status reports the wrong process id."
        }

        Invoke-SmokeCheck -Id "T2" -Name "Win32 native menu thread" {
            [void] (Wait-ForTestStatus -Predicate { param($candidate) $candidate.nativeMenuReady } -Description "native menu thread" -TimeoutSeconds 8)
        }

        # The popup menu is carried by the system menu class (#32768) on the
        # shell's menu thread. This covers the two regressions reported by hand:
        # a second click must replace the open menu instead of stacking another
        # one, and a dismiss request must actually close it.
        Invoke-SmokeCheck -Id "T3" -Name "Win32 native menu replaces and closes" {
            function Get-NativeMenuWindows {
                @([PetsonaSmoke.Native]::GetTopLevelWindows($first.Process.Id) |
                    Where-Object { $_.Visible -and $_.ClassName -eq "#32768" })
            }

            Invoke-TestAction -Action "native-menu"
            $opened = $false
            $deadline = (Get-Date).AddSeconds(5)
            while ((Get-Date) -lt $deadline) {
                if ((Get-NativeMenuWindows).Count -ge 1) {
                    $opened = $true
                    break
                }
                Start-Sleep -Milliseconds 100
            }
            Assert-True $opened "the shell's native menu never appeared."

            Invoke-TestAction -Action "native-menu"
            Start-Sleep -Milliseconds 800
            $stacked = (Get-NativeMenuWindows).Count
            Assert-True ($stacked -eq 1) "expected exactly one native menu window, found $stacked."

            Invoke-TestAction -Action "close-native-menu"
            $closed = $false
            $deadline = (Get-Date).AddSeconds(5)
            while ((Get-Date) -lt $deadline) {
                if ((Get-NativeMenuWindows).Count -eq 0) {
                    $closed = $true
                    break
                }
                Start-Sleep -Milliseconds 100
            }
            Assert-True $closed "the native menu stayed open after a dismiss request."
        }
    }

    Invoke-SmokeCheck -Id "A1" -Name "window exists and is visible" {
        $window = Get-PetsonaWindow -ProcessId $first.Process.Id -Title "Petsona" -TimeoutSeconds 10
        Assert-True ($window.Width -gt 0 -and $window.Height -gt 0) "window has no drawable size."
        Assert-True $window.Visible "window is not visible."
    }

    Invoke-SmokeCheck -Id "A8" -Name "state protocol POST and TTL" {
        $loopStates = @("running", "waiting", "failed", "review", "running-left", "running-right")
        foreach ($state in $loopStates) {
            Invoke-StatePost -State $state -Message ("smoke:{0}" -f $state) -TtlMs 1200
            [void] (Wait-ForHealthState -Expected $state -TimeoutSeconds 3)
        }

        foreach ($state in @("waving", "jumping")) {
            Invoke-StatePost -State $state -Message ("smoke:{0}" -f $state) -TtlMs 2000
            Start-Sleep -Milliseconds 200
            $current = [string] (Get-Health).state
            Assert-True ($current -eq $state -or $current -eq "idle") "one-shot state '$state' was neither observed nor returned to idle (got '$current')."
        }

        Invoke-StatePost -State "waiting" -Message "ttl-check" -TtlMs 900
        [void] (Wait-ForHealthState -Expected "waiting" -TimeoutSeconds 2)
        [void] (Wait-ForHealthNotState -Unexpected "waiting" -TimeoutSeconds 5)

        Invoke-StatePost -State "failed" -TtlMs 0
        Start-Sleep -Milliseconds 1400
        $persistent = [string] (Get-Health).state
        Assert-True ($persistent -eq "failed") "ttlMs=0 state expired or changed (got '$persistent')."
        Invoke-StatePost -State "idle" -TtlMs 0
    }

    Invoke-SmokeCheck -Id "A9" -Name "health and pets endpoints" {
        $snapshot = Get-Health
        Assert-True ($snapshot.ok -eq $true) "health.ok is not true."
        Assert-True (-not [string]::IsNullOrWhiteSpace([string] $snapshot.version)) "health.version is empty."
        Assert-True (-not [string]::IsNullOrWhiteSpace([string] $snapshot.pet)) "health.pet is empty."
        Assert-True (-not [string]::IsNullOrWhiteSpace([string] $snapshot.persona)) "health.persona is empty."
        Assert-True (-not [string]::IsNullOrWhiteSpace([string] $snapshot.state)) "health.state is empty."
        Assert-True (@($snapshot.pets).Count -ge 1) "health.pets is empty."

        $petsResponse = Invoke-WebRequest -Uri ("http://127.0.0.1:{0}/pets" -f $script:Port) -Method Get -UseBasicParsing -TimeoutSec 2
        Assert-True ($petsResponse.StatusCode -eq 200) "/pets returned HTTP $($petsResponse.StatusCode)."
        $petsJson = [string] $petsResponse.Content
        $healthPets = @($snapshot.pets)
        Assert-True ($healthPets.Count -ge 1) "health.pets is empty."
        foreach ($pet in $healthPets) {
            $needle = '"' + [string] $pet + '"'
            Assert-True ($petsJson.Contains($needle)) "/pets JSON does not contain health.pets entry '$pet'."
        }
    }

    Invoke-SmokeCheck -Id "C6" -Name "Win32 pet window styles" {
        $window = Get-PetsonaWindow -ProcessId $first.Process.Id -Title "Petsona" -TimeoutSeconds 10
        $style = $window.Style
        $exStyle = $window.ExStyle
        Assert-True ([PetsonaSmoke.Native]::HasAll($style, [PetsonaSmoke.Native]::WS_POPUP)) "WS_POPUP is missing."
        Assert-True (-not [PetsonaSmoke.Native]::HasAny($style, [PetsonaSmoke.Native]::WS_FRAME)) "WS_CAPTION/WS_BORDER/WS_DLGFRAME is still present."
        Assert-True ([PetsonaSmoke.Native]::HasAll($exStyle, [PetsonaSmoke.Native]::WS_EX_NOACTIVATE)) "WS_EX_NOACTIVATE is missing."
        Assert-True ([PetsonaSmoke.Native]::HasAll($exStyle, [PetsonaSmoke.Native]::WS_EX_TOPMOST)) "WS_EX_TOPMOST is missing."
        Write-Host ("       style=0x{0:X8} exstyle=0x{1:X8}" -f $style, $exStyle) -ForegroundColor DarkGray

        if ($UseTestHooks) {
            Invoke-TestAction -Action "open-menu"
            $menu = Get-PetsonaWindow -ProcessId $first.Process.Id -Title "Petsona 菜单" -TimeoutSeconds 5
            Assert-True ([PetsonaSmoke.Native]::HasAll($menu.Style, [PetsonaSmoke.Native]::WS_POPUP)) "menu WS_POPUP is missing."
            Assert-True (-not [PetsonaSmoke.Native]::HasAny($menu.Style, [PetsonaSmoke.Native]::WS_FRAME)) "menu frame styles are still present."
            Assert-True ([PetsonaSmoke.Native]::HasAll($menu.ExStyle, [PetsonaSmoke.Native]::WS_EX_NOACTIVATE)) "menu WS_EX_NOACTIVATE is missing."
            Assert-True ([PetsonaSmoke.Native]::MouseActivateResult($menu.Hwnd) -eq 3) "menu WM_MOUSEACTIVATE does not return MA_NOACTIVATE."
            Invoke-TestAction -Action "close-menu"
            Wait-WindowGone -ProcessId $first.Process.Id -Title "Petsona 菜单" -TimeoutSeconds 5
        }
    }

    if ($UseTestHooks) {
        Invoke-SmokeCheck -Id "A4" -Name "internal settings action (partial)" {
            Invoke-TestAction -Action "open-settings"
            [void] (Get-PetsonaWindow -ProcessId $first.Process.Id -Title "Petsona 设置" -TimeoutSeconds 5)
            Invoke-TestAction -Action "close-settings"
            Wait-WindowGone -ProcessId $first.Process.Id -Title "Petsona 设置" -TimeoutSeconds 5
        }

        Invoke-SmokeCheck -Id "A11" -Name "internal hide/show action (partial)" {
            Invoke-TestAction -Action "hide-pet"
            [void] (Wait-ForTestStatus -Predicate { param($candidate) -not $candidate.petVisible } -Description "pet hidden" -TimeoutSeconds 3)
            Invoke-TestAction -Action "show-pet"
            [void] (Wait-ForTestStatus -Predicate { param($candidate) $candidate.petVisible } -Description "pet visible" -TimeoutSeconds 3)
        }

        Invoke-SmokeCheck -Id "B9" -Name "bubble keeps the pet window geometry (partial)" {
            $before = Get-PetsonaWindow -ProcessId $first.Process.Id -Title "Petsona" -TimeoutSeconds 5
            Invoke-TestAction -Action "show-bubble" -Text "windows smoke bubble" -TtlMs 3000
            [void] (Wait-ForTestStatus -Predicate { param($candidate) $candidate.bubbleText -eq "windows smoke bubble" } -Description "bubble text" -TimeoutSeconds 3)
            [void] (Wait-ForTestStatus -Predicate { param($candidate) $candidate.bubbleWindowCreated } -Description "bubble overlay window" -TimeoutSeconds 3)
            $during = Get-PetsonaWindow -ProcessId $first.Process.Id -Title "Petsona" -TimeoutSeconds 5
            Assert-True ($before.Left -eq $during.Left -and $before.Top -eq $during.Top -and $before.Width -eq $during.Width -and $before.Height -eq $during.Height) "bubble changed the pet window geometry."
            Invoke-TestAction -Action "clear-bubble"
            [void] (Wait-ForTestStatus -Predicate { param($candidate) $null -eq $candidate.bubbleText } -Description "bubble cleared" -TimeoutSeconds 3)
            [void] (Wait-ForTestStatus -Predicate { param($candidate) -not $candidate.bubbleWindowCreated } -Description "bubble overlay hidden" -TimeoutSeconds 3)
        }

        Invoke-SmokeCheck -Id "B11" -Name "idle uses event-driven mouse wakeups" {
            Start-Sleep -Milliseconds 1000
            $before = Get-TestStatus
            Assert-True ($before.mouseEvents) "Windows mouse wakeup hook is not active."
            $process = Get-Process -Id $first.Process.Id -ErrorAction Stop
            $cpuBefore = $process.CPU
            Start-Sleep -Seconds 4
            $after = Get-TestStatus
            $process.Refresh()
            $cpuAfter = $process.CPU
            $polls = [int64] $after.cursorPollCount - [int64] $before.cursorPollCount
            $mouseEventDelta = [int64] $after.mouseEventCount - [int64] $before.mouseEventCount
            $uiDelta = [int64] $after.uiCount - [int64] $before.uiCount
            $logicDelta = [int64] $after.logicCount - [int64] $before.logicCount
            $oneCorePercent = (($cpuAfter - $cpuBefore) / 4.0) * 100.0
            $fast = [int64] $after.repaintFast - [int64] $before.repaintFast
            $medium = [int64] $after.repaintMedium - [int64] $before.repaintMedium
            $slow = [int64] $after.repaintSlow - [int64] $before.repaintSlow
            Write-Host ("       state={0}; UI/logic in 4s: {1}/{2}; animation/final delay: {3}/{4}ms; cursor polls: {5}; mouse events: {6}; CPU: {7:N2}% of one core" -f $after.state, $uiDelta, $logicDelta, $after.animationRepaintMs, $after.lastRepaintMs, $polls, $mouseEventDelta, $oneCorePercent) -ForegroundColor DarkGray
            $uniqueCauses = @($after.repaintCauses | Where-Object { $_ } | Sort-Object -Unique)
            if ($uiDelta -gt 50 -or $oneCorePercent -ge 3.0) {
                Write-Host ("       repaint causes: {0}" -f ($uniqueCauses -join '; ')) -ForegroundColor DarkGray
            }
            if (-not $before.mousePositionValid) {
                Write-Host "       [SKIP] interactive desktop is unavailable; CPU gate needs a real session." -ForegroundColor Yellow
                return
            }
            if ($mouseEventDelta -gt 0) {
                Write-Host "       [SKIP] mouse input was observed during the idle sample; rerun without moving the mouse." -ForegroundColor Yellow
                return
            }
            Assert-True ($polls -le 20) "idle global cursor polling is still too frequent."
            Assert-True ($uiDelta -le 50) "idle UI repaints are still too frequent ($uiDelta in 4s)."
            Assert-True ($oneCorePercent -lt 3.0) "idle CPU is still above 3% of one core."
        }
        Invoke-SmokeCheck -Id "B8" -Name "Win32 chrome stays undecorated (visual flash manual)" {
            Start-Sleep -Milliseconds 350
            $before = Get-TestStatus
            $styleCountBefore = $before.styleReapplyCount
            $beforeWindow = Get-PetsonaWindow -ProcessId $first.Process.Id -Title "Petsona" -TimeoutSeconds 5

            Invoke-TestAction -Action "set-scale" -Value 1.25
            [void] (Wait-ForTestStatus -Predicate {
                param($candidate)
                [Math]::Abs([double] $candidate.scale - 1.25) -lt 0.001
            } -Description "scale 1.25" -TimeoutSeconds 3)
            Start-Sleep -Milliseconds 400

            $resized = Get-TestStatus
            $afterWindow = Get-PetsonaWindow -ProcessId $first.Process.Id -Title "Petsona" -TimeoutSeconds 5
            Assert-True ($resized.styleReapplyCount -eq $styleCountBefore) "resizing caused a Win32 frame-style rebuild."
            $expectedLeft = [Math]::Round($beforeWindow.Left - (($afterWindow.Width - $beforeWindow.Width) / 2.0))
            $expectedTop = [Math]::Round($beforeWindow.Top - ($afterWindow.Height - $beforeWindow.Height))
            Assert-True ([Math]::Abs($afterWindow.Left - $expectedLeft) -le 1) "scaling moved the bottom-center anchor horizontally (before=$($beforeWindow.Left), after=$($afterWindow.Left), expected=$expectedLeft, width $($beforeWindow.Width)->$($afterWindow.Width))."
            Assert-True ([Math]::Abs($afterWindow.Top - $expectedTop) -le 1) "scaling moved the bottom-center anchor vertically (before=$($beforeWindow.Top), after=$($afterWindow.Top), expected=$expectedTop, height $($beforeWindow.Height)->$($afterWindow.Height))."
            Assert-True ($resized.petClickHits) "opaque hit point missed after scaling."

            Invoke-TestAction -Action "open-menu"
            [void] (Get-PetsonaWindow -ProcessId $first.Process.Id -Title "Petsona 菜单" -TimeoutSeconds 5)
            Invoke-TestAction -Action "close-menu"
            Wait-WindowGone -ProcessId $first.Process.Id -Title "Petsona 菜单" -TimeoutSeconds 5

            Invoke-TestAction -Action "open-settings"
            [void] (Get-PetsonaWindow -ProcessId $first.Process.Id -Title "Petsona 设置" -TimeoutSeconds 5)
            Invoke-TestAction -Action "close-settings"
            Wait-WindowGone -ProcessId $first.Process.Id -Title "Petsona 设置" -TimeoutSeconds 5

            Invoke-TestAction -Action "set-click-through" -Enabled $false
            [void] (Wait-ForTestStatus -Predicate { param($candidate) -not $candidate.clickThrough } -Description "click-through off" -TimeoutSeconds 3)
            Invoke-TestAction -Action "set-click-through" -Enabled $true
            [void] (Wait-ForTestStatus -Predicate { param($candidate) $candidate.clickThrough } -Description "click-through on" -TimeoutSeconds 3)
            Start-Sleep -Milliseconds 300

            $after = Get-TestStatus
            Assert-True ($after.styleReapplyCount -eq $styleCountBefore) "menu/settings/click-through caused a Win32 frame-style rebuild."
            $window = Get-PetsonaWindow -ProcessId $first.Process.Id -Title "Petsona" -TimeoutSeconds 5
            Assert-True ([PetsonaSmoke.Native]::HasAll($window.Style, [PetsonaSmoke.Native]::WS_POPUP)) "WS_POPUP was lost after the interaction sequence."
            Assert-True (-not [PetsonaSmoke.Native]::HasAny($window.Style, [PetsonaSmoke.Native]::WS_FRAME)) "decorated frame styles returned after the interaction sequence."

            Invoke-TestAction -Action "set-scale" -Value 1.0
            [void] (Wait-ForTestStatus -Predicate {
                param($candidate)
                [Math]::Abs([double] $candidate.scale - 1.0) -lt 0.001
            } -Description "scale 1.0" -TimeoutSeconds 3)
            Start-Sleep -Milliseconds 350
            $resetWindow = Get-PetsonaWindow -ProcessId $first.Process.Id -Title "Petsona" -TimeoutSeconds 5
            Assert-True ([Math]::Abs($resetWindow.Left - $beforeWindow.Left) -le 1) "restoring scale moved the pet horizontally."
            Assert-True ([Math]::Abs($resetWindow.Top - $beforeWindow.Top) -le 1) "restoring scale moved the pet vertically."
        }
        Invoke-SmokeCheck -Id "B7" -Name "clicking pet does not steal foreground" {
            $script:NotepadFile = [System.IO.Path]::GetTempFileName()
            $leaf = Split-Path -Leaf $script:NotepadFile
            $existingNotepadPids = @(Get-Process -Name notepad -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
            $script:NotepadProcess = Start-Process -FilePath "notepad.exe" `
                -ArgumentList ('"' + $script:NotepadFile + '"') `
                -PassThru

            $deadline = (Get-Date).AddSeconds(10)
            $targetWindow = $null
            while ((Get-Date) -lt $deadline -and $null -eq $targetWindow) {
                $windows = [PetsonaSmoke.Native]::GetAllTopLevelWindows()
                $matches = @($windows | Where-Object { $_.Visible -and $_.Title.Contains($leaf) })
                if ($matches.Count -gt 0) {
                    $targetWindow = $matches[0]
                    break
                }
                Start-Sleep -Milliseconds 100
            }
            Assert-True ($null -ne $targetWindow) "Notepad window for '$leaf' did not appear."

            if ($existingNotepadPids -notcontains $targetWindow.ProcessId) {
                $script:NotepadWindowPid = $targetWindow.ProcessId
            }
            [void] [PetsonaSmoke.Native]::ActivateWindow($targetWindow.Hwnd)
            Start-Sleep -Milliseconds 350

            $petWindow = Get-PetsonaWindow -ProcessId $first.Process.Id -Title "Petsona" -TimeoutSeconds 5
            Assert-True ([PetsonaSmoke.Native]::MouseActivateResult($petWindow.Hwnd) -eq 3) "pet WM_MOUSEACTIVATE does not return MA_NOACTIVATE."
            $foregroundBefore = [PetsonaSmoke.Native]::ForegroundWindow()
            Assert-True ($foregroundBefore -ne $petWindow.Hwnd) "pet was foreground before the click test."
            $clickStatus = Wait-ForTestStatus -Predicate {
                param($candidate)
                $null -ne $candidate.petClickX -and $null -ne $candidate.petClickY
            } -Description "opaque pet click point" -TimeoutSeconds 3
            $clickX = [int] $clickStatus.petClickX
            $clickY = [int] $clickStatus.petClickY
            $savedCursor = [PetsonaSmoke.Native]::CursorPosition()
            try {
                if (-not [PetsonaSmoke.Native]::MoveCursorWithInput($clickX, $clickY)) {
                    Write-Host "       [SKIP] this session does not accept SendInput; WM_MOUSEACTIVATE guard passed." -ForegroundColor Yellow
                    return
                }
                Start-Sleep -Milliseconds 400
                $foregroundAfterMove = [PetsonaSmoke.Native]::ForegroundWindow()
                Assert-True ($foregroundAfterMove -eq $foregroundBefore) "moving the cursor over the pet changed the foreground window."
                [void] (Wait-ForTestStatus -Predicate {
                    param($candidate)
                    -not $candidate.passthrough
                } -Description "pet became interactive" -TimeoutSeconds 3)
                [PetsonaSmoke.Native]::LeftMouseDown()
                Start-Sleep -Milliseconds 80
                if (-not [PetsonaSmoke.Native]::LeftMouseIsDown()) {
                    [PetsonaSmoke.Native]::LeftMouseUp()
                    Write-Host "       [SKIP] this session does not accept SendInput; WM_MOUSEACTIVATE guard passed." -ForegroundColor Yellow
                    return
                }
                [void] (Wait-ForTestStatus -Predicate {
                    param($candidate)
                    $candidate.pointerLeftDown
                } -Description "pet received left mouse down" -TimeoutSeconds 2)
                [PetsonaSmoke.Native]::LeftMouseUp()

                [void] (Wait-ForTestStatus -Predicate {
                    param($candidate)
                    $candidate.state -eq "waving" -or $null -ne $candidate.bubbleText
                } -Description "pet click reaction" -TimeoutSeconds 4)

                $foregroundAfter = [PetsonaSmoke.Native]::ForegroundWindow()
                Assert-True ($foregroundAfter -eq $foregroundBefore) "clicking the pet changed the foreground window."
            }
            finally {
                if (-not [PetsonaSmoke.Native]::MoveCursorWithInput($savedCursor[0], $savedCursor[1])) {
                    [PetsonaSmoke.Native]::SetCursorPosition($savedCursor[0], $savedCursor[1])
                }
            }
        }
        Invoke-SmokeCheck -Id "A13" -Name "accelerated auto-walk (partial)" {
            $beforeWindow = Get-PetsonaWindow -ProcessId $first.Process.Id -Title "Petsona" -TimeoutSeconds 5
            $startX = $beforeWindow.Left
            Invoke-TestAction -Action "trigger-auto-walk"
            [void] (Wait-ForTestStatus -Predicate {
                param($candidate)
                $candidate.state -eq "running-left" -or $candidate.state -eq "running-right"
            } -Description "auto-walk state" -TimeoutSeconds 5)
            $deadline = (Get-Date).AddSeconds(5)
            $afterWindow = $beforeWindow
            while ((Get-Date) -lt $deadline -and $afterWindow.Left -eq $startX) {
                Start-Sleep -Milliseconds 100
                $afterWindow = Get-PetsonaWindow -ProcessId $first.Process.Id -Title "Petsona" -TimeoutSeconds 2
            }
            Assert-True ($startX -ne $afterWindow.Left) "auto-walk did not move the pet window."
            Invoke-TestAction -Action "stop-auto-walk"
            [void] (Wait-ForTestStatus -Predicate { param($candidate) $candidate.state -eq "idle" } -Description "auto-walk stopped" -TimeoutSeconds 3)
        }
    }
    Invoke-SmokeCheck -Id "B12" -Name "single instance lock" {
        $second = Start-PetsonaProcess -DataHome $script:SmokeHome -Tag "second" -Executable $executable
        $exitCode = Wait-ProcessExit -Entry $second -TimeoutMilliseconds 8000
        if (($null -ne $exitCode) -and ($exitCode -ne 0)) {
            throw "second instance exited with code $exitCode."
        }

        $first.Process.Refresh()
        Assert-True (-not $first.Process.HasExited) "first instance exited after the second instance was rejected."
        $windows = [PetsonaSmoke.Native]::GetTopLevelWindows($first.Process.Id)
        $petWindows = @($windows | Where-Object { $_.Title -eq "Petsona" -and $_.Visible })
        Assert-True ($petWindows.Count -eq 1) "expected exactly one visible Petsona window, found $($petWindows.Count)."
    }

    Invoke-SmokeCheck -Id "B13" -Name "file log records startup and state events" {
        Start-Sleep -Milliseconds 300
        $logPath = Join-Path $script:SmokeHome "logs\petsona.log"
        Assert-True (Test-Path -LiteralPath $logPath) "missing log file: $logPath"
        $log = Get-Content -Raw -LiteralPath $logPath
        Assert-True ($log.Contains("file logging enabled")) "log does not contain 'file logging enabled'."
        Assert-True ($log.Contains("state protocol listening")) "log does not contain 'state protocol listening'."
        Assert-True ($log.Contains("state event")) "log does not contain state events."
        Assert-True ($log.Contains("source=windows-smoke")) "log does not contain the smoke source."
    }

    Invoke-SmokeCheck -Id "B14" -Name "quit and restart" {
        if ($UseTestHooks) {
            Invoke-TestAction -Action "quit"
            $quitCode = Wait-ProcessExit -Entry $first -TimeoutMilliseconds 8000
            if (($null -ne $quitCode) -and ($quitCode -ne 0)) {
                throw "Petsona quit with exit code $quitCode."
            }
        }
        else {
            Stop-PetsonaProcess -Entry $first
        }
        Start-Sleep -Milliseconds 400

        $restart = Start-PetsonaProcess -DataHome $script:SmokeHome -Tag "restart" -Executable $executable
        $restartHealth = Wait-ForHealth -TimeoutSeconds 20 -Entry $restart
        Assert-True ($restartHealth.ok -eq $true) "restart health.ok is not true."
        $restartWindow = Get-PetsonaWindow -ProcessId $restart.Process.Id -Title "Petsona" -TimeoutSeconds 10
        Assert-True $restartWindow.Visible "restart window is not visible."
    }
}
catch {
    $script:ExitCode = 1
    Write-Host ""
    Write-Host "Windows smoke failed: $($_.Exception.Message)" -ForegroundColor Red
}
finally {
    foreach ($entry in $script:Processes) {
        Stop-PetsonaProcess -Entry $entry
    }
    if ($null -ne $script:NotepadWindowPid) {
        Stop-Process -Id $script:NotepadWindowPid -Force -ErrorAction SilentlyContinue
    }
    if ($null -ne $script:NotepadProcess) {
        Stop-PetsonaProcess -Entry ([pscustomobject] @{ Process = $script:NotepadProcess })
    }
    if ($null -ne $script:NotepadFile -and (Test-Path -LiteralPath $script:NotepadFile)) {
        Remove-Item -LiteralPath $script:NotepadFile -Force -ErrorAction SilentlyContinue
    }

    $keep = $KeepArtifacts -or $script:ExitCode -ne 0
    if (-not $keep) {
        try {
            Remove-SmokeHome -Path $script:SmokeHome
        }
        catch {
            Write-Host "Could not remove smoke directory: $($_.Exception.Message)" -ForegroundColor Yellow
        }
    }
    else {
        Write-Host ""
        Write-Host "Smoke artifacts kept at: $script:SmokeHome" -ForegroundColor Yellow
    }
}

Write-Host ""
if ($script:ExitCode -eq 0) {
    Write-Host ("Windows smoke passed: {0} checks." -f $script:ChecksPassed) -ForegroundColor Green
}
else {
    Write-Host ("Windows smoke failed: {0} passed, {1} failed." -f $script:ChecksPassed, $script:ChecksFailed) -ForegroundColor Red
}
exit $script:ExitCode
