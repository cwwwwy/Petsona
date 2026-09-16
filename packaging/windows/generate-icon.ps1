#requires -Version 5.1
<#
.SYNOPSIS
    Regenerate packaging\windows\Petsona.ico from the shared Petsona artwork.
.DESCRIPTION
    packaging\macos\Petsona.icns is the single source of truth for the app icon.
    This script extracts its PNG payloads, re-renders the sizes Windows expects
    (16/24/32/48/64/128/256) and packs them into an ICO container whose entries
    are PNG-compressed (supported by Windows Vista and later).

    Run it after the artwork changes:
        powershell -ExecutionPolicy Bypass -File packaging\windows\generate-icon.ps1
#>
[CmdletBinding()]
param(
    [string] $SourceIcns = "",
    [string] $OutputIco = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if ([string]::IsNullOrWhiteSpace($SourceIcns)) {
    $SourceIcns = Join-Path $root "packaging\macos\Petsona.icns"
}
if ([string]::IsNullOrWhiteSpace($OutputIco)) {
    $OutputIco = Join-Path $root "packaging\windows\Petsona.ico"
}

Add-Type -AssemblyName System.Drawing

function Read-BigEndianUInt32 {
    param([byte[]] $Bytes, [int] $Offset)
    return ([uint32]$Bytes[$Offset] -shl 24) -bor ([uint32]$Bytes[$Offset + 1] -shl 16) -bor ([uint32]$Bytes[$Offset + 2] -shl 8) -bor [uint32]$Bytes[$Offset + 3]
}

function Get-IcnsPngChunks {
    param([string] $Path)

    $bytes = [System.IO.File]::ReadAllBytes($Path)
    $magic = [System.Text.Encoding]::ASCII.GetString($bytes, 0, 4)
    if ($magic -ne "icns") {
        throw "not an icns file: $Path"
    }

    $chunks = @{}
    $offset = 8
    while ($offset -lt $bytes.Length - 8) {
        $type = [System.Text.Encoding]::ASCII.GetString($bytes, $offset, 4)
        $length = Read-BigEndianUInt32 -Bytes $bytes -Offset ($offset + 4)
        if ($length -le 8) {
            break
        }
        $isPng = $bytes[$offset + 8] -eq 0x89 -and $bytes[$offset + 9] -eq 0x50 -and $bytes[$offset + 10] -eq 0x4E -and $bytes[$offset + 11] -eq 0x47
        if ($isPng) {
            $payload = New-Object byte[] ($length - 8)
            [System.Array]::Copy($bytes, $offset + 8, $payload, 0, $length - 8)
            $chunks[$type] = $payload
        }
        $offset += $length
    }
    return $chunks
}

function Convert-PngToSize {
    param([byte[]] $Png, [int] $Size)

    $input = New-Object System.IO.MemoryStream(, $Png)
    $source = [System.Drawing.Image]::FromStream($input)
    try {
        $bitmap = New-Object System.Drawing.Bitmap($Size, $Size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
        try {
            $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
            try {
                $graphics.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
                $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
                $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
                $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
                $graphics.Clear([System.Drawing.Color]::Transparent)
                $graphics.DrawImage($source, 0, 0, $Size, $Size)
            }
            finally {
                $graphics.Dispose()
            }
            $output = New-Object System.IO.MemoryStream
            $bitmap.Save($output, [System.Drawing.Imaging.ImageFormat]::Png)
            return $output.ToArray()
        }
        finally {
            $bitmap.Dispose()
        }
    }
    finally {
        $source.Dispose()
        $input.Dispose()
    }
}

$chunks = Get-IcnsPngChunks -Path $SourceIcns
if ($chunks.Count -eq 0) {
    throw "no PNG payloads found in $SourceIcns"
}

# Largest payload first: it is the resampling source for sizes the artwork does
# not ship as its own chunk.
$exact = @{
    32  = "ic11"
    64  = "ic12"
    128 = "ic07"
    256 = "ic08"
}
$sourceType = @("ic10", "ic09", "ic14", "ic13", "ic08") | Where-Object { $chunks.ContainsKey($_) } | Select-Object -First 1
if (-not $sourceType) {
    $sourceType = $chunks.Keys | Select-Object -First 1
}

$sizes = 16, 24, 32, 48, 64, 128, 256
$payloads = @()
foreach ($size in $sizes) {
    $type = $exact[$size]
    if ($type -and $chunks.ContainsKey($type)) {
        $payloads += , @{ Size = $size; Png = $chunks[$type] }
    }
    else {
        $payloads += , @{ Size = $size; Png = Convert-PngToSize -Png $chunks[$sourceType] -Size $size }
    }
}

$outputDirectory = Split-Path -Parent $OutputIco
if (-not (Test-Path $outputDirectory)) {
    New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
}

# ICONDIR + one ICONDIRENTRY per image + the PNG payloads.
$headerSize = 6 + (16 * $payloads.Count)
$payloadOffset = $headerSize
$stream = New-Object System.IO.MemoryStream
$writer = New-Object System.IO.BinaryWriter($stream)
try {
    $writer.Write([uint16]0)                # reserved
    $writer.Write([uint16]1)                # type: icon
    $writer.Write([uint16]$payloads.Count)
    foreach ($payload in $payloads) {
        $size = [int]$payload.Size
        $writer.Write([byte]$(if ($size -ge 256) { 0 } else { $size }))   # width (0 = 256)
        $writer.Write([byte]$(if ($size -ge 256) { 0 } else { $size }))   # height
        $writer.Write([byte]0)              # palette colours
        $writer.Write([byte]0)              # reserved
        $writer.Write([uint16]1)            # colour planes
        $writer.Write([uint16]32)           # bits per pixel
        $writer.Write([uint32]$payload.Png.Length)
        $writer.Write([uint32]$payloadOffset)
        $payloadOffset += $payload.Png.Length
    }
    foreach ($payload in $payloads) {
        $writer.Write([byte[]]$payload.Png)
    }
}
finally {
    $writer.Dispose()
}

[System.IO.File]::WriteAllBytes($OutputIco, $stream.ToArray())
$stream.Dispose()

$written = Get-Item $OutputIco
Write-Host ("Wrote {0} ({1} bytes, {2} sizes: {3})" -f $written.FullName, $written.Length, $sizes.Count, ($sizes -join "/")) -ForegroundColor Green