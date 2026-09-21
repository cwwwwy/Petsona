$ErrorActionPreference = "Stop"
if ($env:PATHEXT -notlike "*.EXE*") { $env:PATHEXT = ".COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC" }
# Diagnostic: time Start-Process -> first visible pet frame on an isolated home.
# Cold/hot runs differ mostly by the CLR / WindowsAppSDK host, not by our code.
# Evidence: docs/execution/windows-native-rewrite.md E-W15d.
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)

Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class StartProbe {
  public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr param);
  [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "GetClassNameW")] public static extern int GetClassName(IntPtr hWnd, StringBuilder text, int max);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
  public static bool HasVisible(string className, uint pid) {
    bool found = false;
    EnumWindows((h, p) => {
      if (!IsWindowVisible(h)) { return true; }
      uint winPid; GetWindowThreadProcessId(h, out winPid);
      if (winPid != pid) { return true; }
      var sb = new StringBuilder(256); GetClassName(h, sb, 256);
      if (sb.ToString() == className) { found = true; return false; }
      return true;
    }, IntPtr.Zero);
    return found;
  }
}
"@

$home2 = Join-Path $env:TEMP ("petsona-timing-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path (Join-Path $home2 "pets\v2-test-pet") -Force | Out-Null
Copy-Item (Join-Path $root "crates\petsona-core\testdata\v2-test-pet\pet.json") (Join-Path $home2 "pets\v2-test-pet\pet.json") -Force
Copy-Item (Join-Path $root "crates\petsona-core\testdata\v2-test-pet\spritesheet.png") (Join-Path $home2 "pets\v2-test-pet\spritesheet.png") -Force
$port = 17993
[System.IO.File]::WriteAllText((Join-Path $home2 "config.json"), ('{"stateServer":{"enabled":true,"port":' + $port + '}}'), (New-Object System.Text.UTF8Encoding($false)))

$release = Join-Path $root "apps\windows\Petsona\bin\x64\Release\net10.0-windows10.0.26100.0\win-x64"

# Round 0 measures the UNC copy; rounds 1-3 measure a local-disk copy of the
# same build, which is how the acceptance package is actually launched.
$localRoot = Join-Path $env:TEMP "petsona-local-run"
if (Test-Path -LiteralPath $localRoot) { Remove-Item -LiteralPath $localRoot -Recurse -Force }
New-Item -ItemType Directory -Path $localRoot -Force | Out-Null
Copy-Item -Path (Join-Path $release "*") -Destination $localRoot -Recurse -Force

$targets = @(
    @{ Name = "unc"; App = (Join-Path $release "Petsona.exe") },
    @{ Name = "local"; App = (Join-Path $localRoot "Petsona.exe") },
    @{ Name = "local"; App = (Join-Path $localRoot "Petsona.exe") },
    @{ Name = "local"; App = (Join-Path $localRoot "Petsona.exe") }
)
$app = $targets[0].App
Write-Host ("app = {0}" -f $app)
Write-Host ("home = {0}" -f $home2)

for ($round = 0; $round -lt $targets.Count; $round++) {
    $label = $targets[$round].Name
    $exe = $targets[$round].App
    $env:PETSONA_HOME = $home2
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    $proc = Start-Process -FilePath $exe -PassThru
    $procRef = $proc
    $healthAt = -1
    $petAt = -1
    $overlayAt = -1
    $deadline = (Get-Date).AddSeconds(30)
    while ((Get-Date) -lt $deadline -and ($healthAt -lt 0 -or $petAt -lt 0)) {
        if ($healthAt -lt 0) {
            try { $null = Invoke-RestMethod -Uri ("http://127.0.0.1:{0}/health" -f $port) -TimeoutSec 1; $healthAt = $watch.ElapsedMilliseconds } catch { }
        }
        if ($petAt -lt 0 -and [StartProbe]::HasVisible("PetsonaPetWindow", [uint32]$procRef.Id)) { $petAt = $watch.ElapsedMilliseconds }
        if ($overlayAt -lt 0 -and [StartProbe]::HasVisible("PetsonaOverlayWindow", [uint32]$procRef.Id)) { $overlayAt = $watch.ElapsedMilliseconds }
        if ($healthAt -lt 0 -or $petAt -lt 0) { Start-Sleep -Milliseconds 10 }
    }
    # Give the tray/overlay a moment, then sample once more.
    Start-Sleep -Milliseconds 500
    if ($overlayAt -lt 0 -and [StartProbe]::HasVisible("PetsonaOverlayWindow", [uint32]$procRef.Id)) { $overlayAt = $watch.ElapsedMilliseconds }
    $watch.Stop()
    Write-Host ("round {0} ({1}): health={2}ms petWindow={3}ms overlay={4}ms total={5}ms" -f $round, $label, $healthAt, $petAt, $overlayAt, $watch.ElapsedMilliseconds)
    Stop-Process -Id $procRef.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
}
