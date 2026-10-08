# Renders the README / website screenshots with the offscreen `snapshot` example (no window, no
# focus stealing) on public-domain artwork, and saves them as JPEGs in docs/images/.
#
#   powershell -ExecutionPolicy Bypass -File scripts/readme-shots.ps1 -Art C:\path\to\artwork
#
# -Art is a folder holding the public-domain images named in docs/images/SOURCES.md
# (great-wave.jpg, starry-night.jpg, pearl-earring.jpg ...). The 1500 px copies in
# apps/pixelwright-web/samples/ are enough, and fit the CPU canvas path's 2048 px texture limit.
# Each scene is a list of control-protocol calls (docs/control-protocol.md) run after opening a file.
param(
  [Parameter(Mandatory)] [string]$Art,
  [string]$Out = '',
  [double]$Scale = 1.5,
  [string[]]$Only = @()
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not $Out) { $Out = Join-Path $here '..\docs\images' }
$root = Split-Path $here -Parent
Push-Location $root
try {
  cargo build -p pixelwright-ui-egui --example snapshot
  $exe = (Get-ChildItem target\debug\examples\snapshot-*.exe | Where-Object { $_.Extension -eq '.exe' } | Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
} finally { Pop-Location }
$tmp = Join-Path ([IO.Path]::GetTempPath()) 'pixelwright-shots'; New-Item -ItemType Directory -Force $tmp | Out-Null
$jpeg = [System.Drawing.Imaging.ImageCodecInfo]::GetImageEncoders() | Where-Object { $_.MimeType -eq 'image/jpeg' }
$enc = New-Object System.Drawing.Imaging.EncoderParameters 1
$enc.Param[0] = New-Object System.Drawing.Imaging.EncoderParameter ([System.Drawing.Imaging.Encoder]::Quality), 86L

function Shot([string]$name, [string]$open, [string]$script, [string]$size = '1440x900', [int[]]$crop = @()) {
  if ($Only.Count -gt 0 -and $Only -notcontains $name) { return }
  $png = Join-Path $tmp "$name.png"
  # PowerShell 5.1 strips the quotes of a JSON argument unless they are escaped
  # --safe-gpu: the CPU canvas, so a software GPU adapter (WARP, llvmpipe) cannot time out on layer effects
  & $exe --safe-gpu --out $png --size $size --scale $Scale --open (Join-Path $Art $open) --script ($script.Replace('"', '\"')) | Out-Null
  $img = [System.Drawing.Image]::FromFile($png)
  try {
    $bmp = $img
    if ($crop.Count -eq 4) { $r = New-Object System.Drawing.Rectangle ([int]($crop[0]*$Scale)), ([int]($crop[1]*$Scale)), ([int]($crop[2]*$Scale)), ([int]($crop[3]*$Scale)); $bmp = ([System.Drawing.Bitmap]$img).Clone($r, $img.PixelFormat) }
    $dest = Join-Path $Out "pixelwright-$name.jpg"
    $bmp.Save($dest, $jpeg, $enc)
    "{0,-34} {1,6:N0} KB" -f (Split-Path $dest -Leaf), ((Get-Item $dest).Length / 1KB)
  } finally { $img.Dispose() }
}

$wave = 'great-wave.jpg'
# 1. The hero: type with a drop shadow, a Vibrance layer, and a palette taken from the image.
Shot 'demo' $wave @'
[["ui.set",{"dockTabs":{"color":1}}],
 ["engine.execute",{"command":"layer.newAdjustmentLayer.vibrance","params":{"vibrance":30,"saturation":6}}],
 ["engine.execute",{"command":"swatch.presets.fromImage","params":{"count":10}}],
 ["engine.execute",{"command":"type.create","params":{"text":"THE GREAT WAVE","x":70,"y":940,"size":92,"color":"#ffffff"}}],
 ["engine.execute",{"command":"layer.layerStyle.dropShadow","params":{"opacity":75,"distance":9,"size":11,"angle":120}}],
 ["ui.set",{"fit":true}]]
'@

# 2. Filter > Pixelate > Pixel Art on a 1200 px version of the image.
Shot 'pixel-art' $wave @'
[["engine.execute",{"command":"image.imageSize","params":{"width":1200}}],
 ["engine.execute",{"command":"filter.pixelate.pixelArt","params":{"cellSize":6,"colors":16,"dither":"pattern"}}],
 ["ui.set",{"fit":true}]]
'@

# 3. The Swatches panel, cropped, beside the palette it was built from.
Shot 'swatches' 'starry-night.jpg' @'
[["ui.set",{"dockTabs":{"color":1}}],
 ["engine.execute",{"command":"swatch.presets.edit","params":{"action":"deleteGroup","group":"Default"}}],
 ["engine.execute",{"command":"swatch.presets.fromImage","params":{"count":12,"sort":"hue","name":"Palette of The Starry Night"}}]]
'@ '1440x900' @(1150, 66, 290, 135)

# 4. Frequency Separation and a Dodge & Burn layer on a portrait; the Layers panel tells the story.
Shot 'retouch' 'pearl-earring.jpg' @'
[["engine.execute",{"command":"layer.frequencySeparation","params":{"radius":5}}],
 ["engine.execute",{"command":"layer.newDodgeBurn","params":{}}]]
'@
