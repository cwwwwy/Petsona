# Windows desktop acceptance: no mouse, keyboard or wheel input injection.
# All windows/processes, data, credentials, autostart and logs are session-scoped.
param(
    [string]$Exe = '',
    [ValidateSet('debug', 'release')][string]$Profile = 'debug',
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).ProviderPath,
    [ValidateRange(1024, 65535)][int]$PetPort = 17897,
    [ValidateRange(1024, 65535)][int]$EmptyPort = 17898,
    [ValidateRange(1, 10)][int]$StartupRuns = 3,
    [string]$OutDir = $env:TEMP
)
$ErrorActionPreference = 'Stop'

# Isolated verification helpers. All mutations are scoped to this process and session.
function Assert-DesktopTestPorts {
    param([int[]]$Ports)
    if ($Ports.Count -eq 0) { throw 'At least one dedicated test port is required.' }
    if (@($Ports | Select-Object -Unique).Count -ne $Ports.Count) { throw 'Test ports must differ.' }
    $listeners = @()
    try {
        foreach ($port in $Ports) {
            if ($port -lt 1024 -or $port -gt 65535 -or $port -eq 17872) { throw "Dedicated test port required: $port" }
            $listener = New-Object System.Net.Sockets.TcpListener ([System.Net.IPAddress]::Loopback), $port
            $listener.ExclusiveAddressUse = $true
            $listeners += $listener
            $listener.Start()
        }
    } finally { foreach ($listener in $listeners) { $listener.Stop() } }
}

function New-DesktopTestSession {
    param([string]$Exe, [string]$RepoRoot, [int[]]$Ports, [string]$OutDir, [string]$Name)
    if ($env:OS -ne 'Windows_NT') { throw 'Windows desktop required; parsing on another platform is not acceptance.' }
    if (-not (Test-Path -LiteralPath $Exe -PathType Leaf)) { throw "Build the desktop executable first: $Exe" }
    Assert-DesktopTestPorts $Ports
    $identity = [Guid]::NewGuid().ToString('N')
    $directory = Join-Path $OutDir ("petsona-$Name-$identity")
    New-Item -ItemType Directory -Path $directory -ErrorAction Stop | Out-Null
    $resolvedExe = (Resolve-Path -LiteralPath $Exe).ProviderPath
    $stream = [IO.File]::OpenRead($resolvedExe)
    $reader = New-Object IO.BinaryReader $stream
    try {
        $stream.Position = 0x3c
        $offset = $reader.ReadInt32()
        $stream.Position = $offset
        if ($reader.ReadUInt32() -ne 0x00004550) { throw 'Expected a Windows PE executable.' }
        $machine = $reader.ReadUInt16()
        $architecture = switch ($machine) { 0x8664 { 'x86_64' } 0xaa64 { 'arm64' } 0x14c { 'x86' } default { "PE-$machine" } }
    } finally { $reader.Dispose(); $stream.Dispose() }
    $session = [PSCustomObject]@{
        Id = $identity; Directory = $directory; Exe = $resolvedExe; RepoRoot = $RepoRoot
        Architecture = $architecture; ExeHash = (Get-FileHash -LiteralPath $resolvedExe -Algorithm SHA256).Hash
        Ports = $Ports; Processes = (New-Object 'System.Collections.Generic.List[System.Diagnostics.Process]')
        Launches = (New-Object 'System.Collections.Generic.List[object]'); SavedEnvironment = @{}; Checks = (New-Object 'System.Collections.Generic.List[object]')
        Stages = @{}; CurrentStage = 'prepare'; Error = ''; Started = [DateTime]::UtcNow.ToString('o')
        KeyEnvironment = "PETSONA_ACCEPT_KEY_$identity"
    }
    return $session
}

function Set-DesktopTestEnvironment {
    param($Session, [string]$Name, [AllowNull()]$Value)
    if (-not $Session.SavedEnvironment.ContainsKey($Name)) {
        $Session.SavedEnvironment[$Name] = [Environment]::GetEnvironmentVariable($Name, 'Process')
    }
    if ($null -eq $Value) { [Environment]::SetEnvironmentVariable($Name, [NullString]::Value, 'Process') }
    else { [Environment]::SetEnvironmentVariable($Name, [string]$Value, 'Process') }
}
function Enter-DesktopTestEnvironment {
    param($Session)
    Set-DesktopTestEnvironment $Session 'PETSONA_AUTOSTART_LAUNCHER' $null
    Set-DesktopTestEnvironment $Session 'PATHEXT' '.COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC'
    Set-DesktopTestEnvironment $Session 'PETSONA_CREDENTIAL_SERVICE' ("com.petsona.accept.$($Session.Id)")
    Set-DesktopTestEnvironment $Session 'PETSONA_AUTOSTART_VALUE' ("Petsona.Accept.$($Session.Id)")
    Set-DesktopTestEnvironment $Session $Session.KeyEnvironment 'fake-local-acceptance-key'
}
function Restore-DesktopTestEnvironment {
    param($Session)
    foreach ($name in $Session.SavedEnvironment.Keys) {
        Set-DesktopTestEnvironment $Session $name $Session.SavedEnvironment[$name]
    }
}
function New-DesktopTestHome {
    param($Session, [string]$Name, [int]$Port, [bool]$Populated, [double]$Scale = 1.5)
    $directory = Join-Path $Session.Directory $Name
    New-Item -ItemType Directory -Path $directory -ErrorAction Stop | Out-Null
    if ($Populated) {
        $pets = Join-Path $directory 'pets'
        New-Item -ItemType Directory -Path $pets | Out-Null
        Copy-Item -LiteralPath (Join-Path $Session.RepoRoot 'crates\petsona-core\testdata\v2-test-pet') -Destination (Join-Path $pets 'test_fixture_v2') -Recurse
    }
    $config = @{
        firstRun = -not $Populated
        activePet = $(if ($Populated) { 'test_fixture_v2' } else { $null })
        stateServer = @{ enabled = $true; port = $Port }
        window = @{ scale = $Scale; startPosition = @{ x = 200.0; y = 200.0 } }
        greeting = @{ enabled = $false }
        deepseek = @{ provider = 'custom'; baseUrl = 'http://127.0.0.1:1/v1'; model = 'acceptance-only'; apiKeyEnv = $Session.KeyEnvironment }
    }
    Write-DesktopTestJson (Join-Path $directory 'config.json') $config
    return $directory
}
function Write-DesktopTestJson {
    param([string]$Path, $Value)
    [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding $false))
}
function Start-DesktopTestProcess {
    param($Session, [string]$HomeDirectory, [string[]]$Arguments = @())
    Set-DesktopTestEnvironment $Session 'PETSONA_HOME' $HomeDirectory
    $sequence = $Session.Processes.Count + 1
    Set-DesktopTestEnvironment $Session 'PETSONA_SHELL_LOG' (Join-Path $Session.Directory "shell-$sequence.log")
    $options = @{ FilePath = $Session.Exe; PassThru = $true; WorkingDirectory = $Session.RepoRoot }
    if ($Arguments.Count -gt 0) { $options.ArgumentList = $Arguments }
    $process = Start-Process @options
    $Session.Processes.Add($process)
    $Session.Launches.Add([PSCustomObject]@{ pid = $process.Id; startedUtc = [DateTime]::UtcNow.ToString('o'); log = (Join-Path $Session.Directory "shell-$sequence.log"); arguments = $Arguments; home = $HomeDirectory })
    # Hold the original process handle so an exited PID cannot be mistaken for
    # a later unrelated process when cleanup runs.
    $null = $process.Handle
    return $process
}
function Stop-DesktopTestProcesses {
    param($Session)
    $errors = @()
    foreach ($process in $Session.Processes) {
        try {
            if (-not $process.HasExited) {
                $process.Kill()
                if (-not $process.WaitForExit(5000)) { throw "Owned test process did not stop: $($process.Id)" }
            }
        } catch { $errors += $_.Exception.Message }
    }
    if ($errors.Count -gt 0) { throw ($errors -join '; ') }
}
function Add-DesktopTestCheck {
    param($Session, [string]$Id, [bool]$Passed, [string]$Detail, [switch]$Required)
    $status = if ($Passed) { 'PASS' } else { 'FAIL' }
    $Session.Checks.Add([PSCustomObject]@{ id = $Id; stage = $Session.CurrentStage; status = $status; detail = $Detail })
    Write-Host "$status $Id : $Detail"
    if ($Required -and -not $Passed) { throw "$Id : $Detail" }
}
function Invoke-DesktopTestStage {
    param($Session, [string]$Name, [scriptblock]$Body)
    $Session.CurrentStage = $Name
    $before = $Session.Checks.Count
    try {
        & $Body
        $newChecks = @($Session.Checks | Select-Object -Skip $before)
        if ($newChecks.Count -eq 0) { throw 'Stage produced no assertions.' }
        $Session.Stages[$Name] = if (@($newChecks | Where-Object { $_.status -eq 'FAIL' }).Count -gt 0) { 'FAIL' } else { 'PASS' }
    } catch {
        Add-DesktopTestCheck $Session "$Name.exception" $false $_.Exception.Message
        $Session.Stages[$Name] = 'FAIL'
    } finally {
        try { Stop-DesktopTestProcesses $Session }
        catch { Add-DesktopTestCheck $Session "$Name.cleanup" $false $_.Exception.Message; $Session.Stages[$Name] = 'FAIL' }
    }
}
function Complete-DesktopTestSession {
    param($Session)
    $failureCount = @($Session.Checks | Where-Object { $_.status -eq 'FAIL' }).Count
    $status = if ($Session.Error -or $failureCount -gt 0) { 'FAIL' } elseif ($Session.Checks.Count -eq 0 -or @($Session.Stages.Values | Where-Object { $_ -eq 'SKIP' }).Count -gt 0) { 'FAIL' } else { 'PASS' }
    $report = @{
        status = $status; startedUtc = $Session.Started; finishedUtc = [DateTime]::UtcNow.ToString('o')
        cwd = $Session.RepoRoot; exe = $Session.Exe; exeSha256 = $Session.ExeHash; architecture = $Session.Architecture
        ports = $Session.Ports; directory = $Session.Directory; processes = @($Session.Launches.ToArray()); checks = @($Session.Checks.ToArray()); stages = $Session.Stages
        powershellVersion = $PSVersionTable.PSVersion.ToString(); osVersion = [Environment]::OSVersion.ToString()
        error = $Session.Error; pending = @('installer install/upgrade acceptance')
    }
    $path = Join-Path $Session.Directory 'report.json'
    Write-DesktopTestJson $path $report
    Write-Host "$status; evidence: $path"
    return $(if ($status -eq 'FAIL') { 1 } else { 0 })
}
function Get-DesktopTestHealth {
    param([int]$Port)
    try { return Invoke-RestMethod -Uri "http://127.0.0.1:$Port/health" -TimeoutSec 2 }
    catch { return $null }
}
function Wait-DesktopTestCondition {
    param([scriptblock]$Condition, [int]$TimeoutMs = 10000)
    $watch = [Diagnostics.Stopwatch]::StartNew()
    while ($watch.ElapsedMilliseconds -lt $TimeoutMs) {
        if (& $Condition) { return $true }
        Start-Sleep -Milliseconds 60
    }
    return $false
}

# Window lookup is always PID-scoped. No input synthesis entry points exist.
if (-not ('PetsonaTestNative' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class PetsonaTestNative {
    public struct RECT { public int Left, Top, Right, Bottom; }
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", EntryPoint="GetWindowLongPtrW")] static extern IntPtr GetWindowLongPtrW64(IntPtr h, int n);
    [DllImport("user32.dll", EntryPoint="GetWindowLongW")] static extern int GetWindowLongW32(IntPtr h, int n);
    [DllImport("user32.dll")] static extern bool PostMessageW(IntPtr h, uint m, UIntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int height, uint flags);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
    public static long ExStyle(IntPtr h) { return IntPtr.Size == 8 ? GetWindowLongPtrW64(h,-20).ToInt64() : GetWindowLongW32(h,-20); }
    public static IntPtr Find(uint pid, string cls, string titlePrefix, bool visibleOnly) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h,l) => {
            uint owner; GetWindowThreadProcessId(h,out owner);
            if (owner != pid || (visibleOnly && !IsWindowVisible(h))) return true;
            var name = new StringBuilder(256); GetClassNameW(h,name,256);
            if (name.ToString() != cls) return true;
            if (titlePrefix != null) {
                var title = new StringBuilder(512); GetWindowTextW(h,title,512);
                if (!title.ToString().StartsWith(titlePrefix,StringComparison.Ordinal)) return true;
            }
            found = h; return false;
        },IntPtr.Zero);
        return found;
    }
    // Restrict lifecycle messages to WM_CLOSE and confirm ownership at send time.
    public static bool CloseOwned(IntPtr h, uint pid) {
        uint owner; GetWindowThreadProcessId(h,out owner);
        return owner == pid && PostMessageW(h,0x0010,UIntPtr.Zero,IntPtr.Zero);
    }
}
'@
}
function Get-DesktopTestWindow {
    param([Diagnostics.Process]$Process, [string]$Class, [AllowNull()][string]$TitlePrefix = $null, [bool]$VisibleOnly = $true)
    if ($Process.HasExited) { return [IntPtr]::Zero }
    return [PetsonaTestNative]::Find([uint32]$Process.Id, $Class, $TitlePrefix, $VisibleOnly)
}
function Save-DesktopTestWindowShot {
    param([IntPtr]$Window, [string]$Path, [switch]$Screen, [int]$Padding = 0)
    Add-Type -AssemblyName System.Drawing
    $rect = New-Object PetsonaTestNative+RECT
    if (-not [PetsonaTestNative]::GetWindowRect($Window, [ref]$rect)) { throw 'Cannot read screenshot window rectangle.' }
    if (-not $Screen) { $Padding = 0 }
    $bitmap = New-Object Drawing.Bitmap ($rect.Right - $rect.Left + 2 * $Padding), ($rect.Bottom - $rect.Top + 2 * $Padding)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    try {
        if ($Screen) { $graphics.CopyFromScreen($rect.Left - $Padding, $rect.Top - $Padding, 0, 0, $bitmap.Size) }
        else {
            $dc = $graphics.GetHdc()
            try { $printed = [PetsonaTestNative]::PrintWindow($Window, $dc, 2) }
            finally { $graphics.ReleaseHdc($dc) }
            if (-not $printed) { throw 'PrintWindow failed; screenshot not counted as passed.' }
        }
        $bitmap.Save($Path, [Drawing.Imaging.ImageFormat]::Png)
    } finally { $graphics.Dispose(); $bitmap.Dispose() }
}
function Write-DesktopPreflightFailure {
    param([string]$OutDir, [string]$Exe, [string]$RepoRoot, [string]$Message)
    $directory = Join-Path $OutDir ('petsona-preflight-' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $directory -ErrorAction Stop | Out-Null
    $path = Join-Path $directory 'report.json'
    Write-DesktopTestJson $path @{ status='FAIL'; phase='prepare'; exe=$Exe; cwd=$RepoRoot; error=$Message; architecture='not-validated'; checks=@(); stages=@{ prepare='FAIL' } }
    Write-Host "FAIL; preflight evidence: $path"
}

$session = $null
$exitCode = 1
try {
    if (-not $Exe) { $Exe = Join-Path $env:USERPROFILE ("petsona-build\desktop-target\$Profile\petsona-desktop.exe") }
    $session = New-DesktopTestSession $Exe $RepoRoot @($PetPort, $EmptyPort) $OutDir 'smoke'
    foreach ($stage in @('startup', 'geometry-animation', 'settings-lifecycle', 'chat-lifecycle', 'single-instance', 'protocol', 'exit', 'empty')) { $session.Stages[$stage] = 'SKIP' }
    Enter-DesktopTestEnvironment $session
    $homePet = New-DesktopTestHome $session 'populated' $PetPort $true
    $homeEmpty = New-DesktopTestHome $session 'empty' $EmptyPort $false
    Write-Host "Evidence directory: $($session.Directory)"
        Invoke-DesktopTestStage $session 'startup' {
            for ($run = 1; $run -le $StartupRuns; $run++) {
                $process = Start-DesktopTestProcess $session $homePet
                $watch = [Diagnostics.Stopwatch]::StartNew()
                $visible = Wait-DesktopTestCondition { (Get-DesktopTestWindow $process 'PetsonaPetWindow') -ne [IntPtr]::Zero } 20000
                Add-DesktopTestCheck $session "startup.$run" $visible "Pet visible after $($watch.ElapsedMilliseconds) ms" -Required
                Stop-DesktopTestProcesses $session
            }
        }
        Invoke-DesktopTestStage $session 'geometry-animation' {
            $process = Start-DesktopTestProcess $session $homePet
            $visible = Wait-DesktopTestCondition { (Get-DesktopTestWindow $process 'PetsonaPetWindow') -ne [IntPtr]::Zero } 20000
            Add-DesktopTestCheck $session 'pet.visible' $visible 'Only the owned PID pet counts' -Required
            $window = Get-DesktopTestWindow $process 'PetsonaPetWindow'
            $rect = New-Object PetsonaTestNative+RECT
            $read = [PetsonaTestNative]::GetWindowRect($window, [ref]$rect)
            Add-DesktopTestCheck $session 'pet.rectangle' $read 'Native physical rectangle available' -Required
            $dpi = [PetsonaTestNative]::GetDpiForWindow($window)
            $density = [Math]::Max(1.0, $dpi / 96.0)
            $expectedW = [int][Math]::Round(192 * 1.5 * $density)
            $expectedH = [int][Math]::Round(208 * 1.5 * $density)
            Add-DesktopTestCheck $session 'pet.geometry' (($rect.Right - $rect.Left) -eq $expectedW -and ($rect.Bottom - $rect.Top) -eq $expectedH) "DPI=$dpi expected=${expectedW}x${expectedH}"
            # Configured startup position, not a simulated drag/position-memory test.
            Add-DesktopTestCheck $session 'pet.configured-origin' ($rect.Left -eq 200 -and $rect.Top -eq 200) "Expected (200,200), actual ($($rect.Left),$($rect.Top)); use a desktop containing this point"
            $style = [PetsonaTestNative]::ExStyle($window)
            foreach ($flag in @(@('layered',0x00080000),@('toolwindow',0x00000080),@('topmost',0x00000008),@('noactivate',0x08000000))) {
                Add-DesktopTestCheck $session ("pet.style." + $flag[0]) (($style -band $flag[1]) -ne 0) 'Required native extended style'
            }
            Add-DesktopTestCheck $session 'pet.no-focus' ([PetsonaTestNative]::GetForegroundWindow() -ne $window) 'Pet must not own foreground focus'
            Add-DesktopTestCheck $session 'pet.no-edit-strip' ((Get-DesktopTestWindow $process 'PetsonaStripWindow' $null $false) -eq [IntPtr]::Zero) 'Retired edit strip absent in owned PID'
            $hashes = @()
            for ($frame = 1; $frame -le 14; $frame++) {
                $shot = Join-Path $session.Directory "animation-$frame.png"
                Save-DesktopTestWindowShot $window $shot -Screen
                $hashes += (Get-FileHash -LiteralPath $shot -Algorithm SHA256).Hash
                Start-Sleep -Milliseconds 350
            }
            $distinct = @($hashes | Sort-Object -Unique).Count
            Add-DesktopTestCheck $session 'pet.screen-frames' ($distinct -gt 1) "Distinct screen captures=$distinct; window must remain unobscured, animation appearance is also manual"
        }
        Invoke-DesktopTestStage $session 'settings-lifecycle' {
            $process = Start-DesktopTestProcess $session $homePet @('--show-settings')
            $visible = Wait-DesktopTestCondition { (Get-DesktopTestWindow $process 'Tauri Window' 'Petsona 设置') -ne [IntPtr]::Zero } 20000
            Add-DesktopTestCheck $session 'settings.visible' $visible 'Owned settings window visible' -Required
            $focused = Wait-DesktopTestCondition { [PetsonaTestNative]::GetForegroundWindow() -eq (Get-DesktopTestWindow $process 'Tauri Window' 'Petsona 设置') } 3000
            Add-DesktopTestCheck $session 'settings.focus' $focused '--show-settings receives foreground focus'
            $petReady = Wait-DesktopTestCondition { (Get-DesktopTestWindow $process 'PetsonaPetWindow') -ne [IntPtr]::Zero } 20000
            Add-DesktopTestCheck $session 'settings.pet-before-close' $petReady 'Owned pet visible before closing settings' -Required
            $pet = Get-DesktopTestWindow $process 'PetsonaPetWindow'
            $settingsWindow = Get-DesktopTestWindow $process 'Tauri Window' 'Petsona 设置'
            Add-DesktopTestCheck $session 'settings.close-request' ([PetsonaTestNative]::CloseOwned($settingsWindow, [uint32]$process.Id)) 'WM_CLOSE lifecycle only, ownership rechecked' -Required
            $hidden = Wait-DesktopTestCondition { -not [PetsonaTestNative]::IsWindowVisible($settingsWindow) } 5000
            Add-DesktopTestCheck $session 'settings.close-hides' ($hidden -and -not $process.HasExited) 'Close hides settings and keeps shell alive'
            Add-DesktopTestCheck $session 'settings.pet-remains' ([PetsonaTestNative]::IsWindowVisible($pet) -and (Get-DesktopTestWindow $process 'PetsonaPetWindow') -eq $pet) 'Same owned pet remains visible'
            Add-DesktopTestCheck $session 'settings.health-remains' ([bool]((Get-DesktopTestHealth $PetPort).ok)) 'Dedicated runtime remains healthy'
            [void][PetsonaTestNative]::ShowWindow($settingsWindow, 5)
            $reopened = Wait-DesktopTestCondition { [PetsonaTestNative]::IsWindowVisible($settingsWindow) } 3000
            Add-DesktopTestCheck $session 'settings.re-show' $reopened 'Owned window re-show proxy; tray/visual controls remain manual'
        }
        Invoke-DesktopTestStage $session 'chat-lifecycle' {
            $process = Start-DesktopTestProcess $session $homePet @('--show-chat')
            $visible = Wait-DesktopTestCondition { (Get-DesktopTestWindow $process 'Tauri Window' 'Petsona 聊天') -ne [IntPtr]::Zero } 20000
            Add-DesktopTestCheck $session 'chat.visible' $visible 'Owned chat window visible' -Required
            # Content windows may appear before the runtime/atlas has presented
            # the native pet. Establish that precondition before testing close.
            $petReady = Wait-DesktopTestCondition { (Get-DesktopTestWindow $process 'PetsonaPetWindow') -ne [IntPtr]::Zero } 20000
            Add-DesktopTestCheck $session 'chat.pet-before-close' $petReady 'Owned pet visible before closing chat' -Required
            $pet = Get-DesktopTestWindow $process 'PetsonaPetWindow'
            $chat = Get-DesktopTestWindow $process 'Tauri Window' 'Petsona 聊天'
            Add-DesktopTestCheck $session 'chat.close-request' ([PetsonaTestNative]::CloseOwned($chat, [uint32]$process.Id)) 'WM_CLOSE lifecycle only, ownership rechecked' -Required
            $hidden = Wait-DesktopTestCondition { -not [PetsonaTestNative]::IsWindowVisible($chat) } 5000
            Add-DesktopTestCheck $session 'chat.close-hides' ($hidden -and -not $process.HasExited) 'Close hides window and keeps shell alive'
            Add-DesktopTestCheck $session 'chat.pet-remains' ([PetsonaTestNative]::IsWindowVisible($pet) -and (Get-DesktopTestWindow $process 'PetsonaPetWindow') -eq $pet) 'Same owned pet remains visible after closing chat'
            $healthy = Wait-DesktopTestCondition { (Get-DesktopTestHealth $PetPort).ok } 3000
            Add-DesktopTestCheck $session 'chat.health-remains' $healthy 'Runtime still healthy'
        }
        Invoke-DesktopTestStage $session 'single-instance' {
            $first = Start-DesktopTestProcess $session $homePet
            $visible = Wait-DesktopTestCondition { (Get-DesktopTestWindow $first 'PetsonaPetWindow') -ne [IntPtr]::Zero } 20000
            Add-DesktopTestCheck $session 'instance.first' $visible 'First owned instance visible' -Required
            $second = Start-DesktopTestProcess $session $homePet
            $notice = Wait-DesktopTestCondition { (Get-DesktopTestWindow $second 'PetsonaOverlayWindow') -ne [IntPtr]::Zero } 2500
            Add-DesktopTestCheck $session 'instance.notice' $notice 'Second owned PID shows conflict notice'
            Add-DesktopTestCheck $session 'instance.second-exits' ($second.WaitForExit(12000)) 'Second process exits'
            $alive = @($session.Processes | Where-Object { -not $_.HasExited }).Count
            Add-DesktopTestCheck $session 'instance.one-owned-survivor' ($alive -eq 1 -and -not $first.HasExited) 'Exactly one owned shell survives; daily instances excluded'
            Add-DesktopTestCheck $session 'instance.first-healthy' ([bool]((Get-DesktopTestHealth $PetPort).ok)) 'First instance still healthy'
        }
        Invoke-DesktopTestStage $session 'protocol' {
            $process = Start-DesktopTestProcess $session $homePet
            $ready = Wait-DesktopTestCondition { (Get-DesktopTestHealth $PetPort).ok } 10000
            Add-DesktopTestCheck $session 'protocol.health' $ready 'Dedicated port /health' -Required
            $listed = Invoke-RestMethod -Uri "http://127.0.0.1:$PetPort/pets" -TimeoutSec 2
            Add-DesktopTestCheck $session 'protocol.pets' ($listed -contains 'test_fixture_v2') 'Fixture listed'
            $uri = "http://127.0.0.1:$PetPort/state"
            $body = @{ source = 'smoke'; state = 'waiting'; message = 'smoke'; ttlMs = 2000 } | ConvertTo-Json
            Invoke-RestMethod -Method Post -Uri $uri -ContentType 'application/json; charset=utf-8' -Body $body -TimeoutSec 2 | Out-Null
            $waiting = Wait-DesktopTestCondition { (Get-DesktopTestHealth $PetPort).state -eq 'waiting' } 1500
            Add-DesktopTestCheck $session 'protocol.state' $waiting 'POST state becomes waiting'
            $bubble = Wait-DesktopTestCondition { (Get-DesktopTestWindow $process 'PetsonaOverlayWindow') -ne [IntPtr]::Zero } 2000
            Add-DesktopTestCheck $session 'protocol.bubble' $bubble 'Protocol message shows owned bubble'
            $expired = Wait-DesktopTestCondition {
                $health = Get-DesktopTestHealth $PetPort
                $health -and $health.ok -and ($health.state -ne 'waiting')
            } 5000
            $expiredHealth = Get-DesktopTestHealth $PetPort
            $expiredState = if ($expiredHealth) { $expiredHealth.state } else { 'health unavailable' }
            Add-DesktopTestCheck $session 'protocol.ttl' $expired "TTL removed waiting override; current state=$expiredState"
            $gone = Wait-DesktopTestCondition { (Get-DesktopTestWindow $process 'PetsonaOverlayWindow') -eq [IntPtr]::Zero } 12000
            Add-DesktopTestCheck $session 'protocol.bubble-expires' $gone 'Bubble lifetime ends; keep real pointer away from bubble'
            foreach ($bad in @('{"state":"nope"}', 'not-json')) {
                $status = 0
                try { Invoke-RestMethod -Method Post -Uri $uri -ContentType 'application/json; charset=utf-8' -Body $bad -TimeoutSec 2 | Out-Null }
                catch { if ($_.Exception.Response) { $status = [int]$_.Exception.Response.StatusCode } }
                Add-DesktopTestCheck $session "protocol.invalid.$bad" ($status -eq 400) 'Invalid request returns HTTP400'
            }
            Invoke-RestMethod -Method Post -Uri $uri -ContentType 'application/json; charset=utf-8' -Body '{"source":"smoke","state":"waiting","ttlMs":0}' -TimeoutSec 2 | Out-Null
            $sticky = Wait-DesktopTestCondition { (Get-DesktopTestHealth $PetPort).state -eq 'waiting' } 2000
            Start-Sleep -Milliseconds 2500
            Add-DesktopTestCheck $session 'protocol.sticky' ($sticky -and (Get-DesktopTestHealth $PetPort).state -eq 'waiting') 'ttlMs0 remains until clear'
            Invoke-RestMethod -Method Post -Uri $uri -ContentType 'application/json; charset=utf-8' -Body '{"source":"smoke","action":"CLEAR"}' -TimeoutSec 2 | Out-Null
            $cleared = Wait-DesktopTestCondition {
                $health = Get-DesktopTestHealth $PetPort
                $health -and $health.ok -and ($health.state -ne 'waiting')
            } 3000
            $clearedHealth = Get-DesktopTestHealth $PetPort
            $clearedState = if ($clearedHealth) { $clearedHealth.state } else { 'health unavailable' }
            Add-DesktopTestCheck $session 'protocol.clear' $cleared "CLEAR removed waiting override; current state=$clearedState"
        }
        Invoke-DesktopTestStage $session 'exit' {
            $process = Start-DesktopTestProcess $session $homePet @('--exit-after-ms', '3000')
            $ready = Wait-DesktopTestCondition { (Get-DesktopTestHealth $PetPort).ok } 10000
            Add-DesktopTestCheck $session 'exit.health' $ready 'Health available before exit' -Required
            $exited = $process.WaitForExit(10000)
            Add-DesktopTestCheck $session 'exit.process' ($exited -and $process.ExitCode -eq 0) 'Scheduled exit completes with code0'
            $released = Wait-DesktopTestCondition { -not (Get-DesktopTestHealth $PetPort) } 3000
            Add-DesktopTestCheck $session 'exit.health-released' $released 'Health endpoint released'
            $listener = New-Object Net.Sockets.TcpListener ([Net.IPAddress]::Loopback), $PetPort
            try { $listener.ExclusiveAddressUse = $true; $listener.Start(); Add-DesktopTestCheck $session 'exit.port-released' $true 'Port can be bound again' }
            finally { $listener.Stop() }
        }
        Invoke-DesktopTestStage $session 'empty' {
            $process = Start-DesktopTestProcess $session $homeEmpty
            $visible = Wait-DesktopTestCondition { (Get-DesktopTestWindow $process 'Tauri Window' 'Petsona 设置') -ne [IntPtr]::Zero } 20000
            Add-DesktopTestCheck $session 'empty.settings' $visible 'Empty library presents settings'
            Add-DesktopTestCheck $session 'empty.pet-hidden' ((Get-DesktopTestWindow $process 'PetsonaPetWindow') -eq [IntPtr]::Zero) 'No pet visible in owned empty instance'
            $ready = Wait-DesktopTestCondition { (Get-DesktopTestHealth $EmptyPort).ok } 8000
            $health = Get-DesktopTestHealth $EmptyPort
            Add-DesktopTestCheck $session 'empty.health' ($ready -and -not $health.pet) 'Health ready with empty pet field'
        }
} catch {
    if ($session) { $session.Error = $_.Exception.Message; Add-DesktopTestCheck $session 'runner.exception' $false $session.Error }
    else { Write-Error $_ -ErrorAction Continue; Write-DesktopPreflightFailure $OutDir $Exe $RepoRoot $_.Exception.Message }
} finally {
    if ($session) {
        try { Stop-DesktopTestProcesses $session }
        catch { $session.Error = $_.Exception.Message; Add-DesktopTestCheck $session 'runner.cleanup' $false $session.Error }
        finally { Restore-DesktopTestEnvironment $session }
        try { $exitCode = Complete-DesktopTestSession $session }
        catch {
            Stop-DesktopTestProcesses $session
            Write-Error $_ -ErrorAction Continue
            $exitCode = 1
        }
    }
}
exit $exitCode
