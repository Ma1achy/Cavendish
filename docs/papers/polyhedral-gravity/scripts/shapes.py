"""The shapes study's renders for the paper.

    python scripts/shapes.py          # called by `make.py shapes`

Crops the study's rendered `<shape>-64-cutaway.png` (docs/research/exact-skin-and-partial-fill/
shapes/, drawn by its `figures.py`) to its panels and writes figures/shapes-<shape>.png. Draft 4's
measured shapes tables and figures use the real interferometer read and come from
scripts/realread.py; draft 3's leading-read versions, drawn here before, are retired.
"""

from __future__ import annotations

import pathlib

import numpy as np
from PIL import Image

HERE = pathlib.Path(__file__).resolve().parents[1]
REPO = HERE.parents[2]
STUDY = REPO / "docs" / "research" / "exact-skin-and-partial-fill" / "shapes"
FIG = HERE / "figures"
RENDERS = ("torus", "bunny")
RENDER_DPI = 150  # the renders' resolution at print size
RENDER_ACROSS = 64  # the renders' fine lattice, cells across (the study's own are 128, as measured)


def crop_render(shape: str) -> None:
    """The study's render without its caption (the paper gives its own)."""
    img = np.asarray(Image.open(STUDY / f"{shape}-{RENDER_ACROSS}-cutaway.png").convert("RGB")).astype(float)
    busy = np.abs(img - img[0, 0]).sum(axis=2).max(axis=1) > 30
    rows = np.nonzero(busy)[0]
    # From the bottom: the caption block (lines a few pixels apart), then the first blank run of
    # at least 15 rows, above which the panels end.
    k = rows[-1]
    while k > 0:
        if not busy[k]:
            run = k
            while run > 0 and not busy[run]:
                run -= 1
            if k - run >= 15:
                k = run
                break
            k = run
        else:
            k -= 1
    if k <= rows[0]:  # no clear gap: keep everything
        k = rows[-1]
    top, bottom = max(rows[0] - 6, 0), min(k + 8, len(img))
    out = Image.fromarray(img[top:bottom].astype(np.uint8))
    # Printed at \linewidth (A4, 2.2 cm margins: 16.6 cm), rasterised at RENDER_DPI.
    width = round(16.6 / 2.54 * RENDER_DPI)
    if out.width > width:
        out = out.resize((width, round(out.height * width / out.width)), Image.Resampling.LANCZOS)
    out.save(FIG / f"shapes-{shape}.png", optimize=True, dpi=(RENDER_DPI, RENDER_DPI))


def main() -> None:
    FIG.mkdir(exist_ok=True)
    for s in RENDERS:
        crop_render(s)


if __name__ == "__main__":
    main()
