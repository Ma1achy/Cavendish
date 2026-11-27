"""Shared set-up for the box-coalescing and precision studies.

Imports the shapes study (`../exact-skin-and-partial-fill/shapes/shapes_lib.py`) unchanged, so the
meshes, lattice placement (`GENERIC`), tower and phase are exactly the paper's. Lowers this process to
below-normal priority and caps numeric libraries at four threads (the owner's build-PC rule).

Units: metres, m³ for volumes; every g is per unit G·ρ (m) until `phase` turns it into radians.
"""

from __future__ import annotations

import ctypes
import os
import sys
from pathlib import Path

for _v in ("OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "NUMEXPR_NUM_THREADS"):
    os.environ.setdefault(_v, "4")

if sys.platform == "win32":  # BELOW_NORMAL_PRIORITY_CLASS, this process only
    ctypes.windll.kernel32.SetPriorityClass(ctypes.windll.kernel32.GetCurrentProcess(), 0x4000)

HERE = Path(__file__).resolve().parent
STUDY = HERE.parent / "exact-skin-and-partial-fill" / "shapes"
DATA = HERE / "data"
FIG = HERE / "figures"
DATA.mkdir(exist_ok=True)
FIG.mkdir(exist_ok=True)
sys.path.insert(0, str(STUDY))

import shapes_lib as L  # noqa: E402

SHAPES = ["sphere", "torus", "teapot", "suzanne", "bunny", "spot", "dragon", "armadillo", "buddha",
          "bracket"]
NAME = {"sphere": "sphere", "torus": "torus", "teapot": "teapot", "suzanne": "Suzanne",
        "bunny": "bunny", "spot": "Spot", "dragon": "dragon", "armadillo": "Armadillo",
        "buddha": "Buddha", "bracket": "mk7 part"}
# Okabe–Ito, as the paper's diagrams.py
BLUE, ORANGE, GREEN, VERMILION, SKY, YELLOW, PURPLE = (
    "#0072B2", "#E69F00", "#009E73", "#D55E00", "#56B4E9", "#F0E442", "#CC79A7")
INK, INK2, GRID = "#0b0b0b", "#52514e", "#d9d8d4"


def style() -> None:
    """The paper's plot style (scripts/shapes.py)."""
    import matplotlib

    matplotlib.use("Agg")
    matplotlib.rcParams.update({
        "font.family": "serif", "font.size": 7, "axes.edgecolor": INK2, "axes.labelcolor": INK,
        "xtick.color": INK2, "ytick.color": INK2, "axes.grid": True, "grid.color": GRID,
        "grid.linewidth": 0.5, "axes.spines.top": False, "axes.spines.right": False,
        "lines.linewidth": 1.1, "lines.markersize": 2.8, "legend.frameon": False,
        "savefig.bbox": "tight", "pdf.fonttype": 42, "axes.titlesize": 7,
        "xtick.labelsize": 6, "ytick.labelsize": 6,
    })
