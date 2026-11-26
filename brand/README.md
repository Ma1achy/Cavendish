# Cavendish brand art: the source

The scripts, fonts and data that make the icon, the README title, the banners and the animations in
`docs/readme/` and `crates/cav-app/` (the owner's brand bundle v2, its `source/` folder).

- `BUILD.md`: the pipeline, script by script, the data in `work/`, and what was changed since the bundle.
- `build_all.sh`, `build_all.ps1`: rebuild everything into this folder (ignored by git).
- `install_into_repo.py`: compare a rebuild with the shipped art, and copy it into place with `--write`.
- `fonts/`: TeX Gyre Heros, under the GUST Font License (`fonts/README.md`).
- `canvas-builders/`: the scripts that laid out the design canvases; a record, not something to rerun.

The shipped art is the owner's delivery. A rebuild replaces it only when someone runs
`install_into_repo.py --write` on purpose.

## Regenerating on Windows

Measured on the owner's PC: Windows 10, Python 3.14.3 (numpy 2.4.3, scipy 1.18.1, matplotlib 3.10.8,
Pillow 12.1.1, fontTools 4.62.1), librsvg 2.61.3 from MSYS2.

### Set up, once

```powershell
# Python packages (for the user, no admin)
python -m pip install --user numpy scipy matplotlib Pillow fontTools
# librsvg, for rsvg-convert, from MSYS2 (C:\msys64; about 190 MB with its dependencies)
C:\msys64\usr\bin\pacman.exe -S --noconfirm --needed mingw-w64-ucrt-x86_64-librsvg
```

`rsvg-convert.exe` lands in `C:\msys64\ucrt64\bin`. `build_all.ps1` adds that folder to its own `PATH`
when `rsvg-convert` is not already found; for `build_all.sh`, put it on `PATH` yourself.
`python3` on Windows is usually the Microsoft Store stub, so both drivers look for an interpreter that can
import the packages (`python3`, `python`, `py`); `PYTHON=...` or `-Python ...` chooses one.

### Build

```powershell
# from the repository root; either one
powershell -NoProfile -ExecutionPolicy Bypass -File brand\build_all.ps1
& 'C:\Program Files\Git\bin\bash.exe' brand/build_all.sh
```

To keep the desktop usable while it runs, start it below normal priority:

```powershell
$p = Start-Process powershell -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','brand\build_all.ps1' -NoNewWindow -PassThru
$p.PriorityClass = 'BelowNormal'; $p.WaitForExit()
```

Then compare with the shipped art, and install only if that is what you mean to do:

```powershell
python brand\install_into_repo.py            # the table below; exit code 1 if any pair fails
python brand\install_into_repo.py --write    # copy the rebuild over docs/readme and crates/cav-app
```

The build writes about 190 MB into this folder (`svg/` and `png/` hold 192 frames each). Delete `out*/`,
`svg/`, `png/`, `colour/`, `icon/`, `titles/`, `banners/` and `*.gif` here when done; git ignores them.

### Timings

One process at a time, below normal priority.

| Run | Wall time |
|---|---|
| `build_all.sh` under Git Bash, from clean outputs, `work/plume.npz` present | 4 min 40 s |
| `build_all.ps1`, also regenerating the plume sim, while another build was using the machine | 15 min 51 s |

Per step, from the second run (the busy machine, so these are upper bounds; without `plume.py` they add up
to 13 min 23 s against the first run's 4 min 40 s): `plume.py` 145 s, `periodic.py` 3 s, `room4.py` 9 s, `make_icons.py` 11 s, `make_lockup.py` 104 s (7 s when
the machine was idle), `render.py` 275 s, `make_wordmark.py` 3 s, `make_gifs.py` 41 s, `make_colour.py`
193 s, the six banners 8 to 47 s each. The history sims: `sim2.py` 5 s, `simloop.py` 10 s, `field.py` 5 s.

### Does a rebuild give the shipped art?

The criteria, written before anything was compared:

1. Sims (seeded): `periodic.py` and `plume.py` give arrays bit-identical to the bundle's. If not, report the
   difference; the art then passes only when built from the stored arrays.
2. SVG: byte-identical to the shipped file.
3. PNG and ICO (rendered by rsvg): same pixel size; mean absolute difference at most 1.0 of 255 levels,
   maximum reported. A different librsvg may anti-alias and blur differently.
4. GIF, WebP and APNG: same size, frame count and frame duration; the worst frame's mean absolute
   difference at most 2.0 levels.
5. JPEG banners: same size; mean absolute difference at most 1.0 levels, built from the same plume data.

What was measured (`python brand/install_into_repo.py`, 93 pairs, all pass; the same from both drivers):

| Shipped (`docs/readme/` unless said) | Rebuilt (`brand/`) | Difference |
|---|---|---|
| `title-light.svg`, `title-dark.svg`, `title-bw-light.svg`, `title-bw-dark.svg` | `titles/` (same names) | identical bytes |
| `icon/cavendish-icon-{dark,light,bw-dark,bw-light}{,-transparent}.svg` (8) | `icon/` (same names) | identical bytes |
| `icon/favicon.svg`, `crates/cav-app/web/favicon.svg` | `icon/favicon.svg` | identical bytes |
| `icon/png/cavendish-icon-*-{16…1024}.png` (40) | `icon/png/` (same names) | 5 identical bytes; 35 the same pixels (max 0, mean 0) in a differently compressed PNG |
| `icon/apple-touch-icon.png`, `crates/cav-app/web/apple-touch-icon.png` | `icon/apple-touch-icon.png` | max 0, mean 0 |
| `icon/favicon.ico`, `crates/cav-app/web/favicon.ico` | `icon/favicon.ico` | 5 sizes, max 0, mean 0 |
| `crates/cav-app/web/icon-192.png`, `icon-512.png` | `icon/png/cavendish-icon-dark-192.png`, `-512.png` | max 0, mean 0 |
| `crates/cav-app/assets/icon-256.png` | `icon/png/cavendish-icon-dark-256.png` | max 0, mean 0 |
| `title-anim-light.gif`, `title-anim-dark.gif` | `title-anim-*.gif` | identical bytes (48 frames, 80 ms) |
| `icon/anim-bw/cavendish-icon-anim-{dark,light}{,-512}.gif` (4) | `cavendish-icon-anim-*.gif` | identical bytes |
| `icon/anim-colour/*.gif` (8), `*.webp` (4) | `colour/` (same names) | identical bytes |
| `icon/anim-colour/*.png` (8, APNG) | `colour/` (same names) | 48 frames each, worst frame max 0, mean 0 |
| `banner-octree-light.jpg`, `banner-octree-dark.jpg` | `banners/` (same names) | identical bytes |
| `hero-light.jpg`, `hero-dark.jpg` | `banners/` (same names) | max 7, mean 0.001 |
| `footer-light.jpg`, `footer-dark.jpg` | `banners/` (same names) | max 10 and 9, mean 0.001 |

Against the criteria:

- **1 fails as written, and the fallback holds.** `periodic.py` is not bit-identical but agrees to 5e-15,
  and every animation built from it is identical to the shipped one. `plume.py` does not reproduce: it is
  a chaotic sim, and on this PC it gives a different field (dye up to 2.9 apart on a 3.7 range). Banners
  drawn from that fresh run are different pictures (mean difference 12 to 17 levels, maximum about 200).
  The shipped banners came from the stored run, so its dye field is committed as `work/plume.npz`, and from
  it the banners pass criterion 5.
- **2 passes** after one change to `room4.py` (the filter-id counter, `BUILD.md`). Before it, the eleven
  icon and title SVGs differed from the shipped ones in the ids `fl24`/`fl32`/`fl33`/`fl40` against `fl1`,
  and in nothing else.
- **3, 4 and 5 pass** with room to spare: every raster rsvg draws has the same pixels as the shipped one.

What cannot be regenerated: the fresh plume field (above); `work/crop_a.npy` (a hand-made crop, committed);
and the files no script makes: `signature-*.svg`, `social-preview.jpg` and `site.webmanifest`.
The history scripts (`title.py`, `mark.py`, `room.py`, …) were not run; `sim2.py`, `simloop.py` and
`field.py` run here, and the two chaotic ones also give fields different from the bundle's.
