"""Shared pieces for the shape studies: the meshes, a voxeliser that clips cells by any closed mesh, the
fields of every representation, and a small z-buffered renderer.

    meshes      sphere (icosphere, 5120 triangles), torus (64 × 32 quads), Utah teapot, Suzanne
    voxelise    mesh + lattice -> interior cells, boundary cells with their exact clipped pieces
    fields      g of a mesh, of cubes, of point masses, of point + quadrupole, of a body multipole
    Canvas      orthographic z-buffer rasteriser for the pictures (no window, no OpenGL)

It reuses `skin_bench.py` one directory up: `polyhedron_field` (Werner & Scheeres' closed form, with
its new `stable` numerator), `cube_tris`, `cube_quads`, `icosphere`, `fan`, `volume_centroid`,
`second_moment` and the AION-10 constants. Its `clip` (a convex polyhedron cut by one plane) is
replaced here by `_clip_to_box`, the same Sutherland–Hodgman step for axis walls in plain floats,
about ten times faster per call, which is what makes ~10⁴ boundary cells a few seconds.

The clipping (`voxelise`) works for any closed, consistently wound mesh, convex or not. A boundary
cell's piece is bounded by (1) the mesh triangles clipped to the cell and (2) a cap on each cell wall,
the part of the wall inside the solid. A cap's boundary is the reversed wall edges of (1) plus the
parts of the cell's edges that lie inside the solid; the latter come from casting lattice lines through
the mesh (winding number along each line). The cap is then a signed fan of triangles from the wall's
centre, which is exact for the polyhedral field (the field is linear in signed faces).

Meshes not in the repo (teapot, Suzanne) are downloaded at run time from a pinned commit and checked
against a SHA-256; they are cached in `shapes/.cache/`, which git ignores.
"""

from __future__ import annotations

import hashlib
import math
import sys
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
import numpy.typing as npt

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
import skin_bench as sb  # noqa: E402

F64 = npt.NDArray[np.float64]
I64 = npt.NDArray[np.int64]

G = sb.G
RHO = sb.RHO  # kg/m³, every body
PHASE_PER_DG = sb.K_EFF * sb.T**2  # rad per m s⁻² of Δg_z over the baseline: 4.80e9
BASELINE = sb.BASELINE  # m
CACHE = HERE / ".cache"

# A lattice origin off every symmetric position (in units of h), so no mesh vertex, edge or face
# lands exactly on a lattice line or wall. Small enough that the pictures look aligned.
GENERIC = np.array([0.001_37, 0.002_91, 0.000_73])


# ================================ MESHES ================================


@dataclass(frozen=True)
class Mesh:
    """A closed triangle mesh, wound anticlockwise seen from outside, metres.

    Attributes:
        name   (str):         Short name used in file names.
        verts  (F64):         Vertices, (V, 3).
        faces  (I64):         Triangles as vertex indices, (F, 3).
        source (str):         Where it came from and what was done to it, for captions.
    """

    name:   str
    verts:  F64
    faces:  I64
    source: str

    @property
    def tris(self) -> F64:
        return self.verts[self.faces]

    def moved(self, rot: F64 | None = None, shift: F64 | None = None) -> Mesh:
        v = self.verts if rot is None else self.verts @ rot.T
        v = v if shift is None else v + shift
        return Mesh(self.name, v, self.faces, self.source)


def check_closed(faces: I64) -> tuple[bool, str]:
    """True if every directed edge appears once and its reverse once (closed, consistently wound)."""
    e = np.concatenate([faces[:, [0, 1]], faces[:, [1, 2]], faces[:, [2, 0]]]).astype(np.int64)
    big = int(faces.max()) + 1
    key = e[:, 0] * big + e[:, 1]
    rev = e[:, 1] * big + e[:, 0]
    uniq, counts = np.unique(key, return_counts=True)
    dup = int((counts > 1).sum())
    missing = int((~np.isin(rev, uniq)).sum())
    ok = dup == 0 and missing == 0
    return ok, f"{len(faces)} triangles, {dup} repeated directed edges, {missing} edges without a twin"


def _oriented(verts: F64, faces: I64) -> I64:
    vol = np.einsum("nk,nk->n", verts[faces[:, 0]], np.cross(verts[faces[:, 1]], verts[faces[:, 2]]))
    return faces if vol.sum() > 0 else faces[:, ::-1].copy()


def sphere_mesh(radius: float = 1.0) -> Mesh:
    v, f = sb.icosphere(4)
    faces = np.array(f, dtype=np.int64)
    v = v * radius
    n = np.cross(v[faces[:, 1]] - v[faces[:, 0]], v[faces[:, 2]] - v[faces[:, 0]])
    flip = np.einsum("nk,nk->n", n, v[faces].mean(axis=1)) < 0
    faces[flip] = faces[flip][:, ::-1]
    return Mesh("sphere", v, faces, f"icosphere of 5120 triangles, vertices on the sphere of radius {radius} m")


def torus_mesh(big_r: float = 1.0, small_r: float = 0.4, nu: int = 64, nv: int = 32) -> Mesh:
    u = 2 * np.pi * np.arange(nu) / nu
    w = 2 * np.pi * np.arange(nv) / nv
    uu, ww = np.meshgrid(u, w, indexing="ij")
    rho = big_r + small_r * np.cos(ww)
    v = np.stack([rho * np.cos(uu), rho * np.sin(uu), small_r * np.sin(ww)], axis=-1).reshape(-1, 3)
    i, j = np.meshgrid(np.arange(nu), np.arange(nv), indexing="ij")
    a = i * nv + j
    b = ((i + 1) % nu) * nv + j
    c = ((i + 1) % nu) * nv + (j + 1) % nv
    d = i * nv + (j + 1) % nv
    faces = np.concatenate([np.stack([a, b, c], -1).reshape(-1, 3), np.stack([a, c, d], -1).reshape(-1, 3)])
    return Mesh("torus", v, _oriented(v, faces.astype(np.int64)),
                f"{nu} × {nv} quads split into {2 * nu * nv} triangles, vertices on the torus "
                f"R = {big_r} m, r = {small_r} m")


# Pinned sources (alecjacobson/common-3d-test-models at a fixed commit). Not redistributed here.
_COMMIT = "8a4f8642acaf43f9cd7b67858a1502e1055ef202"
SOURCES = {
    "teapot": (f"https://raw.githubusercontent.com/alecjacobson/common-3d-test-models/{_COMMIT}/data/teapot.obj",
               "1b5396fedd74b577e32cef41146582c2f2e1a050d5b4915193c0ac1ad4187ed4"),
    "suzanne": (f"https://raw.githubusercontent.com/alecjacobson/common-3d-test-models/{_COMMIT}/data/suzanne.obj",
                "d8684326f9bd8cfc24d3d302c1042fa16f63d2e66e49ed56b413fa20bed271e6"),
}


def _download(name: str) -> Path:
    url, sha = SOURCES[name]
    CACHE.mkdir(exist_ok=True)
    path = CACHE / f"{name}.obj"
    if not path.exists():
        with urllib.request.urlopen(url, timeout=60) as r:  # noqa: S310 (pinned https URL)
            path.write_bytes(r.read())
    got = hashlib.sha256(path.read_bytes()).hexdigest()
    if got != sha:
        path.unlink()
        raise RuntimeError(f"{name}: SHA-256 {got} does not match the pinned {sha}")
    return path


def _read_obj(path: Path) -> tuple[F64, I64]:
    verts: list[list[float]] = []
    faces: list[list[int]] = []
    for line in path.read_text().splitlines():
        p = line.split()
        if not p:
            continue
        if p[0] == "v":
            verts.append([float(x) for x in p[1:4]])
        elif p[0] == "f":
            idx = [int(q.split("/")[0]) for q in p[1:]]
            idx = [i - 1 if i > 0 else len(verts) + i for i in idx]
            faces.extend([idx[0], idx[k], idx[k + 1]] for k in range(1, len(idx) - 1))
    return np.array(verts), np.array(faces, dtype=np.int64)


def winding(tris: F64, pts: F64) -> F64:
    """Generalised winding number Σω/4π of a triangle soup at `pts` (Jacobson et al. 2013)."""
    out = np.zeros(len(pts))
    step = max(1, 2_000_000 // len(tris))
    for s in range(0, len(pts), step):
        p = pts[s : s + step]
        r = [tris[:, i][:, None, :] - p[None] for i in range(3)]
        ln = [np.linalg.norm(x, axis=2) for x in r]
        triple = np.einsum("nmk,nmk->nm", r[0], np.cross(r[1], r[2]))
        den = (ln[0] * ln[1] * ln[2] + ln[0] * np.einsum("nmk,nmk->nm", r[1], r[2])
               + ln[1] * np.einsum("nmk,nmk->nm", r[2], r[0]) + ln[2] * np.einsum("nmk,nmk->nm", r[0], r[1]))
        out[s : s + step] = (2.0 * np.arctan2(triple, den)).sum(axis=0) / (4 * np.pi)
    return out


def closed_from_soup(name: str, verts: F64, faces: I64, n: int, up: tuple[int, int, int]) -> Mesh:
    """Close an open or self-intersecting mesh: the zero set of a signed distance whose sign is the
    generalised winding number's (inside where it exceeds 0.5).

    The raw mesh is turned so `up` (its up axis, signed index list) is +z, centred on its bounding box
    and scaled so its largest extent is 1 m. The winding number is sampled on a grid of `n` cells
    across that 1 m (grid offset off-lattice). Within two grid steps of a change of side, the exact
    distance to the raw triangles gives the magnitude (the winding number alone jumps by 1 across the
    surface, and marching cubes on it terraces); further out the magnitude is clamped. Marching cubes
    (Lewiner, scikit-image) extracts the zero set, which is closed and consistently wound by
    construction. The result is checked (`check_closed`) and rejected if it is not.
    """
    from scipy import ndimage
    from skimage import measure

    cache = CACHE / f"{name}-closed-sdf-{n}.npz"
    perm = [abs(a) - 1 for a in up]
    sign = np.array([1.0 if a > 0 else -1.0 for a in up])
    v = verts[:, perm] * sign
    if np.linalg.det(np.eye(3)[perm] * sign[:, None]) < 0:
        faces = faces[:, ::-1]
    lo, hi = v.min(axis=0), v.max(axis=0)
    v = (v - 0.5 * (lo + hi)) / float((hi - lo).max())
    if cache.exists():
        z = np.load(cache)
        mv, mf = z["v"], z["f"]
    else:
        s = 1.0 / n
        o = v.min(axis=0) - 3 * s + s * np.array([0.318, 0.427, 0.271])
        shape = np.ceil((v.max(axis=0) - o) / s).astype(int) + 3
        grid = np.stack(np.meshgrid(*[o[i] + s * np.arange(shape[i]) for i in range(3)], indexing="ij"), -1)
        wn_cache = CACHE / f"{name}-winding-{n}.npy"
        if wn_cache.exists():
            wn = np.load(wn_cache)
        else:
            wn = winding(v[faces], grid.reshape(-1, 3)).reshape(tuple(shape))
            CACHE.mkdir(exist_ok=True)
            np.save(wn_cache, wn)
        if np.median(wn[wn.shape[0] // 2]) < -0.25:  # the soup is wound inside out
            wn = -wn
        inside = wn > 0.5
        edge = np.zeros_like(inside)
        for a in range(3):
            diff = np.diff(inside, axis=a)
            lo_idx = [slice(None)] * 3
            hi_idx = [slice(None)] * 3
            lo_idx[a], hi_idx[a] = slice(0, -1), slice(1, None)
            edge[tuple(lo_idx)] |= diff
            edge[tuple(hi_idx)] |= diff
        band = ndimage.binary_dilation(edge, iterations=2)
        dist = np.full(inside.shape, 3.0 * s)
        tris_raw = v[faces]
        for idx in np.argwhere(band):
            dist[tuple(idx)] = distance_to_mesh(tris_raw, grid[tuple(idx)])
        sdf = np.where(inside, dist, -dist)
        mv, mf, _, _ = measure.marching_cubes(sdf, level=0.0, spacing=(s, s, s), allow_degenerate=False)
        mv = mv + o
        mf = mf.astype(np.int64)
        CACHE.mkdir(exist_ok=True)
        np.savez(cache, v=mv, f=mf)
    mf = _oriented(mv, mf)
    ok, why = check_closed(mf)
    if not ok:
        raise RuntimeError(f"{name}: closed mesh rejected: {why}")
    return Mesh(name, mv, mf, f"marching cubes on a 1/{n} m grid, {len(mf)} triangles")


def teapot_mesh() -> Mesh:
    v, f = _read_obj(_download("teapot"))
    m = closed_from_soup("teapot", v, f, 48, (1, -3, 2))
    return Mesh(m.name, m.verts, m.faces,
                "Utah teapot (Newell 1975) as 6320 triangles in four separate open surfaces (body, lid, spout, "
                "handle; open bottom, unwelded patch seams: 1036 open edges; spout and handle pushed through "
                "the body), closed by the sign of its generalised winding number and the exact distance to "
                "its triangles: a solid teapot, " + m.source)


def suzanne_mesh() -> Mesh:
    v, f = _read_obj(_download("suzanne"))
    m = closed_from_soup("suzanne", v, f, 48, (-1, 3, 2))  # facing +y, towards the camera
    return Mesh(m.name, m.verts, m.faces,
                "Blender's Suzanne as 968 triangles in three parts (the head: 856 triangles, 26 open edges, "
                "1 non-manifold edge; two eyes: 56 triangles each, open at the back), closed by the sign of "
                "its generalised winding number and the exact distance to its triangles (eyes merged into "
                "the head where they meet it), " + m.source)


def mesh_by_name(name: str) -> Mesh:
    return {"sphere": sphere_mesh, "torus": torus_mesh, "teapot": teapot_mesh, "suzanne": suzanne_mesh}[name]()


def rot_about(axis: F64, degrees: float) -> F64:
    k = np.asarray(axis, dtype=float)
    k = k / np.linalg.norm(k)
    a = math.radians(degrees)
    kx = np.array([[0, -k[2], k[1]], [k[2], 0, -k[0]], [-k[1], k[0], 0]])
    return np.eye(3) + math.sin(a) * kx + (1 - math.cos(a)) * kx @ kx


# ================================ LINE CASTING ================================


def crossings(tris: F64, axis: int, o2: F64, pitch: float, n2: tuple[int, int]) -> tuple[I64, F64, F64]:
    """Where lines along `axis` cross the mesh.

    Lines sit at (o2[0] + j·pitch, o2[1] + k·pitch) in the two other axes, taken cyclically
    (b = axis+1, c = axis+2), j < n2[0], k < n2[1]. Returns (line id j·n2[1] + k, coordinate along
    `axis`, winding step +1 entering / −1 leaving when moving towards +axis), sorted by line then t.
    """
    b, c = (axis + 1) % 3, (axis + 2) % 3
    p = tris[:, :, [b, c]]
    lo, hi = p.min(axis=1), p.max(axis=1)
    j0 = np.maximum(np.ceil((lo[:, 0] - o2[0]) / pitch), 0).astype(np.int64)
    j1 = np.minimum(np.floor((hi[:, 0] - o2[0]) / pitch), n2[0] - 1).astype(np.int64)
    k0 = np.maximum(np.ceil((lo[:, 1] - o2[1]) / pitch), 0).astype(np.int64)
    k1 = np.minimum(np.floor((hi[:, 1] - o2[1]) / pitch), n2[1] - 1).astype(np.int64)
    nj, nk = np.maximum(j1 - j0 + 1, 0), np.maximum(k1 - k0 + 1, 0)
    cnt = nj * nk
    t_idx = np.repeat(np.arange(len(tris)), cnt)
    local = np.arange(int(cnt.sum())) - np.repeat(np.cumsum(cnt) - cnt, cnt)
    nk_r = np.repeat(np.maximum(nk, 1), cnt)
    j = np.repeat(j0, cnt) + local // nk_r
    k = np.repeat(k0, cnt) + local % nk_r
    q = np.stack([o2[0] + j * pitch, o2[1] + k * pitch], axis=1)
    pp = p[t_idx]

    def cross2(u: F64, v: F64) -> F64:
        return u[:, 0] * v[:, 1] - u[:, 1] * v[:, 0]

    w0 = cross2(pp[:, 2] - pp[:, 1], q - pp[:, 1])
    w1 = cross2(pp[:, 0] - pp[:, 2], q - pp[:, 2])
    w2 = cross2(pp[:, 1] - pp[:, 0], q - pp[:, 0])
    area = cross2(pp[:, 1] - pp[:, 0], pp[:, 2] - pp[:, 0])
    hit = ((w0 > 0) & (w1 > 0) & (w2 > 0)) | ((w0 < 0) & (w1 < 0) & (w2 < 0))
    hit &= area != 0
    t3 = tris[t_idx[hit]][:, :, axis]
    lam = np.stack([w0[hit], w1[hit], w2[hit]], axis=1) / area[hit][:, None]
    t = (lam * t3).sum(axis=1)
    dw = -np.sign(area[hit])
    line = j[hit] * n2[1] + k[hit]
    order = np.lexsort((t, line))
    return line[order], t[order], dw[order]


def status_on_lines(line: I64, t: F64, dw: F64, o: float, pitch: float, n_nodes: int, n_lines: int) -> I64:
    """Winding number at nodes o + m·pitch (m < n_nodes) along each line: (n_lines, n_nodes)."""
    diff = np.zeros((n_lines, n_nodes + 1))
    m0 = np.clip(np.ceil((t - o) / pitch).astype(np.int64), 0, n_nodes)
    np.add.at(diff, (line, m0), dw)
    return np.rint(np.cumsum(diff, axis=1)[:, :n_nodes]).astype(np.int64)


def grid_inside(tris: F64, origin: F64, pitch: float, shape: tuple[int, int, int]) -> npt.NDArray[np.bool_]:
    """Inside (winding > 0) at grid points origin + (i, j, k)·pitch, by casting z-lines."""
    line, t, dw = crossings(tris, 2, origin[[0, 1]], pitch, (shape[0], shape[1]))
    w = status_on_lines(line, t, dw, float(origin[2]), pitch, shape[2], shape[0] * shape[1])
    return (w > 0).reshape(shape)


# ================================ VOXELISER ================================


def _clip_to_box(poly: list[list[float]], walls: list[tuple[int, float, float]]) -> list[list[float]]:
    """Sutherland–Hodgman clip of a planar polygon to an axis-aligned box, in plain floats.

    New vertices get the wall's coordinate exactly, so edges on a wall are found by equality.
    (`skin_bench.clip` does the same for a general plane but is ~10× slower per call here.)
    """
    for a, lo, hi in walls:
        for wall, below in ((lo, False), (hi, True)):
            if not poly:
                return poly
            out: list[list[float]] = []
            k = len(poly)
            for i in range(k):
                p, q = poly[i], poly[(i + 1) % k]
                pin = p[a] <= wall if below else p[a] >= wall
                qin = q[a] <= wall if below else q[a] >= wall
                if pin:
                    out.append(p)
                if pin != qin:
                    t = (wall - p[a]) / (q[a] - p[a])
                    x = [p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1]), p[2] + t * (q[2] - p[2])]
                    x[a] = wall
                    out.append(x)
            poly = out
    return poly


@dataclass
class Voxels:
    """One voxelisation of a mesh on a lattice of pitch h.

    Cells are indexed (i, j, k) from `base` (the lower corner of cell 0), `n` cells per axis.
    Interior cells are wholly inside; boundary cells are those the surface passes through, each with
    its exact piece. Per boundary cell: `ijk`, `frac` (piece volume / h³), `centroid`, `second`
    (central second moment, m⁵), `ctr_in` (cell centre inside: the 0/1 rule), `ss` (fill sampled on
    4³ points, Cavendish today), and the piece's closed boundary as triangles `piece_tris` with
    `piece_of` the cell each triangle belongs to. `polys` and `caps` keep the drawing data.
    """

    h:          float
    base:       F64
    n:          I64
    interior:   I64
    ijk:        I64
    frac:       F64
    centroid:   F64
    second:     F64
    ctr_in:     F64
    ss:         F64
    piece_tris: F64
    piece_of:   I64
    mesh_volume: float
    polys:      list[list[F64]] = field(default_factory=list)
    caps:       list[list[tuple[int, F64, F64]]] = field(default_factory=list)
    stats:      dict[str, float] = field(default_factory=dict)

    def centres(self, ijk: I64) -> F64:
        return self.base + (ijk + 0.5) * self.h

    @property
    def interior_centres(self) -> F64:
        return self.centres(self.interior)

    @property
    def boundary_centres(self) -> F64:
        return self.centres(self.ijk)


def voxelise(mesh: Mesh, h: float, origin: F64 | None = None, keep_drawing: bool = False) -> Voxels:
    """Interior cells and exact boundary pieces of `mesh` on the lattice with a node at `origin`.

    `origin` defaults to h·GENERIC. See the module docstring for the method. The pieces satisfy
    Σ piece volume + interior·h³ = mesh volume to rounding (recorded in `stats`).
    """
    tris = mesh.tris
    o = h * GENERIC if origin is None else np.asarray(origin, dtype=float)
    vmin, vmax = tris.reshape(-1, 3).min(axis=0), tris.reshape(-1, 3).max(axis=0)
    lo = np.floor((vmin - o) / h).astype(np.int64) - 1
    hi = np.floor((vmax - o) / h).astype(np.int64) + 1
    base = o + lo * h
    n = hi - lo + 1
    node = [base[a] + np.arange(n[a] + 1) * h for a in range(3)]

    # Lines through the nodes along each axis: crossings and winding at the nodes.
    lines = []
    for a in range(3):
        b, c = (a + 1) % 3, (a + 2) % 3
        n2 = (int(n[b]) + 1, int(n[c]) + 1)
        line, t, dw = crossings(tris, a, np.array([base[b], base[c]]), h, n2)
        w = status_on_lines(line, t, dw, float(base[a]), h, int(n[a]) + 1, n2[0] * n2[1])
        starts = np.searchsorted(line, np.arange(n2[0] * n2[1] + 1))
        lines.append((n2, t, starts, w))
    wz = lines[2][3].reshape(int(n[0]) + 1, int(n[1]) + 1, int(n[2]) + 1)  # node winding from z-lines
    wx = np.moveaxis(lines[0][3].reshape(int(n[1]) + 1, int(n[2]) + 1, int(n[0]) + 1), 2, 0)
    disagree = int((np.sign(wz) != np.sign(wx)).sum())

    def edge_intervals(a: int, i_b: int, i_c: int, m: int) -> list[tuple[float, float]]:
        """Inside parts of the cell edge along axis a from node m to m+1, at nodes (i_b, i_c)."""
        n2, t, starts, w = lines[a]
        lid = i_b * n2[1] + i_c
        t0, t1 = node[a][m], node[a][m + 1]
        seg = t[starts[lid] : starts[lid + 1]]
        cuts = seg[(seg > t0) & (seg < t1)]
        inside = w[lid, m] > 0
        out: list[tuple[float, float]] = []
        cur = t0
        for x in cuts:
            if inside:
                out.append((cur, float(x)))
            inside = not inside
            cur = float(x)
        if inside:
            out.append((cur, t1))
        return out

    # Candidate cells: every cell a triangle's bounding box touches.
    clo = np.floor((tris.min(axis=1) - base) / h).astype(np.int64)
    chi = np.floor((tris.max(axis=1) - base) / h).astype(np.int64)
    span = chi - clo + 1
    cnt = span.prod(axis=1)
    tri_of = np.repeat(np.arange(len(tris)), cnt)
    local = np.arange(int(cnt.sum())) - np.repeat(np.cumsum(cnt) - cnt, cnt)
    sy, sz = np.repeat(span[:, 1], cnt), np.repeat(span[:, 2], cnt)
    cell = np.repeat(clo, cnt, axis=0) + np.stack([local // (sy * sz), (local // sz) % sy, local % sz], 1)
    cid = (cell[:, 0] * n[1] + cell[:, 1]) * n[2] + cell[:, 2]
    order = np.argsort(cid, kind="stable")
    cid, tri_of = cid[order], tri_of[order]
    ucid, first = np.unique(cid, return_index=True)
    bounds = np.append(first, len(cid))

    sub = (np.arange(4) + 0.5) / 4
    samples_in = grid_inside(tris, base + h / 8, h / 4, (int(n[0]) * 4, int(n[1]) * 4, int(n[2]) * 4))
    ss_all = samples_in.reshape(int(n[0]), 4, int(n[1]), 4, int(n[2]), 4).sum(axis=(1, 3, 5)) / 64.0
    ctr_all = grid_inside(tris, base + h / 2, h, (int(n[0]), int(n[1]), int(n[2])))
    del sub

    is_boundary = np.zeros(tuple(int(x) for x in n), dtype=bool)
    b_ijk, b_frac, b_cen, b_sec, b_ctr, b_ss, p_tris, p_of = [], [], [], [], [], [], [], []
    draw_polys: list[list[F64]] = []
    draw_caps: list[list[tuple[int, F64, F64]]] = []
    for u in range(len(ucid)):
        c_id = int(ucid[u])
        ijk = np.array([c_id // (n[1] * n[2]), (c_id // n[2]) % n[1], c_id % n[2]])
        c_lo = np.array([node[a][ijk[a]] for a in range(3)])
        c_hi = np.array([node[a][ijk[a] + 1] for a in range(3)])
        group = tris[tri_of[bounds[u] : bounds[u + 1]]]
        within = np.all(group > c_lo, axis=(1, 2)) & np.all(group < c_hi, axis=(1, 2))
        polys: list[F64] = list(group[within])
        walls = [(a, float(c_lo[a]), float(c_hi[a])) for a in range(3)]
        for f in group[~within]:
            p = _clip_to_box(f.tolist(), walls)
            if len(p) >= 3:
                arr = np.array(p)
                area = np.linalg.norm(np.cross(arr[1:-1] - arr[0], arr[2:] - arr[0]), axis=1).sum()
                if area > 1e-14 * h * h:
                    polys.append(arr)
        if not polys:
            continue
        is_boundary[tuple(ijk)] = True
        # Caps: segments on each wall, then a signed fan from the wall's centre.
        segs: list[tuple[int, F64, F64]] = []
        for p in polys:
            q = np.roll(p, -1, axis=0)
            for a in range(3):
                for s_i, wall in ((0, c_lo[a]), (1, c_hi[a])):
                    for e in np.nonzero((p[:, a] == wall) & (q[:, a] == wall))[0]:
                        segs.append((2 * a + s_i, q[e], p[e]))
        for a in range(3):
            b, c = (a + 1) % 3, (a + 2) % 3
            for s_i in (0, 1):
                ia = ijk[a] + s_i
                # Square corners anticlockwise about +e_a: (lo,lo) (hi,lo) (hi,hi) (lo,hi) in (b, c).
                corners = [(0, 0), (1, 0), (1, 1), (0, 1)]
                if s_i == 0:
                    corners = corners[::-1]
                for q in range(4):
                    (pb, pc), (qb, qc) = corners[q], corners[(q + 1) % 4]
                    along = b if pb != qb else c
                    fixed_b, fixed_c = ijk[b] + pb, ijk[c] + pc  # the start corner's node indices
                    idx = {a: ia, b: fixed_b, c: fixed_c}
                    m = int(ijk[along])
                    e1, e2 = (along + 1) % 3, (along + 2) % 3
                    ivs = edge_intervals(along, int(idx[e1]), int(idx[e2]), m)
                    forward = (qb > pb) if along == b else (qc > pc)
                    for t0, t1 in (ivs if forward else [(y, x) for x, y in ivs[::-1]]):
                        p0 = np.array([node[0][idx[0]], node[1][idx[1]], node[2][idx[2]]], dtype=float)
                        p1 = p0.copy()
                        p0[along], p1[along] = t0, t1
                        segs.append((2 * a + s_i, p0, p1))
        cap_tris = []
        for f_i, p0, p1 in segs:
            a = f_i // 2
            ctr = 0.5 * (c_lo + c_hi)
            ctr[a] = c_hi[a] if f_i % 2 else c_lo[a]
            tri = np.array([ctr, p0, p1])
            if np.linalg.norm(np.cross(p0 - ctr, p1 - ctr)) > 1e-14 * h * h:
                cap_tris.append(tri)
        surface = sb.fan(polys, h)
        t_all = np.concatenate([surface, np.array(cap_tris)]) if cap_tris else surface
        centre = 0.5 * (c_lo + c_hi)
        vol, cen = sb.volume_centroid(t_all, centre)
        if vol <= 1e-15 * h**3:
            cen = centre.copy()
        b_ijk.append(ijk)
        b_frac.append(vol / h**3)
        b_cen.append(cen)
        b_sec.append(sb.second_moment(t_all, cen) if vol > 1e-15 * h**3 else np.zeros((3, 3)))
        b_ctr.append(float(ctr_all[tuple(ijk)]))
        b_ss.append(float(ss_all[tuple(ijk)]))
        p_tris.append(t_all)
        p_of.append(np.full(len(t_all), len(b_ijk) - 1))
        if keep_drawing:
            draw_polys.append(polys)
            draw_caps.append(segs)

    corner_in = wz[:-1, :-1, :-1] > 0
    interior = np.argwhere(corner_in & ~is_boundary).astype(np.int64)
    mv, _ = sb.volume_centroid(tris, np.zeros(3))
    vox = Voxels(
        h=h, base=base, n=n, interior=interior, ijk=np.array(b_ijk, dtype=np.int64),
        frac=np.array(b_frac), centroid=np.array(b_cen), second=np.array(b_sec),
        ctr_in=np.array(b_ctr), ss=np.array(b_ss),
        piece_tris=np.concatenate(p_tris), piece_of=np.concatenate(p_of), mesh_volume=float(mv),
        polys=draw_polys, caps=draw_caps,
    )
    vsum = (len(interior) + vox.frac.sum()) * h**3
    vox.stats = {"volume_rel_error": float(vsum / mv - 1), "node_disagreements": disagree,
                 "boundary": len(b_ijk), "interior": len(interior)}
    return vox


# ================================ FIELDS ================================
# Every g below is per unit G·ρ (m), pointing at the mass, at points `pts` (M, 3); masses are
# volumes (m³). Multiply by G·RHO for m s⁻².


def g_mesh(tris: F64, pts: F64, weights: F64 | None = None) -> F64:
    """g of a closed triangle soup (signed faces add), Werner & Scheeres with the stable numerator."""
    w = np.ones((len(tris), 1)) if weights is None else weights.reshape(len(tris), -1)
    chunk = max(8, int(300_000 // max(len(pts), 1)))
    _, g = sb.polyhedron_field(tris, w, pts, chunk=chunk, stable=True)
    return g[0] if weights is None or weights.ndim == 1 else g


def g_cubes(centres: F64, h: float, fill: F64, pts: F64) -> F64:
    """Uniform cubes of side h at `centres`, density `fill` (the cube's own Nagy-equivalent field)."""
    if len(centres) == 0:
        return np.zeros((len(pts), 3))
    out = np.zeros((len(pts), 3))
    step = 4000
    for s in range(0, len(centres), step):
        t = sb.cube_tris(centres[s : s + step], h)
        out += g_mesh(t, pts, np.repeat(fill[s : s + step], 12))
    return out


def g_points(mass: F64, pos: F64, pts: F64) -> F64:
    out = np.zeros((len(pts), 3))
    step = max(1, 2_000_000 // max(len(pts), 1))
    for s in range(0, len(mass), step):
        r = pos[None, s : s + step] - pts[:, None]
        rl = np.linalg.norm(r, axis=2)
        out += (mass[None, s : s + step, None] * r / rl[..., None] ** 3).sum(axis=1)
    return out


def g_quadrupole(second: F64, pos: F64, pts: F64) -> F64:
    """The extra g of second moments I (N, 3, 3) at `pos` (skin_bench's frac_cen_q term)."""
    out = np.zeros((len(pts), 3))
    step = max(1, 1_000_000 // max(len(pts), 1))
    for s in range(0, len(pos), step):
        big_i = second[s : s + step]
        r = pos[None, s : s + step] - pts[:, None]
        rl = np.linalg.norm(r, axis=2)
        tr = np.trace(big_i, axis1=1, axis2=2)
        ir = np.einsum("cij,pcj->pci", big_i, r)
        rir = np.einsum("pci,pci->pc", r, ir)
        r2 = rl**2
        du = 0.5 * (6 * ir / rl[..., None] ** 5 - 2 * tr[None, :, None] * r / rl[..., None] ** 5
                    - 5 * ((3 * rir - r2 * tr[None, :]) / rl**7)[..., None] * r)
        out -= du.sum(axis=1)
    return out


def body_moments(tris: F64) -> tuple[float, F64, F64, F64]:
    """Volume, centroid, central second and third moments (m³, m, m⁵, m⁶) of a closed mesh."""
    vol, cen = sb.volume_centroid(tris, np.zeros(3))
    a, b, c = (tris[:, i] - cen for i in range(3))
    apex = np.broadcast_to(-cen, a.shape)
    v4 = np.stack([apex, a, b, c], axis=1)  # (T, 4, 3)
    tv = np.einsum("nk,nk->n", a - apex, np.cross(b - apex, c - apex)) / 6.0
    e2 = (np.ones((4, 4)) + np.eye(4)) / 20.0
    e3 = np.zeros((4, 4, 4))
    for i in range(4):
        for j in range(4):
            for k in range(4):
                same = len({i, j, k})
                e3[i, j, k] = {3: 1 / 120, 2: 1 / 60, 1: 1 / 20}[same]
    m2 = np.einsum("n,ij,nia,njb->ab", tv, e2, v4, v4)
    m3 = np.einsum("n,ijk,nia,njb,nkc->abc", tv, e3, v4, v4, v4)
    return float(vol), cen, m2, m3


def g_multipole(vol: float, cen: F64, m2: F64 | None, m3: F64 | None, pts: F64) -> F64:
    """g of a body's Cartesian multipole about its centroid: mass, + second, + third moments.

    U = Σₙ 1/n! Mⁿ : ∇ⁿ(1/R) with R = centroid − point; g = −∂U/∂centroid.
    """
    r = cen[None] - pts
    rl = np.linalg.norm(r, axis=1)
    d = np.eye(3)
    g = vol * r / rl[:, None] ** 3
    if m2 is not None:
        rr = np.einsum("pi,pj->pij", r, r)
        d3 = (-15 * np.einsum("pi,pj,pk->pijk", r, r, r) / rl[:, None, None, None] ** 7
              + 3 * (np.einsum("pi,jk->pijk", r, d) + np.einsum("pj,ik->pijk", r, d)
                     + np.einsum("pk,ij->pijk", r, d)) / rl[:, None, None, None] ** 5)
        g -= 0.5 * np.einsum("ij,pijm->pm", m2, d3)
        del rr
    if m3 is not None:
        r4 = rl[:, None, None, None, None]
        rr = np.einsum("pi,pj->pij", r, r)
        t1 = 105 * np.einsum("pi,pj,pk,pl->pijkl", r, r, r, r) / r4**9
        t2 = sum(np.einsum(f"p{a}{b},{c}{e}->pijkl", rr, d) for a, b, c, e in
                 ("ijkl", "ikjl", "iljk", "jkil", "jlik", "klij"))
        t3 = sum(np.einsum(f"{a}{b},{c}{e}->ijkl", d, d) for a, b, c, e in ("ijkl", "ikjl", "iljk"))
        d4 = t1 - 15 * t2 / r4**7 + 3 * t3[None] / r4**5
        g -= np.einsum("ijk,pijkm->pm", m3, d4) / 6.0
    return g


def phase(g: F64) -> F64:
    """AION-10 phase (rad) from g at [lower points; upper points] (2M, 3), per unit G·ρ."""
    m = len(g) // 2
    return PHASE_PER_DG * G * RHO * (g[m:, 2] - g[:m, 2])


def distance_to_mesh(tris: F64, p: F64) -> float:
    """Exact distance from point `p` to a triangle soup (plane if the foot is inside, else edges)."""
    a, b, c = tris[:, 0], tris[:, 1], tris[:, 2]
    n = np.cross(b - a, c - a)
    n /= np.linalg.norm(n, axis=1)[:, None]
    h = np.einsum("nk,nk->n", p - a, n)
    foot = p - h[:, None] * n
    inside = np.ones(len(tris), dtype=bool)
    for u, v in ((a, b), (b, c), (c, a)):
        inside &= np.einsum("nk,nk->n", np.cross(v - u, foot - u), n) >= 0
    best = np.where(inside, np.abs(h), np.inf)
    for u, v in ((a, b), (b, c), (c, a)):
        e = v - u
        t = np.clip(np.einsum("nk,nk->n", p - u, e) / np.einsum("nk,nk->n", e, e), 0, 1)
        best = np.minimum(best, np.linalg.norm(p - (u + t[:, None] * e), axis=1))
    return float(best.min())


def tower(mesh: Mesh, distances: F64, direction: F64 | None = None) -> F64:
    """Lower points at each distance from the surface along `direction` from the mesh's bounding-box
    centre (the outermost such point), then the upper points 5 m above: (2M, 3).

    Beyond the furthest vertex projection along the ray, the distance to every vertex grows with t,
    so the distance to the surface does too once outside; bisection there, a backward march inside.
    """
    d = np.array([1.0, 0.2, -0.3]) if direction is None else np.asarray(direction, dtype=float)
    d = d / np.linalg.norm(d)
    tris = mesh.tris
    v = mesh.verts
    c = 0.5 * (v.min(axis=0) + v.max(axis=0))
    t_max = float(((v - c) @ d).max())
    lower = []
    for dist in distances:
        def near(t: float) -> bool:
            return distance_to_mesh(tris, c + t * d) < dist
        if near(t_max):
            a, b = t_max, t_max + dist + 1.0
        else:
            b = t_max
            a = b - 0.5 * dist
            while not near(a):
                b, a = a, a - 0.5 * dist
        for _ in range(50):
            mid = 0.5 * (a + b)
            a, b = (mid, b) if near(mid) else (a, mid)
        lower.append(c + 0.5 * (a + b) * d)
    lo = np.array(lower)
    return np.concatenate([lo, lo + [0.0, 0.0, BASELINE]])


# ================================ RENDERING ================================


class Canvas:
    """An orthographic z-buffered rasteriser with Lambert shading, supersampled 2×.

    Args:
        size      (int):   Output width and height, pixels.
        azimuth   (float): Camera azimuth, degrees from +x towards +y.
        elevation (float): Camera elevation, degrees.
        centre    (F64):   Point at the image centre, m.
        half      (float): Half the visible width, m.
    """

    def __init__(self, size: int, azimuth: float, elevation: float, centre: F64, half: float) -> None:
        self.n = 2 * size
        az, el = math.radians(azimuth), math.radians(elevation)
        eye = np.array([math.cos(el) * math.cos(az), math.cos(el) * math.sin(az), math.sin(el)])
        self.fwd = -eye
        self.right = np.cross(self.fwd, [0.0, 0.0, 1.0])
        self.right /= np.linalg.norm(self.right)
        self.up = np.cross(self.right, self.fwd)
        self.centre = np.asarray(centre, dtype=float)
        self.scale = self.n / (2 * half)
        self.light = 0.8 * eye + 0.5 * self.up - 0.3 * self.right
        self.light /= np.linalg.norm(self.light)
        self.z = np.full((self.n, self.n), np.inf)
        self.img = np.ones((self.n, self.n, 3)) * np.array([0xFC, 0xFC, 0xFB]) / 255.0
        self.depth_bias = 2.0 / self.scale

    def project(self, p: F64) -> F64:
        q = p - self.centre
        x = q @ self.right * self.scale + self.n / 2
        y = self.n / 2 - q @ self.up * self.scale
        return np.stack([x, y, q @ self.fwd], axis=-1)

    def shade(self, base: F64, normal: F64) -> F64:
        lam = max(0.0, float(normal @ self.light))
        rim = max(0.0, float(-normal @ self.fwd))
        return np.clip(base * (0.5 + 0.45 * lam + 0.1 * rim), 0, 1)

    def __raster(self, s: F64) -> tuple[npt.NDArray[np.intp], npt.NDArray[np.intp], F64, F64] | None:
        x0 = max(int(np.floor(s[:, 0].min())), 0)
        x1 = min(int(np.ceil(s[:, 0].max())), self.n - 1)
        y0 = max(int(np.floor(s[:, 1].min())), 0)
        y1 = min(int(np.ceil(s[:, 1].max())), self.n - 1)
        if x1 < x0 or y1 < y0:
            return None
        xs, ys = np.meshgrid(np.arange(x0, x1 + 1) + 0.5, np.arange(y0, y1 + 1) + 0.5)
        a, b, c = s[0], s[1], s[2]
        area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
        if abs(area) < 1e-12:
            return None
        w0 = ((b[0] - xs) * (c[1] - ys) - (b[1] - ys) * (c[0] - xs)) / area
        w1 = ((c[0] - xs) * (a[1] - ys) - (c[1] - ys) * (a[0] - xs)) / area
        w2 = 1 - w0 - w1
        inside = (w0 >= 0) & (w1 >= 0) & (w2 >= 0)
        if not inside.any():
            return None
        depth = w0 * a[2] + w1 * b[2] + w2 * c[2]
        iy, ix = np.nonzero(inside)
        return iy + y0, ix + x0, depth[inside], np.sign(area)

    def triangles(self, tris: F64, colour: F64, cull: bool = True) -> None:
        """Opaque triangles, outward normals from their winding; back faces culled."""
        colour = np.asarray(colour, dtype=float)
        for t in tris:
            nrm = np.cross(t[1] - t[0], t[2] - t[0])
            ln = np.linalg.norm(nrm)
            if ln == 0:
                continue
            nrm /= ln
            if cull and nrm @ self.fwd > 0:
                continue
            r = self.__raster(self.project(t))
            if r is None:
                continue
            iy, ix, dep, _ = r
            near = dep < self.z[iy, ix]
            self.z[iy[near], ix[near]] = dep[near]
            self.img[iy[near], ix[near]] = self.shade(colour, nrm)

    def signed_region(self, tris: F64, normal: F64, colour: F64) -> None:
        """A planar region given as a signed fan (coverage > 0 is inside), e.g. a cap."""
        normal = np.asarray(normal, dtype=float)
        if normal @ self.fwd > 0 or len(tris) == 0:
            return
        cov = np.zeros((self.n, self.n), dtype=np.int32)
        dep = np.full((self.n, self.n), np.inf)
        for t in tris:
            sign = 1 if np.cross(t[1] - t[0], t[2] - t[0]) @ normal > 0 else -1
            r = self.__raster(self.project(t))
            if r is None:
                continue
            iy, ix, d, _ = r
            cov[iy, ix] += sign
            dep[iy, ix] = d
        sel = (cov > 0) & (dep < self.z)
        self.z[sel] = dep[sel]
        self.img[sel] = self.shade(np.asarray(colour, dtype=float), normal)

    def lines(self, segs: F64, colour: F64, alpha: float) -> None:
        """Segments (N, 2, 3), depth-tested against what is drawn, blended at `alpha`."""
        if len(segs) == 0:
            return
        a, b = self.project(segs[:, 0]), self.project(segs[:, 1])
        length = np.linalg.norm((b - a)[:, :2], axis=1)
        k = np.maximum(np.ceil(length * 1.5).astype(int), 1)
        idx = np.repeat(np.arange(len(segs)), k + 1)
        frac = np.concatenate([np.linspace(0, 1, m + 1) for m in k])
        p = a[idx] + (b - a)[idx] * frac[:, None]
        ix, iy = np.floor(p[:, 0]).astype(int), np.floor(p[:, 1]).astype(int)
        ok = (ix >= 0) & (ix < self.n) & (iy >= 0) & (iy < self.n)
        ix, iy, p = ix[ok], iy[ok], p[ok]
        vis = p[:, 2] <= self.z[iy, ix] + self.depth_bias
        ix, iy = ix[vis], iy[vis]
        mask = np.zeros((self.n, self.n), dtype=bool)
        mask[iy, ix] = True
        self.img[mask] = (1 - alpha) * self.img[mask] + alpha * np.asarray(colour, dtype=float)

    def image(self) -> F64:
        m = self.n // 2
        return self.img.reshape(m, 2, m, 2, 3).mean(axis=(1, 3))
