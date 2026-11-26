#!/usr/bin/env bash
# Rebuilds every Cavendish brand asset from scratch into this folder (out6/, icon/, titles/, svg/, png/,
# colour/, banners/ and the animated GIFs).  Needs: Python 3 with numpy, scipy, matplotlib, Pillow,
# fontTools; and rsvg-convert (librsvg).  Runs on Linux, macOS and Windows (Git Bash); build_all.ps1 is
# the same steps for PowerShell.  Set PYTHON to choose the interpreter.
set -euo pipefail
cd "$(dirname "$0")"
# On Windows `python3` is often the Microsoft Store stub, which runs nothing: take the first that works.
PY="${PYTHON:-}"
if [ -z "$PY" ]; then
  for c in python3 python py; do
    if "$c" -c "import numpy, scipy, matplotlib, PIL, fontTools" >/dev/null 2>&1; then PY="$c"; break; fi
  done
fi
[ -n "$PY" ] || { echo "no Python with numpy, scipy, matplotlib, Pillow and fontTools found (set PYTHON)" >&2; exit 1; }
command -v rsvg-convert >/dev/null || { echo "rsvg-convert (librsvg) is not on PATH; see README.md" >&2; exit 1; }
export PYTHONUTF8=1 MPLBACKEND=Agg
mkdir -p out out6 svg png colour icon titles banners work
# 1. Smoke for the icon walls is work/crop_a.npy (a crop of plume.py's big convection run; committed).
#    The banners read the whole run, work/plume.npz: about a minute to make, so it is made once.
#    The animated smoke is exactly periodic: periodic.py -> work/frames_periodic.npy
[ -f work/plume.npz ] || "$PY" plume.py
"$PY" periodic.py
# 2. Static marks: every room/cube variant into out6/
"$PY" room4.py
# 3. Icon set (four marks, transparent versions, favicons, PNG sizes) and README title lockups
"$PY" make_icons.py out6 icon
"$PY" make_lockup.py icon titles
# 4. Animations (48-frame seamless loops) for the four marks, then GIF/WebP/APNG
"$PY" render.py bw-dark bw-light magma-dark tertiary-b-light
"$PY" make_wordmark.py
"$PY" make_gifs.py
"$PY" make_colour.py
# 5. Banners: hero (octree + voxel terrain + tower), octree banner, footer; light and dark
"$PY" hero3.py dark banners/hero-dark.jpg;            "$PY" hero3.py light banners/hero-light.jpg
"$PY" octree2.py dark banners/banner-octree-dark.jpg; "$PY" octree2.py light banners/banner-octree-light.jpg
"$PY" render_plume.py dark banners/footer-dark.jpg;   "$PY" render_plume.py light banners/footer-light.jpg
echo "done"
