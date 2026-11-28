"""The oracle tier's fixtures for the paper's 40-digit comparison (P8, docs/slices/S5.md §3).

    python scripts/oracle_fixtures.py        # writes oracle/fixtures/paper/polyhedral/ (a minute)

`data/esa_fixtures.json` holds the bodies' triangles, the points and ESA's values;
`data/mp_reference.json` holds each code's error against a 40-digit evaluation, but not the
40-digit values themselves, which the engine's test needs. This script evaluates them with
`mp_reference.field` (the same function, unchanged) and writes, as flat float64 `.npy` files the
oracle crate reads:

    points.npy                     [24 * 3]  3 directions x 8 distances (1 ... 1000 m), distance-major;
                                             the first 18 are esa_fixtures.json's points, bit for bit
    <body>/triangles.npy           [F * 9]   esa_fixtures.json's triangles (plate_aligned: the
                                             research benchmark's box, docs/research/paper-benchmarks)
    <body>/ref_U_hi.npy, _lo.npy   [24]      U = integral dV/r (m^2, density 1, no G) at 40 digits,
                                             as a double-double: hi + lo
    <body>/ref_g_hi.npy, _lo.npy   [24 * 3]  g = grad U (m), the same way
    <body>/esa_U.npy, esa_g.npy    [18], [18 * 3]  ESA's values from esa_fixtures.json
    <body>/recorded_esa_U.npy, recorded_esa_g.npy, recorded_ours_g.npy  [18]
                                             the errors mp_reference.json records

Nothing here is a new reference: before writing, every one of ESA's 108 errors against these values
is checked to reproduce the error mp_reference.json records (to 1e-9 of it), so the values are the
reference that file was made from. Regenerate only if esa_fixtures.json changes, never to pass a test.
"""

from __future__ import annotations

import json
import pathlib
import sys

import mpmath as mp
import numpy as np

HERE = pathlib.Path(__file__).resolve().parent
PAPER = HERE.parent
REPO = PAPER.parents[2]
OUT = REPO / "oracle" / "fixtures" / "paper" / "polyhedral"
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(REPO / "docs" / "research" / "exact-skin-and-partial-fill"))
import mp_reference as mpr  # noqa: E402
import skin_bench as sb  # noqa: E402

DISTANCES = (1.0, 1.5, 3.0, 10.0, 30.0, 100.0, 300.0, 1000.0)


def split(x: mp.mpf) -> tuple[float, float]:
    """A 40-digit number as a double-double: hi the nearest f64, lo the nearest f64 to the rest."""
    hi = float(x)
    return hi, float(x - mp.mpf(hi))


def plate_aligned() -> np.ndarray:
    """docs/research/paper-benchmarks/common.py's `box_tris` for the 1 m x 1 m x 5 cm plate."""
    lo, hi = np.array([-0.5, -0.5, -0.025]), np.array([0.5, 0.5, 0.025])
    return sb.cube_tris(np.zeros((1, 3)), 1.0) * (hi - lo) + 0.5 * (lo + hi)


def relative(value: mp.mpf, ref: mp.mpf) -> mp.mpf:
    return abs(value / ref - 1)


def relative_vec(value: list[mp.mpf], ref: list[mp.mpf]) -> mp.mpf:
    return mp.sqrt(sum((value[i] - ref[i]) ** 2 for i in range(3))) / mp.sqrt(mpr.dot(ref, ref))


def main() -> None:
    fx = json.loads((PAPER / "data" / "esa_fixtures.json").read_text(encoding = "utf-8"))
    rec = json.loads((PAPER / "data" / "mp_reference.json").read_text(encoding = "utf-8"))["bodies"]
    pts = np.array([d * u for d in DISTANCES for u in sb.DIRECTIONS])
    esa_pts = np.array(fx["points"])
    assert np.array_equal(pts[: len(esa_pts)], esa_pts), "the points are not esa_fixtures.json's"
    OUT.mkdir(parents = True, exist_ok = True)
    np.save(OUT / "points.npy", pts.ravel())
    bodies = {name: np.array(b["triangles"]) for name, b in fx["bodies"].items()}
    bodies["plate_aligned"] = plate_aligned()
    for name, tris in bodies.items():
        mtris = [[mpr.vec(v) for v in t] for t in tris.tolist()]
        ref = [mpr.field(mtris, mpr.vec(p.tolist())) for p in pts]
        u = np.array([split(r[0]) for r in ref])
        g = np.array([[split(c) for c in r[1]] for r in ref])
        d = OUT / name
        d.mkdir(exist_ok = True)
        np.save(d / "triangles.npy", tris.ravel())
        np.save(d / "ref_U_hi.npy", u[:, 0])
        np.save(d / "ref_U_lo.npy", u[:, 1])
        np.save(d / "ref_g_hi.npy", g[:, :, 0].ravel())
        np.save(d / "ref_g_lo.npy", g[:, :, 1].ravel())
        if name in fx["bodies"]:
            b, rows = fx["bodies"][name], rec[name]
            worst = 0.0
            for k in range(len(esa_pts)):
                eu = relative(mp.mpf(b["U"][k]), ref[k][0])
                eg = relative_vec([mp.mpf(x) for x in b["g"][k]], ref[k][1])
                for got, want in ((eu, rows[k]["esa_U"]), (eg, rows[k]["esa_g"])):
                    worst = max(worst, float(abs(got / want - 1)))
            assert worst < 1e-9, f"{name}: ESA's errors differ from mp_reference.json's by {worst:.1e}"
            np.save(d / "esa_U.npy", np.array(b["U"], dtype = np.float64))
            np.save(d / "esa_g.npy", np.array(b["g"], dtype = np.float64).ravel())
            for key in ("esa_U", "esa_g", "ours_g"):
                np.save(d / f"recorded_{key}.npy", np.array([r[key] for r in rows], dtype = np.float64))
            print(f"{name:13s} {len(tris):5d} triangles; ESA's recorded errors reproduced to {worst:.1e}")
        else:
            print(f"{name:13s} {len(tris):5d} triangles (no ESA values)")


if __name__ == "__main__":
    main()
