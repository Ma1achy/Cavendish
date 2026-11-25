"""A 40-digit evaluation of the Werner & Scheeres face form, to referee the f64 cross-check.

Takes the same triangles and points as `data/esa_fixtures.json` (the f64 vertices, converted
exactly), evaluates U = ∫dV/r and g = ∇U with mpmath at 40 significant digits, and reports the
error of each f64 implementation against it: ours (`skin_bench.polyhedron_field`, numerator
h_f·2A_f) and ESA's. Writes `data/mp_reference.json`.

At 40 digits the closed form's own cancellation (at most (R/s)³ ≈ 10⁶ here) leaves ≥ 30 correct
digits, so the reference is exact for this purpose.

    python scripts/mp_reference.py
"""

from __future__ import annotations

import json
import pathlib
import sys

import mpmath as mp
import numpy as np

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[2] / "research" / "exact-skin-and-partial-fill"))
import skin_bench  # noqa: E402

skin_bench.edge_numerator = True
mp.mp.dps = 40


def vec(x: list[float]) -> list[mp.mpf]:
    return [mp.mpf(float(c)) for c in x]


def sub(a: list[mp.mpf], b: list[mp.mpf]) -> list[mp.mpf]:
    return [a[i] - b[i] for i in range(3)]


def dot(a: list[mp.mpf], b: list[mp.mpf]) -> mp.mpf:
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def cross(a: list[mp.mpf], b: list[mp.mpf]) -> list[mp.mpf]:
    return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]


def field(tris: list[list[list[mp.mpf]]], p: list[mp.mpf]) -> tuple[mp.mpf, list[mp.mpf]]:
    u = mp.mpf(0)
    g = [mp.mpf(0)] * 3
    for t in tris:
        nn = cross(sub(t[1], t[0]), sub(t[2], t[0]))
        a2 = mp.sqrt(dot(nn, nn))
        n = [x / a2 for x in nn]
        r = [sub(x, p) for x in t]
        ln = [mp.sqrt(dot(x, x)) for x in r]
        h = dot(n, r[0])
        num = dot(r[0], cross(r[1], r[2]))
        den = (ln[0] * ln[1] * ln[2] + ln[0] * dot(r[1], r[2]) + ln[1] * dot(r[2], r[0])
               + ln[2] * dot(r[0], r[1]))
        om = 2 * mp.atan2(num, den)
        s = mp.mpf(0)
        for i in range(3):
            j = (i + 1) % 3
            e = sub(t[j], t[i])
            el = mp.sqrt(dot(e, e))
            ne = [x / el for x in cross(e, n)]
            ab = ln[i] + ln[j]
            s += dot(ne, r[i]) * mp.log((ab + el) / (ab - el))
        br = s - h * om
        u += h * br / 2
        g = [g[k] - br * n[k] for k in range(3)]
    return u, g


def main() -> None:
    fx = json.loads((HERE.parent / "data" / "esa_fixtures.json").read_text(encoding="utf-8"))
    pts = np.array(fx["points"])
    out: dict[str, object] = {"digits": 40, "bodies": {}}
    for name, b in fx["bodies"].items():
        tris = np.array(b["triangles"])
        u64, g64 = skin_bench.polyhedron_field(tris, np.ones((len(tris), 1)), pts)
        mtris = [[vec(v) for v in t] for t in tris.tolist()]
        rows = []
        for k, p in enumerate(pts):
            um, gm = field(mtris, vec(p.tolist()))
            gmn = mp.sqrt(dot(gm, gm))
            ours_u = abs(mp.mpf(float(u64[0, k])) / um - 1)
            esa_u = abs(mp.mpf(b["U"][k]) / um - 1)
            ours_g = mp.sqrt(sum((mp.mpf(float(g64[0, k, i])) - gm[i]) ** 2 for i in range(3))) / gmn
            esa_g = mp.sqrt(sum((mp.mpf(b["g"][k][i]) - gm[i]) ** 2 for i in range(3))) / gmn
            rows.append({"R": round(float(np.linalg.norm(p)), 6), "ours_U": float(ours_u),
                         "esa_U": float(esa_u), "ours_g": float(ours_g), "esa_g": float(esa_g)})
        out["bodies"][name] = rows  # type: ignore[index]
        for dist in sorted({r["R"] for r in rows}):
            sel = [r for r in rows if r["R"] == dist]
            print(f"{name:13s} R {dist:6.1f} m  ours U {max(r['ours_U'] for r in sel):.1e}"
                  f" g {max(r['ours_g'] for r in sel):.1e}   ESA U {max(r['esa_U'] for r in sel):.1e}"
                  f" g {max(r['esa_g'] for r in sel):.1e}")
    (HERE.parent / "data" / "mp_reference.json").write_text(json.dumps(out, indent=1),
                                                            encoding="utf-8")


if __name__ == "__main__":
    main()
