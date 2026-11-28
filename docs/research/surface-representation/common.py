"""Shared set-up for the surface-representation study.

Imports the shapes study (`../exact-skin-and-partial-fill/shapes/shapes_lib.py`) and the flight read
(`../paper-flight-phase/flight.py`) unchanged, so the meshes, lattice placement, tower directions and
phase are the earlier studies' own. Lowers this process to below-normal priority and caps numeric
libraries at four threads (the owner's build-PC rule).

Units: metres; potentials V/G in kg/m (negative, the repository's convention); phases in radians.
"""

from __future__ import annotations

import ctypes
import json
import os
import sys
from pathlib import Path

for _v in ("OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "NUMEXPR_NUM_THREADS"):
    os.environ.setdefault(_v, "4")

if sys.platform == "win32":  # BELOW_NORMAL_PRIORITY_CLASS, this process only
    ctypes.windll.kernel32.SetPriorityClass(ctypes.windll.kernel32.GetCurrentProcess(), 0x4000)

import numpy as np  # noqa: E402
import numpy.typing as npt  # noqa: E402

HERE = Path(__file__).resolve().parent
STUDY = HERE.parent / "exact-skin-and-partial-fill" / "shapes"
FLIGHT = HERE.parent / "paper-flight-phase"
DATA = HERE / "data"
WORK = HERE / ".work"  # big binary intermediates, git-ignored
FIG = HERE / "figures"
for _d in (DATA, WORK, FIG):
    _d.mkdir(exist_ok=True)
sys.path.insert(0, str(STUDY))
sys.path.insert(0, str(FLIGHT))

import flight as fl  # noqa: E402
import shapes_lib as L  # noqa: E402

F64 = npt.NDArray[np.float64]

RHO = 2700.0
SHAPES = ["sphere", "torus", "bunny", "bracket", "dragon"]
NAME = {"sphere": "sphere", "torus": "torus", "bunny": "bunny", "bracket": "CAD part (mk7)",
        "dragon": "dragon"}
# Pitches per shape, coarsest first; every finer lattice nests in the coarsest (same origin).
PITCHES = {"sphere": [1 / 4, 1 / 8, 1 / 16, 1 / 32], "torus": [1 / 4, 1 / 8, 1 / 16, 1 / 32],
           "bunny": [1 / 8, 1 / 16, 1 / 32, 1 / 64], "bracket": [1 / 8, 1 / 16, 1 / 32, 1 / 64],
           "dragon": [1 / 8, 1 / 16, 1 / 32, 1 / 64]}
DISTANCES = [0.5, 1.5, 5.0, 20.0]  # tower base from the surface, m (0.5 m: the near regime)
# The engine's rule (Gauss order 6, panels <= 0.1 s), static source.
INSTRUMENT = fl.replace(fl.AION10, order = 6, panel = 0.1)


def tower_points(base: F64) -> tuple[F64, F64]:
    """The flight read of a static body as a weighted sum of potentials: points [4N, 3] m and
    coefficients [4N] rad per (kg/m of V/G), so that phase = Σ c · V/G(points)."""
    ins = INSTRUMENT
    tau, w, z_lo, z_up = fl.nodes(ins)
    heights = np.concatenate([z_up, z_lo, ins.separation + z_up, ins.separation + z_lo])
    coef = ins.scale * np.concatenate([w, -w, -w, w])
    pts = np.repeat(base[None, :], len(heights), axis = 0)
    pts[:, 2] += heights
    return pts, coef


def towers(mesh: L.Mesh) -> list[tuple[F64, F64]]:
    """One tower per distance in `DISTANCES`, along the shapes study's direction."""
    bases = L.tower(mesh, np.array(DISTANCES))[: len(DISTANCES)]
    return [tower_points(b) for b in bases]


def save_json(name: str, obj: object) -> None:
    (DATA / name).write_text(json.dumps(obj, indent = 1))


def load_json(name: str) -> object:
    return json.loads((DATA / name).read_text())


def style() -> None:
    """The paper's plot style (as `paper-flight-phase/style.py`)."""
    sys.path.insert(0, str(FLIGHT))
    import style as st  # noqa: PLC0415

    st.apply()
