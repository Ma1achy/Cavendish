"""Lighting and drawing for the paper's 3-D panels, shared by diagrams.py and octree_diagrams.py.

Three things:

- **Light.** Lambert from one key light (50 degrees up, from the picture's left and a little
  towards the viewer), a weaker fill from the right and an ambient term, all fixed in the camera's
  frame so every panel is lit alike. The three visible faces of a cube come out clearly different
  tones (about 0.95, 0.80 and 0.71 of the colour; they stay apart in greyscale). The shapes study's
  rasteriser (`shapes_lib.Canvas`) uses the same weights and directions.
- **Ambient occlusion (AO).** For voxels, Minecraft-style per corner: the three cells next to a
  face corner on the open side (side, side, corner) give 0-3 open neighbours. For everything else,
  short-range ray casting against nearby geometry (`ray_occlusion`). Both stay subtle (at most
  about a quarter darker, in the tightest voxel corner): the figures are technical drawings.
- **Painting.** Matplotlib cannot shade a polygon with a gradient, so a face is split into sub-quads
  (or sub-triangles) with the AO interpolated; and its 3-D axes sort every polygon by mean depth, which
  cannot keep a face's sub-quads under that face's outline. `Scene` therefore projects
  orthographically itself (the same view as matplotlib's `view_init(elev, azim)`) and paints back
  to front onto a plain 2-D axes, in one `PolyCollection` whose order is the painter's order: for
  each face its flat base colour (hides antialiasing seams), its sub-pieces, then its outline.

The true body's edges are drawn over voxels as a hidden-line wireframe (`Scene.wireframe`): each
edge is sampled, every sample is ray-tested towards the eye, and the runs in front are dashed black,
the runs behind or inside faint grey.

Lengths are in whatever unit the caller's geometry uses; angles in degrees.
"""

from __future__ import annotations

from collections.abc import Callable, Sequence

import matplotlib as mpl
import matplotlib.pyplot as plt
import numpy as np
from matplotlib.collections import LineCollection, PolyCollection

# Light weights: brightness = AMBIENT + KEY (n·key)+ + FILL (n·fill)+, then times the AO factor.
AMBIENT, KEY, FILL = 0.50, 0.55, 0.22
# Voxel AO: the factor for 0, 1, 2, 3 open neighbours at a face corner.
VOXEL_AO = np.array([0.74, 0.83, 0.92, 1.0])
# Ray-cast AO: the darkening when every ray is blocked at zero distance.
RAY_AO = 0.30

INK = "#111111"
HIDDEN = "#9a9a9a"


def unit(v: np.ndarray) -> np.ndarray:
    return v / np.linalg.norm(v, axis=-1, keepdims=True)


class View:
    """An orthographic camera matching matplotlib's `view_init(elev, azim)` (degrees).

    `eye` points from the scene towards the viewer; `right` and `up` span the picture plane.
    """

    def __init__(self, elev: float, azim: float) -> None:
        e, a = np.radians(elev), np.radians(azim)
        self.eye = np.array([np.cos(e) * np.cos(a), np.cos(e) * np.sin(a), np.sin(e)])
        self.right = np.array([-np.sin(a), np.cos(a), 0.0])
        self.up = np.cross(self.eye, self.right)
        # key: 50 degrees up, from the picture's left and a little towards the viewer
        across = unit(-self.right + 0.3 * unit(self.eye * [1.0, 1.0, 0.0]))
        self.key = np.cos(np.radians(50)) * across + np.sin(np.radians(50)) * np.array([0.0, 0.0, 1.0])
        self.fill = unit(0.60 * self.eye + 0.75 * self.right - 0.15 * self.up)

    def project(self, p: np.ndarray) -> np.ndarray:
        """Picture-plane coordinates (…, 2) of points (…, 3)."""
        return np.stack([p @ self.right, p @ self.up], axis=-1)

    def depth(self, p: np.ndarray) -> np.ndarray:
        """Distance towards the viewer: larger is nearer."""
        return p @ self.eye

    def facing(self, normals: np.ndarray) -> np.ndarray:
        """Which (unnormalised) normals face the viewer."""
        return normals @ self.eye > 0

    def brightness(self, normals: np.ndarray) -> np.ndarray:
        n = unit(np.atleast_2d(normals))
        return AMBIENT + KEY * np.clip(n @ self.key, 0, None) + FILL * np.clip(n @ self.fill, 0, None)


def tint(color: str | np.ndarray, k: np.ndarray | float) -> np.ndarray:
    """`color` at brightness `k` (1 is the colour as given; above 1 lifts towards white): (…, 3)."""
    base = np.array(mpl.colors.to_rgb(color)) if isinstance(color, str) else np.asarray(color)
    k = np.asarray(k, dtype=float)[..., None]
    dark = base * np.minimum(k, 1.0)
    return np.clip(dark + (1.0 - base) * np.clip(k - 1.0, 0, None) * 0.6, 0, 1)


def normal(poly: np.ndarray) -> np.ndarray:
    """A planar polygon's unit normal from its winding (Newell's method)."""
    p, q = poly, np.roll(poly, -1, axis=0)
    n = np.array([((p[:, 1] - q[:, 1]) * (p[:, 2] + q[:, 2])).sum(),
                  ((p[:, 2] - q[:, 2]) * (p[:, 0] + q[:, 0])).sum(),
                  ((p[:, 0] - q[:, 0]) * (p[:, 1] + q[:, 1])).sum()])
    return n / np.linalg.norm(n)


# ------------------------------------------------------------------ voxel AO

def voxel_corner_ao(solid: Callable[[np.ndarray], np.ndarray], cell: np.ndarray,
                    quad: np.ndarray, centre: np.ndarray, axis: int, sign: int) -> np.ndarray:
    """AO factors at the four corners of one cube face (`VOXEL_AO`), Minecraft style.

    Args:
        solid  (callable): (M, 3) integer cell indices -> (M,) bool occupancy.
        cell   (ndarray):  The face's cell index (3,).
        quad   (ndarray):  The face's corners (4, 3).
        centre (ndarray):  The cell centre (3,).
        axis, sign:        The face's outward normal, along `axis` with `sign` (±1).
    """
    u, v = (axis + 1) % 3, (axis + 2) % 3
    out = cell.copy()
    out[axis] += sign
    nb = []
    for q in quad:
        du = np.zeros(3, int)
        dv = np.zeros(3, int)
        du[u] = 1 if q[u] > centre[u] else -1
        dv[v] = 1 if q[v] > centre[v] else -1
        nb += [out + du, out + dv, out + du + dv]
    occ = solid(np.array(nb)).reshape(4, 3)
    open_ = 3 - occ.sum(axis=1)
    open_[occ[:, 0] & occ[:, 1]] = 0
    return VOXEL_AO[open_]


def cube_face(centre: np.ndarray, h: float, axis: int, sign: int) -> np.ndarray:
    """One face (4, 3) of the cube of side `h` at `centre`, anticlockwise seen from outside."""
    u, v = (axis + 1) % 3, (axis + 2) % 3
    q = np.zeros((4, 3))
    q[:, axis] = sign * h / 2
    for i, (a, b) in enumerate([(-1, -1), (1, -1), (1, 1), (-1, 1)]):
        q[i, u], q[i, v] = a * h / 2, b * h / 2
    if sign < 0:
        q = q[::-1]
    return q + centre


def add_voxels(scene: Scene, ijk: np.ndarray, origin: np.ndarray, h: float,
               solid: Callable[[np.ndarray], np.ndarray], colors: Sequence[str], edges: Sequence[str],
               lw: float, m: int = 6, leaf: Callable[[np.ndarray], np.ndarray] | None = None) -> int:
    """The visible faces of lattice cells `ijk` (M, 3): AO-shaded, outlined, painted cell by cell.

    Only faces open to an empty cell and facing the viewer are drawn. Cells are painted in order of
    their centres' depth, which is a correct painter's order for equal cubes on one lattice. With
    `leaf` (cell indices -> block ids), a face is outlined only where its block ends, so merged
    blocks (an octree's leaves) show as one box. Returns the number of faces drawn.
    """
    centres = origin + (ijk + 0.5) * h
    depth = scene.view.depth(centres)
    nfaces = 0
    for c in np.argsort(depth):
        cell, ctr = ijk[c], centres[c]
        for axis in range(3):
            for sign in (-1, 1):
                nrm = np.zeros(3)
                nrm[axis] = sign
                if not scene.view.facing(nrm):
                    continue
                nb = cell.copy()
                nb[axis] += sign
                if solid(nb[None])[0]:
                    continue
                quad = cube_face(ctr, h, axis, sign)
                ao = voxel_corner_ao(solid, cell, quad, ctr, axis, sign)
                pieces, vals = quad_grid(quad, ao, m)
                segs = None
                if leaf is not None:
                    segs = []
                    me = leaf(cell[None])[0]
                    for i in range(4):
                        a, b = quad[i], quad[(i + 1) % 4]
                        t = np.rint(((a + b) / 2 - ctr) / (h / 2)).astype(int)
                        t[axis] = 0
                        if leaf((cell + t)[None])[0] != me:
                            segs.append(np.array([a, b]))
                scene.add_shaded(quad, pieces, vals, colors[c], edge=edges[c], lw=lw,
                                 depth=float(depth[c]), segments=segs)
                nfaces += 1
    return nfaces


# ------------------------------------------------------------------ ray-cast AO

def _directions(n: int) -> np.ndarray:
    """`n` cosine-weighted directions about +z (a Fibonacci spiral, so the result is repeatable)."""
    i = np.arange(n) + 0.5
    r = np.sqrt(i / n)
    phi = i * np.pi * (3 - np.sqrt(5))
    return np.stack([r * np.cos(phi), r * np.sin(phi), np.sqrt(1 - r * r)], -1)


def ray_hits(origins: np.ndarray, dirs: np.ndarray, tris: np.ndarray) -> np.ndarray:
    """Nearest hit distance of each ray against triangles (Möller–Trumbore), inf if none."""
    best = np.full(len(origins), np.inf)
    if len(tris) == 0:
        return best
    a, e1, e2 = tris[:, 0], tris[:, 1] - tris[:, 0], tris[:, 2] - tris[:, 0]
    chunk = max(1, 400_000 // max(len(tris), 1))
    for s in range(0, len(origins), chunk):
        o, d = origins[s:s + chunk, None], dirs[s:s + chunk, None]
        p = np.cross(d, e2[None])
        det = (p * e1[None]).sum(-1)
        ok = np.abs(det) > 1e-14
        inv = np.where(ok, 1.0 / np.where(ok, det, 1.0), 0.0)
        tv = o - a[None]
        uu = (tv * p).sum(-1) * inv
        q = np.cross(tv, e1[None])
        vv = (d * q).sum(-1) * inv
        t = (e2[None] * q).sum(-1) * inv
        hit = ok & (uu >= 0) & (vv >= 0) & (uu + vv <= 1) & (t > 0)
        best[s:s + chunk] = np.where(hit, t, np.inf).min(axis=1)
    return best


def ray_occlusion(points: np.ndarray, normals: np.ndarray, tris: np.ndarray, radius: float,
                  n_rays: int = 48) -> np.ndarray:
    """AO factor (1 open … 1 − RAY_AO closed) of surface points from occluders within `radius`.

    Each blocked ray counts by (1 − distance/radius), so near geometry darkens most and nothing
    beyond `radius` darkens at all.
    """
    if len(points) == 0:
        return np.ones(0)
    base = _directions(n_rays)
    nrm = unit(normals)
    helper = np.where(np.abs(nrm[:, 2:3]) < 0.9, [[0.0, 0.0, 1.0]], [[1.0, 0.0, 0.0]])
    t1 = unit(np.cross(helper, nrm))
    t2 = np.cross(nrm, t1)
    dirs = (base[None, :, 0:1] * t1[:, None] + base[None, :, 1:2] * t2[:, None]
            + base[None, :, 2:3] * nrm[:, None]).reshape(-1, 3)
    orig = np.repeat(points + 1e-6 * radius * nrm, n_rays, axis=0)
    t = ray_hits(orig, dirs, tris).reshape(len(points), n_rays)
    occ = np.clip(1.0 - t / radius, 0, 1).mean(axis=1)
    return 1.0 - RAY_AO * occ


# ------------------------------------------------------------------ subdivision

def quad_grid(quad: np.ndarray, corner: np.ndarray, m: int) -> tuple[list[np.ndarray], np.ndarray]:
    """A quad split into sub-quads with `corner` values bilinearly interpolated at their centres.

    Splits only along a direction in which the corner values change, so a face with uniform AO stays
    one polygon.
    """
    c = np.asarray(corner, dtype=float)
    ms = m if abs(c[0] - c[1]) > 1e-9 or abs(c[3] - c[2]) > 1e-9 else 1
    mt = m if abs(c[0] - c[3]) > 1e-9 or abs(c[1] - c[2]) > 1e-9 else 1

    def at(s: np.ndarray, t: np.ndarray, v: np.ndarray) -> np.ndarray:
        s, t = s[..., None], t[..., None]
        return (1 - s) * (1 - t) * v[0] + s * (1 - t) * v[1] + s * t * v[2] + (1 - s) * t * v[3]

    polys, vals = [], []
    for i in range(ms):
        for j in range(mt):
            s = np.array([i, i + 1, i + 1, i]) / ms
            t = np.array([j, j, j + 1, j + 1]) / mt
            polys.append(at(s, t, quad))
            vals.append(at(np.array([(i + 0.5) / ms]), np.array([(j + 0.5) / mt]), c[:, None])[0, 0])
    return polys, np.array(vals)


def split_polygon(poly: np.ndarray, levels: int) -> list[np.ndarray]:
    """A convex polygon as a fan of triangles, each split into 4**levels."""
    tris = [np.array([poly[0], poly[i], poly[i + 1]]) for i in range(1, len(poly) - 1)]
    for _ in range(levels):
        nxt = []
        for a, b, c in tris:
            ab, bc, ca = (a + b) / 2, (b + c) / 2, (c + a) / 2
            nxt += [np.array(t) for t in ((a, ab, ca), (ab, b, bc), (ca, bc, c), (ab, bc, ca))]
        tris = nxt
    return tris


# ------------------------------------------------------------------ hidden lines

def occluded_by_tris(view: View, tris: np.ndarray, eps: float) -> Callable[[np.ndarray], np.ndarray]:
    """A test: is each point hidden from the eye by any of `tris`?"""
    def test(pts: np.ndarray) -> np.ndarray:
        orig = pts + eps * view.eye
        return np.isfinite(ray_hits(orig, np.broadcast_to(view.eye, pts.shape).copy(), tris))
    return test


def occluded_by_cells(view: View, solid: Callable[[np.ndarray], np.ndarray], origin: np.ndarray,
                      h: float, reach: float) -> Callable[[np.ndarray], np.ndarray]:
    """A test: is each point inside, or hidden from the eye by, a solid cell (lattice `origin`, `h`)?"""
    steps = np.arange(0.0, reach, h / 8)

    def test(pts: np.ndarray) -> np.ndarray:
        p = pts[:, None, :] + steps[None, :, None] * view.eye
        ijk = np.floor((p - origin) / h).astype(int).reshape(-1, 3)
        return solid(ijk).reshape(len(pts), len(steps)).any(axis=1)
    return test


def despeckle(flags: np.ndarray, shortest: int) -> np.ndarray:
    """Flip runs shorter than `shortest` samples to their surroundings, so a grazing ray does not
    leave a speck of dash."""
    f = flags.copy()
    i = 0
    while i < len(f):
        j = i
        while j < len(f) and f[j] == f[i]:
            j += 1
        if j - i < shortest and (i > 0 or j < len(f)):
            f[i:j] = f[i - 1] if i > 0 else f[j]
        i = j
    return f


def runs(points: np.ndarray, hidden: np.ndarray) -> tuple[list[np.ndarray], list[np.ndarray]]:
    """Split a sampled polyline into its visible and hidden runs (each run shares its end samples)."""
    vis, hid = [], []
    start = 0
    for i in range(1, len(points) + 1):
        if i == len(points) or hidden[i] != hidden[start]:
            seg = points[start:min(i + 1, len(points))]
            if len(seg) >= 2:
                (hid if hidden[start] else vis).append(seg)
            start = i
    return vis, hid


# ------------------------------------------------------------------ the painter

class Scene:
    """Faces of a 3-D drawing, painted back to front onto a 2-D axes.

    Every `add_*` call adds one painter item at a depth (default: its centroid's depth); items are
    painted farthest first. One item can hold several polygons, painted in the order given.
    """

    def __init__(self, view: View) -> None:
        self.view = view
        self.items: list[tuple[float, int, list[np.ndarray], list, list, list]] = []
        self.lines: list[tuple[list[np.ndarray], dict]] = []
        self.pts: list[np.ndarray] = []

    def _add(self, depth: float, polys: list[np.ndarray], fcs: list, ecs: list, lws: list) -> None:
        self.items.append((depth, len(self.items), polys, fcs, ecs, lws))
        self.pts += polys

    def add_face(self, poly: np.ndarray, color: str, ao: float = 1.0, edge: str | None = None,
                 lw: float = 0.0, depth: float | None = None, alpha: float = 1.0) -> None:
        """One flat-lit face with a single AO factor."""
        k = self.view.brightness(normal(poly))[0] * ao
        fc = (*tint(color, k), alpha)
        d = self.view.depth(poly.mean(axis=0)) if depth is None else depth
        self._add(d, [poly], [fc], [edge if edge else "none"], [lw if edge else 0.0])

    def add_shaded(self, poly: np.ndarray, pieces: Sequence[np.ndarray], ao: np.ndarray, color: str,
                   edge: str | None = None, lw: float = 0.0, depth: float | None = None,
                   segments: Sequence[np.ndarray] | None = None) -> None:
        """A face split into `pieces` with an AO factor each: base, pieces, then the outline (the
        whole polygon, or only `segments` (2, 3) of it when given)."""
        k = self.view.brightness(normal(poly))[0]
        ao = np.asarray(ao, dtype=float)
        cols = tint(color, k * ao)
        base = tint(color, k * float(np.mean(ao)))
        polys = [poly] + (list(pieces) if len(pieces) > 1 else [])
        fcs = [(*base, 1.0)] + ([(*c, 1.0) for c in cols] if len(pieces) > 1 else [])
        if len(pieces) == 1:
            fcs = [(*cols[0], 1.0)]
        ecs = ["none"] * len(polys)
        lws = [0.0] * len(polys)
        if edge:
            outl = [poly] if segments is None else list(segments)
            polys += outl
            fcs += ["none"] * len(outl)
            ecs += [edge] * len(outl)
            lws += [lw] * len(outl)
        d = self.view.depth(poly.mean(axis=0)) if depth is None else depth
        self._add(d, polys, fcs, ecs, lws)

    def add_ground(self, centre: np.ndarray, half: float, tris: np.ndarray, radius: float,
                   n: int = 36, strength: float = 1.0) -> None:
        """A contact shadow on the plane z = centre[2]: white where open, greyer under and near
        the body (`tris`). Painted behind everything; the background is white, so it has no edge."""
        g = np.linspace(-half, half, n + 1)
        cells = [(i, j) for i in range(n) for j in range(n)]
        quads = [np.array([[g[i], g[j], 0], [g[i + 1], g[j], 0], [g[i + 1], g[j + 1], 0],
                           [g[i], g[j + 1], 0]]) + centre for i, j in cells]
        ctr = np.array([q.mean(axis=0) for q in quads])
        ao = ray_occlusion(ctr, np.tile([0.0, 0.0, 1.0], (len(ctr), 1)), tris, radius, 64)
        dark = 1.0 - strength * (1.0 - ao) / RAY_AO * 0.22
        fcs = [(*np.full(3, k), 1.0) for k in dark]
        polys = list(quads)
        # Faint: as many pieces as the shadow needs, no outline; the shadow fades to white.
        self.items.append((-np.inf, -1, polys, fcs, [c for c in fcs], [0.3] * len(polys)))
        self.pts += polys

    @staticmethod
    def classify(edges: Sequence[np.ndarray], hidden: Callable[[np.ndarray], np.ndarray],
                 step: float) -> tuple[list[np.ndarray], list[np.ndarray]]:
        """Polylines (each (k, 3), e.g. an edge's two ends) split into visible and hidden runs."""
        vis, hid = [], []
        for line in edges:
            line = np.asarray(line)
            pts = [line[0:1]]
            for a, b in zip(line[:-1], line[1:]):
                k = max(int(np.ceil(np.linalg.norm(b - a) / step)), 2)
                pts.append(a + np.linspace(0, 1, k + 1)[1:, None] * (b - a))
            p = np.vstack(pts)
            v, h = runs(p, despeckle(hidden(p), 3))
            vis += v
            hid += h
        return vis, hid

    def wireframe(self, edges: Sequence[np.ndarray], hidden: Callable[[np.ndarray], np.ndarray],
                  step: float, show_hidden: bool = True) -> None:
        """The true body's edges (polylines) as a hidden-line drawing on top of everything painted:
        dashed black in front, faint grey behind or inside."""
        vis, hid = self.classify(edges, hidden, step)
        if show_hidden and hid:
            self.lines.append((hid, dict(color=HIDDEN, lw=0.45, linestyle=(0, (2.0, 1.6)), zorder=4)))
        if vis:
            self.lines.append((vis, dict(color=INK, lw=0.8, linestyle=(0, (3.2, 2.0)), zorder=5)))

    def edges(self, edges: Sequence[np.ndarray], hidden: Callable[[np.ndarray], np.ndarray],
              step: float, zorder: float = 3, **style) -> None:
        """Only the visible runs of `edges`, on top of the faces (an exact hidden-line outline)."""
        vis, _ = self.classify(edges, hidden, step)
        if vis:
            self.lines.append((vis, dict(zorder=zorder, capstyle="round", **style)))

    def add_lines(self, segs: list[np.ndarray], **kw) -> None:
        """Lines drawn on top of all faces (no hiding)."""
        self.lines.append((segs, kw))

    def draw(self, ax: plt.Axes, limits: np.ndarray | None = None) -> None:
        """Paint everything on `ax` (made bare, equal aspect). `limits`: 3-D points that must fit
        (e.g. a bounding box's corners), so panels drawn at the same size share one scale."""
        polys, fcs, ecs, lws = [], [], [], []
        for _, _, p, f, e, w in sorted(self.items, key=lambda it: (it[0], it[1])):
            polys += [self.view.project(q) for q in p]
            fcs += f
            ecs += e
            lws += w
        ax.add_collection(PolyCollection(polys, facecolors=fcs, edgecolors=ecs, linewidths=lws,
                                         joinstyle="round", capstyle="round"))
        for segs, kw in self.lines:
            ax.add_collection(LineCollection([self.view.project(s) for s in segs], **kw))
        ax.set_aspect("equal")
        ax.set_axis_off()
        box = self.view.project(np.asarray(limits) if limits is not None else np.vstack(self.pts))
        lo, hi = box.min(axis=0), box.max(axis=0)
        ax.set_xlim(lo[0], hi[0])
        ax.set_ylim(lo[1], hi[1])
