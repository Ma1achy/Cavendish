"""
The five series colours of the light figure presets (``PAPER_SERIES``), and how they were found.

    python scripts/palette_five.py          (a few minutes; needs numpy; deterministic)

The criterion is S2's, as first written (``docs/slices/S2.md`` §3.3, S3's §3.2 item 1): every pair
at least 12 apart in CIEDE2000 under seven conditions (normal vision, Machado's protanopia,
deuteranopia and tritanopia, Viénot's protanopia and deuteranopia, Brettel's tritanopia); the
``dataviz`` validator's floors in OKLab × 100 (15 in normal vision, 8 under Machado's); the first
four at least 8 L* apart; 3:1 on white and on the validator's surface. Five colours, because no
eight were found at 12 (``palette_search.py``).

    blue #0b0dff (L* 33) and orange #e76f11 (L* 60), kept from the review's five
        -> a colour of L* 41.5 to 44.5 and one of 49.5 to 52.3, each hue at least 40° (OKLab)
           from every other, every candidate on a 4-byte sRGB grid that a mark may be
        -> the pair whose worst difference to each other and to blue and orange is greatest
           (of the best 400 pairs) -> the fifth colour, any lightness, that keeps it greatest

The review's own five (``#0b0dff, #e76f11, #28a682, #cb64fe, #910c59``, 17.49 at worst) cannot
be ordered so that four are 8 L* apart: three of them lie within 2 L* of 60. The search printed
``#0b0dff, #c01850, #1088a8, #e76f11, #703088``; the teal was then darkened two bytes
(``#1086a6``: L* 51.7, from 52.4, which missed the 8 L* gap to orange by 0.1). Cavendish's test
measures the result (``the_palette_survives_colour_blindness_and_print``).
"""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np
import numpy.typing as npt

sys.path.insert(0, str(Path(__file__).resolve().parent))
import palette_search as ps  # noqa: E402

Array = npt.NDArray[np.float64]

KEPT: Array = np.array([[0x0B, 0x0D, 0xFF], [0xE7, 0x6F, 0x11]], dtype = np.float64)
"""Blue and orange, kept from the review's five."""


def worst(first: Array, second: Array) -> Array:
    """The least of every measure the criterion holds a pair to, shifted so that 12 passes all."""
    a, b = ps.seen(ps.linear_of(first)), ps.seen(ps.linear_of(second))
    out  = np.full((len(first), len(second)), np.inf)
    for n, (left, right) in enumerate(zip(a, b)):
        out = np.minimum(out, ps.ciede2000(ps.lab_of(left), ps.lab_of(right)))
        if n < 4:
            ok  = np.linalg.norm(ps.oklab_of(left)[:, None, :] - ps.oklab_of(right)[None, :, :], axis = 2)
            out = np.minimum(out, ok - 3.0 if n == 0 else ok + 4.0)
    return out


def hue(rgb: Array) -> Array:
    """OKLab hue of each colour, degrees."""
    ok = ps.oklab_of(ps.linear_of(rgb))
    return np.degrees(np.arctan2(ok[:, 2], ok[:, 1])) % 360.0


def away(rgb: Array, hues: Array) -> npt.NDArray[np.bool_]:
    """Which colours are at least 40° of hue from every one of ``hues``."""
    turn = np.abs((hue(rgb)[:, None] - hues[None, :] + 180.0) % 360.0 - 180.0)
    return turn.min(axis = 1) >= 40.0


def main() -> int:
    """Runs the search and prints the five, in order."""
    kept     = hue(KEPT)
    grid     = ps.candidates(4)
    grid     = grid[away(grid, kept)]
    light    = ps.lab_of(ps.linear_of(grid))[:, 0]
    low      = grid[(light >= 41.5) & (light <= 44.5)]
    high     = grid[(light >= 49.5) & (light <= 52.5)]
    low      = low[worst(low, KEPT).min(axis = 1) >= 13.0]
    high     = high[worst(high, KEPT).min(axis = 1) >= 13.0]
    pairs    = np.minimum(np.minimum(worst(low, KEPT).min(axis = 1)[:, None], worst(high, KEPT).min(axis = 1)[None, :]), worst(low, high))
    fifth    = ps.candidates(8)
    fifth    = fifth[away(fifth, kept)]
    to_kept  = worst(fifth, KEPT).min(axis = 1)
    best: tuple[float, tuple[Array, Array, Array] | None] = (0.0, None)
    for k in np.argsort(pairs, axis = None)[::-1][:400]:
        i, j = (int(n) for n in np.unravel_index(int(k), pairs.shape))
        if not away(low[i:i + 1], hue(high[j:j + 1]))[0]:
            continue
        ok    = away(fifth, np.concatenate([kept, hue(np.array([low[i], high[j]]))]))
        score = np.where(ok, np.minimum(np.minimum(to_kept, worst(fifth, low[i:i + 1])[:, 0]), worst(fifth, high[j:j + 1])[:, 0]), -1.0)
        m     = int(np.argmax(score))
        total = min(float(pairs[i, j]), float(score[m]))
        if total > best[0]:
            best = (total, (low[i], high[j], fifth[m]))
    if best[1] is None:
        print("nothing found")
        return 1
    low_c, high_c, fifth_c = best[1]
    order = [KEPT[0], low_c, high_c, KEPT[1], fifth_c]
    print(f"worst {best[0]:.2f}:", ", ".join("#%02x%02x%02x" % tuple(int(v) for v in colour) for colour in order))
    return 0


if __name__ == "__main__":
    sys.exit(main())
