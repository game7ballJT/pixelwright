# Regenerates every Pixelwright app-icon file from the one drawing below.
#
#   powershell -ExecutionPolicy Bypass -File packaging/icons.ps1
#
# The icon is original work for this project: a pixel-art "P" built from rounded cells on a
# gradient tile, with one amber "spark" cell. It uses nothing but System.Drawing (part of Windows),
# so no SVG renderer or extra tools are needed. Outputs (all in assets/app-icon/):
#   pixelwright.svg, pixelwright-small.svg, hicolor/scalable/apps/<id>.svg   vector master
#   pixelwright-1024.png                                                      macOS grid render
#   pixelwright.icns                                                          macOS bundle icon
#   pixelwright.ico                                                           Windows icon (16-256 px)
#   hicolor/<n>x<n>/apps/<id>.png                                             Linux icon theme
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$root = Join-Path (Split-Path $PSScriptRoot -Parent) 'assets\app-icon'
$id = 'online.jonmcgee.pixelwright'

# --- the drawing, in a 512-unit square -------------------------------------------------------
$ink = '#0e1233'; $blue = '#3b6cff'; $paper = '#f4f1e8'; $amber = '#ffb02e'
$cell = 64; $gap = 12; $radius = 14
$grid = @('1110', '1001', '1110', '1000', '1000')     # a pixel-art P, 4 columns x 5 rows
$spark = @(1, 3)                                        # row, column of the amber cell
$gridW = 4 * $cell + 3 * $gap; $gridH = 5 * $cell + 4 * $gap
$x0 = (512 - $gridW) / 2; $y0 = (512 - $gridH) / 2

function Get-Cells {
  for ($r = 0; $r -lt 5; $r++) { for ($c = 0; $c -lt 4; $c++) { if ($grid[$r][$c] -eq '1') {
    [pscustomobject]@{ X = $x0 + $c * ($cell + $gap); Y = $y0 + $r * ($cell + $gap); Spark = ($r -eq $spark[0] -and $c -eq $spark[1]) } } } }
}

# --- SVG master -----------------------------------------------------------------------------
$svg = New-Object System.Text.StringBuilder
[void]$svg.AppendLine('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512">')
[void]$svg.AppendLine('  <title>Pixelwright</title>')
[void]$svg.AppendLine("  <defs><linearGradient id=`"bg`" x1=`"0`" y1=`"0`" x2=`"1`" y2=`"1`"><stop offset=`"0`" stop-color=`"$ink`"/><stop offset=`"1`" stop-color=`"$blue`"/></linearGradient>")
[void]$svg.AppendLine('  <linearGradient id="sheen" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity="0.16"/><stop offset="0.55" stop-color="#fff" stop-opacity="0"/></linearGradient></defs>')
[void]$svg.AppendLine('  <rect width="512" height="512" rx="112" fill="url(#bg)"/>')
[void]$svg.AppendLine('  <rect width="512" height="512" rx="112" fill="url(#sheen)"/>')
foreach ($c in Get-Cells) { [void]$svg.AppendLine(('  <rect x="{0}" y="{1}" width="{2}" height="{2}" rx="{3}" fill="#000" fill-opacity="0.28"/>' -f $c.X, ($c.Y + 7), $cell, $radius)) }
foreach ($c in Get-Cells) { $f = if ($c.Spark) { $amber } else { $paper }; [void]$svg.AppendLine(('  <rect x="{0}" y="{1}" width="{2}" height="{2}" rx="{3}" fill="{4}"/>' -f $c.X, $c.Y, $cell, $radius, $f)) }
[void]$svg.AppendLine('</svg>')
$svgText = $svg.ToString()
foreach ($p in @('pixelwright.svg', 'pixelwright-small.svg', "hicolor\scalable\apps\$id.svg")) {
  $dest = Join-Path $root $p; New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
  [IO.File]::WriteAllText($dest, $svgText, [Text.UTF8Encoding]::new($false))
}

# --- raster renders -------------------------------------------------------------------------
function New-RoundedPath([single]$x, [single]$y, [single]$w, [single]$h, [single]$r) {
  $p = New-Object System.Drawing.Drawing2D.GraphicsPath; $d = 2 * $r
  $p.AddArc($x, $y, $d, $d, 180, 90); $p.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
  $p.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90); $p.AddArc($x, $y + $h - $d, $d, $d, 90, 90); $p.CloseFigure(); $p
}
function Hex([string]$h, [int]$a = 255) { [System.Drawing.Color]::FromArgb($a, [Convert]::ToInt32($h.Substring(1, 2), 16), [Convert]::ToInt32($h.Substring(3, 2), 16), [Convert]::ToInt32($h.Substring(5, 2), 16)) }

# $canvas = output size in px; $tile = size of the icon tile inside it (smaller on the macOS grid)
function Render-Icon([int]$canvas, [int]$tile) {
  $bmp = New-Object System.Drawing.Bitmap $canvas, $canvas, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = 'AntiAlias'; $g.PixelOffsetMode = 'HighQuality'; $g.InterpolationMode = 'HighQualityBicubic'; $g.Clear([System.Drawing.Color]::Transparent)
  $off = ($canvas - $tile) / 2.0; $k = $tile / 512.0
  $rect = New-Object System.Drawing.RectangleF $off, $off, $tile, $tile
  $path = New-RoundedPath $off $off $tile $tile (112 * $k)
  $bg = New-Object System.Drawing.Drawing2D.LinearGradientBrush $rect, (Hex $ink), (Hex $blue), 45.0
  $g.FillPath($bg, $path)
  $sheen = New-Object System.Drawing.Drawing2D.LinearGradientBrush $rect, (Hex '#ffffff' 41), (Hex '#ffffff' 0), 90.0
  $g.FillPath($sheen, $path)
  foreach ($c in Get-Cells) {   # shadows first, then cells
    $sp = New-RoundedPath ($off + $c.X * $k) ($off + ($c.Y + 7) * $k) ($cell * $k) ($cell * $k) ($radius * $k)
    $sb = New-Object System.Drawing.SolidBrush (Hex '#000000' 71); $g.FillPath($sb, $sp); $sb.Dispose(); $sp.Dispose() }
  foreach ($c in Get-Cells) {
    $cp = New-RoundedPath ($off + $c.X * $k) ($off + $c.Y * $k) ($cell * $k) ($cell * $k) ($radius * $k)
    $cb = New-Object System.Drawing.SolidBrush (Hex $(if ($c.Spark) { $amber } else { $paper })); $g.FillPath($cb, $cp); $cb.Dispose(); $cp.Dispose() }
  $bg.Dispose(); $sheen.Dispose(); $path.Dispose(); $g.Dispose(); $bmp
}
function Png-Bytes($bmp) { $ms = New-Object System.IO.MemoryStream; $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png); $b = $ms.ToArray(); $ms.Dispose(); $b }
function Render-Bytes([int]$canvas, [int]$tile) { $b = Render-Icon $canvas $tile; $x = Png-Bytes $b; $b.Dispose(); , $x }

# Linux hicolor + Windows: the tile fills the canvas. macOS: Apple's 824/1024 grid.
foreach ($n in 16, 24, 32, 48, 64, 128, 256, 512) {
  $dir = Join-Path $root "hicolor\${n}x${n}\apps"; New-Item -ItemType Directory -Force $dir | Out-Null
  [IO.File]::WriteAllBytes((Join-Path $dir "$id.png"), (Render-Bytes $n $n))
}
$mac = @{}; foreach ($n in 64, 128, 256, 512, 1024) { $mac[$n] = Render-Bytes $n ([int][Math]::Round($n * 824 / 1024)) }
[IO.File]::WriteAllBytes((Join-Path $root 'pixelwright-1024.png'), $mac[1024])

# --- .ico (PNG-compressed entries) ----------------------------------------------------------
$sizes = 16, 24, 32, 48, 64, 128, 256
$blobs = New-Object System.Collections.Generic.List[byte[]]; foreach ($n in $sizes) { $blobs.Add([byte[]](Render-Bytes $n $n)) }
$ms = New-Object System.IO.MemoryStream; $w = New-Object System.IO.BinaryWriter $ms
$w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
  $n = $sizes[$i]; $w.Write([byte]$(if ($n -ge 256) { 0 } else { $n })); $w.Write([byte]$(if ($n -ge 256) { 0 } else { $n }))
  $w.Write([byte]0); $w.Write([byte]0); $w.Write([uint16]1); $w.Write([uint16]32); $w.Write([uint32]$blobs[$i].Length); $w.Write([uint32]$offset); $offset += $blobs[$i].Length }
foreach ($b in $blobs) { $w.Write([byte[]]$b) }
$w.Flush(); [IO.File]::WriteAllBytes((Join-Path $root 'pixelwright.ico'), $ms.ToArray()); $w.Dispose()

# --- .icns (PNG chunks; big-endian lengths) -------------------------------------------------
function BE([int]$v) { [byte[]]@((($v -shr 24) -band 255), (($v -shr 16) -band 255), (($v -shr 8) -band 255), ($v -band 255)) }
$chunks = @( @('ic07', 128), @('ic08', 256), @('ic09', 512), @('ic10', 1024), @('ic11', 64), @('ic12', 128), @('ic13', 256), @('ic14', 512) )
$body = New-Object System.IO.MemoryStream
foreach ($c in $chunks) { $data = $mac[[int]$c[1]]; $t = [Text.Encoding]::ASCII.GetBytes($c[0]); $body.Write($t, 0, 4); $l = BE ($data.Length + 8); $body.Write($l, 0, 4); $body.Write($data, 0, $data.Length) }
$icns = New-Object System.IO.MemoryStream; $h = [Text.Encoding]::ASCII.GetBytes('icns'); $icns.Write($h, 0, 4); $l = BE ($body.Length + 8); $icns.Write($l, 0, 4)
$bb = $body.ToArray(); $icns.Write($bb, 0, $bb.Length); [IO.File]::WriteAllBytes((Join-Path $root 'pixelwright.icns'), $icns.ToArray())
'icon set written to ' + $root
