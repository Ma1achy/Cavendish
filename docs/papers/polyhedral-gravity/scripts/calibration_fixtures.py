"""The calibration set for P8's rounding law (docs/slices/S5.md §7.18): bodies and points that are
not the paper's six, with the 40-digit reference at each point.

    python scripts/calibration_fixtures.py   # writes oracle/fixtures/paper/polyhedral_calibration/

The owner's ruling of 28 Nov 2026 restates P8 as a rounding law: the relative error of `U` and of
`g` is at most `c·ε·(R/s)²` (`ε` f64's machine epsilon, `R` the point's distance from the body's
centroid, `s` its size: twice its farthest vertex from the centroid). The constant `c` is fixed on
this set, not on the six bodies whose numbers prompted the law, and only then are the six measured.

The set is fixed here, by a seed, before anything is measured on it:

- nine bodies of about a metre, each turned by its own random rotation and moved so its volume
  centroid is the origin: a regular tetrahedron, an octahedron, a cube, a 1 x 0.6 x 0.3 box, plates
  of 10:1 and 100:1, a 20:1 rod, a 320-face icosphere and a non-convex L-shaped prism. The
  aspect ratios run past the six's (their plate is 20:1) because walls and slabs are what the
  engine voxelises, and the law's constant is a property of the form only if it holds for thin
  bodies too;
- at each of R/s = 0.75, 1.5, 3, 10, 30, 100, three random directions (a fresh generator, not the
  six's directions).

Writes, as flat float64 `.npy` files the oracle crate reads, for each body:

    <body>/triangles.npy           [F * 9]   the triangles, metres, outward
    <body>/points.npy              [18 * 3]  the points, metres, distance-major
    <body>/ref_U_hi.npy, _lo.npy   [18]      U = integral dV/r (m^2, density 1, no G) at 40 digits,
                                             as a double-double: hi + lo
    <body>/ref_g_hi.npy, _lo.npy   [18 * 3]  g = grad U (m), the same way

The reference is `mp_reference.field`, the same function as the six's. Regenerate only if this
file changes, never to pass a test.
"""

from __future__ import annotations

import pathlib
import sys

import mpmath as mp
import numpy as np

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parents[3]
OUT = REPO / "oracle" / "fixtures" / "paper" / "polyhedral_calibration"
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(REPO / "docs" / "research" / "exact-skin-and-partial-fill"))
import mp_reference as mpr  # noqa: E402
import skin_bench as sb  # noqa: E402

SEED = 20261128
RATIOS = (0.75, 1.5, 3.0, 10.0, 30.0, 100.0)
DIRECTIONS_PER_RATIO = 3


def box(size: tuple[float, float, float]) -> np.ndarray:
    """An axis-aligned box centred on the origin, outward."""
    return sb.cube_tris(np.zeros((1, 3)), 1.0) * np.array(size)


def from_faces(v: np.ndarray, faces: list[tuple[int, int, int]]) -> np.ndarray:
    return np.array([[v[a], v[b], v[c]] for a, b, c in faces])


def tetrahedron() -> np.ndarray:
    v = np.array([[1.0, 1.0, 1.0], [1.0, -1.0, -1.0], [-1.0, 1.0, -1.0], [-1.0, -1.0, 1.0]])
    return from_faces(v * 0.5 / np.sqrt(2.0), [(0, 1, 2), (0, 3, 1), (0, 2, 3), (1, 3, 2)])


def octahedron() -> np.ndarray:
    v = 0.6 * np.array([[1.0, 0, 0], [-1.0, 0, 0], [0, 1.0, 0], [0, -1.0, 0], [0, 0, 1.0], [0, 0, -1.0]])
    faces = [(0, 2, 4), (2, 1, 4), (1, 3, 4), (3, 0, 4), (2, 0, 5), (1, 2, 5), (3, 1, 5), (0, 3, 5)]
    return from_faces(v, faces)


def icosphere() -> np.ndarray:
    v, f = sb.icosphere(2)
    return from_faces(v * 0.5, f)


def l_prism() -> np.ndarray:
    """The L of (0,0) (2,0) (2,1) (1,1) (1,2) (0,2), times 0.5 m, 0.3 m deep: non-convex."""
    ring = 0.5 * np.array([[0, 0], [2, 0], [2, 1], [1, 1], [1, 2], [0, 2], [0, 1]], dtype = float)
    depth = 0.3
    lo = np.column_stack([ring, np.zeros(len(ring))])
    hi = np.column_stack([ring, np.full(len(ring), depth)])
    tris = []
    for i in range(len(ring)):
        j = (i + 1) % len(ring)
        # The ring is anticlockwise seen from +z, so each side's outward quad is lo_i lo_j hi_j hi_i.
        tris += [[lo[i], lo[j], hi[j]], [lo[i], hi[j], hi[i]]]
    # The caps: the pentagon 0 1 2 3 6 fanned from 0, and the square 6 3 4 5.
    cap = [(0, 1, 2), (0, 2, 3), (0, 3, 6), (6, 3, 4), (6, 4, 5)]
    for a, b, c in cap:
        tris.append([hi[a], hi[b], hi[c]])
        tris.append([lo[a], lo[c], lo[b]])
    return np.array(tris)


BODIES = {
    "tetrahedron": tetrahedron,
    "octahedron": octahedron,
    "cube": lambda: box((1.0, 1.0, 1.0)),
    "box": lambda: box((1.0, 0.6, 0.3)),
    "plate_10": lambda: box((1.0, 1.0, 0.1)),
    "plate_100": lambda: box((1.0, 1.0, 0.01)),
    "rod_20": lambda: box((1.0, 0.05, 0.05)),
    "icosphere_320": icosphere,
    "l_prism": l_prism,
}


def split(x: mp.mpf) -> tuple[float, float]:
    """A 40-digit number as a double-double: hi the nearest f64, lo the nearest f64 to the rest."""
    hi = float(x)
    return hi, float(x - mp.mpf(hi))


def main() -> None:
    rng = np.random.default_rng(SEED)
    OUT.mkdir(parents = True, exist_ok = True)
    for name, make in BODIES.items():
        tris = make()
        if name != "l_prism":
            # A convex body about the origin is outward when n·(face centroid) > 0.
            n = np.cross(tris[:, 1] - tris[:, 0], tris[:, 2] - tris[:, 0])
            flip = np.einsum("nk,nk->n", n, tris.mean(axis = 1)) < 0
            tris[flip] = tris[flip][:, ::-1]
        q, r = np.linalg.qr(rng.normal(size = (3, 3)))
        q *= np.sign(np.diag(r))
        q = q if np.linalg.det(q) > 0 else -q
        tris = tris @ q.T
        volume, centroid = sb.volume_centroid(tris.reshape(-1, 3, 3), np.zeros(3))
        assert volume > 0, f"{name}: not outward ({volume})"
        tris = tris - centroid
        size = 2.0 * float(np.max(np.linalg.norm(tris.reshape(-1, 3), axis = 1)))
        dirs = rng.normal(size = (len(RATIOS) * DIRECTIONS_PER_RATIO, 3))
        dirs /= np.linalg.norm(dirs, axis = 1)[:, None]
        pts = np.array([ratio * size * dirs[k * DIRECTIONS_PER_RATIO + i]
                        for k, ratio in enumerate(RATIOS) for i in range(DIRECTIONS_PER_RATIO)])
        mtris = [[mpr.vec(v) for v in t] for t in tris.tolist()]
        ref = [mpr.field(mtris, mpr.vec(p.tolist())) for p in pts]
        u = np.array([split(x[0]) for x in ref])
        g = np.array([[split(c) for c in x[1]] for x in ref])
        d = OUT / name
        d.mkdir(exist_ok = True)
        np.save(d / "triangles.npy", tris.ravel())
        np.save(d / "points.npy", pts.ravel())
        np.save(d / "ref_U_hi.npy", u[:, 0])
        np.save(d / "ref_U_lo.npy", u[:, 1])
        np.save(d / "ref_g_hi.npy", g[:, :, 0].ravel())
        np.save(d / "ref_g_lo.npy", g[:, :, 1].ravel())
        print(f"{name:14s} {len(tris):4d} triangles, size {size:.3f} m, volume {volume:.4f} m^3")


if __name__ == "__main__":
    main()
