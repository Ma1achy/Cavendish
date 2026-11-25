# Rebuilds every Cavendish brand asset: the same steps as build_all.sh, for Windows PowerShell 5.1 or later.
# Needs Python 3 with numpy, scipy, matplotlib, Pillow and fontTools, and rsvg-convert (librsvg); see README.md.
#   powershell -NoProfile -ExecutionPolicy Bypass -File brand\build_all.ps1 [-Python C:\Python314\python.exe]
param([string]$Python = $env:PYTHON)
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
# MSYS2's librsvg, if it is installed but not on PATH.
if (-not (Get-Command rsvg-convert -ErrorAction SilentlyContinue) -and (Test-Path 'C:\msys64\ucrt64\bin\rsvg-convert.exe')) {
    $env:PATH = "C:\msys64\ucrt64\bin;$env:PATH"
}
if (-not (Get-Command rsvg-convert -ErrorAction SilentlyContinue)) { throw 'rsvg-convert (librsvg) is not on PATH; see README.md' }
# `python3` on Windows is often the Microsoft Store stub, which runs nothing: take the first that works.
if (-not $Python) {
    foreach ($c in 'python', 'python3', 'py') {
        if (Get-Command $c -ErrorAction SilentlyContinue) {
            $ErrorActionPreference = 'Continue'
            & $c -c 'import numpy, scipy, matplotlib, PIL, fontTools' 2>$null
            $ErrorActionPreference = 'Stop'
            if ($LASTEXITCODE -eq 0) { $Python = $c; break }
        }
    }
}
if (-not $Python) { throw 'no Python with numpy, scipy, matplotlib, Pillow and fontTools found (pass -Python)' }
$env:PYTHONUTF8 = '1'; $env:MPLBACKEND = 'Agg'
foreach ($d in 'out', 'out6', 'svg', 'png', 'colour', 'icon', 'titles', 'banners', 'work') { New-Item -ItemType Directory -Force $d | Out-Null }

function Invoke-Step {
    # One script, timed; a failure stops the build.
    $watch = [Diagnostics.Stopwatch]::StartNew()
    & $Python @args
    if ($LASTEXITCODE -ne 0) { throw "$($args -join ' ') failed with exit code $LASTEXITCODE" }
    '{0,7:n1} s  {1}' -f $watch.Elapsed.TotalSeconds, ($args -join ' ')
}

# 1. Sims: the banners' convection run (made once, about a minute) and the periodic animated smoke.
if (-not (Test-Path 'work\plume.npz')) { Invoke-Step plume.py }
Invoke-Step periodic.py
# 2. Static marks into out6/
Invoke-Step room4.py
# 3. Icon set and README title lockups
Invoke-Step make_icons.py out6 icon
Invoke-Step make_lockup.py icon titles
# 4. Animations: 48 frames for the four marks, then GIF/WebP/APNG
Invoke-Step render.py bw-dark bw-light magma-dark tertiary-b-light
Invoke-Step make_wordmark.py
Invoke-Step make_gifs.py
Invoke-Step make_colour.py
# 5. Banners, light and dark
foreach ($mode in 'dark', 'light') {
    Invoke-Step hero3.py $mode "banners/hero-$mode.jpg"
    Invoke-Step octree2.py $mode "banners/banner-octree-$mode.jpg"
    Invoke-Step render_plume.py $mode "banners/footer-$mode.jpg"
}
'done'
