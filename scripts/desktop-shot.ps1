# Captures the pet window area (with padding) to a PNG for visual inspection.
# Usage: powershell -ExecutionPolicy Bypass -File scripts\desktop-shot.ps1 -Out C:\path\shot.png
param(
  [Parameter(Mandatory = $true)][string]$Out,
  [int]$Padding = 60,
  [string]$WindowClass = 'PetsonaPetWindow'
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class ShotNative
{
    [DllImport("user32.dll", CharSet = CharSet.Unicode, ExactSpelling = true)]
    public static extern IntPtr FindWindowW(string c, string n);
    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr h, out RECT r);
    public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
}
'@

$hwnd = [ShotNative]::FindWindowW($WindowClass, [NullString]::Value)
if ($hwnd -eq [IntPtr]::Zero) { throw "window not found: $WindowClass" }

$rect = New-Object ShotNative+RECT
[void][ShotNative]::GetWindowRect($hwnd, [ref]$rect)
$x = $rect.Left - $Padding
$y = $rect.Top - $Padding
$w = ($rect.Right - $rect.Left) + 2 * $Padding
$h = ($rect.Bottom - $rect.Top) + 2 * $Padding

$bitmap = New-Object System.Drawing.Bitmap $w, $h
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($x, $y, 0, 0, $bitmap.Size)
$bitmap.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bitmap.Dispose()
Write-Host "saved $Out"
