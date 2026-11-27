"""Criteria A1–A3 and B2: four codes against a 40-digit reference, near, far and on the surface.

    KERNEL_BENCH=<path to kernel_bench.exe> .venv-bench/Scripts/python bench_accuracy.py

Writes `accuracy.json` (every error, per body, code, point) and prints the worst of three
directions per distance. Harmonica runs only on the axis-aligned boxes (it models prisms).
"""

from __future__ import annotations

import json
import math

import mpmath as mp
import numpy as np

import common as c

DISTANCES = (1.0, 1.5, 3.0, 10.0, 30.0, 100.0, 300.0, 1000.0)  # metres from the centre; s ≈ 1 m


def rel_errors(u: np.ndarray, g: np.ndarray, ref: list[tuple[mp.mpf, list[mp.mpf]]]) -> tuple[list[float], list[float]]:
    eu, eg = [], []
    for k, (um, gm) in enumerate(ref):
        gn = mp.sqrt(sum(x * x for x in gm))
        eu.append(float(abs(mp.mpf(float(u[k])) / um - 1)))
        eg.append(float(mp.sqrt(sum((mp.mpf(float(g[k, i])) - gm[i]) ** 2 for i in range(3))) / gn))
    return eu, eg


def far_and_near() -> dict[str, object]:
    out: dict[str, object] = {}
    pts = np.array([d * u for d in DISTANCES for u in c.DIRECTIONS])
    for name, tris in c.bodies().items():
        sel = pts if name != "spot" else pts[[i for i, d in enumerate(np.repeat(DISTANCES, 3))
                                              if d in (1.5, 3.0, 100.0, 1000.0)]]
        ref = [c.mp_field(tris, p) for p in sel]
        codes = {"ours_py": c.ours_py(tris, sel), "ours_rs": c.ours_rs(tris, sel), "esa": c.esa(tris, sel)}
        if name in c.BOXES:
            lo, hi = c.BOXES[name]
            codes["harmonica"] = c.harmonica_boxes(np.array([[lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]]]), sel)
        rows = {}
        for code, (u, g) in codes.items():
            eu, eg = rel_errors(u, g, ref)
            rows[code] = {"R": np.linalg.norm(sel, axis = 1).round(6).tolist(), "U": eu, "g": eg}
        out[name] = {"triangles": len(tris), "codes": rows}
        print(f"\n{name} ({len(tris)} triangles): worst over 3 directions, relative error of g [U]")
        for d in sorted(set(rows["ours_py"]["R"])):
            line = f"  R {d:7.1f} m"
            for code, r in rows.items():
                idx = [i for i, x in enumerate(r["R"]) if x == d]
                line += f"  {code} {max(r['g'][i] for i in idx):.1e} [{max(r['U'][i] for i in idx):.1e}]"
            print(line)
    return out


def _perm(v: np.ndarray, k: int) -> np.ndarray:
    """Rotate axes so that axis k becomes z: (x, y, z) → for k=0 (y, z, x), k=1 (z, x, y)."""
    return np.roll(v, -(k + 1))


def mp_box_g(lo: np.ndarray, hi: np.ndarray, p: np.ndarray) -> tuple[mp.mpf, list[mp.mpf]]:
    """U and the full g of an aligned box, 40 digits, from Nagy's corner sums (finite on it)."""
    u, _ = c.mp_box(lo, hi, p)
    g = [c.mp_box(_perm(lo, k), _perm(hi, k), _perm(p, k))[1] for k in range(3)]
    return u, g


def singular() -> dict[str, object]:
    lo, hi = c.BOXES["cube_aligned"]
    tris = c.box_tris(lo, hi)
    cases = {
        "face centre":         [0.4, 0.0, 0.0],
        "on a face, generic":  [0.4, 0.13, -0.21],
        "on an edge":          [0.4, 0.4, 0.1],
        "at a vertex":         [0.4, 0.4, 0.4],
        "face plane, outside": [0.4, 0.9, 0.1],
        "edge line, outside":  [0.4, 0.4, 0.9],
        "1e-9 m off an edge":  [0.4 + 1e-9, 0.4 + 1e-9, 0.1],
        "1e-12 m off a face":  [0.4 + 1e-12, 0.13, -0.21],
    }
    pts = np.array(list(cases.values()))
    box = np.array([[lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]]])
    codes = {"ours_py": c.ours_py(tris, pts), "ours_rs": c.ours_rs(tris, pts), "esa": c.esa(tris, pts),
             "harmonica": c.harmonica_boxes(box, pts)}
    out: dict[str, object] = {}
    print("\nsingular points of the aligned 0.8 m cube: relative error of U, |g| (or the value if NaN/inf)")
    for k, (label, p) in enumerate(cases.items()):
        um, gm = mp_box_g(lo, hi, np.array(p))
        gn = mp.sqrt(sum(x * x for x in gm))
        row: dict[str, object] = {"point": p}
        line = f"  {label:22s}"
        for code, (u, g) in codes.items():
            finite = bool(np.isfinite(u[k]) and np.all(np.isfinite(g[k])))
            if finite:
                eu = float(abs(mp.mpf(float(u[k])) / um - 1))
                eg = float(mp.sqrt(sum((mp.mpf(float(g[k, i])) - gm[i]) ** 2 for i in range(3))) / gn)
                row[code] = {"U": eu, "g": eg}
                line += f"  {code} {eu:.0e} {eg:.0e}"
            else:
                row[code] = {"U": repr(float(u[k])), "g": [repr(float(x)) for x in g[k]]}
                line += f"  {code} NONFINITE"
        out[label] = row
        print(line)
    return out


def main() -> None:
    result = {"reference": "Werner & Scheeres face form at 40 digits (mp_reference.field); boxes on "
                           "the surface: Nagy's corner sum at 40 digits",
              "far_and_near": far_and_near(), "singular": singular(),
              "eps": math.ulp(1.0)}
    (c.HERE / "accuracy.json").write_text(json.dumps(result, indent = 1), encoding = "utf-8")


if __name__ == "__main__":
    main()
