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
# Licences: that repository's README (read at this commit, 25 Nov 2026) states none for either model.
# The teapot's source is given as Martin Newell (1975, the ftp.funet.fi teaset); Suzanne's as Blender
# (the Blender Foundation's test mesh). With no licence stated, we only download them at run time and
# commit nothing derived from them except numbers and pictures of our own measurements.
_COMMIT = "8a4f8642acaf43f9cd7b67858a1502e1055ef202"
SOURCES = {
    "teapot": (f"https://raw.githubusercontent.com/alecjacobson/common-3d-test-models/{_COMMIT}/data/teapot.obj",
               "1b5396fedd74b577e32cef41146582c2f2e1a050d5b4915193c0ac1ad4187ed4"),
    "suzanne": (f"https://raw.githubusercontent.com/alecjacobson/common-3d-test-models/{_COMMIT}/data/suzanne.obj",
                "d8684326f9bd8cfc24d3d302c1042fa16f63d2e66e49ed56b413fa20bed271e6"),
}


# More famous meshes (added 26 Nov 2026). Licences, read on the sources' pages that day:
# - Stanford 3D Scanning Repository (bunny, dragon, Happy Buddha, Armadillo): "You are welcome to use
#   the data and models for research purposes ... as long as credit is given to the Stanford Computer
#   Graphics Laboratory"; not for commercial use; "You can do anything you want to the Stanford bunny
#   or the armadillo". Credit: Stanford Computer Graphics Laboratory (Armadillo: Venkat Krishnamurthy).
# - Spot (Keenan Crane, CMU 3D model repository): "dedicated under CC0 1.0 Universal".
# - Thingi10K file 44950 (Thingiverse thing 13727, "mk7 ball bearing extruder drive" by jag): licence
#   "Public Domain" in Thingi10K's metadata; from the Thingi10K Hugging Face mirror at a pinned commit.
# The Stanford URLs are not versioned; the SHA-256 pins the content.
_HF = "https://huggingface.co/datasets/Thingi10K/Thingi10K/resolve/2d5d3b2f3cd3711028ad75b12788c13b25559ec6"
SOURCES.update({
    "bunny": ("https://graphics.stanford.edu/pub/3Dscanrep/bunny.tar.gz",
              "a5720bd96d158df403d153381b8411a727a1d73cff2f33dc9b212d6f75455b84"),
    "spot": ("https://www.cs.cmu.edu/~kmcrane/Projects/ModelRepository/spot.zip",
             "1d4082da4e940f6a4cfdf648fd77e1647ac613c618dbff933f1ba23fad254608"),
    "dragon": ("https://graphics.stanford.edu/pub/3Dscanrep/dragon/dragon_recon.tar.gz",
               "74ac1d90989c9b1732edee82d57e9ce71452144cf4355f108d8c9c616d28d02f"),
    "armadillo": ("https://graphics.stanford.edu/pub/3Dscanrep/armadillo/Armadillo.ply.gz",
                  "8b9b56cc36e66d54429b1e1e75bd89e833645bfe0dc7c1afd1205877a7356a3f"),
    "buddha": ("https://graphics.stanford.edu/pub/3Dscanrep/happy/happy_recon.tar.gz",
               "409cd294efbfd8244e15a382b95a9423f153b7776e736c9b09f19ec9d3c10ed0"),
    "bracket": (f"{_HF}/raw_meshes/44950.stl",
                "7cfc8f73f29a61da358c15268971fd8026e0680a2aa6edb4d2a60bb023ad24ce"),
})
# The member of each archive that is the mesh (the full-resolution reconstruction).
MEMBER = {"bunny": "bunny/reconstruction/bun_zipper.ply", "spot": "spot/spot_triangulated.obj",
          "dragon": "dragon_recon/dragon_vrip.ply", "buddha": "happy_recon/happy_vrip.ply"}


def _download(name: str) -> Path:
    url, sha = SOURCES[name]
    CACHE.mkdir(exist_ok=True)
    tail = url.rsplit("/", 1)[1]
    path = CACHE / (f"{name}.obj" if tail.endswith(".obj") else f"{name}-{tail}")
    if not path.exists():
        with urllib.request.urlopen(url, timeout=600) as r:  # noqa: S310 (pinned https URL)
            path.write_bytes(r.read())
    got = hashlib.sha256(path.read_bytes()).hexdigest()
    if got != sha:
        path.unlink()
        raise RuntimeError(f"{name}: SHA-256 {got} does not match the pinned {sha}")
    return path


def _load(name: str) -> tuple[F64, I64]:
    """The downloaded mesh as it is (vertices in its own units, triangles), from its archive."""
    import gzip
    import io
    import tarfile
    import zipfile

    path = _download(name)
    if path.suffix == ".obj":
        return _read_obj(path)
    raw_cache = CACHE / f"{name}-original.npz"
    if raw_cache.exists():
        z = np.load(raw_cache)
        return z["v"], z["f"]
    import trimesh

    if path.name.endswith(".tar.gz"):
        with tarfile.open(path) as tar:
            member = tar.extractfile(MEMBER[name])
            assert member is not None
            data, kind = member.read(), MEMBER[name].rsplit(".", 1)[1]
    elif path.suffix == ".zip":
        with zipfile.ZipFile(path) as z:
            data, kind = z.read(MEMBER[name]), MEMBER[name].rsplit(".", 1)[1]
    elif path.name.endswith(".ply.gz"):
        data, kind = gzip.decompress(path.read_bytes()), "ply"
    else:
        data, kind = path.read_bytes(), path.suffix[1:]
    if kind == "obj":
        tmp = CACHE / f"{name}-member.obj"
        tmp.write_bytes(data)
        v, f = _read_obj(tmp)
        tmp.unlink()
    else:
        m = trimesh.load(io.BytesIO(data), file_type=kind, process=False, force="mesh")
        v, f = np.asarray(m.vertices, dtype=np.float64), np.asarray(m.faces, dtype=np.int64)
    np.savez(raw_cache, v=v, f=f)
    return v, f


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


def closed_from_soup(name: str, v: F64, faces: I64, n: int, inside_tris: F64 | None = None) -> Mesh:
    """Close an open or self-intersecting mesh: the zero set of a signed distance whose sign is the
    generalised winding number's (inside where it exceeds 0.5).

    REJECTED for the study (it re-meshes the surface: terraced rims, a stray spout fragment, a
    column under Suzanne's chin); kept only as the control arm of `check_repair`. `v` is already in
    the study's frame (`_raw`). The winding number is sampled on a grid of `n` cells
    across that 1 m (grid offset off-lattice). Within two grid steps of a change of side, the exact
    distance to the raw triangles gives the magnitude (the winding number alone jumps by 1 across the
    surface, and marching cubes on it terraces); further out the magnitude is clamped. Marching cubes
    (Lewiner, scikit-image) extracts the zero set, which is closed and consistently wound by
    construction. The result is checked (`check_closed`) and rejected if it is not.

    With `inside_tris` (a closed mesh, e.g. the repaired one) the side comes from casting lattice lines
    through it (`grid_inside`) instead of the winding number of the soup: the same set away from the
    surface, and fast for the 50,000-triangle scans.
    """
    from scipy import ndimage
    from skimage import measure

    cache = CACHE / f"{name}-closed-sdf-{n}{'-cast' if inside_tris is not None else ''}.npz"
    if cache.exists():
        z = np.load(cache)
        mv, mf = z["v"], z["f"]
    else:
        s = 1.0 / n
        o = v.min(axis=0) - 3 * s + s * np.array([0.318, 0.427, 0.271])
        shape = np.ceil((v.max(axis=0) - o) / s).astype(int) + 3
        grid = np.stack(np.meshgrid(*[o[i] + s * np.arange(shape[i]) for i in range(3)], indexing="ij"), -1)
        wn_cache = CACHE / f"{name}-winding-{n}.npy"
        if inside_tris is not None:
            wn = grid_inside(inside_tris, o, s, tuple(int(x) for x in shape)).astype(np.float64)
        elif wn_cache.exists():
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
        dist[band] = distances_to_mesh(v[faces], grid[band])
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


def _raw(name: str, up: tuple[int, int, int], original: bool = False) -> tuple[F64, I64, float, F64]:
    """The downloaded mesh turned so `up` is +z, its bounding box centred and its largest extent 1 m.

    Returns (vertices, faces, scale, shift) with v_new = (v_old[:, perm]·sign − shift)·scale.
    Meshes in `DECIMATE` come back decimated (`decimated`) unless `original`; the frame is always the
    original's, so the two can be compared point by point.
    """
    verts, faces = _load(name)
    perm = [abs(a) - 1 for a in up]
    sign = np.array([1.0 if a > 0 else -1.0 for a in up])
    v = verts[:, perm] * sign
    if np.linalg.det(np.eye(3)[perm] * sign[:, None]) < 0:
        faces = faces[:, ::-1]
    lo, hi = v.min(axis=0), v.max(axis=0)
    shift = 0.5 * (lo + hi)
    scale = 1.0 / float((hi - lo).max())
    v = (v - shift) * scale
    if name in DECIMATE and not original:
        v, faces = decimated(name, v, faces, DECIMATE[name])
    return v, faces, scale, shift


# Scans too big to voxelise in Python at size/h = 128 are decimated first (triangles kept after).
# At 50,000 triangles the Armadillo missed the 1/256 m check by 0.1 mm (60,000 passes). The dragon
# and the Buddha miss it at 50,000 and at 150,000 alike, in the same few places (a thin fin under the
# dragon, ~2 cm; thin walls under the Buddha's robe, ~1 cm) whichever decimator: see the note.
DECIMATE = {"dragon": 50_000, "armadillo": 60_000, "buddha": 50_000}


def decimated(name: str, v: F64, f: I64, target: int) -> tuple[F64, I64]:
    """The mesh (study frame) decimated to about `target` triangles and made a clean closed mesh.

    1. Weld (8 decimals), drop triangles on non-manifold edges (`drop_nonmanifold`: the Happy
       Buddha has 32 such edges).
    2. Quadric-error decimation (Garland & Heckbert 1997, `fast_simplification`, MIT).
    3. `repair` (ear clipping) the few non-manifold edges and holes it leaves.
    4. Where the result folds through itself (`self_crossings`, the test `cav`'s importer makes),
       cut out the crossing triangles' one-ring and close the holes again; repeat until none.
    manifold3d's `simplify(tolerance)` was tried first: no crossings at the start, but it removes
    thin parts (the dragon's whiskers): original vertices 1–2 cm from its result at 50,000 and
    112,000 triangles. Cached in `.cache/`; `decimation_check` measures the result against the
    original download."""
    cache = CACHE / f"{name}-decimated-{target}.npz"
    if cache.exists():
        z = np.load(cache)
        return z["v"], z["f"]
    q = np.round(v, 8)
    _, first, inv = np.unique(q, axis=0, return_index=True, return_inverse=True)
    v, f = v[first], inv.reshape(-1)[f]
    f = f[(f[:, 0] != f[:, 1]) & (f[:, 1] != f[:, 2]) & (f[:, 2] != f[:, 0])]
    n_bad = len(f)
    f = drop_nonmanifold(f)
    n_bad -= len(f)
    import fast_simplification

    dv, df = fast_simplification.simplify(v.astype(np.float64), f.astype(np.int64),
                                          target_reduction=1.0 - target / len(f))
    dv, df = np.asarray(dv, dtype=np.float64), np.asarray(df, dtype=np.int64)
    n_simplified = len(df)
    mended = repair(name, (1, 2, 3), source=(dv, df), ear=True).mesh  # its few non-manifold edges
    dv, df = mended.verts, mended.faces
    # Where the simplified surface folds through itself (thin parts: the dragon's whiskers and
    # claws, the Buddha's folds), cut out the crossing triangles and their one-ring and close the
    # holes again (ear clipping); repeat until `self_crossings` finds none.
    n_cut = rounds = 0
    while len(x := self_crossings(dv, df)):
        if rounds == 8:
            raise RuntimeError(f"{name}: still {len(x)} self-crossings after 8 rounds of cutting them out")
        hot = np.zeros(len(dv), dtype=bool)
        hot[df[np.unique(x)].ravel()] = True
        cut = hot[df].any(axis=1)
        n_cut += int(cut.sum())
        mended = repair(name, (1, 2, 3), source=(dv, df[~cut]), ear=True).mesh
        dv, df = mended.verts, mended.faces
        rounds += 1
    np.savez(cache, v=dv, f=df, dropped_nonmanifold=n_bad, simplified_triangles=n_simplified,
             crossing_cut=n_cut, crossing_rounds=rounds)
    return dv, df


def _from_manifold(man: object) -> tuple[F64, I64]:
    """Vertices and triangles of a manifold3d `Manifold`, its split vertices merged again."""
    out = man.to_mesh64()  # type: ignore[attr-defined]
    mv = np.array(out.vert_properties)[:, :3].astype(np.float64)
    mf = np.array(out.tri_verts).astype(np.int64)
    # manifold3d splits a vertex where properties differ between input meshes; its merge vectors say
    # which copies are the same point. Apply them so the mesh is one connected surface.
    to = np.arange(len(mv))
    mfrom, mto = np.array(out.merge_from_vert, dtype=np.int64), np.array(out.merge_to_vert, dtype=np.int64)
    to[mfrom] = mto
    mf = to[mf]
    used = np.unique(mf)
    remap = np.full(len(mv), -1, dtype=np.int64)
    remap[used] = np.arange(len(used))
    mv, mf = mv[used], remap[mf]
    return mv, _oriented(mv, mf)


def manifold_mask(f: I64) -> npt.NDArray[np.bool_]:
    """False for every triangle on an edge of more than two triangles, or used twice the same way
    (the Happy Buddha has 32 such edges; welding can make more); `repair` caps the holes that
    dropping them opens. Dropping cannot make a new such edge, so one pass is enough."""
    e = np.concatenate([f[:, [0, 1]], f[:, [1, 2]], f[:, [2, 0]]])
    big = int(f.max()) + 1
    key = e[:, 0] * big + e[:, 1]
    undirected = np.minimum(e[:, 0], e[:, 1]) * big + np.maximum(e[:, 0], e[:, 1])
    _, inv_u, cnt_u = np.unique(undirected, return_inverse=True, return_counts=True)
    _, inv_d, cnt_d = np.unique(key, return_inverse=True, return_counts=True)
    bad_edge = (cnt_u[inv_u.reshape(-1)] > 2) | (cnt_d[inv_d.reshape(-1)] > 1)
    return ~bad_edge.reshape(3, -1).any(axis=0)


def drop_nonmanifold(f: I64) -> I64:
    """`f` without the triangles `manifold_mask` flags."""
    return f[manifold_mask(f)]


def self_crossings(verts: F64, faces: I64, depth: float = 1e-9) -> I64:
    """Pairs (edge's triangle, triangle) where an edge of the mesh passes through a triangle that
    shares neither of its ends, by more than `depth` metres: the test `cav`'s importer makes
    (`Mesh::first_crossing`), here counting them all. Candidates come from a uniform grid of cells
    about twice the mean edge length; each is tested with signed volumes. Returns (K, 2) triangle
    indices."""
    e_all = np.concatenate([faces[:, [0, 1]], faces[:, [1, 2]], faces[:, [2, 0]]])
    owner = np.tile(np.arange(len(faces)), 3)
    keep = e_all[:, 0] < e_all[:, 1]  # each undirected edge once (closed mesh: the other way is its twin)
    edges, owner = e_all[keep], owner[keep]
    cell = 2.0 * float(np.linalg.norm(verts[edges[:, 0]] - verts[edges[:, 1]], axis=1).mean())
    lo = verts.min(axis=0)

    def boxes(pts: F64) -> tuple[I64, I64]:
        a = np.floor((pts.min(axis=1) - lo) / cell).astype(np.int64)
        b = np.floor((pts.max(axis=1) - lo) / cell).astype(np.int64)
        return a, b

    def cells_of(a: I64, b: I64) -> tuple[I64, I64]:
        span = b - a + 1
        cnt = span.prod(axis=1)
        item = np.repeat(np.arange(len(a)), cnt)
        local = np.arange(int(cnt.sum())) - np.repeat(np.cumsum(cnt) - cnt, cnt)
        sy, sz = np.repeat(span[:, 1], cnt), np.repeat(span[:, 2], cnt)
        c = np.repeat(a, cnt, axis=0) + np.stack([local // (sy * sz), (local // sz) % sy, local % sz], 1)
        return item, (c[:, 0] * 100_003 + c[:, 1]) * 100_019 + c[:, 2]

    ti, tc = cells_of(*boxes(verts[faces]))
    ei, ec = cells_of(*boxes(verts[edges]))
    order = np.argsort(tc, kind="stable")
    ti, tc = ti[order], tc[order]
    start = np.searchsorted(tc, ec, "left")
    stop = np.searchsorted(tc, ec, "right")
    cnt = stop - start
    pe = np.repeat(ei, cnt)
    pt = ti[np.repeat(start, cnt) + (np.arange(int(cnt.sum())) - np.repeat(np.cumsum(cnt) - cnt, cnt))]
    pairs = np.unique(np.stack([pe, pt], axis=1), axis=0)
    del pe, pt, ti, tc, ei, ec
    step = 2_000_000
    if len(pairs) > step:  # in chunks, so the large scans stay within a few hundred MB
        return np.concatenate([_crossing_test(verts, faces, edges, owner, pairs[s : s + step], depth)
                               for s in range(0, len(pairs), step)])
    return _crossing_test(verts, faces, edges, owner, pairs, depth)


def _crossing_test(verts: F64, faces: I64, edges: I64, owner: I64, pairs: I64, depth: float) -> I64:
    """`self_crossings`' exact test on candidate (edge, triangle) pairs."""
    f = faces[pairs[:, 1]]
    ed = edges[pairs[:, 0]]
    share = (f == ed[:, [0]]).any(axis=1) | (f == ed[:, [1]]).any(axis=1)
    pairs, f, ed = pairs[~share], f[~share], ed[~share]
    p, q = verts[ed[:, 0]], verts[ed[:, 1]]
    a, b, c = verts[f[:, 0]], verts[f[:, 1]], verts[f[:, 2]]

    def vol(w: F64, x: F64, y: F64, z: F64) -> F64:
        return np.einsum("nk,nk->n", x - w, np.cross(y - w, z - w))

    sp, sq = vol(a, b, c, p), vol(a, b, c, q)
    s1, s2, s3 = vol(p, q, a, b), vol(p, q, b, c), vol(p, q, c, a)
    hit = (sp * sq < 0) & (((s1 > 0) & (s2 > 0) & (s3 > 0)) | ((s1 < 0) & (s2 < 0) & (s3 < 0)))
    # How deep each crossing is: the nearer end's distance from the triangle's plane, and the
    # crossing point's distance from the triangle's edges. Below `depth` it is a touch at rounding
    # (manifold3d's union leaves a few such slivers; `cav`'s importer accepts them).
    p, q, a, b, c, sp, sq = p[hit], q[hit], a[hit], b[hit], c[hit], sp[hit], sq[hit]
    n = np.cross(b - a, c - a)
    nl = np.linalg.norm(n, axis=1)
    x = p + (sp / (sp - sq))[:, None] * (q - p)
    deep = np.minimum(np.abs(sp), np.abs(sq)) / nl
    for u, w in ((a, b), (b, c), (c, a)):
        deep = np.minimum(deep, np.linalg.norm(np.cross(w - u, x - u), axis=1) / np.linalg.norm(w - u, axis=1))
    keep = deep > depth
    return np.stack([owner[pairs[hit, 0]], pairs[hit, 1]], axis=1)[keep]


def _pt_tri_dist(p: F64, a: F64, b: F64, c: F64) -> F64:
    """Distance from points to triangles, broadcast over leading axes: the same rule as
    `distance_to_mesh` (plane if the foot is inside, else the nearest edge)."""
    with np.errstate(invalid="ignore", divide="ignore"):
        n = np.cross(b - a, c - a)
        n = n / np.linalg.norm(n, axis=-1, keepdims=True)
        h = np.einsum("...k,...k->...", p - a, n)
        foot = p - h[..., None] * n
        inside = np.ones(h.shape, dtype=bool)
        for u, w in ((a, b), (b, c), (c, a)):
            inside &= np.einsum("...k,...k->...", np.cross(w - u, foot - u), n) >= 0
        best = np.where(inside, np.abs(h), np.inf)
        for u, w in ((a, b), (b, c), (c, a)):
            e = w - u
            t = np.clip(np.einsum("...k,...k->...", p - u, e) / np.einsum("...k,...k->...", e, e), 0, 1)
            best = np.minimum(best, np.linalg.norm(p - (u + t[..., None] * e), axis=-1))
    return np.where(np.isnan(best), np.inf, best)


def distances_to_mesh(tris: F64, pts: F64) -> F64:
    """Exact distance from each point to a triangle soup, as `distance_to_mesh` but for many points.

    Large triangles (circumradius about the centroid > 4× the median, e.g. hole caps) are checked
    against every point; the rest through a k-d tree on their centroids: with the k nearest
    centroids, any other small triangle is at least (k-th centroid distance − largest small radius)
    away, so a point is settled once its best distance is below that; otherwise k grows.
    """
    from scipy.spatial import cKDTree

    cen = tris.mean(axis=1)
    rad = np.linalg.norm(tris - cen[:, None], axis=2).max(axis=1)
    big = rad > 4 * np.median(rad)
    out = np.full(len(pts), np.inf)
    bt = tris[big]
    step_b = max(1, 1_000_000 // max(len(bt), 1))
    for s in range(0, len(pts) if len(bt) else 0, step_b):
        p = pts[s : s + step_b]
        out[s : s + len(p)] = _pt_tri_dist(p[:, None], bt[None, :, 0], bt[None, :, 1], bt[None, :, 2]).min(axis=1)
    small = tris[~big]
    if len(small) == 0:
        return out
    r_max = float(rad[~big].max())
    tree = cKDTree(cen[~big])
    todo = np.arange(len(pts))
    k = 16
    while len(todo):
        k = min(k, len(small))
        done = np.zeros(len(todo), dtype=bool)
        step = max(1, 400_000 // k)
        for s in range(0, len(todo), step):
            idx = todo[s : s + step]
            dk, ik = tree.query(pts[idx], k=k)
            dk, ik = dk.reshape(len(idx), -1), ik.reshape(len(idx), -1)
            t = small[ik]
            d = _pt_tri_dist(pts[idx][:, None], t[:, :, 0], t[:, :, 1], t[:, :, 2]).min(axis=1)
            out[idx] = np.minimum(out[idx], d)
            done[s : s + step] = (out[idx] <= dk[:, -1] - r_max) | (k == len(small))
        todo = todo[~done]
        k *= 4
    return out


def decimation_check(name: str, up: tuple[int, int, int]) -> dict[str, object]:
    """The decimated mesh against the original download, in the study's frame (1 m largest extent):
    two-sided vertex-sampled Hausdorff distance (every decimated vertex to the original triangles,
    every original vertex to the decimated triangles), and the volume of each (signed sum, the
    original's holes left open). Pass (written before measuring): both distances below half the
    finest pitch of the sweep (1/256 m = 3.9e-3 m), so the decimation is finer than every lattice.
    The decimated mesh measured is the one the study uses: decimated, non-manifold triangles dropped,
    holes capped (`famous_repair`)."""
    v0, f0, _, _ = _raw(name, up, original=True)
    _, f_dec, _, _ = _raw(name, up)
    z = np.load(CACHE / f"{name}-decimated-{DECIMATE[name]}.npz")
    m = famous_repair(name).mesh
    v1, f1 = m.verts, m.faces
    a = distances_to_mesh(v0[f0], v1)
    b = distances_to_mesh(v1[f1], np.unique(v0[f0].reshape(-1, 3), axis=0))
    tol = 1.0 / 256
    far = np.unique(v0[f0].reshape(-1, 3), axis=0)[b > tol]
    vol0 = float(sb.volume_centroid(v0[f0], np.zeros(3))[0])
    vol1 = float(sb.volume_centroid(v1[f1], np.zeros(3))[0])
    return {"volume_original_m3": vol0, "volume_decimated_m3": vol1, "volume_rel": vol1 / vol0 - 1,
            "original_vertices_beyond_tol": int(len(far)),
            "far_box_m": [np.round(far.min(0), 3).tolist(), np.round(far.max(0), 3).tolist()] if len(far) else None,
            "original_triangles": int(len(f0)), "dropped_nonmanifold": int(z["dropped_nonmanifold"]),
            "simplified_triangles": int(z["simplified_triangles"]),
            "crossing_triangles_cut": int(z["crossing_cut"]), "crossing_rounds": int(z["crossing_rounds"]),
            "decimator_triangles": int(len(f_dec)),
            "decimated_triangles": int(len(f1)),
            "max_decimated_to_original_m": float(a.max()), "max_original_to_decimated_m": float(b.max()),
            "rms_original_to_decimated_m": float(np.sqrt((b**2).mean())),
            "tolerance_m": tol, "passed": bool(a.max() < tol and b.max() < tol)}


def _components(faces: I64) -> list[I64]:
    """Face index sets of the components connected through shared edges (not mere shared vertices)."""
    from scipy.sparse import coo_matrix
    from scipy.sparse.csgraph import connected_components

    nf = len(faces)
    e = np.sort(np.concatenate([faces[:, [0, 1]], faces[:, [1, 2]], faces[:, [2, 0]]]), axis=1)
    owner = np.tile(np.arange(nf), 3)
    key = e[:, 0] * (int(faces.max()) + 1) + e[:, 1]
    order = np.argsort(key, kind="stable")
    k, o = key[order], owner[order]
    same = np.nonzero(k[1:] == k[:-1])[0]
    adj = coo_matrix((np.ones(len(same)), (o[same], o[same + 1])), shape=(nf, nf))
    _, lab = connected_components(adj, directed=False)
    return [np.nonzero(lab == c)[0] for c in np.unique(lab)]


def _simple_loops(open_e: I64) -> list[list[int]]:
    """The open (boundary) edges a→b as simple loops of vertices, each edge in exactly one loop.

    Walks the edges; when the walk comes back to a vertex already on its path, the part of the path
    since then is a loop and is cut off. (A hole is a loop; holes touching at a vertex become separate
    loops, which a single grouping by shared vertices would merge.)
    """
    out: dict[int, list[int]] = {}
    for a, b in open_e:
        out.setdefault(int(a), []).append(int(b))
    loops: list[list[int]] = []
    for start in list(out):
        while out[start]:
            path = [start]
            pos = {start: 0}
            while True:
                cur = path[-1]
                if not out.get(cur):
                    raise RuntimeError(f"open edges do not form loops at vertex {cur}")
                nxt = out[cur].pop()
                if nxt in pos:
                    i = pos[nxt]
                    loops.append(path[i:])
                    for u in path[i + 1 :]:
                        del pos[u]
                    path = path[: i + 1]
                    if i == 0:
                        break
                else:
                    pos[nxt] = len(path)
                    path.append(nxt)
    return loops


def _ear_clip(v: F64, loop: list[int], edges: set[tuple[int, int]]) -> list[list[int]] | None:
    """Triangles over a hole, made of its own vertices: ear clipping of the loop projected on its
    Newell plane. Each triangle is wound against the loop (as a cap must be). An ear whose new edge
    is already an edge of the mesh (`edges`, undirected, updated) is skipped, as it would make an
    edge of three triangles. None if no ear is left (the projected loop folds over itself in that
    plane); the caller then fans to the centroid."""
    p = v[np.array(loop)]
    q = np.roll(p, -1, axis=0)
    n = np.array([((p[:, 1] - q[:, 1]) * (p[:, 2] + q[:, 2])).sum(), ((p[:, 2] - q[:, 2]) * (p[:, 0] + q[:, 0])).sum(),
                  ((p[:, 0] - q[:, 0]) * (p[:, 1] + q[:, 1])).sum()])
    if np.linalg.norm(n) == 0:
        return None
    n /= np.linalg.norm(n)
    a = np.cross(n, [1.0, 0, 0] if abs(n[0]) < 0.9 else [0, 1.0, 0])
    a /= np.linalg.norm(a)
    b = np.cross(n, a)
    xy = np.stack([p @ a, p @ b], axis=1)  # anticlockwise in (a, b), since n is the loop's normal
    idx = list(range(len(loop)))
    tris: list[list[int]] = []
    new_edges: list[tuple[int, int]] = []

    def cross2(o: F64, s: F64, t: F64) -> float:
        return float((s[0] - o[0]) * (t[1] - o[1]) - (s[1] - o[1]) * (t[0] - o[0]))

    while len(idx) > 3:
        for k in range(len(idx)):
            i0, i1, i2 = idx[k - 1], idx[k], idx[(k + 1) % len(idx)]
            if cross2(xy[i0], xy[i1], xy[i2]) <= 0:
                continue
            diag = (min(loop[i0], loop[i2]), max(loop[i0], loop[i2]))
            if diag in edges or diag in new_edges:
                continue
            others = [j for j in idx if j not in (i0, i1, i2)]
            o = xy[others]
            if len(o):
                d0 = (xy[i1][0] - xy[i0][0]) * (o[:, 1] - xy[i0][1]) - (xy[i1][1] - xy[i0][1]) * (o[:, 0] - xy[i0][0])
                d1 = (xy[i2][0] - xy[i1][0]) * (o[:, 1] - xy[i1][1]) - (xy[i2][1] - xy[i1][1]) * (o[:, 0] - xy[i1][0])
                d2 = (xy[i0][0] - xy[i2][0]) * (o[:, 1] - xy[i2][1]) - (xy[i0][1] - xy[i2][1]) * (o[:, 0] - xy[i2][0])
                if np.any((d0 >= 0) & (d1 >= 0) & (d2 >= 0)):
                    continue
            tris.append([loop[i2], loop[i1], loop[i0]])
            new_edges.append(diag)
            idx.pop(k)
            break
        else:
            return None
    if cross2(xy[idx[0]], xy[idx[1]], xy[idx[2]]) <= 0:
        return None
    tris.append([loop[idx[2]], loop[idx[1]], loop[idx[0]]])
    edges.update(new_edges)
    return tris


@dataclass
class Repair:
    """What `repair` did and what its check measured.

    Attributes:
        mesh        (Mesh):  The repaired closed mesh.
        parts       (list):  Per source part: faces, open edges, holes capped, kept or dropped and why.
        caps        (F64):   The cap triangles added (each a fan from its hole's centroid), m.
        source_tris (F64):   The original triangles after welding, in the repaired mesh's frame, m.
        dropped_tris (F64):  Original triangles of dropped parts (zero-volume), m.
        check       (dict):  The check's measurements and verdict (see `check_repair`).
    """

    mesh:         Mesh
    parts:        list[dict[str, object]]
    caps:         F64
    source_tris:  F64
    dropped_tris: F64
    closed_parts: list[tuple[F64, F64]] = field(default_factory=list)
    check:        dict[str, object] = field(default_factory=dict)


def repair(name: str, up: tuple[int, int, int], digits: int = 8,
           source: tuple[F64, I64] | None = None, ear: bool = False) -> Repair:
    """Close an open mesh while keeping its triangles.

    1. Weld vertices equal to `digits` decimals (unwelded patch seams become shared edges).
    2. Split into edge-connected parts. A part of zero volume (a two-sided sheet) is dropped.
    3. Cap every remaining hole of every part: one triangle per open edge, to the hole's centroid,
       wound against the edge so the part becomes closed and consistently wound. With `ear` (the
       famous meshes), a hole is first ear-clipped in its own plane, using only its own vertices;
       a centroid fan through a bent hole can pierce the surface next to it (the bunny's base did:
       `cav` refused it as crossing itself), ear clipping stays on the rim. The fan is the fallback.
    4. Union the closed parts with manifold3d (Apache-2.0, double precision `Mesh64`), so parts that
       pass through each other (spout and handle into a teapot body, eyes into a head) count once.
    Then `check_repair` measures the result against the original triangles. `source` (vertices,
    triangles in the study's frame) replaces the download, e.g. a decimation of it.
    """
    import manifold3d

    v, f = source if source is not None else _raw(name, up)[:2]
    q = np.round(v / 1.0, digits)
    _, first, inv = np.unique(q, axis=0, return_index=True, return_inverse=True)
    inv = inv.reshape(-1)
    v = v[first]
    f = inv[f]
    f = f[(f[:, 0] != f[:, 1]) & (f[:, 1] != f[:, 2]) & (f[:, 2] != f[:, 0])]
    # Back-to-back triangle pairs (the same three vertices, opposite winding) are a zero-thickness
    # sheet: no volume, and glued to the rest they make non-manifold edges. Drop both.
    tri_key = np.sort(f, axis=1)
    _, inv_t, cnt_t = np.unique(tri_key, axis=0, return_inverse=True, return_counts=True)
    sheet = cnt_t[inv_t.reshape(-1)] > 1
    dropped_sheet = v[f[sheet]]
    f = f[~sheet]
    parts_info: list[dict[str, object]] = []
    if ear:
        # Welding can join two copies of a point that the source kept apart (a pinch in a scan or in
        # manifold3d's output), making an edge of four triangles: drop those, the holes are capped.
        mask = manifold_mask(f)
        if not mask.all():
            parts_info.append({"faces": int((~mask).sum()), "open_edges": 0, "holes": 0, "volume_m3": 0.0,
                               "kept": "dropped: on an edge of more than two triangles after welding"})
            dropped_sheet = np.concatenate([dropped_sheet, v[f[~mask]]])
            f = f[mask]
    if sheet.any():
        parts_info.append({"faces": int(sheet.sum()), "open_edges": 0, "holes": 0, "volume_m3": 0.0,
                           "kept": "dropped: back-to-back triangles (a zero-thickness sheet)"})
    closed: list[tuple[F64, I64]] = []
    closed_parts: list[tuple[F64, F64]] = []
    caps: list[F64] = []
    dropped: list[F64] = [dropped_sheet] if len(dropped_sheet) else []
    for comp in _components(f):
        pf = f[comp]
        e = np.concatenate([pf[:, [0, 1]], pf[:, [1, 2]], pf[:, [2, 0]]])
        key = e[:, 0] * len(v) + e[:, 1]
        rev = e[:, 1] * len(v) + e[:, 0]
        open_e = e[~np.isin(key, rev)]
        cap_f: list[list[int]] = []
        pv = v.copy()
        n_holes = n_ear = 0
        if len(open_e):
            # Split the open edges into simple loops (holes), cap each to its centroid. Two holes that
            # touch at a vertex are two loops, so no cap edge is made twice.
            loops = _simple_loops(open_e)
            n_holes = len(loops)
            extra = []
            part_edges = {(int(min(a, b)), int(max(a, b))) for a, b in e} if ear else set()
            for loop in loops:
                if ear:
                    clipped = _ear_clip(v, loop, part_edges)
                    if clipped is not None:
                        cap_f.extend(clipped)
                        n_ear += 1
                        continue
                c = v[np.array(loop)].mean(axis=0)
                ci = len(v) + len(extra)
                extra.append(c)
                for k, p0 in enumerate(loop):
                    cap_f.append([int(loop[(k + 1) % len(loop)]), int(p0), ci])
            pv = np.concatenate([v, np.array(extra)]) if extra else v.copy()
        allf = np.concatenate([pf, np.array(cap_f, dtype=np.int64)]) if cap_f else pf
        vol = float(np.einsum("nk,nk->n", pv[allf[:, 0]], np.cross(pv[allf[:, 1]], pv[allf[:, 2]])).sum() / 6)
        info: dict[str, object] = {"faces": int(len(pf)), "open_edges": int(len(open_e)), "holes": int(n_holes),
                                   "volume_m3": abs(vol)}
        if ear:
            info["holes_ear_clipped"] = n_ear
        if abs(vol) < 1e-9:
            info["kept"] = "dropped: zero volume (a two-sided sheet)"
            dropped.append(v[pf])
        elif ear and abs(vol) < 1e-7 and len(pf) < 1000:
            # The decimated scans leave a few specks of a few triangles (pinched off where the
            # decimator folded the surface); under a 4.6 mm cube of a 1 m body, they are dropped.
            info["kept"] = "dropped: a speck under 1e-7 m³ (decimation debris)"
            dropped.append(v[pf])
        else:
            if vol < 0:
                allf = allf[:, ::-1]
            ok, why = check_closed(allf)
            info["kept"] = "closed" if ok else f"NOT closed: {why}"
            if not ok:
                raise RuntimeError(f"{name}: part could not be closed: {why}")
            closed.append((pv, allf))
            closed_parts.append((v[pf], pv[allf]))
            if cap_f:
                caps.append(pv[np.array(cap_f)])
        parts_info.append(info)
    union = manifold3d.Manifold()
    for pv, pf in closed:
        used = np.unique(pf)
        remap = np.full(len(pv), -1, dtype=np.int64)
        remap[used] = np.arange(len(used))
        mesh64 = manifold3d.Mesh64(np.ascontiguousarray(pv[used], dtype=np.float64),
                                   np.ascontiguousarray(remap[pf], dtype=np.uint64))
        part = manifold3d.Manifold(mesh64)
        if part.status() != manifold3d.Error.NoError:
            raise RuntimeError(f"{name}: manifold3d rejected a part: {part.status()}")
        union = union + part
    mv, mf = _from_manifold(union)
    src = v[f]
    return Repair(Mesh(name, mv, mf, ""), parts_info,
                  np.concatenate(caps) if caps else np.zeros((0, 3, 3)), src,
                  np.concatenate(dropped) if dropped else np.zeros((0, 3, 3)), closed_parts)


def check_repair(rep: Repair, mesh: Mesh, expected_components: int, tol: float = 1e-6) -> dict[str, object]:
    """Measure a closed mesh against the original triangles; pass/fail written before measuring.

    - closed: every directed edge has its twin (`check_closed`);
    - components: the number of edge-connected pieces equals `expected_components` (a stray fragment
      or a column fails this);
    - to_source: every vertex of `mesh` lies within `tol` (m) of the original triangles or of the
      added caps (a re-meshed surface fails this: its vertices sit off the original);
    - from_source: every original vertex that is not inside another part lies within `tol` of `mesh`
      (lost or moved detail fails this). "Inside another part" is judged by that part's winding
      number > 0.5.
    """
    ok_closed, why = check_closed(mesh.faces)
    comps = _components(mesh.faces)
    n_comp = len(comps)
    comp_info = []
    for cf in comps:
        t = mesh.verts[mesh.faces[cf]]
        vol = float(np.einsum("nk,nk->n", t[:, 0], np.cross(t[:, 1], t[:, 2])).sum() / 6)
        comp_info.append({"triangles": int(len(cf)), "volume_m3": round(vol, 6),
                          "lo": np.round(t.reshape(-1, 3).min(0), 3).tolist(),
                          "hi": np.round(t.reshape(-1, 3).max(0), 3).tolist()})
    ref = np.concatenate([rep.source_tris, rep.caps]) if len(rep.caps) else rep.source_tris
    tris_m = mesh.tris
    to_src = float(distances_to_mesh(ref, mesh.verts).max())
    # Original vertices of each kept part that lie outside every other kept part.
    keep_list = []
    n_inside = 0
    for i, (src_i, _) in enumerate(rep.closed_parts):
        sv = np.unique(src_i.reshape(-1, 3), axis=0)
        inside = np.zeros(len(sv), dtype=bool)
        for j, (_, closed_j) in enumerate(rep.closed_parts):
            if j != i:
                inside |= winding(closed_j, sv) > 0.5
        n_inside += int(inside.sum())
        keep_list.append(sv[~inside])
    keep = np.concatenate(keep_list)
    from_src = float(distances_to_mesh(tris_m, keep).max())
    # Added 26 Nov 2026, before measuring: no edge may pass through a triangle (`cav` refuses such a
    # mesh, and inside is then ambiguous).
    crossings = int(len(self_crossings(mesh.verts, mesh.faces)))
    passed = bool(ok_closed and n_comp == expected_components and to_src < tol and from_src < tol and crossings == 0)
    return {"closed": ok_closed, "closed_detail": why, "components": n_comp, "component_info": comp_info,
            "expected_components": expected_components, "max_vertex_to_source_m": float(to_src),
            "max_source_to_mesh_m": float(from_src), "self_crossings": crossings,
            "source_vertices_checked": int(len(keep)),
            "source_vertices_inside_other_parts": n_inside, "tolerance_m": tol,
            "passed": passed}


TEAPOT_UP = (1, -3, 2)
SUZANNE_UP = (-1, 3, 2)  # facing +y, towards the camera


def teapot_repair() -> Repair:
    # One body: the lid's capped rim (radius 1.3 in Newell's units) sits on the body's capped opening
    # (radius 1.4), both at height 2.4, so the union joins them there; spout and handle pass into it.
    return _cached_repair("teapot", TEAPOT_UP, 1)


def suzanne_repair() -> Repair:
    return _cached_repair("suzanne", SUZANNE_UP, 1)


def _cached_repair(name: str, up: tuple[int, int, int], expected: int) -> Repair:
    import pickle

    path = CACHE / f"{name}-repair{f'-{DECIMATE[name]}' if name in DECIMATE else ''}.pkl"
    if path.exists():
        return pickle.loads(path.read_bytes())
    rep = repair(name, up, ear=name in FAMOUS)
    rep.check = check_repair(rep, rep.mesh, expected)
    if not rep.check["passed"]:
        raise RuntimeError(f"{name}: repaired mesh fails its check: {rep.check}")
    CACHE.mkdir(exist_ok=True)
    path.write_bytes(pickle.dumps(rep))
    return rep


def teapot_mesh() -> Mesh:
    rep = teapot_repair()
    p = rep.parts
    return Mesh("teapot", rep.mesh.verts, rep.mesh.faces,
                f"Utah teapot (Newell 1975) as 6320 triangles: patch seams welded, giving {len(p)} open "
                f"parts (body open at the top, lid open at its rim, spout and handle open at both ends: "
                f"{sum(int(x['holes']) for x in p)} holes), each hole capped by a fan to its centroid, the "
                f"closed parts united with manifold3d (spout and handle pass into the body; the lid sits "
                f"on the rim); {len(rep.mesh.faces)} triangles, the original triangles kept")


def suzanne_mesh() -> Mesh:
    rep = suzanne_repair()
    p = rep.parts
    kept = [x for x in p if str(x["kept"]).startswith("closed")]
    return Mesh("suzanne", rep.mesh.verts, rep.mesh.faces,
                f"Blender's Suzanne as 968 triangles: welded, {len(p)} parts (head with two open eye "
                f"sockets, two eyes open at the back, one two-sided sheet of 2 triangles dropped), "
                f"{sum(int(x['holes']) for x in kept)} holes capped by fans to their centroids, the head "
                f"and eyes united (manifold3d); {len(rep.mesh.faces)} triangles, original triangles kept")


@dataclass(frozen=True)
class Famous:
    """A famous mesh added on 26 Nov 2026: how to stand it up, how many pieces its repair must give,
    and the words for captions.

    Attributes:
        up       (tuple): Source axes that become (x, y, z), 1-based, negative to flip (see `_raw`).
        expected (int):   Edge-connected pieces the repaired mesh must have.
        title    (str):   Picture title.
        credit   (str):   Source and licence, for captions.
    """

    up:       tuple[int, int, int]
    expected: int
    title:    str
    credit:   str


FAMOUS = {
    "bunny": Famous((-1, 3, 2), 1, "Stanford Bunny",
                    "Stanford Bunny (Turk & Levoy 1994, Stanford 3D Scanning Repository, bun_zipper.ply)"),
    "spot": Famous((1, -3, 2), 1, "Spot", "Spot (Keenan Crane, CC0 1.0, spot_triangulated.obj)"),
    "dragon": Famous((-1, 3, 2), 1, "Stanford Dragon",
                     "Stanford Dragon (Curless & Levoy 1996, Stanford 3D Scanning Repository, dragon_vrip.ply)"),
    "armadillo": Famous((1, -3, 2), 1, "Armadillo",
                        "Armadillo (Krishnamurthy & Levoy 1996, Stanford 3D Scanning Repository, Armadillo.ply)"),
    "buddha": Famous((-1, 3, 2), 1, "Happy Buddha",
                     "Happy Buddha (Curless & Levoy 1996, Stanford 3D Scanning Repository, happy_vrip.ply)"),
    "bracket": Famous((1, 2, 3), 1, "mk7 extruder drive (CAD part)",
                      "mk7 ball bearing extruder drive by jag (Thingiverse 13727, Thingi10K 44950, public domain)"),
}


def famous_repair(name: str) -> Repair:
    return _cached_repair(name, FAMOUS[name].up, FAMOUS[name].expected)


def famous_mesh(name: str) -> Mesh:
    rep = famous_repair(name)
    p = rep.parts
    kept = [x for x in p if str(x["kept"]).startswith("closed")]
    holes = sum(int(x["holes"]) for x in kept)
    n_ear = sum(int(x.get("holes_ear_clipped", 0)) for x in kept)  # type: ignore[call-overload]
    how = ("" if holes == 0 else " by ear clipping" if n_ear == holes
           else f" ({n_ear} by ear clipping, {holes - n_ear} by fans to their centroids)")
    dec = (f"decimated (quadric error, self-crossings cut out and closed) from {len(_load(name)[1])} to "
           f"{len(rep.source_tris)} triangles, " if name in DECIMATE else "")
    dropped = len(p) - len(kept)
    return Mesh(name, rep.mesh.verts, rep.mesh.faces,
                f"{FAMOUS[name].credit}: {dec}welded, {len(kept)} part{'s' if len(kept) != 1 else ''}"
                f"{f' ({dropped} zero-volume dropped)' if dropped else ''}, "
                f"{holes} hole{'s' if holes != 1 else ''} capped{how}"
                f"{', parts united (manifold3d)' if len(kept) > 1 else ''}; {len(rep.mesh.faces)} triangles, "
                f"{'the decimated' if name in DECIMATE else 'the original'} triangles kept")


def study_repair(name: str) -> Repair:
    return {"teapot": teapot_repair, "suzanne": suzanne_repair}.get(name, lambda: famous_repair(name))()


def remeshed_by_winding(name: str) -> Mesh:
    """The earlier, rejected closing (marching cubes on a 1/48 m grid), for the check's control arm.
    For the famous meshes the side comes from the repaired mesh (see `closed_from_soup`)."""
    if name in FAMOUS:
        v, f, _, _ = _raw(name, FAMOUS[name].up)
        return closed_from_soup(name, v, f, 48, inside_tris=famous_repair(name).mesh.tris)
    up = TEAPOT_UP if name == "teapot" else SUZANNE_UP
    v, f, _, _ = _raw(name, up)
    return closed_from_soup(name, v, f, 48)


CHECKED = ["teapot", "suzanne", *FAMOUS]


def mesh_check(names: list[str] | None = None) -> dict[str, object]:
    """Run `check_repair` on each repaired mesh and, as the control arm, on the rejected marching-cubes
    closing of the same download. Pass: every repaired mesh passes and every control fails (the
    teapot's control has a detached spout fragment; both controls sit off the original triangles).
    Decimated meshes also get `decimation_check` against the original download.
    Writes mesh-check.json and mesh-check.md here (`names` updates only those entries, the rest are
    kept from the json); raises if the verdict is not as expected."""
    import json

    path = HERE / "mesh-check.json"
    out: dict[str, object] = json.loads(path.read_text()) if path.exists() and names else {}
    for name in names or CHECKED:
        rep = study_repair(name)
        exp = FAMOUS[name].expected if name in FAMOUS else 1
        control = check_repair(rep, remeshed_by_winding(name), exp)
        out[name] = {"repaired": rep.check, "control_marching_cubes": control, "parts": rep.parts}
        if name in DECIMATE:
            out[name]["decimation"] = decimation_check(name, FAMOUS[name].up)  # type: ignore[index]
        print(name, "checked", flush=True)
    lines = ["| mesh | closed | pieces (expected) | max vertex → original or caps (m) | "
             "max original vertex → mesh (m) | self-crossings | verdict |", "|---|---|---|---|---|---|---|"]
    dec = ["| mesh | original triangles | on non-manifold edges, dropped | decimated to | "
           "cut out where it crossed itself (rounds) | after repair | "
           "max decimated vertex → original (m) | max original vertex → decimated (m) | "
           "RMS original → decimated (m) | original vertices beyond 1/256 m | volume change | "
           "verdict (< 1/256 m) |",
           "|---|---|---|---|---|---|---|---|---|---|---|---|"]
    ok = dec_ok = True
    for name in [n for n in CHECKED if n in out]:
        o = out[name]
        for label, c in (("repaired", o["repaired"]), ("control: marching cubes 1/48 m", o["control_marching_cubes"])):
            lines.append(f"| {name}, {label} | {c['closed']} | {c['components']} ({c['expected_components']}) | "
                         f"{c['max_vertex_to_source_m']:.1e} | {c['max_source_to_mesh_m']:.1e} | "
                         f"{c['self_crossings']} | {'pass' if c['passed'] else 'FAIL'} |")
        ok &= bool(o["repaired"]["passed"]) and not bool(o["control_marching_cubes"]["passed"])
        if "decimation" in o:
            d = o["decimation"]
            dec.append(f"| {name} | {d['original_triangles']} | {d['dropped_nonmanifold']} | "
                       f"{d['simplified_triangles']} | "
                       f"{d['crossing_triangles_cut']} ({d['crossing_rounds']}) | {d['decimated_triangles']} | "
                       f"{d['max_decimated_to_original_m']:.1e} | {d['max_original_to_decimated_m']:.1e} | "
                       f"{d['rms_original_to_decimated_m']:.1e} | {d['original_vertices_beyond_tol']} | "
                       f"{d['volume_rel']:+.1e} | {'pass' if d['passed'] else 'FAIL'} |")
            dec_ok &= bool(d["passed"])
    # A failed decimation is reported (the table says FAIL and the note says where), not raised:
    # the voxel study measures against the decimated mesh, which passed its own repair check.
    out["as_expected"] = ok
    out["decimations_within_tolerance"] = dec_ok
    path.write_text(json.dumps(out, indent=1, default=str))
    (HERE / "mesh-check.md").write_text("\n".join(lines) + "\n\nDecimation (the decimated mesh against the "
                                        "original download, same frame, vertex-sampled both ways):\n\n"
                                        + "\n".join(dec) + "\n", encoding="utf-8")
    if not ok:
        raise RuntimeError("mesh check: a repaired mesh failed, or a control passed")
    if not dec_ok:
        print("mesh check: a decimation is beyond 1/256 m of its original (see the table)", flush=True)
    return out


if __name__ == "__main__":
    # Through the module, not __main__, so the cached Repair pickles under `shapes_lib`.
    import shapes_lib

    shapes_lib.mesh_check(sys.argv[1:] or None)
    print((HERE / "mesh-check.md").read_text(encoding="utf-8"))


def mesh_by_name(name: str) -> Mesh:
    if name in FAMOUS:
        return famous_mesh(name)
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


def _sampled_fill(tris: F64, base: F64, h: float, ijk: I64) -> F64:
    """Cavendish's sampled fill: the fraction of a cell's 4³ sample points (at (i + ½)h/4) inside.

    Only the boundary cells' 16 z-columns are cast: winding at each sample = the sum of crossing
    steps below it on its column (`searchsorted` on (column, z) keys).
    """
    if len(ijk) == 0:
        return np.zeros(0)
    sub = (np.arange(4) + 0.5) * h / 4
    nx = int(ijk[:, 0].max()) + 1
    ny = int(ijk[:, 1].max()) + 1
    n2 = (4 * nx, 4 * ny)
    line, t, dw = crossings(tris, 2, base[[0, 1]] + h / 8, h / 4, n2)
    cum = np.concatenate([[0.0], np.cumsum(dw)])
    t0 = float(base[2]) - 1.0
    scale = float(np.ceil((t.max() - t0) if len(t) else 1.0)) + 2.0
    keys = line * scale + (t - t0)  # sorted, since crossings are sorted by (line, t)
    # Sample columns of each cell: (4i + a, 4j + b), a, b < 4; heights base_z + kh + sub.
    a, b = np.meshgrid(np.arange(4), np.arange(4), indexing="ij")
    cols = ((4 * ijk[:, 0, None] + a.ravel()[None]) * n2[1] + 4 * ijk[:, 1, None] + b.ravel()[None])  # (N, 16)
    first = np.searchsorted(keys, cols * scale)  # first crossing on each column
    count = np.zeros(len(ijk))
    for kz in range(4):
        z = base[2] + ijk[:, 2] * h + sub[kz]
        below = np.searchsorted(keys, cols * scale + (z[:, None] - t0))
        wind = cum[below] - cum[first]
        count += (np.rint(wind) > 0).sum(axis=1)
    return count / 64.0


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

    ctr_all = grid_inside(tris, base + h / 2, h, (int(n[0]), int(n[1]), int(n[2])))

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
        p_tris.append(t_all)
        p_of.append(np.full(len(t_all), len(b_ijk) - 1))
        if keep_drawing:
            draw_polys.append(polys)
            draw_caps.append(segs)

    corner_in = wz[:-1, :-1, :-1] > 0
    interior = np.argwhere(corner_in & ~is_boundary).astype(np.int64)
    b_ss = list(_sampled_fill(tris, base, h, np.array(b_ijk, dtype=np.int64)))
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


def g_series4(mass: F64, c: F64, h: float, pts: F64) -> F64:
    """Uniform cubes of side h as their series to order 4 (Cavendish's far kernel): point mass + the
    l = 4 term U₄ = A(5Σxᵢ⁴/r⁹ − 3/r⁵), A = −(7/60)(h/2)⁴·mass, x = point − centre. (A cube has no
    quadrupole, so order 2 is the point mass.)"""
    out = g_points(mass, c, pts)
    step = max(1, 2_000_000 // max(len(pts), 1))
    for s in range(0, len(mass), step):
        x = pts[:, None] - c[None, s : s + step]
        r = np.linalg.norm(x, axis=2)
        big_s = (x**4).sum(axis=2)
        a = -(7 / 60) * (h / 2) ** 4 * mass[None, s : s + step]
        g = a[..., None] * (20 * x**3 / r[..., None] ** 9 - 45 * (big_s / r**11)[..., None] * x
                            + 15 * x / r[..., None] ** 7)
        out += g.sum(axis=1)
    return out


def g_cubes_fast(centres: F64, h: float, fill: F64, pts: F64, ratio: float = 0.01) -> F64:
    """Uniform cubes: the series to order 4 where (h/2)/distance < `ratio` (its l = 6 remainder is
    then < ratio⁶ ≈ 1e-12 relative), the exact polyhedral cube elsewhere."""
    if len(centres) == 0:
        return np.zeros((len(pts), 3))
    dmin = np.full(len(centres), np.inf)
    for p in pts:
        dmin = np.minimum(dmin, np.linalg.norm(centres - p, axis=1))
    far = 0.5 * h / dmin < ratio
    return (g_series4(fill[far] * h**3, centres[far], h, pts)
            + g_cubes(centres[~far], h, fill[~far], pts))


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
    """An orthographic z-buffered rasteriser, supersampled 2x, lit when the picture is taken.

    Faces are stored with their colour and normal; `image()` lights them the way the paper's
    diagrams are lit (docs/papers/polyhedral-gravity/scripts/lighting.py): Lambert from a key light
    50 degrees up from the picture's left and a little towards the viewer, a weaker fill from the
    right and an ambient term, times a screen-space ambient occlusion from the depth buffer (each
    pixel's neighbours within `ao_radius` that rise above its own tangent plane darken it, by at most
    `AO_STRENGTH`). Lines are composited over the lit picture.

    Args:
        size      (int):   Output width and height, pixels.
        azimuth   (float): Camera azimuth, degrees from +x towards +y.
        elevation (float): Camera elevation, degrees.
        centre    (F64):   Point at the image centre, m.
        half      (float): Half the visible width, m.
        ao_radius (float): The occlusion's reach, m (default 6 % of the half width).
    """

    AMBIENT, KEY, FILL = 0.50, 0.55, 0.22  # as lighting.py
    AO_STRENGTH = 0.30
    BACKGROUND = np.array([0xFC, 0xFC, 0xFB]) / 255.0

    def __init__(self, size: int, azimuth: float, elevation: float, centre: F64, half: float,
                 ao_radius: float | None = None) -> None:
        self.n = 2 * size
        az, el = math.radians(azimuth), math.radians(elevation)
        eye = np.array([math.cos(el) * math.cos(az), math.cos(el) * math.sin(az), math.sin(el)])
        self.fwd = -eye
        self.right = np.cross(self.fwd, [0.0, 0.0, 1.0])
        self.right /= np.linalg.norm(self.right)
        self.up = np.cross(self.right, self.fwd)
        self.centre = np.asarray(centre, dtype=float)
        self.scale = self.n / (2 * half)
        flat = eye * np.array([1.0, 1.0, 0.0])
        across = -self.right + 0.3 * flat / np.linalg.norm(flat)
        across /= np.linalg.norm(across)
        self.key = (math.cos(math.radians(50)) * across
                    + math.sin(math.radians(50)) * np.array([0.0, 0.0, 1.0]))
        self.fill = 0.60 * eye + 0.75 * self.right - 0.15 * self.up
        self.fill /= np.linalg.norm(self.fill)
        self.ao_radius = 0.06 * half if ao_radius is None else ao_radius
        self.z = np.full((self.n, self.n), np.inf)
        self.alb = np.ones((self.n, self.n, 3)) * self.BACKGROUND
        self.nrm = np.zeros((self.n, self.n, 3))
        self.overlays: list[tuple[npt.NDArray[np.bool_], F64, float]] = []
        self.depth_bias = 2.0 / self.scale

    def project(self, p: F64) -> F64:
        q = p - self.centre
        x = q @ self.right * self.scale + self.n / 2
        y = self.n / 2 - q @ self.up * self.scale
        return np.stack([x, y, q @ self.fwd], axis=-1)

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
            self.alb[iy[near], ix[near]] = colour
            self.nrm[iy[near], ix[near]] = nrm

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
        self.alb[sel] = np.asarray(colour, dtype=float)
        self.nrm[sel] = normal / np.linalg.norm(normal)

    def dots(self, lines: list[F64], colour: F64, pitch: float, radius: tuple[float, float],
             front: F64 | None = None, hair: float = 2.0) -> None:
        """Polylines (each (k, 3)) as round dots every `pitch` output pixels, part of the 3-D picture:
        each pixel of a dot is depth-tested against what is drawn, so geometry in front covers it
        (a dot within `hair` supersampled pixels of depth behind a surface still shows). With
        `front`, a depth buffer of the same camera (e.g. the true surface's own render), a dot must
        also lie in front of it, so a wireframe of a surface hides its own far side. Dot radii taper
        from radius[0] (far) to radius[1] (near), output pixels."""
        pts = []
        for line in lines:
            q = self.project(line)
            seg = np.linalg.norm(np.diff(q[:, :2], axis=0), axis=1)
            cum = np.concatenate([[0.0], np.cumsum(seg)])
            s = np.arange(0.0, cum[-1] + 1e-9, 2.0 * pitch)
            i = np.clip(np.searchsorted(cum, s, side="right") - 1, 0, len(q) - 2)
            t = (s - cum[i]) / np.maximum(seg[i], 1e-12)
            pts.append(q[i] + t[:, None] * (q[i + 1] - q[i]))
        if not pts:
            return
        p = np.vstack(pts)
        lo, hi = p[:, 2].min(), p[:, 2].max()
        near = (hi - p[:, 2]) / (hi - lo) if hi > lo else np.ones(len(p))
        rad = 2.0 * (radius[0] + (radius[1] - radius[0]) * near)
        mask = np.zeros((self.n, self.n), dtype=bool)
        r = int(np.ceil(rad.max()))
        oy, ox = np.mgrid[-r:r + 1, -r:r + 1]
        for (x, y, d), rr in zip(p, rad):
            iy, ix = np.floor(y).astype(int) + oy, np.floor(x).astype(int) + ox
            ok = ((iy + 0.5 - y) ** 2 + (ix + 0.5 - x) ** 2 <= rr * rr) & (iy >= 0) & (iy < self.n) \
                & (ix >= 0) & (ix < self.n)
            iy, ix = iy[ok], ix[ok]
            vis = d <= self.z[iy, ix] + hair * self.depth_bias
            if front is not None:
                vis &= d <= front[iy, ix] + hair * self.depth_bias
            mask[iy[vis], ix[vis]] = True
        self.overlays.append((mask, np.asarray(colour, dtype=float), 1.0))

    def lines(self, segs: F64, colour: F64, alpha: float) -> None:
        """Segments (N, 2, 3), depth-tested against what is drawn, blended at `alpha` over the lit
        picture."""
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
        self.overlays.append((mask, np.asarray(colour, dtype=float), alpha))

    def occlusion(self) -> F64:
        """Screen-space ambient occlusion factor per pixel (1 open, 1 - AO_STRENGTH closed)."""
        n, z, nrm = self.n, self.z, self.nrm
        hit = np.isfinite(z)
        zf = np.where(hit, z, 0.0)
        # the depth change per pixel step right (x) and down (y) on each pixel's tangent plane
        nf = nrm @ self.fwd
        nf = np.where(np.abs(nf) > 0.05, nf, -0.05)
        gx = -(nrm @ self.right) / nf / self.scale
        gy = (nrm @ self.up) / nf / self.scale
        reach = 2.0 * self.ao_radius
        m = 32
        i = np.arange(m) + 0.5
        rad = self.ao_radius * self.scale * np.sqrt(i / m)
        ang = i * math.pi * (3 - math.sqrt(5))
        occ = np.zeros((n, n))
        for rr, aa in zip(rad, ang):
            dx, dy = int(round(rr * math.cos(aa))), int(round(rr * math.sin(aa)))
            if dx == 0 and dy == 0:
                continue
            zs = np.full((n, n), np.inf)
            ys0, ys1 = max(0, dy), min(n, n + dy)
            xs0, xs1 = max(0, dx), min(n, n + dx)
            zs[ys0 - dy:ys1 - dy, xs0 - dx:xs1 - dx] = z[ys0:ys1, xs0:xs1]
            rise = zf + gx * dx + gy * dy - zs  # > 0: the neighbour is nearer than the tangent plane
            near = np.isfinite(zs) & (rise > 2 * self.depth_bias) & (rise < reach)
            occ += np.where(near, 1.0 - rise / reach, 0.0)
        occ /= m
        # a small blur over covered pixels hides the sampling pattern
        w = hit.astype(float)
        num, den = np.zeros((n, n)), np.zeros((n, n))
        for dy in range(-2, 3):
            for dx in range(-2, 3):
                num += np.roll(np.roll(occ * w, dy, 0), dx, 1)
                den += np.roll(np.roll(w, dy, 0), dx, 1)
        occ = np.where(den > 0, num / np.maximum(den, 1e-9), 0.0)
        return 1.0 - self.AO_STRENGTH * np.clip(2.0 * occ, 0, 1)

    def image(self) -> F64:
        hit = np.isfinite(self.z)
        k = (self.AMBIENT + self.KEY * np.clip(self.nrm @ self.key, 0, None)
             + self.FILL * np.clip(self.nrm @ self.fill, 0, None)) * self.occlusion()
        k = k[..., None]
        lit = self.alb * np.minimum(k, 1.0) + (1.0 - self.alb) * np.clip(k - 1.0, 0, None) * 0.6
        img = np.where(hit[..., None], np.clip(lit, 0, 1), self.BACKGROUND)
        for mask, colour, alpha in self.overlays:
            img[mask] = (1 - alpha) * img[mask] + alpha * colour
        m = self.n // 2
        return img.reshape(m, 2, m, 2, 3).mean(axis=(1, 3))
