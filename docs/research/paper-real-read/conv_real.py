"""The paper's convergence benchmark (Table 2, and Table 8's measured rows) with the real read.

    python conv_real.py run <body> <seed>      # data/conv-<body>-s<seed>.json, h = 1/8 ... 1/64 m
    python conv_real.py table                  # data/conv.json: Table 2's two blocks, real and leading

`../exact-skin-and-partial-fill/skin_bench.py`'s `errors`, unchanged in substance: the same five
convex bodies, the same placements (rng(1000 n + seed)), the same exact clipping and the same seven
representations, each against the exact pieces. The read is the pair (real, leading): the lower
launch point is where the benchmark put its lower point (1.5 m and 10 m from the body's centre along
three directions), and the real read follows both interferometers' arms up from it
(`realread.AxisRead`). Fields come from the compiled kernels (R2), with the h·2A numerator.
"""

from __future__ import annotations

import json
import math
import sys
import time

import numpy as np

import realread as R

L = R.L
sb = L.sb
NS = [8, 16, 32, 64]
DIST = (1.5, 10.0)
BODIES = ["cube_aligned", "cube", "icosahedron", "plate", "sphere"]
REPS = ("zero_one_raw", "zero_one", "ss4", "frac", "frac_cen", "frac_pt", "frac_cen_q")
OUT = R.HERE / "data"


def lowers() -> np.ndarray:
    return np.array([d * u for d in DIST for u in sb.DIRECTIONS])  # [6, 3]: distance-major


def run(name: str, seed: int) -> None:
    R.below_normal()
    OUT.mkdir(exist_ok = True)
    tris = sb.body(name)
    read = R.AxisRead(48)
    pts = read.points(lowers())
    path = OUT / f"conv-{name}-s{seed}.json"
    rows = json.loads(path.read_text())["rows"] if path.exists() else []
    vol_body, _ = sb.volume_centroid(tris, np.zeros(3))
    g_b = R.tri_field(tris, np.ones((len(tris), 1)), pts)[1][0]
    clearance = [min(L.distance_to_mesh(tris, p + np.array([0.0, 0.0, z])) for z in np.linspace(0, 10.01, 41))
                 for p in lowers()]
    for n in NS:
        if any(r["n"] == n for r in rows):
            continue
        h = 1.0 / n
        offset = np.random.default_rng(1000 * n + seed).uniform(0, h, 3)
        t0 = time.perf_counter()
        lat = sb.lattice(tris, h, offset)
        t_clip = time.perf_counter() - t0
        g_p = R.tri_field(lat["pieces"], np.ones((len(lat["pieces"]), 1)), pts)[1][0]
        ctr, frac, ss, cen = lat["ctr_in"], lat["frac"], lat["ss"], lat["centroids"]
        w = np.repeat(np.column_stack([ctr, ss, frac]), 12, axis = 0)
        g_c = R.tri_field(sb.cube_tris(lat["centres"], h), w, pts)[1]
        g_m = R.tri_field(sb.cube_tris(cen, h), np.repeat(frac[:, None], 12, axis = 0), pts)[1][0]
        _, g_pt, _, g_q2 = R.point_field(cen, frac * h**3, lat["second"], pts)
        g_q = g_pt + g_q2
        n_in = lat["n_inside"]
        s1 = vol_body / (h**3 * (n_in + ctr.sum()))
        s4 = vol_body / (h**3 * (n_in + ss.sum()))
        rep = {"zero_one": (s1 - 1) * (g_b - g_p) + s1 * g_c[0] - g_p, "zero_one_raw": g_c[0] - g_p,
               "ss4": (s4 - 1) * (g_b - g_p) + s4 * g_c[1] - g_p, "frac": g_c[2] - g_p, "frac_cen": g_m - g_p,
               "frac_pt": g_pt - g_p, "frac_cen_q": g_q - g_p}
        row = {"n": n, "h": h, "seed": seed, "boundary_cells": len(frac), "seconds_clip": t_clip}
        for k, dg in rep.items():
            real, lead = read.phases(dg)
            row[k] = {"real": real.tolist(), "leading": lead.tolist()}
        rows.append(row)
        rows.sort(key = lambda r: int(r["n"]))
        sig_r, sig_l = read.phases(g_b)
        path.write_text(json.dumps({"body": name, "seed": seed, "signal_real": sig_r.tolist(),
                                    "signal_leading": sig_l.tolist(), "clearance": clearance, "rows": rows}),
                        encoding = "utf-8")
        print(name, seed, n, f"clip {t_clip:.0f}s", {k: f"{np.abs(v['real']).max():.1e}" for k, v in row.items()
                                                    if isinstance(v, dict)}, flush = True)


def table() -> None:
    """Per body, distance, representation and read: RMS over the placements of the worst-direction
    error at each pitch, and the least-squares order over 1/8 ... 1/64 m (tables.py's definition)."""
    out: dict[str, object] = {}
    for name in BODIES:
        runs = [json.loads(p.read_text()) for p in sorted(OUT.glob(f"conv-{name}-s*.json"))]
        if not runs:
            continue
        per: dict[str, object] = {"seeds": len(runs), "clearance": runs[0]["clearance"]}
        for di, d in enumerate(DIST):
            sl = slice(3 * di, 3 * di + 3)
            ent: dict[str, object] = {}
            for read in ("real", "leading"):
                sig = float(np.mean([np.abs(r[f"signal_{read}"][sl]).max() for r in runs]))
                sig_dirs = np.mean([r[f"signal_{read}"][sl] for r in runs], axis = 0).tolist()
                reps = {}
                for rep in REPS:
                    ns = sorted({row["n"] for r in runs for row in r["rows"]})
                    errs = []
                    for n in ns:
                        e = [np.abs(row[rep][read][sl]).max() for r in runs for row in r["rows"] if row["n"] == n]
                        errs.append(math.sqrt(np.mean(np.square(e))))
                    order = float(np.polyfit(np.log([1.0 / n for n in ns]), np.log(errs), 1)[0])
                    reps[rep] = {"n": ns, "err": errs, "order": order}
                ent[read] = {"signal": sig, "signal_by_direction": sig_dirs, "reps": reps}
            per[str(d)] = ent
        out[name] = per
    (OUT / "conv.json").write_text(json.dumps(out, indent = 1), encoding = "utf-8")


if __name__ == "__main__":
    if sys.argv[1] == "run":
        for nm in sys.argv[2].split(","):
            for s in sys.argv[3].split(","):
                run(nm, int(s))
    else:
        table()
