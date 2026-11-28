"""The benchmark of the paper's Table 2 with the tower standing beside each body (draft 4).

    python conv_level.py run <body,...> <seed,...>   # data/level/conv-<body>-s<seed>.json
    python conv_level.py table                       # data/level/conv.json

This is `docs/research/paper-real-read/conv_real.py` (the real flight-integral read and the leading
read of the same evaluation) with one change, the tower's geometry. There the lower launch point sat
1.5 m (10 m) from the body's centre along three directions, one of which points 60 degrees down, so
that tower's arms rose past the body at 0.19-0.26 m from its surface. Here each of the three
directions is projected onto the horizontal: the lower launch point is level with the body's centre,
1.5 m (10 m) from it horizontally, and both interferometers' arms rise from there. Every arm point is
then at least 1.5 m from the centre, more than twice the largest circumradius of the five bodies (0.71 m, the plate), so
the arms stay outside the sphere that holds the body and its multipole series converges along every
arm. Bodies, placements (rng(1000 n + seed)), clipping, representations and kernels are conv_real's,
imported unchanged.
"""

from __future__ import annotations

import pathlib
import sys

import numpy as np

HERE = pathlib.Path(__file__).resolve().parent
REAL = HERE.parents[2] / "research" / "paper-real-read"
sys.path.insert(0, str(REAL))
import conv_real as C  # noqa: E402

OUT = HERE.parent / "data" / "level"


def lowers() -> np.ndarray:
    """[6, 3], distance-major: the benchmark's three directions made horizontal, at 1.5 and 10 m."""
    flat = C.sb.DIRECTIONS * np.array([1.0, 1.0, 0.0])
    flat /= np.linalg.norm(flat, axis = 1, keepdims = True)
    return np.array([d * u for d in C.DIST for u in flat])


C.lowers = lowers
C.OUT = OUT

if __name__ == "__main__":
    OUT.mkdir(parents = True, exist_ok = True)
    if sys.argv[1] == "run":
        for nm in sys.argv[2].split(","):
            for s in sys.argv[3].split(","):
                C.run(nm, int(s))
    else:
        C.table()
