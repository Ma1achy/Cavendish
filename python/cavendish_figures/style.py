"""
The house style for matplotlib: the same palette, dashes and sizes as the figures Cavendish
writes (``crates/cav-plot/src/style.rs``).

    sheets("paper-column") -> Ok((cavendish.mplstyle, paper-column.mplstyle))
                           -> matplotlib.style.context([...])

The look is in style sheets beside this module: ``cavendish.mplstyle`` (white paper, near-black
ink, grey rules, the eight series colours with a dash each) and one small sheet for each light
preset (its size and its type). Use them on their own with matplotlib:

    import matplotlib.style
    from cavendish_figures import sheets

    with matplotlib.style.context([str(sheet) for sheet in sheets("paper-full").unwrap()]):
        ...

The series colours are safe under protanopia, deuteranopia and tritanopia (every pair at least
12 apart in CIEDE2000 and in OKLab × 100) and each has a dash of its own for print in black and
white; Cavendish's own tests hold the palette to that, and this package's tests hold these
constants to the palette a figure's ``.json`` records.
"""

from __future__ import annotations

from difflib import get_close_matches
from pathlib import Path

from .errors import FigureError, UnknownPreset
from .result import Err, Ok, Result

# ======================================== THE PALETTE ========================================

SERIES: tuple[str, ...] = (
    "#900c60", "#2c84b2", "#846000", "#a878fc", "#3a469e", "#8200f2", "#ca4076", "#c08400",
)
"""The series colours, in order: wine, blue, ochre, lavender, indigo, violet, rose, gold (Cavendish's ``PAPER_SERIES``)."""

DASHES: tuple[tuple[float, ...], ...] = (
    (),
    (6.0, 2.5),
    (1.5, 2.0),
    (7.0, 2.0, 1.5, 2.0),
    (3.5, 3.5),
    (10.0, 2.5),
    (7.0, 2.0, 1.5, 2.0, 1.5, 2.0),
    (3.0, 1.5),
)
"""Each series' dash, in units of the line's width: ink, gap, ink, gap; empty for solid."""

INK:       str = "#111111"
RULE:      str = "#cfcfcf"
SPECTRUM:  str = "#6e6e6e"
REFERENCE: str = "#8c8c8c"

# ======================================== THE PRESETS ========================================

PRESETS: dict[str, tuple[float, float]] = {
    "paper-column": (85.0, 60.0),
    "paper-full":   (178.0, 90.0),
    "slide":        (254.0, 143.0),
    "poster":       (400.0, 250.0),
}
"""The light presets and the width and height of each, millimetres."""

_SHEETS: Path = Path(__file__).parent / "styles"


def sheets(preset: str) -> Result[tuple[Path, Path], FigureError]:
    """
    The style sheets of a preset: the house style, then the preset's own.

    The ``screen`` preset is the application's dark theme and has no sheet here: a figure in
    it is redrawn as ``paper-full``.

    Args:
        preset (str): ``paper-column``, ``paper-full``, ``slide`` or ``poster``.
    Returns:
        Result[tuple[Path, Path], FigureError]: The two ``.mplstyle`` files, in the order to
            apply them; ``UnknownPreset`` with the nearest name for any other.
    """
    if preset not in PRESETS:
        names   = tuple(PRESETS)
        nearest = get_close_matches(preset, names, n = 1, cutoff = 0.0)
        return Err(UnknownPreset(name = preset, nearest = nearest[0], others = names))
    return Ok((_SHEETS / "cavendish.mplstyle", _SHEETS / f"{preset}.mplstyle"))


def dash(series: int, width: float) -> tuple[float, tuple[float, ...]] | str:
    """
    The line style of a series, as matplotlib takes it.

    Args:
        series (int):   The series, from 0; past the last colour the dashes go on a step.
        width  (float): The line's width, points: dashes are multiples of it.
    Returns:
        tuple[float, tuple[float, ...]] | str: ``"solid"``, or ``(0, (ink, gap, ...))`` in points.
    """
    pattern = DASHES[(series + series // len(SERIES)) % len(DASHES)]
    if not pattern:
        return "solid"
    return (0.0, tuple(length * width for length in pattern))
