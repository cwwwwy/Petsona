$ErrorActionPreference = "Stop"
if ($env:PATHEXT -notlike "*.EXE*") { $env:PATHEXT = ".COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC" }
# Diagnostic: the pet / overlay windows must own the cursor (WM_SETCURSOR -> 1).
# Registers a NULL-class-cursor control window that must answer 0, so the probe
# can tell "explicitly own the cursor" from "no cursor installed at all".
# Evidence: docs/execution/windows-native-rewrite.md E-W15b; smoke N23/N24 automates it.
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)

Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class ReturnProbe {
  public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
  public delegate IntPtr WndProcDelegate(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr param);
  [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "GetClassNameW")] public static extern int GetClassName(IntPtr hWnd, StringBuilder text, int max);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
  [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "SendMessageTimeoutW")]
  public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam, uint flags, uint timeout, out IntPtr result);
  [DllImport("kernel32.dll")] public static extern IntPtr GetModuleHandleW(IntPtr name);
  [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "RegisterClassExW")] public static extern ushort RegisterClassEx(ref WNDCLASSEX info);
  [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "CreateWindowExW")]
  public static extern IntPtr CreateWindowEx(uint exStyle, string className, string windowName, uint style, int x, int y, int w, int h, IntPtr parent, IntPtr menu, IntPtr instance, IntPtr param);
  [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "DefWindowProcW")] public static extern IntPtr DefWindowProc(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);

  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
  public struct WNDCLASSEX {
    public uint cbSize; public uint style; public IntPtr lpfnWndProc;
    public int cbClsExtra; public int cbWndExtra; public IntPtr hInstance;
    public IntPtr hIcon; public IntPtr hCursor; public IntPtr hbrBackground;
    [MarshalAs(UnmanagedType.LPWStr)] public string lpszMenuName;
    [MarshalAs(UnmanagedType.LPWStr)] public string lpszClassName;
    public IntPtr hIconSm;
  }

  private static WndProcDelegate _keepAlive;

  /// <summary>Hidden window whose class cursor is NULL and whose procedure is
  /// DefWindowProc - the pre-fix shape of the pet window.</summary>
  public static IntPtr CreateNullCursorWindow() {
    _keepAlive = new WndProcDelegate(DefWindowProc);
    var instance = GetModuleHandleW(IntPtr.Zero);
    var info = new WNDCLASSEX();
    info.cbSize = (uint)Marshal.SizeOf(typeof(WNDCLASSEX));
    info.lpfnWndProc = Marshal.GetFunctionPointerForDelegate(_keepAlive);
    info.hInstance = instance;
    info.lpszClassName = "PetsonaSmokeNullCursorProbe";
    RegisterClassEx(ref info);
    return CreateWindowEx(0, "PetsonaSmokeNullCursorProbe", "", 0, 0, 0, 20, 20, IntPtr.Zero, IntPtr.Zero, instance, IntPtr.Zero);
  }

  public static long Poke(IntPtr hWnd) {
    IntPtr result;
    var lParam = new IntPtr((0x0200L << 16) | 1);   // WM_MOUSEMOVE, HTCLIENT
    SendMessageTimeout(hWnd, 0x0020, hWnd, lParam, 0x0002, 1000, out result);
    return result.ToInt64();
  }

  public static List<IntPtr> VisibleByClassAndPid(string className, uint pid) {
    var found = new List<IntPtr>();
    EnumWindows((h, p) => {
      if (!IsWindowVisible(h)) { return true; }
      uint winPid; GetWindowThreadProcessId(h, out winPid);
      if (winPid != pid) { return true; }
      var sb = new StringBuilder(256); GetClassName(h, sb, 256);
      if (sb.ToString() == className) { found.Add(h); }
      return true;
    }, IntPtr.Zero);
    return found;
  }
}
"@

$home2 = Join-Path $env:TEMP ("petsona-return-probe-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path (Join-Path $home2 "pets\v2-test-pet") -Force | Out-Null
Copy-Item (Join-Path $root "crates\petsona-core\testdata\v2-test-pet\pet.json") (Join-Path $home2 "pets\v2-test-pet\pet.json") -Force
Copy-Item (Join-Path $root "crates\petsona-core\testdata\v2-test-pet\spritesheet.png") (Join-Path $home2 "pets\v2-test-pet\spritesheet.png") -Force
$port = 17991
[System.IO.File]::WriteAllText((Join-Path $home2 "config.json"), ('{"stateServer":{"enabled":true,"port":' + $port + '}}'), (New-Object System.Text.UTF8Encoding($false)))

$app = Join-Path $root "apps\windows\Petsona\bin\x64\Release\net10.0-windows10.0.26100.0\win-x64\Petsona.exe"
$env:PETSONA_HOME = $home2
$proc = Start-Process -FilePath $app -PassThru
try {
    $pet = [IntPtr]::Zero
    $deadline = (Get-Date).AddSeconds(20)
    while ((Get-Date) -lt $deadline -and $pet -eq [IntPtr]::Zero) {
        $found = [ReturnProbe]::VisibleByClassAndPid("PetsonaPetWindow", [uint32]$proc.Id)
        if ($found.Count -gt 0) { $pet = $found[0] }
        Start-Sleep -Milliseconds 200
    }
    if ($pet -eq [IntPtr]::Zero) { throw "pet window never appeared" }
    Start-Sleep -Seconds 1

    $overlays = [ReturnProbe]::VisibleByClassAndPid("PetsonaOverlayWindow", [uint32]$proc.Id)
    $control = [ReturnProbe]::CreateNullCursorWindow()
    Write-Host ("control window (null class cursor, DefWindowProc) = {0}" -f $control)
    Write-Host ("control poke result = {0}" -f [ReturnProbe]::Poke($control))
    Write-Host ("pet window = {0} poke result = {1}" -f $pet, [ReturnProbe]::Poke($pet))
    foreach ($ov in $overlays) {
        Write-Host ("overlay window = {0} poke result = {1}" -f $ov, [ReturnProbe]::Poke($ov))
    }
    Write-Host ("pet poke again = {0}" -f [ReturnProbe]::Poke($pet))
}
finally {
    if ($proc -and -not $proc.HasExited) { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue }
}
