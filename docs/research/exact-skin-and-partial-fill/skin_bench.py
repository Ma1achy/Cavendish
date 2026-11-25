"""How well do boundary voxels carry a body's gravity? 0/1 cells, fill fractions, fill plus centroid.

Each test body is a convex polyhedron (an icosphere standing in for a sphere, a turned cube, a turned
icosahedron, a turned thin plate). Its exact field is the polyhedral closed form (Werner & Scheeres
1997, written face by face). The body is laid on a cubic lattice of pitch h at a random sub-cell
offset; every cell the surface crosses is clipped against the body's planes, which gives the exact
inside piece (a convex polyhedron). Because interior cubes + pieces = the body exactly, the error of
any boundary representation is a sum over boundary cells only:

    error = Σ_boundary (approximate cell) − Σ_boundary (exact piece)          (+ the renormalisation)

so no interior cell is ever summed. The representations (each a uniform cube of side h unless said):

    zero_one      cell full if its centre is inside, mass renormalised to the body's (Cavendish, k = 1)
    zero_one_raw  the same, not renormalised
    ss4           fill = count of 4³ sample points inside / 64, at the cell centre, renormalised
                  (Cavendish today: supersample 4)
    frac          exact inside fraction, at the cell centre
    frac_cen      exact inside fraction, the cube moved to the piece's centroid
    frac_pt       exact inside fraction as a point mass at the piece's centroid
    frac_cen_q    frac_pt plus the piece's own second moment (a quadrupole) at its centroid
    (exact)       the pieces themselves: error zero by construction; checked by `selftest`

The read: a vertical 5 m baseline (AION-10's Δr), lower point at distance D from the body's centre
in three directions; the quantity is Δg_z = g_z(upper) − g_z(lower), and the phase k_eff T² Δg_z.

    python -I skin_bench.py selftest
    python -I skin_bench.py run sphere 8,16,32,64 4 > results/sphere.json

Bounded: at most 64 cells across a 1 m body (≈ 2×10⁴ boundary cells), a few minutes a body.
"""

from __future__ import annotations

import json
import math
import sys
import time

import numpy as np
import numpy.typing as npt

F64 = npt.NDArray[np.float64]

G = 6.674_30e-11
RHO = 2700.0  # kg/m³, every body
K_EFF = 2.0 * math.pi * 1000.0 / 698e-9  # LMT_KICKS · 2π / CLOCK_WAVELENGTH, rad/m
T = 0.73  # HALF_SEQUENCE, s
BASELINE = 5.0  # IFO_SEPARATION, m
DISTANCES = (1.0, 1.5, 3.0, 5.0, 10.0, 20.0)
DIRECTIONS = np.array([[1.0, 0.2, -0.3], [-0.4, 1.0, 0.1], [0.3, -0.5, -1.0]])
DIRECTIONS /= np.linalg.norm(DIRECTIONS, axis=1)[:, None]


# ================================ POLYHEDRAL FIELD ================================


def polyhedron_field(
    tris: F64, weights: F64, pts: F64, chunk: int = 2048, stable: bool = False
) -> tuple[F64, F64]:
    """Potential and attraction over G of a weighted triangle soup, at `pts`.

    `tris` (N, 3, 3): triangles wound anticlockwise seen from outside their solid. `weights`
    (N, V): the density of the solid each triangle bounds, for V weightings at once. Returns
    U (V, M), kg/m, positive (U = ∫ρ/r dV), and g = ∇U (V, M, 3), kg/m², pointing at the mass.

    Face by face (Werner & Scheeres 1997, eq. 15–16, the edge dyads split between their two faces):
    U_f = ½ h_f [Σ_e (n_e·r_e) L_e − h_f ω_f], g_f = −n_f [Σ_e (n_e·r_e) L_e − h_f ω_f], with
    r the offset from the point to the face, h_f = n_f·r, L_e = ln((a+b+e)/(a+b−e)) and ω_f the
    signed solid angle (Van Oosterom & Strackee 1983).

    `stable = True` forms the solid angle's numerator as h_f·2A_f from the face's own edge vectors
    instead of the triple product r₁·(r₂×r₃): ε(R/s)² far-field rounding instead of ε(R/s)³ (§1.1 of
    the note). The default keeps the numbers in `results/` reproducible.
    """
    m = len(pts)
    v = weights.shape[1]
    u_out = np.zeros((v, m))
    g_out = np.zeros((v, m, 3))
    for s in range(0, len(tris), chunk):
        t = tris[s : s + chunk]
        w = weights[s : s + chunk]
        a, b, c = t[:, 0], t[:, 1], t[:, 2]
        n = np.cross(b - a, c - a)
        twice_area = np.linalg.norm(n, axis=1)
        n /= twice_area[:, None]
        r = [x[:, None, :] - pts[None, :, :] for x in (a, b, c)]  # (n, m, 3) each
        ln = [np.linalg.norm(x, axis=2) for x in r]
        h = np.einsum("nk,nmk->nm", n, r[0])
        if stable:
            triple = h * twice_area[:, None]
        else:
            triple = np.einsum("nmk,nmk->nm", r[0], np.cross(r[1], r[2]))
        den = (
            ln[0] * ln[1] * ln[2]
            + ln[0] * np.einsum("nmk,nmk->nm", r[1], r[2])
            + ln[1] * np.einsum("nmk,nmk->nm", r[2], r[0])
            + ln[2] * np.einsum("nmk,nmk->nm", r[0], r[1])
        )
        omega = 2.0 * np.arctan2(triple, den)
        edges = np.zeros_like(h)
        for i in range(3):
            j = (i + 1) % 3
            e = t[:, j] - t[:, i]
            el = np.linalg.norm(e, axis=1)
            ne = np.cross(e, n) / el[:, None]  # outward in the face's plane
            ab = ln[i] + ln[j]
            big_l = np.log1p(2.0 * el[:, None] / (ab - el[:, None]))
            edges += np.einsum("nk,nmk->nm", ne, r[i]) * big_l
        bracket = edges - h * omega
        u_out += np.einsum("nv,nm->vm", w, 0.5 * h * bracket)
        g_out += -np.einsum("nv,nm,nk->vmk", w, bracket, n)
    return u_out, g_out


def quads_to_tris(quads: F64) -> F64:
    """(N, 4, 3) anticlockwise quads → (2N, 3, 3) triangles."""
    return np.concatenate([quads[:, [0, 1, 2]], quads[:, [0, 2, 3]]])


def cube_quads(centres: F64, side: float) -> F64:
    """The six faces of axis-aligned cubes, anticlockwise from outside: (6N, 4, 3)."""
    hs = 0.5 * side
    unit = []
    for axis in range(3):
        for sign in (-1.0, 1.0):
            u, v = (axis + 1) % 3, (axis + 2) % 3
            quad = np.zeros((4, 3))
            quad[:, axis] = sign * hs
            for i, (p, q) in enumerate([(-1, -1), (1, -1), (1, 1), (-1, 1)]):
                quad[i, u], quad[i, v] = p * hs, q * hs
            normal = np.cross(quad[1] - quad[0], quad[2] - quad[0])
            if normal[axis] * sign < 0:
                quad = quad[::-1]
            unit.append(quad)
    unit_arr = np.array(unit)  # (6, 4, 3)
    return (centres[:, None, None, :] + unit_arr[None]).reshape(-1, 4, 3)


def cube_tris(centres: F64, side: float) -> F64:
    """12 triangles per cube, cube-major: (12N, 3, 3)."""
    q = cube_quads(centres, side).reshape(len(centres), 6, 4, 3)
    return np.concatenate([q[:, :, [0, 1, 2]], q[:, :, [0, 2, 3]]], axis=1).reshape(-1, 3, 3)


# ================================ BODIES ================================


def rotation(seed: int) -> F64:
    """A fixed random rotation (QR of a Gaussian matrix)."""
    q, r = np.linalg.qr(np.random.default_rng(seed).normal(size=(3, 3)))
    q *= np.sign(np.diag(r))
    return q if np.linalg.det(q) > 0 else -q


def icosahedron() -> tuple[F64, list[tuple[int, int, int]]]:
    p = (1 + 5**0.5) / 2
    v = np.array(
        [[-1, p, 0], [1, p, 0], [-1, -p, 0], [1, -p, 0], [0, -1, p], [0, 1, p],
         [0, -1, -p], [0, 1, -p], [p, 0, -1], [p, 0, 1], [-p, 0, -1], [-p, 0, 1]],
        dtype=float,
    )
    f = [(0, 11, 5), (0, 5, 1), (0, 1, 7), (0, 7, 10), (0, 10, 11), (1, 5, 9), (5, 11, 4),
         (11, 10, 2), (10, 7, 6), (7, 1, 8), (3, 9, 4), (3, 4, 2), (3, 2, 6), (3, 6, 8),
         (3, 8, 9), (4, 9, 5), (2, 4, 11), (6, 2, 10), (8, 6, 7), (9, 8, 1)]
    return v / np.linalg.norm(v[0]), f


def icosphere(levels: int) -> tuple[F64, list[tuple[int, int, int]]]:
    v, f = icosahedron()
    verts = list(v)
    for _ in range(levels):
        cache: dict[tuple[int, int], int] = {}

        def mid(i: int, j: int) -> int:
            key = (min(i, j), max(i, j))
            if key not in cache:
                p = verts[i] + verts[j]
                verts.append(p / np.linalg.norm(p))
                cache[key] = len(verts) - 1
            return cache[key]

        f = [t for a, b, c in f for t in
             ((a, mid(a, b), mid(a, c)), (b, mid(b, c), mid(a, b)), (c, mid(a, c), mid(b, c)),
              (mid(a, b), mid(b, c), mid(a, c)))]
    return np.array(verts), f


def body(name: str) -> F64:
    """The body's triangles, outward, centred on the origin, metres."""
    if name in ("sphere", "icosahedron"):
        v, f = icosphere(3) if name == "sphere" else icosahedron()
        v = v * (0.5 if name == "sphere" else 0.6)
        tris = np.array([[v[a], v[b], v[c]] for a, b, c in f])
    else:
        size = np.array([1.0, 1.0, 0.05]) if name == "plate" else np.array([0.8, 0.8, 0.8])
        tris = cube_tris(np.zeros((1, 3)), 1.0) * size
    if name != "cube_aligned":  # the aligned cube: faces on lattice planes, 0/1's worst case
        tris = tris @ rotation({"sphere": 1, "icosahedron": 2, "cube": 3, "plate": 4}[name]).T
    # Outward winding: a convex body about the origin has n·(face centroid) > 0.
    n = np.cross(tris[:, 1] - tris[:, 0], tris[:, 2] - tris[:, 0])
    flip = np.einsum("nk,nk->n", n, tris.mean(axis=1)) < 0
    tris[flip] = tris[flip][:, ::-1]
    return tris


def planes_of(tris: F64) -> tuple[F64, F64]:
    """Unit normals and offsets (n·x ≤ d inside) of a convex body's faces, merged where coplanar."""
    n = np.cross(tris[:, 1] - tris[:, 0], tris[:, 2] - tris[:, 0])
    n /= np.linalg.norm(n, axis=1)[:, None]
    d = np.einsum("nk,nk->n", n, tris[:, 0])
    key = np.round(np.column_stack([n, d]), 9)
    _, keep = np.unique(key, axis=0, return_index=True)
    return n[np.sort(keep)], d[np.sort(keep)]


def second_moment(tris: F64, centre: F64) -> F64:
    """∫(x − c)(x − c)ᵀ dV of the solid bounded by `tris`, about `centre` (tetrahedra from it)."""
    a, b, c = (tris[:, i] - centre for i in range(3))
    vol = np.einsum("nk,nk->n", a, np.cross(b, c)) / 6.0
    s = a + b + c
    outer = sum(np.einsum("ni,nj->nij", v, v) for v in (a, b, c)) + np.einsum("ni,nj->nij", s, s)
    return (vol[:, None, None] / 20.0 * outer).sum(axis=0)


def volume_centroid(tris: F64, origin: F64) -> tuple[float, F64]:
    a, b, c = (tris[:, i] - origin for i in range(3))
    vol = np.einsum("nk,nk->n", a, np.cross(b, c)) / 6.0
    total = float(vol.sum())
    return total, origin + (vol[:, None] * (a + b + c) / 4.0).sum(axis=0) / total


# ================================ CLIPPING ================================


def clip(faces: list[F64], n: F64, d: float, scale: float) -> list[F64]:
    """The convex polyhedron `faces` (anticlockwise polygons) cut to n·x ≤ d, with its cap."""
    out: list[F64] = []
    cut: list[F64] = []
    for f in faces:
        s = f @ n - d
        s[np.abs(s) < 1e-13 * scale] = 0.0
        if np.all(s <= 0):
            out.append(f)
            cut.extend(f[s == 0])
            continue
        if np.all(s >= 0):
            cut.extend(f[s == 0])
            continue
        poly = []
        k = len(f)
        for i in range(k):
            p, q, sp, sq = f[i], f[(i + 1) % k], s[i], s[(i + 1) % k]
            if sp <= 0:
                poly.append(p)
            if sp == 0:
                cut.append(p)
            if (sp < 0 < sq) or (sq < 0 < sp):
                x = p + (q - p) * (sp / (sp - sq))
                poly.append(x)
                cut.append(x)
        if len(poly) >= 3:
            out.append(np.array(poly))
    if len(cut) >= 3 and out:
        pts = np.array(cut)
        keep = [0]
        for i in range(1, len(pts)):
            if np.min(np.linalg.norm(pts[keep] - pts[i], axis=1)) > 1e-12 * scale:
                keep.append(i)
        pts = pts[keep]
        if len(pts) >= 3:
            m = pts.mean(axis=0)
            u = np.cross(n, [1.0, 0.0, 0.0] if abs(n[0]) < 0.9 else [0.0, 1.0, 0.0])
            u /= np.linalg.norm(u)
            w = np.cross(n, u)
            ang = np.arctan2((pts - m) @ w, (pts - m) @ u)
            out.append(pts[np.argsort(ang)])
    return out


def fan(faces: list[F64], scale: float) -> F64:
    tris = [np.array([f[0], f[i], f[i + 1]]) for f in faces for i in range(1, len(f) - 1)]
    t = np.array(tris)
    area = np.linalg.norm(np.cross(t[:, 1] - t[:, 0], t[:, 2] - t[:, 0]), axis=1)
    return t[area > 1e-20 * scale * scale]


# ================================ THE LATTICE ================================


def lattice(tris: F64, h: float, offset: F64) -> dict[str, F64 | int]:
    """Classify every cell of the lattice (origin `offset`, pitch `h`) and clip the boundary ones."""
    pn, pd = planes_of(tris)
    verts = tris.reshape(-1, 3)
    lo = np.floor((verts.min(axis=0) - offset) / h).astype(int)
    hi = np.floor((verts.max(axis=0) - offset) / h).astype(int)
    axes = [np.arange(lo[i], hi[i] + 1) for i in range(3)]
    ijk = np.stack(np.meshgrid(*axes, indexing="ij"), axis=-1).reshape(-1, 3)
    centres = offset + (ijk + 0.5) * h
    support = 0.5 * h * np.abs(pn).sum(axis=1)  # the cube's reach along each normal
    n_inside = 0
    boundary: list[int] = []
    planes: list[F64] = []
    for s in range(0, len(centres), 8192):
        c = centres[s : s + 8192]
        dist = c @ pn.T - pd[None, :]
        separated = np.any(dist - support[None, :] >= 0, axis=1)
        violated = dist + support[None, :] > 0
        inside = ~np.any(violated, axis=1)
        n_inside += int(inside.sum())
        for i in np.nonzero(~separated & ~inside)[0]:
            boundary.append(s + i)
            v = np.nonzero(violated[i])[0]
            planes.append(v[np.argsort(-dist[i, v])])  # the deepest cut first
    sub = (np.arange(4) + 0.5) / 4 - 0.5
    samples = h * np.stack(np.meshgrid(sub, sub, sub, indexing="ij"), axis=-1).reshape(-1, 3)
    keep_c, frac, ss, ctr_in, cen, piece_tris, second = [], [], [], [], [], [], []
    cube0 = [q for q in cube_quads(np.zeros((1, 3)), h)]
    for idx, pl in zip(boundary, planes):
        c = centres[idx]
        faces = [q + c for q in cube0]
        for p in pl:
            if max(float(np.max(f @ pn[p])) for f in faces) <= pd[p]:
                continue  # the piece so far is already inside this plane
            faces = clip(faces, pn[p], float(pd[p]), h)
            if not faces:
                break
        if not faces:
            continue
        t = fan(faces, h)
        if len(t) < 4:
            continue
        vol, centroid = volume_centroid(t, c)
        if vol <= 1e-15 * h**3:
            continue
        keep_c.append(c)
        second.append(second_moment(t, centroid))
        frac.append(vol / h**3)
        cen.append(centroid)
        piece_tris.append(t)
        x = c + samples
        ss.append(np.all(x @ pn[pl].T <= pd[pl][None, :], axis=1).sum() / 64.0)
        ctr_in.append(float(np.all(c @ pn[pl].T <= pd[pl])))
    return {
        "centres": np.array(keep_c), "frac": np.array(frac), "ss": np.array(ss),
        "ctr_in": np.array(ctr_in), "centroids": np.array(cen),
        "pieces": np.concatenate(piece_tris), "n_inside": n_inside, "second": np.array(second),
    }


def arm_points() -> F64:
    lower = np.array([d * u for d in DISTANCES for u in DIRECTIONS])
    return np.concatenate([lower, lower + [0.0, 0.0, BASELINE]])


def errors(name: str, n_across: int, seed: int) -> dict[str, object]:
    """Every representation's error at the arm points, for one lattice placement."""
    tris = body(name)
    h = 1.0 / n_across
    offset = np.random.default_rng(1000 * n_across + seed).uniform(0, h, 3)
    t0 = time.perf_counter()
    lat = lattice(tris, h, offset)
    t_clip = time.perf_counter() - t0
    pts = arm_points()
    vol_body, _ = volume_centroid(tris, np.zeros(3))
    one = lambda t: np.ones((len(t), 1))  # noqa: E731
    u_b, g_b = polyhedron_field(tris, one(tris), pts)
    u_p, g_p = polyhedron_field(lat["pieces"], one(lat["pieces"]), pts)
    ctr, frac, ss, cen = lat["ctr_in"], lat["frac"], lat["ss"], lat["centroids"]
    w = np.repeat(np.column_stack([ctr, ss, frac]), 12, axis=0)
    u_c, g_c = polyhedron_field(cube_tris(lat["centres"], h), w, pts)
    u_m, g_m = polyhedron_field(cube_tris(cen, h), np.repeat(frac[:, None], 12, axis=0), pts)
    r = cen[None, :, :] - pts[:, None, :]
    rl = np.linalg.norm(r, axis=2)
    mass = frac * h**3
    u_pt = (mass[None, :] / rl).sum(axis=1)
    g_pt = (mass[None, :, None] * r / rl[..., None] ** 3).sum(axis=1)
    # Point mass + the piece's own second moment (quadrupole) at its centroid:
    # U₂ = ½ I_ij ∂_i∂_j(1/R), R = centroid − point; g = −∂U/∂R.
    big_i = lat["second"]
    tr = np.trace(big_i, axis1=1, axis2=2)
    ir = np.einsum("cij,pcj->pci", big_i, r)
    rir = np.einsum("pci,pci->pc", r, ir)
    r2 = rl**2
    u_q = u_pt + 0.5 * ((3 * rir - r2 * tr[None, :]) / rl**5).sum(axis=1)
    du_dr = 0.5 * (6 * ir / rl[..., None] ** 5 - 2 * tr[None, :, None] * r / rl[..., None] ** 5
                   - 5 * ((3 * rir - r2 * tr[None, :]) / rl**7)[..., None] * r)
    g_q = g_pt - du_dr.sum(axis=1)
    n_in = lat["n_inside"]
    s1 = vol_body / (h**3 * (n_in + ctr.sum()))
    s4 = vol_body / (h**3 * (n_in + ss.sum()))
    t_fields = time.perf_counter() - t0 - t_clip
    rep = {
        "zero_one": ((s1 - 1) * (u_b[0] - u_p[0]) + s1 * u_c[0] - u_p[0],
                     (s1 - 1) * (g_b[0] - g_p[0]) + s1 * g_c[0] - g_p[0]),
        "zero_one_raw": (u_c[0] - u_p[0], g_c[0] - g_p[0]),
        "ss4": ((s4 - 1) * (u_b[0] - u_p[0]) + s4 * u_c[1] - u_p[0],
                (s4 - 1) * (g_b[0] - g_p[0]) + s4 * g_c[1] - g_p[0]),
        "frac": (u_c[2] - u_p[0], g_c[2] - g_p[0]),
        "frac_cen": (u_m[0] - u_p[0], g_m[0] - g_p[0]),
        "frac_pt": (u_pt - u_p[0], g_pt - g_p[0]),
        "frac_cen_q": (u_q - u_p[0], g_q - g_p[0]),
    }
    k = len(DISTANCES) * len(DIRECTIONS)
    signal = (g_b[0][k:, 2] - g_b[0][:k, 2]) * G * RHO
    out: dict[str, object] = {
        "body": name, "n": n_across, "seed": seed, "h": h, "boundary_cells": len(frac),
        "inside_cells": n_in, "seconds_clip": t_clip, "seconds_fields": t_fields,
        "signal_dgz": signal.tolist(), "signal_u": (u_b[0][:k] * G * RHO).tolist(),
    }
    for key, (du, dg) in rep.items():
        dgz = (dg[k:, 2] - dg[:k, 2]) * G * RHO
        out[key] = {"dgz": dgz.tolist(), "u": (du[:k] * G * RHO).tolist()}
    return out


# ================================ SELF-TEST ================================


def nagy(lo: F64, hi: F64) -> float:
    """∫dV/r over a box, corners as offsets from the point (Nagy 1966; kernel.rs's form)."""
    def corner(x: float, y: float, z: float) -> float:
        r = math.sqrt(x * x + y * y + z * z)
        lg = lambda c, u: 0.0 if c == 0 else c * math.log(u + r)  # noqa: E731
        an = lambda p, q, w: 0.0 if p == 0 else 0.5 * p * p * math.atan(q * w / (p * r))  # noqa: E731
        return lg(x * y, z) + lg(y * z, x) + lg(z * x, y) - an(x, y, z) - an(y, z, x) - an(z, x, y)
    total = 0.0
    for i, x in enumerate((lo[0], hi[0])):
        for j, y in enumerate((lo[1], hi[1])):
            for k, z in enumerate((lo[2], hi[2])):
                total += (-1.0 if (i + j + k) % 2 == 0 else 1.0) * corner(x, y, z)
    return -total


def selftest() -> None:
    """Pass criteria written first: the closed form equals Nagy's prism to 1e-12 relative near and
    the monopole + l=4 series to 1e-9 relative far; g equals −∇ of a central difference of U to
    1e-6; interior cubes + clipped pieces equal the whole body to 1e-12 relative."""
    side = 0.3
    tris = cube_tris(np.zeros((1, 3)), side)
    pts = np.array([[0.4, 0.1, -0.2], [0.2, 0.05, 0.16], [3.0, 1.0, 2.0]])
    u, g = polyhedron_field(tris, np.ones((len(tris), 1)), pts)
    for p, val in zip(pts, u[0]):
        ref = -nagy(-p - side / 2, -p + side / 2)
        print(f"cube vs Nagy at {p}: {val:.15e} {ref:.15e} rel {abs(val / ref - 1):.1e}")
    p = np.array([[40.0, 13.0, -21.0]])
    u, g = polyhedron_field(tris, np.ones((len(tris), 1)), p)
    rr = float(np.linalg.norm(p))
    nn = p[0] / rr
    q4 = -(7.0 / 60.0) * (5 * np.sum(nn**4) - 3)
    series = side**3 * (1 / rr + (side / 2) ** 4 * q4 / rr**5)
    print(f"cube far (r = {rr:.0f} m, {rr / side:.0f} sides) vs series: rel {abs(u[0, 0] / series - 1):.1e}")
    for side_far in (1 / 64, 1 / 128):
        t2 = cube_tris(np.zeros((1, 3)), side_far)
        u2, g2 = polyhedron_field(t2, np.ones((len(t2), 1)), p)
        mono = side_far**3 / rr
        gm = -side_far**3 * p[0] / rr**3
        print(f"cube of side {side_far:.4f} at {rr:.1f} m: U rel {abs(u2[0, 0] / mono - 1):.1e}, "
              f"g rel {np.linalg.norm(g2[0, 0] - gm) / np.linalg.norm(gm):.1e}")
    eps = 1e-5
    q = np.array([[0.5, 0.2, -0.1]])
    u0, g0 = polyhedron_field(tris, np.ones((len(tris), 1)), q)
    fd = [(polyhedron_field(tris, np.ones((len(tris), 1)), q + eps * e)[0][0, 0]
           - polyhedron_field(tris, np.ones((len(tris), 1)), q - eps * e)[0][0, 0]) / (2 * eps)
          for e in np.eye(3)]
    print(f"g vs ∇U: {g0[0, 0]} {np.array(fd)} rel {np.linalg.norm(g0[0, 0] - fd) / np.linalg.norm(fd):.1e}")
    for name in ("sphere", "cube", "icosahedron", "plate"):
        tris = body(name)
        h = 1.0 / 8
        offset = np.random.default_rng(7).uniform(0, h, 3)
        lat = lattice(tris, h, offset)
        pn, pd = planes_of(tris)
        verts = tris.reshape(-1, 3)
        lo = np.floor((verts.min(axis=0) - offset) / h).astype(int)
        hi = np.floor((verts.max(axis=0) - offset) / h).astype(int)
        axes = [np.arange(lo[i], hi[i] + 1) for i in range(3)]
        cs = offset + (np.stack(np.meshgrid(*axes, indexing="ij"), -1).reshape(-1, 3) + 0.5) * h
        sup = 0.5 * h * np.abs(pn).sum(axis=1)
        inside = cs[np.all(cs @ pn.T - pd + sup <= 0, axis=1)]
        pts = arm_points()[:6]
        one = lambda t: np.ones((len(t), 1))  # noqa: E731
        ub, gb = polyhedron_field(tris, one(tris), pts)
        ct = cube_tris(inside, h)
        ui, gi = polyhedron_field(ct, one(ct), pts) if len(inside) else (0 * ub, 0 * gb)
        up, gp = polyhedron_field(lat["pieces"], one(lat["pieces"]), pts)
        vb, _ = volume_centroid(tris, np.zeros(3))
        vsum = len(inside) * h**3 + lat["frac"].sum() * h**3
        print(f"{name}: {len(inside)} inside + {len(lat['frac'])} pieces; volume rel {abs(vsum / vb - 1):.1e}; "
              f"U rel {np.max(np.abs((ui + up - ub) / ub)):.1e}; "
              f"g rel {np.max(np.linalg.norm(gi + gp - gb, axis=2) / np.linalg.norm(gb, axis=2)):.1e}")


if __name__ == "__main__":
    if sys.argv[1] == "selftest":
        selftest()
    else:
        _, _, name, ns, seeds = sys.argv
        runs = [errors(name, int(n), s) for n in ns.split(",") for s in range(int(seeds))]
        json.dump(runs, sys.stdout)
