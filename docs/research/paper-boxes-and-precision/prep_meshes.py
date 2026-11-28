"""Download, check and repair the ten meshes into the study's git-ignored cache (shapes/.cache/).

    python prep_meshes.py
"""

from __future__ import annotations

import time

import common as C

for name in C.SHAPES:
    t0 = time.perf_counter()
    m = C.L.mesh_by_name(name)
    print(name, len(m.faces), f"{time.perf_counter() - t0:.0f} s", flush=True)
