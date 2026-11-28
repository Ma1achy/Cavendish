"""The paper's plot style (docs/papers/polyhedral-gravity/make.py rcParams; Okabe–Ito colours as in
its scripts/diagrams.py)."""

from __future__ import annotations

import matplotlib

matplotlib.use("Agg")
import matplotlib as mpl  # noqa: E402

BLACK, ORANGE, SKY, GREEN, YELLOW, BLUE, VERMILION, PURPLE = (
    "#000000", "#E69F00", "#56B4E9", "#009E73", "#F0E442", "#0072B2", "#D55E00", "#CC79A7")
SERIES = [BLUE, ORANGE, GREEN, VERMILION, PURPLE, SKY, BLACK]
INK, INK2, GRID = "#0b0b0b", "#52514e", "#d9d8d4"
MARKERS = ["o", "s", "^", "D", "v", "P", "X"]


def apply() -> None:
    mpl.rcParams.update({
        "font.family": "serif", "font.size": 8, "axes.edgecolor": INK2, "axes.labelcolor": INK,
        "xtick.color": INK2, "ytick.color": INK2, "axes.grid": True, "grid.color": GRID,
        "grid.linewidth": 0.5, "axes.spines.top": False, "axes.spines.right": False,
        "lines.linewidth": 1.4, "lines.markersize": 4, "legend.frameon": False,
        "savefig.bbox": "tight", "pdf.fonttype": 42,
    })
