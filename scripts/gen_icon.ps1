param(
    [string]$OutPath = (Join-Path $PSScriptRoot "..\assets\icon.ico")
)

$ErrorActionPreference = "Stop"
$OutPath = [System.IO.Path]::GetFullPath($OutPath)
$dir = Split-Path $OutPath -Parent
if (-not (Test-Path $dir)) {
    New-Item -ItemType Directory -Path $dir | Out-Null
}

function Get-SunPixels([int]$size) {
    $px = New-Object 'byte[]' ($size * $size * 4)
    $cx = ($size - 1) * 0.5
    $cy = $cx
    $rBody = $size * 0.22
    $rInner = $size * 0.14
    $rRay = $size * 0.42
    $rRayIn = $size * 0.28
    $pi = [Math]::PI

    for ($y = 0; $y -lt $size; $y++) {
        for ($x = 0; $x -lt $size; $x++) {
            $dx = $x - $cx
            $dy = $y - $cy
            $dist = [Math]::Sqrt($dx * $dx + $dy * $dy)
            $a = 0.0

            if ($dist -le $rBody) {
                $a = 1.0
                if ($dist -lt $rInner) { $a = 0.92 }
            }

            $angle = [Math]::Atan2($dy, $dx)
            $sector = $pi / 8.0
            $wrapped = $angle
            while ($wrapped -lt 0) { $wrapped += $pi * 2.0 }
            $local = (($wrapped + $sector) % ($sector * 2.0)) - $sector
            $rayWidth = 0.18 + 0.08 * [Math]::Min(1.0, [Math]::Max(0.0, $dist / $rRay))
            if ($dist -gt $rRayIn -and $dist -lt $rRay -and [Math]::Abs($local) -lt $rayWidth) {
                $fade = 1.0 - [Math]::Min(1.0, [Math]::Max(0.0, ($dist - $rRayIn) / ($rRay - $rRayIn)))
                if ($fade * 0.95 -gt $a) { $a = $fade * 0.95 }
            }

            if ($a -gt 0.02) {
                $i = ($y * $size + $x) * 4
                $px[$i] = 40
                $px[$i + 1] = 186
                $px[$i + 2] = 255
                $px[$i + 3] = [byte][Math]::Round($a * 255)
            }
        }
    }
    ,$px
}

function New-IcoDib([int]$size, [byte[]]$topDownBgra) {
    $xorSize = $size * $size * 4
    $andRow = [Math]::Ceiling($size / 32.0) * 4
    $andSize = $andRow * $size
    $dib = New-Object 'byte[]' (40 + $xorSize + $andSize)

    # BITMAPINFOHEADER; height is 2*size (XOR + AND) for ICO
    $h = [BitConverter]::GetBytes([int32]40); [Array]::Copy($h, 0, $dib, 0, 4)
    $w = [BitConverter]::GetBytes([int32]$size); [Array]::Copy($w, 0, $dib, 4, 4)
    $hh = [BitConverter]::GetBytes([int32]($size * 2)); [Array]::Copy($hh, 0, $dib, 8, 4)
    $planes = [BitConverter]::GetBytes([int16]1); [Array]::Copy($planes, 0, $dib, 12, 2)
    $bpp = [BitConverter]::GetBytes([int16]32); [Array]::Copy($bpp, 0, $dib, 14, 2)
    $img = [BitConverter]::GetBytes([int32]$xorSize); [Array]::Copy($img, 0, $dib, 20, 4)

    # Bottom-up XOR
    for ($y = 0; $y -lt $size; $y++) {
        $src = ($size - 1 - $y) * $size * 4
        $dst = 40 + $y * $size * 4
        [Array]::Copy($topDownBgra, $src, $dib, $dst, $size * 4)
    }
    ,$dib
}

$sizes = @(16, 32, 48, 256)
$images = @()
foreach ($s in $sizes) {
    $pixels = Get-SunPixels $s
    $images += ,@($s, (New-IcoDib $s $pixels))
}

$header = New-Object 'byte[]' (6 + 16 * $images.Count)
$header[2] = 1
$header[4] = [byte]$images.Count

$offset = $header.Length
$chunks = New-Object System.Collections.Generic.List[byte[]]
$chunks.Add($header) | Out-Null

for ($i = 0; $i -lt $images.Count; $i++) {
    $s = $images[$i][0]
    $data = $images[$i][1]
    $entry = 6 + $i * 16
    $header[$entry] = if ($s -ge 256) { [byte]0 } else { [byte]$s }
    $header[$entry + 1] = $header[$entry]
    $header[$entry + 4] = 1
    $header[$entry + 6] = 32
    $sizeBytes = [BitConverter]::GetBytes([int32]$data.Length)
    [Array]::Copy($sizeBytes, 0, $header, $entry + 8, 4)
    $offBytes = [BitConverter]::GetBytes([int32]$offset)
    [Array]::Copy($offBytes, 0, $header, $entry + 12, 4)
    $chunks.Add($data) | Out-Null
    $offset += $data.Length
}

$fs = [System.IO.File]::Create($OutPath)
try {
    foreach ($c in $chunks) { $fs.Write($c, 0, $c.Length) }
} finally {
    $fs.Dispose()
}

Write-Host "Wrote $OutPath ($((Get-Item $OutPath).Length) bytes)"
