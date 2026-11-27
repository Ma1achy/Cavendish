"""Criterion T: cost per field point per triangle (polyhedra) or per prism (voxels), one thread.

    KERNEL_BENCH=<path to kernel_bench.exe> .venv-bench/Scripts/python bench_timing.py

256 field points 1.5–3 m from the body's centre, uniformly in direction. Best of five runs, after
a warm-up (Numba compiles Harmonica's kernels on first call). What each code computes per call
differs, and is recorded with the numbers:
  - Cavendish `Faceted::field`: U, g and the solid-angle sum;
  - ESA `GravityEvaluable` (precomputed per-face data): U, g and the full tensor (10 numbers);
  - the study's numpy: U and g, vectorised over triangles × points;
  - Harmonica `prism_gravity`: one field per call (U, g_e, g_n, g_z are four calls);
  - Cavendish `cube_potential` (U only) and `prism_potential` (Nagy, U only).
Writes `timing.json`.
"""

from __future__ import annotations

import json
import os
import platform
import time
import winreg

import numpy as np

import common as c

REPEATS = 5


def cpu_name() -> str:
    key = winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r"HARDWARE\DESCRIPTION\System\CentralProcessor\0")
    return str(winreg.QueryValueEx(key, "ProcessorNameString")[0]).strip()


def best(fn: object, reps: int = REPEATS) -> float:
    assert callable(fn)
    fn()
    times = []
    for _ in range(reps):
        t0 = time.perf_counter()
        fn()
        times.append(time.perf_counter() - t0)
    return min(times)


def main() -> None:
    import harmonica as hm
    import polyhedral_gravity as pg

    rng = np.random.default_rng(11)
    d = rng.normal(size = (256, 3))
    pts = d / np.linalg.norm(d, axis = 1)[:, None] * rng.uniform(1.5, 3.0, size = (256, 1))
    every = c.bodies()
    out: dict[str, object] = {"cpu": cpu_name(), "python": platform.python_version(),
                              "numba_threads": os.environ.get("NUMBA_NUM_THREADS"), "points": len(pts),
                              "polyhedra_ns_per_point_per_triangle": {}, "voxels_ns_per_point_per_prism": {}}
    poly = out["polyhedra_ns_per_point_per_triangle"]
    assert isinstance(poly, dict)
    for name in ("cube_aligned", "sphere", "spot"):
        tris = every[name]
        n = len(tris) * len(pts)
        rs = c.kernel_time(tris, None, 0.0, pts, repeats = max(1, 2_000_000 // n))
        verts, faces = c.mesh(tris)
        ev = pg.GravityEvaluable(pg.Polyhedron((verts, faces), 1.0, pg.NormalOrientation.OUTWARDS,
                                               pg.PolyhedronIntegrity.VERIFY, pg.MetricUnit.UNITLESS))
        plist = pts.tolist()
        t_esa = best(lambda: ev(plist, parallel = False))
        t_py = best(lambda: c.ours_py(tris, pts), reps = 2)
        poly[name] = {"triangles": len(tris),
                      "cavendish_faceted": rs["faceted_ns_per_point_per_triangle"],
                      "esa_evaluable": t_esa / n * 1e9, "study_numpy": t_py / n * 1e9}
        print(name, poly[name])
    vox = out["voxels_ns_per_point_per_prism"]
    assert isinstance(vox, dict)
    h = 1.0 / 16.0
    centres = c.voxelise(every["icosahedron"], h, np.full(3, 0.013))
    boxes = c.cube_boxes(centres, h)
    n = len(boxes) * len(pts)
    rs = c.kernel_time(None, centres, h, pts, repeats = 3)
    coords = (pts[:, 0], pts[:, 1], pts[:, 2])
    dens = np.full(len(boxes), 2700.0)
    hm_one = {f: best(lambda f = f: hm.prism_gravity(coords, boxes, dens, field = f, parallel = False,
                                                      disable_checks = True)) / n * 1e9
              for f in ("potential", "g_e", "g_n", "g_z")}
    hm_par = best(lambda: hm.prism_gravity(coords, boxes, dens, field = "g_z", parallel = True,
                                           disable_checks = True)) / n * 1e9
    t_py = best(lambda: c.ours_py(c.sb.cube_tris(centres, h), pts), reps = 1)
    vox.update({"prisms": len(boxes), "cavendish_cube_potential": rs["cube_kernel_ns_per_point_per_cube"],
                "cavendish_prism_potential": rs["prism_ns_per_point_per_cube"],
                "harmonica_one_thread": hm_one, "harmonica_g_z_parallel": hm_par,
                "study_numpy_12_triangles_U_and_g": t_py / n * 1e9})
    print(vox)
    (c.HERE / "timing.json").write_text(json.dumps(out, indent = 1), encoding = "utf-8")


if __name__ == "__main__":
    main()
