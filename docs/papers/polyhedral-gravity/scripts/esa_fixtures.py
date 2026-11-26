"""Second oracle: fixtures from ESA's polyhedral-gravity-model, for the paper's cross-check.

ESA's library (https://github.com/esa/polyhedral-gravity-model, `pip install polyhedral-gravity`)
implements Tsoulis (2012)'s line-integral form with the singular terms of Tsoulis & Petrović (2001):
a different formulation from the Werner & Scheeres face form the paper uses, so agreement tests the
algebra, not only the code. It is GPL-3.0: this script runs it as a *tool* in its own virtual
environment and writes plain numbers to `data/esa_fixtures.json` (the tool and its version are
recorded in the file). Nothing in Cavendish imports or links it.

    python -m venv esa-venv && esa-venv/Scripts/python -m pip install polyhedral-gravity numpy
    esa-venv/Scripts/python scripts/esa_fixtures.py          # writes data/esa_fixtures.json

Bodies: the research benchmark's five (from `skin_bench.body`) and a non-convex one (an icosahedron
with a pyramid raised on every face, a small stellation). Points: 3 directions × distances from
just outside the body to R/s = 100 of the body's size. With `MetricUnit.UNITLESS` and density 1 the
library returns U = ∫dV/r and ∇U raw (no G), the quantities `polyhedron_field` returns.
"""

from __future__ import annotations

import importlib.metadata
import json
import pathlib
import sys

import numpy as np
import polyhedral_gravity as pg

HERE = pathlib.Path(__file__).resolve().parent
BENCH = HERE.parents[2] / "research" / "exact-skin-and-partial-fill"
sys.path.insert(0, str(BENCH))
import skin_bench  # noqa: E402

DIRECTIONS = np.array([[1.0, 0.2, -0.3], [-0.4, 1.0, 0.1], [0.3, -0.5, -1.0]])
DIRECTIONS /= np.linalg.norm(DIRECTIONS, axis=1)[:, None]
DISTANCES = (1.0, 1.5, 3.0, 10.0, 30.0, 100.0)  # metres from the centre; bodies are ~1 m


def stellated() -> np.ndarray:
    """A non-convex body: an icosahedron (circumradius 0.6 m) with a pyramid on every face."""
    v, f = skin_bench.icosahedron()
    v = v * 0.6
    tris = []
    for a, b, c in f:
        apex = (v[a] + v[b] + v[c]) / 3.0 * 1.6
        tris += [[v[a], v[b], apex], [v[b], v[c], apex], [v[c], v[a], apex]]
    t = np.array(tris)
    n = np.cross(t[:, 1] - t[:, 0], t[:, 2] - t[:, 0])
    assert np.all(np.einsum("nk,nk->n", n, t.mean(axis=1)) > 0), "faces must wind outward"
    return t


def mesh(tris: np.ndarray) -> tuple[list[list[float]], list[list[int]]]:
    """Triangle soup → shared vertices and index triples (ESA's integrity check wants a mesh)."""
    verts: dict[tuple[float, float, float], int] = {}
    faces = []
    for t in tris:
        idx = []
        for p in t:
            key = (float(p[0]), float(p[1]), float(p[2]))  # exact: shared vertices are bit-equal
            idx.append(verts.setdefault(key, len(verts)))
        faces.append(idx)
    return [list(k) for k in verts], faces


def main() -> None:
    bodies = {name: skin_bench.body(name) for name in
              ("cube_aligned", "cube", "icosahedron", "plate", "sphere")}
    bodies["stellated"] = stellated()
    pts = np.array([d * u for d in DISTANCES for u in DIRECTIONS])
    out: dict[str, object] = {
        "tool": "polyhedral-gravity (ESA), GPL-3.0, used as a tool only",
        "version": importlib.metadata.version("polyhedral-gravity"),
        "units": "U = ∫dV/r (m²), g = ∇U (m), density 1, no G",
        "points": pts.tolist(),
        "bodies": {},
    }
    for name, tris in bodies.items():
        verts, faces = mesh(tris)
        poly = pg.Polyhedron((verts, faces), 1.0, pg.NormalOrientation.OUTWARDS,
                             pg.PolyhedronIntegrity.VERIFY, pg.MetricUnit.UNITLESS)
        res = pg.evaluate(poly, pts.tolist(), parallel=False)
        out["bodies"][name] = {  # type: ignore[index]
            "triangles": np.array(tris).tolist(),
            "U": [r[0] for r in res],
            "g": [list(r[1]) for r in res],
        }
        print(f"{name}: {len(faces)} faces, {len(verts)} vertices")
    (HERE.parent / "data").mkdir(exist_ok=True)
    (HERE.parent / "data" / "esa_fixtures.json").write_text(json.dumps(out), encoding="utf-8")


if __name__ == "__main__":
    main()
