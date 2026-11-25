# Cavendish brand: source

Everything that made the icon, the README title, the banners and the animations. All paths are relative to this folder.

## Requirements

- Python 3 with numpy, scipy, matplotlib, Pillow and fontTools.
- `rsvg-convert` (librsvg) for SVG → PNG.
- Fonts: TeX Gyre Heros Bold and Regular, in `fonts/` (GUST Font License, free to redistribute; see `fonts/README.md`). These are the Helvetica-style outlines for the wordmark.

`README.md` has the Windows set-up, the measured timings and the comparison with the shipped art.

## Rebuild everything

```sh
./build_all.sh                                                           # Linux, macOS, Git Bash
powershell -NoProfile -ExecutionPolicy Bypass -File build_all.ps1        # Windows PowerShell
```

It takes about 3 minutes on the author's machine (about 5 on the owner's PC) and writes `out6/`, `icon/`, `titles/`, `svg/`, `png/`, `colour/`, `banners/` and the `title-anim-*.gif` and `cavendish-icon-anim-*.gif` files in this folder. None of that is committed (`.gitignore`). `python install_into_repo.py` then compares the build with the art the repository ships, and `--write` copies it into `docs/readme/` and `crates/cav-app/`.

## Data (`work/`)

Committed: only what the build reads and no script here can remake.

| File | Committed | Made by | Used for |
|---|---|---|---|
| `crop_a.npy` | yes (0.4 MB) | by hand: a processed 230 × 230 crop of the plume run's dye field. No script makes it. | the static icons' smoke wall |
| `plume.npz` | yes (2.0 MB): the `dye` and `GY` arrays of the run the shipped banners were drawn from, bit for bit, compressed. `T`, `u` and `v` are left out; nothing reads them. | `plume.py`, a buoyant-plume (Boussinesq) sim, 832 × 312, 700 steps with vorticity confinement | hero, banners, footer |
| `vox.pkl` | yes (6 kB) | `field.py` | the first field-line hero (`render_field.py`), kept for reference |
| `frames_periodic.npy` | no (9.8 MB): `python periodic.py`, 3 s; `build_all` runs it | `periodic.py`: domain-warped turbulence built only from modes with whole numbers of cycles per loop | the animated smoke; frame 48 equals frame 0, so the loop is seamless |
| `wm-dark.png`, `wm-light.png`, `wm.svg`, `ic.png` | no: `python make_wordmark.py`, 3 s; `build_all` runs it (`ic.png` is `make_gifs.py`'s scratch frame) | `make_wordmark.py` | the wordmark strip of the animated titles |
| `h2ink.png`, `oct.png`, `plink.png` | no: scratch layers the banner scripts write and read back | `hero3.py`, `octree2.py`, `render_plume.py` | the banners |
| `plume2.npy` | no (0.3 MB): `python sim2.py`, 5 s | `sim2.py`, a single turbulent plume | an earlier icon version; nothing in the build reads it |
| `frames.npy` | no (15 MB): `python simloop.py`, 10 s | `simloop.py`, a short convection run | the first animation attempt (cross-faded, not seamless); nothing reads it |
| `pot.npy` | no (5 MB): `python field.py`, 5 s (it also rewrites `vox.pkl`) | `field.py` | the first field-line hero; only `render_field.py` reads it |

**The sims are seeded but the turbulent ones do not reproduce across machines.** `periodic.py` is a sum of cosines and comes out the same to 5e-15 here, and the animations built from it are byte-identical to the shipped ones. `plume.py`, `sim2.py` and `simloop.py` are chaotic: a last-bit difference in the FFT or the interpolation grows over the run, and on the owner's PC (numpy 2.4, scipy 1.18) each gives a different, equally plausible field (for `plume.py` the dye differs by up to 2.9 of a 3.7 range). So `python plume.py` (145 s here) makes *new* banners, not the shipped ones; that is why the shipped run's dye field is committed. To draw from a fresh run, delete `work/plume.npz` and run the build: it calls `plume.py` when the file is missing.

## What the build does not make

- `docs/readme/signature-*.svg`: the same files as Principia's.
- `docs/readme/social-preview.jpg` (and its copy in `crates/cav-app/web/`): the dark hero under the black-and-white lockup. No script in the bundle makes it.
- `crates/cav-app/web/site.webmanifest`: written by hand.

## Changes since the bundle

The art code is the owner's bundle (v2) with these changes, none of which changes a pixel:

- Every SVG is written with `newline='\n'`, so Windows does not turn line ends into CR LF.
- `room4.py` resets the filter-id counter before each `c-` mark, as `render.py` does, so every mark's ids start at `fl1`. The shipped SVGs all start at `fl1`; without the reset a full `room4.py` run numbers them `fl24`, `fl32`, `fl33` and `fl40`, and the SVGs differ from the shipped ones in those ids only.
- `make_lockup.py` also writes `title-bw-light.svg` and `title-bw-dark.svg` (the icon in black and white, the inks of `make_wordmark.py`). The bundle shipped these files but not the two calls; with them the files come out byte for byte.
- `build_all.sh` finds a working Python (`python3` on Windows is usually the Store stub), checks for `rsvg-convert`, and runs `plume.py` only when `work/plume.npz` is missing. `build_all.ps1` is the same steps for PowerShell.

## The icon pipeline

- `vox.py`: isometric projection and the cube drawer. Each cube is three faces in top, left and right tones, each sealed with a stroke in its own colour so no seams show.
- `room4.py`: the final marks.
  - `scene(name, palette, mode)`: the room corner (`mode='centre'` is the C alone in the room, `'sketch'` the first-sketch layout).
  - `topcube(...)`: the C on top of an outside cube.
  - Walls:
    - The gravity field: `field_letter`, the C's voxels drawn in five Gaussian-blurred layers, in screen space, clipped to the wall.
    - The smoke: posterised contours of the field, 40 levels, sealed.
    - The quadtree floor: `quad_cells`, which refines under the C's footprint.
  - Lighting: per-face light overlays, ambient occlusion along interior edges, contact shadows, and sun shadows (cubes projected along the sun direction and clipped to each face).
  - `PAL` holds every palette: `magma-dark` (the dark mark), `tertiary-b-light` (the light mark), `bw-dark`, `bw-light`, and the rest of the exploration.
  - Run it to render every variant into `out6/`.
- `make_icons.py out6 icon`: the icon set (tightened viewBox, transparent versions, theme-switching `favicon.svg`, PNGs from 16 to 1024 px, `.ico`, Apple touch icon).
- `make_lockup.py icon titles`: the README title lockups (`glyphs.py` outlines the type with fontTools).

## The animation pipeline

- `render.py <palettes…>`: 48 frames per palette into `svg/` and `png/`. Over one loop:
  - **Smoke:** frame *i* of `frames_periodic.npy`.
  - **Quadtree:** depth goes 2 → 5 → 2 (`QMAXD`).
  - **Gravity field:** its blur goes soft → sharp → soft (`BLUR`) and its brightness rises with the refinement (`GLOW`). All three follow one cosine cycle, so the loop is seamless.
  - The black-and-white palettes get GitHub's page colours as background.
- `make_wordmark.py`: wordmark strips on GitHub's colours.
- `make_gifs.py`: black-and-white icon GIFs and the animated README titles.
- `make_colour.py`: colour icons as GIF (one global palette, so no flicker), WebP and APNG, plus animated favicons.

## Banners

- `hero3.py dark|light out.jpg`: plume field, octree through the air (cell-averaged towards the right), voxel terrain and ground cut-away, tower, pencil texture.
- `octree2.py dark|light out.jpg`: the quadtree banner.
- `render_plume.py dark|light out.jpg`: the footer.

## History

Earlier explorations are kept in case you want to revisit them:

- `title.py`: the first wordmark.
- `mark.py`, `cmark.py`, `hc.py`: the gradiometer glyph, the "pull" C and the Helvetica C.
- `variants.py`, `v2.py`, `planes.py`, `room.py`, `room2.py`, `outer.py`, `flatc.py`: the isometric and flat voxel Cs, the tower on another face, the rooms and the outer cube.
- `field.py`, `render_field.py`: the first field-line hero.
- `canvas-builders/`: the scripts that laid out the design canvases. They reference that session's artifact ids, so they're a record rather than something to rerun.
