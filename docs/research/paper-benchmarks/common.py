"""Shared pieces of the paper benchmarks: bodies, the 40-digit references, and the codes compared.

Every code is brought to one convention, the paper's: density 1, no G, `U = ∫dV/r` (m², positive)
and `g = ∇U` (m, pointing at the mass), in the body's frame (x, y, z) = (east, north, up).

    codes:  ours_py  (the study's numpy, `skin_bench.polyhedron_field`, h_f·2A_f numerator)
            ours_rs  (Cavendish's `cav_core::polyhedral::Faceted`, via `kernel_bench`)
            esa      (ESA's polyhedral-gravity, Tsoulis 2012's line integrals; GPL-3.0, run as a tool)
            harmonica(Fatiando's Harmonica / choclo prism, Nagy et al. 2000 with safe log/atan)

Run from the venv that holds harmonica and polyhedral-gravity (see `README` in
`docs/research/paper-benchmarks.md`); nothing here is imported by Cavendish.
"""

from __future__ import annotations

import hashlib
import io
import pathlib
import subprocess
import sys
import urllib.request
import zipfile

import mpmath as mp
import numpy as np
import numpy.typing as npt

HERE   = pathlib.Path(__file__).resolve().parent
REPO   = HERE.parents[2]
BENCH  = REPO / "docs" / "research" / "exact-skin-and-partial-fill"
PAPER  = REPO / "docs" / "papers" / "polyhedral-gravity" / "scripts"
CACHE  = HERE / ".cache"
sys.path.insert(0, str(BENCH))
sys.path.insert(0, str(PAPER))
import skin_bench as sb  # noqa: E402
import mp_reference as mpr  # noqa: E402  (sets sb.edge_numerator = True: the h_f·2A_f numerator)

F64 = npt.NDArray[np.float64]
mp.mp.dps = 40

KERNEL_BENCH = pathlib.Path(
    __import__("os").environ.get("KERNEL_BENCH", str(HERE / "kernel_bench" / "target" / "release"
                                                     / "kernel_bench.exe")))
DIRECTIONS = sb.DIRECTIONS
PHASE_PER_DG = sb.K_EFF * sb.T**2  # rad per m s⁻² of Δg_z over the 5 m baseline
SPOT = ("https://www.cs.cmu.edu/~kmcrane/Projects/ModelRepository/spot.zip",
        "1d4082da4e940f6a4cfdf648fd77e1647ac613c618dbff933f1ba23fad254608")


# ========================================= BODIES =========================================


def box_tris(lo: F64, hi: F64) -> F64:
    """The 12 outward triangles of the axis-aligned box from `lo` to `hi` (metres)."""
    centre = 0.5 * (lo + hi)
    return sb.cube_tris(np.zeros((1, 3)), 1.0) * (hi - lo) + centre


def stellated() -> F64:
    """A non-convex body: an icosahedron (circumradius 0.6 m) with a pyramid on every face."""
    v, f = sb.icosahedron()
    v = v * 0.6
    tris = []
    for a, b, c in f:
        apex = (v[a] + v[b] + v[c]) / 3.0 * 1.6
        tris += [[v[a], v[b], apex], [v[b], v[c], apex], [v[c], v[a], apex]]
    return np.array(tris)


def spot() -> F64:
    """Keenan Crane's Spot (CC0 1.0), 5856 triangles, upright, 1 m largest extent, centroid at 0.

    Downloaded at run time into `.cache/` (not committed) and checked against the study's pinned
    SHA-256 (`shapes_lib.SOURCES`).
    """
    CACHE.mkdir(exist_ok = True)
    path = CACHE / "spot.zip"
    if not path.exists():
        with urllib.request.urlopen(SPOT[0], timeout = 120) as r:  # noqa: S310 (pinned https URL)
            path.write_bytes(r.read())
    if hashlib.sha256(path.read_bytes()).hexdigest() != SPOT[1]:
        path.unlink()
        raise RuntimeError("spot.zip does not match its pinned SHA-256")
    with zipfile.ZipFile(path) as z:
        text = z.read("spot/spot_triangulated.obj").decode()
    verts: list[list[float]] = []
    faces: list[list[int]] = []
    for line in io.StringIO(text):
        p = line.split()
        if p and p[0] == "v":
            verts.append([float(x) for x in p[1:4]])
        elif p and p[0] == "f":
            idx = [int(q.split("/")[0]) - 1 for q in p[1:]]
            faces += [[idx[0], idx[k], idx[k + 1]] for k in range(1, len(idx) - 1)]
    v = np.array(verts)
    v = v[:, [0, 2, 1]] * np.array([1.0, -1.0, 1.0])  # +y up in the file → +z up, kept right-handed
    tris = v[np.array(faces)]
    vol, cen = sb.volume_centroid(tris, np.zeros(3))
    if vol < 0:
        tris = tris[:, ::-1]
        vol, cen = sb.volume_centroid(tris, np.zeros(3))
    tris = (tris - cen) / np.ptp(tris.reshape(-1, 3), axis = 0).max()
    return tris


def bodies() -> dict[str, F64]:
    """The benchmark bodies, ~1 m, centred near the origin, outward triangles."""
    out = {name: sb.body(name) for name in ("cube_aligned", "cube", "icosahedron", "plate", "sphere")}
    out["plate_aligned"] = box_tris(np.array([-0.5, -0.5, -0.025]), np.array([0.5, 0.5, 0.025]))
    out["stellated"] = stellated()
    out["spot"] = spot()
    return out


BOXES = {"cube_aligned": (np.full(3, -0.4), np.full(3, 0.4)),
         "plate_aligned": (np.array([-0.5, -0.5, -0.025]), np.array([0.5, 0.5, 0.025]))}


def voxelise(tris: F64, h: float, offset: F64) -> F64:
    """Centres of the cubes of pitch `h` whose centre is inside the closed mesh `tris`.

    Inside by the winding number (the sum of the triangles' solid angles over 4π), computed with the
    same closed form, so any closed mesh works. The lattice is `offset + h·(i, j, k)`.
    """
    lo, hi = tris.reshape(-1, 3).min(axis = 0), tris.reshape(-1, 3).max(axis = 0)
    axes = [offset[k] + h * np.arange(np.floor((lo[k] - offset[k]) / h) - 1,
                                      np.ceil((hi[k] - offset[k]) / h) + 2) for k in range(3)]
    grid = np.stack(np.meshgrid(*axes, indexing = "ij"), axis = -1).reshape(-1, 3)
    inside = np.zeros(len(grid), dtype = bool)
    for s in range(0, len(grid), 4096):
        pts = grid[s : s + 4096]
        w = np.zeros(len(pts))
        a, b, c = (tris[:, i, None, :] - pts[None] for i in range(3))
        la, lb, lc = (np.linalg.norm(x, axis = 2) for x in (a, b, c))
        num = np.einsum("nmk,nmk->nm", a, np.cross(b, c))
        den = (la * lb * lc + la * np.einsum("nmk,nmk->nm", b, c)
               + lb * np.einsum("nmk,nmk->nm", c, a) + lc * np.einsum("nmk,nmk->nm", a, b))
        w = (2.0 * np.arctan2(num, den)).sum(axis = 0) / (4.0 * np.pi)
        inside[s : s + 4096] = w > 0.5
    return grid[inside]


# ======================================= REFERENCES =======================================


def mp_field(tris: F64, p: F64) -> tuple[mp.mpf, list[mp.mpf]]:
    """U and g = ∇U of `tris` at `p`, 40 digits (the paper's `mp_reference.field`)."""
    mt = [[mpr.vec(v) for v in t] for t in tris.tolist()]
    return mpr.field(mt, mpr.vec(p.tolist()))


def _mp_corner_u(x: mp.mpf, y: mp.mpf, z: mp.mpf) -> mp.mpf:
    """Nagy's antiderivative of ∫dV/r at one corner (offsets from the point)."""
    r = mp.sqrt(x * x + y * y + z * z)
    out = mp.mpf(0)
    for a, b, c in ((x, y, z), (y, z, x), (z, x, y)):
        if a != 0 and b != 0:
            out += a * b * mp.log(c + r)
        if c != 0:
            out -= c * c / 2 * mp.atan(a * b / (c * r))
    return out


def _mp_corner_gz(x: mp.mpf, y: mp.mpf, z: mp.mpf) -> mp.mpf:
    """Antiderivative of ∫(z'−z_p)/r³ dV (the up component of ∇U) at one corner."""
    r = mp.sqrt(x * x + y * y + z * z)
    out = mp.mpf(0)
    if x != 0:
        out -= x * mp.log(y + r)
    if y != 0:
        out -= y * mp.log(x + r)
    if z != 0:
        out += z * mp.atan(x * y / (z * r))
    return out


def mp_box(lo: F64, hi: F64, p: F64) -> tuple[mp.mpf, mp.mpf]:
    """U and g_z (up) of the box `lo`..`hi` at `p`, 40 digits, by Nagy's corner sum."""
    u = mp.mpf(0)
    gz = mp.mpf(0)
    for i, x in enumerate((lo[0], hi[0])):
        for j, y in enumerate((lo[1], hi[1])):
            for k, z in enumerate((lo[2], hi[2])):
                s = 1 if (i + j + k) % 2 == 1 else -1
                xx, yy, zz = (mp.mpf(float(x)) - mp.mpf(float(p[0])),
                              mp.mpf(float(y)) - mp.mpf(float(p[1])),
                              mp.mpf(float(z)) - mp.mpf(float(p[2])))
                u += s * _mp_corner_u(xx, yy, zz)
                gz += s * _mp_corner_gz(xx, yy, zz)
    return u, gz


# ========================================= CODES =========================================


def ours_py(tris: F64, pts: F64) -> tuple[F64, F64]:
    """The study's numpy closed form, h_f·2A_f numerator: U (M,), g (M, 3)."""
    u, g = sb.polyhedron_field(tris, np.ones((len(tris), 1)), pts, stable = True)
    return u[0], g[0]


def _fixture(path: pathlib.Path, tris: F64 | None, cubes: F64 | None, h: float, pts: F64) -> None:
    lines: list[str] = []
    if tris is not None:
        lines.append(f"T {len(tris)}")
        lines += [" ".join(repr(float(x)) for x in t.reshape(-1)) for t in tris]
    if cubes is not None:
        lines.append(f"C {len(cubes)}")
        lines += [f"{c[0]!r} {c[1]!r} {c[2]!r} {h!r}" for c in cubes.tolist()]
    lines.append(f"P {len(pts)}")
    lines += [" ".join(repr(float(x)) for x in p) for p in pts]
    path.write_text("\n".join(lines) + "\n", encoding = "utf-8")


def ours_rs(tris: F64, pts: F64) -> tuple[F64, F64]:
    """Cavendish's `Faceted::field` at density 1: U (M,), g (M, 3)."""
    CACHE.mkdir(exist_ok = True)
    fx, out = CACHE / "field.txt", CACHE / "field.out"
    _fixture(fx, tris, None, 0.0, pts)
    subprocess.run([str(KERNEL_BENCH), "field", str(fx), str(out)], check = True)
    a = np.loadtxt(out, ndmin = 2)
    return a[:, 0], a[:, 1:4]


def cavendish_cubes(centres: F64, h: float, pts: F64) -> tuple[F64, F64]:
    """Σ over cubes of Cavendish's `cube_potential` and of `prism_potential`, as U = −V/G."""
    CACHE.mkdir(exist_ok = True)
    fx, out = CACHE / "cubes.txt", CACHE / "cubes.out"
    _fixture(fx, None, centres, h, pts)
    subprocess.run([str(KERNEL_BENCH), "cubes", str(fx), str(out)], check = True)
    a = np.loadtxt(out, ndmin = 2)
    return -a[:, 0], -a[:, 1]


def kernel_time(tris: F64 | None, cubes: F64 | None, h: float, pts: F64, repeats: int) -> dict[str, float]:
    """`kernel_bench time`: ns per point per triangle / per cube."""
    CACHE.mkdir(exist_ok = True)
    fx = CACHE / "time.txt"
    _fixture(fx, tris, cubes, h, pts)
    res = subprocess.run([str(KERNEL_BENCH), "time", str(fx), str(repeats)], check = True,
                         capture_output = True, text = True)
    out: dict[str, float] = {}
    for line in res.stdout.splitlines():
        k, v = line.split()
        if k != "sink":
            out[k] = float(v)
    return out


def mesh(tris: F64) -> tuple[list[list[float]], list[list[int]]]:
    """Triangle soup → shared vertices and index triples (ESA's integrity check wants a mesh)."""
    verts: dict[tuple[float, float, float], int] = {}
    faces = []
    for t in tris:
        faces.append([verts.setdefault((float(p[0]), float(p[1]), float(p[2])), len(verts)) for p in t])
    return [list(k) for k in verts], faces


def esa(tris: F64, pts: F64) -> tuple[F64, F64]:
    """ESA's polyhedral-gravity at density 1, UNITLESS: U (M,), g (M, 3) — its g is ∇U."""
    import polyhedral_gravity as pg

    verts, faces = mesh(tris)
    poly = pg.Polyhedron((verts, faces), 1.0, pg.NormalOrientation.OUTWARDS,
                         pg.PolyhedronIntegrity.VERIFY, pg.MetricUnit.UNITLESS)
    res = pg.evaluate(poly, pts.tolist(), parallel = False)
    return np.array([r[0] for r in res]), np.array([list(r[1]) for r in res])


def harmonica_boxes(boxes: F64, pts: F64, parallel: bool = False) -> tuple[F64, F64]:
    """Harmonica's prism sum at density 1 over `boxes` (N, 6: w e s n bottom top): U, g.

    Harmonica returns G·∫ρ/r in J/kg and the accelerations in mGal with g_z *downward*; this
    divides out G (choclo's own constant) and mGal, and flips g_z to the up component.
    """
    import harmonica as hm
    from choclo.constants import GRAVITATIONAL_CONST as GC

    coords = (pts[:, 0], pts[:, 1], pts[:, 2])
    dens = np.ones(len(boxes))
    kw = {"parallel": parallel, "disable_checks": True}
    u = hm.prism_gravity(coords, boxes, dens, field = "potential", **kw) / GC
    ge = hm.prism_gravity(coords, boxes, dens, field = "g_e", **kw) * 1e-5 / GC
    gn = hm.prism_gravity(coords, boxes, dens, field = "g_n", **kw) * 1e-5 / GC
    gz = -hm.prism_gravity(coords, boxes, dens, field = "g_z", **kw) * 1e-5 / GC
    return np.asarray(u), np.stack([ge, gn, gz], axis = 1)


def cube_boxes(centres: F64, h: float) -> F64:
    """Cube centres → Harmonica's (w, e, s, n, bottom, top) rows."""
    c = centres
    return np.stack([c[:, 0] - h / 2, c[:, 0] + h / 2, c[:, 1] - h / 2, c[:, 1] + h / 2,
                     c[:, 2] - h / 2, c[:, 2] + h / 2], axis = 1)
