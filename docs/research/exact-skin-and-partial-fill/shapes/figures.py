"""Pictures: each shape analytic or repaired mesh, beside a 2 x 2 grid of voxel panels (rows: coarse
and fine lattice; columns: cube voxels and clipped polyhedra), whole and cut.

    python figures.py                 # the first four shapes
    python figures.py sphere bunny    # any (also spot, dragon, armadillo, buddha, bracket)
    python figures.py torus@64        # at another number of cells across than ACROSS (the paper's)

Writes `<shape>.png` and `<shape>-cutaway.png` here (`<shape>-<n>...` at another count n; the
paper's Fig. 8 uses torus@64 and bunny@64, so the stepping still shows at print size). Same camera and light in every panel. The
lattices are the sweep's (sweep.py): pitch = largest extent / cells across, origin h·GENERIC, so the
fine panels show the very cells and pieces the sweep measured at that pitch.

Colours (the paper's): the shape green; cube voxels blue; clipped polyhedra blue (interior cubes)
and orange (boundary pieces). Voxel faces are flat-shaded with the key and fill light and a per-corner
ambient occlusion from the solid neighbours of each face corner (the voxel-game rule), on top of the
screen-space occlusion of `shapes_lib.Canvas`, so a fine staircase reads as a stepped surface rather
than a pile of boxes. Cell edges are drawn only where a cell is big enough on screen for them to read
(the coarse panel). Over the voxels the true surface is a sparse dotted 3-D cage (rings for the
analytic shapes; for meshes, about CAGE_LEVELS contours in height and the silhouette), each dot
depth-tested against the voxels and the true surface, so it shows where the true surface is in front
of the voxels or within a hair of them (two supersampled pixels; half a cell where cells are too small
to outline, so the cage does not break into flecks). The cutaway removes every cell whose centre lies beyond the
lattice wall nearest the body's centre plane y = const, and cuts the shape at the same wall; the true
surface's section on that wall is dotted over the voxels.
"""

from __future__ import annotations

import sys
import textwrap
import time

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402

sys.path.insert(0, str(__import__("pathlib").Path(__file__).resolve().parent))
import shapes_lib as L  # noqa: E402

# Cells across the largest extent: the fine panels (the sweep's finest lattice is 128 across) and the
# coarse one, which is there so the reader still sees what a voxel is.
ACROSS = 128
COARSE = 16
# The paper's palette (Okabe-Ito): the true body green, whole cells sky blue with blue edges,
# clipped pieces orange with vermilion edges; ink for the true surface's dotted cage.
GREEN = np.array([0x7F, 0xCF, 0xB9]) / 255   # Okabe-Ito green lightened halfway to white
SKY = np.array([0x56, 0xB4, 0xE9]) / 255
BLUE = np.array([0x00, 0x72, 0xB2]) / 255
ORANGE = np.array([0xE6, 0x9F, 0x00]) / 255
VERMILION = np.array([0xD5, 0x5E, 0x00]) / 255
INK = np.array([0x11, 0x11, 0x11]) / 255
CAGE_LEVELS = 10  # the mesh cage: about this many contours over the largest extent
RING_DEG = 15     # the analytic cages' ring spacing, degrees
SIZE = 800        # pixels per panel, rendered (supersampled 2x); the PNG is drawn at DPI
DPI = 160
OUTLINE_PX = 12   # cell edges are drawn only if a cell spans at least this many output pixels
VOXEL_AO = 0.12   # light lost at a voxel face corner per solid neighbour beside it (at most 3)
AZIMUTH, ELEVATION = 58.0, 24.0
MODEL_W = 1.3     # the model panel's width, in voxel panels' widths


def _canvas(mesh: L.Mesh) -> L.Canvas:
    v = mesh.verts
    c = 0.5 * (v.min(axis=0) + v.max(axis=0))
    probe = L.Canvas(10, AZIMUTH, ELEVATION, c, 1.0)
    corners = np.array([[x, y, z] for x in (v.min(0)[0], v.max(0)[0]) for y in (v.min(0)[1], v.max(0)[1])
                        for z in (v.min(0)[2], v.max(0)[2])])
    q = corners - c
    half = 1.12 * max(np.abs(q @ probe.right).max(), np.abs(q @ probe.up).max())
    return L.Canvas(SIZE, AZIMUTH, ELEVATION, c, half)


def _cell_px(cv: L.Canvas, h: float) -> float:
    """A cell's width on screen, output pixels."""
    return h * cv.scale / 2


def _exposed(ijk: np.ndarray, solid: np.ndarray) -> np.ndarray:
    """(N, 6) faces of cells `ijk` whose neighbour (−x, +x, −y, +y, −z, +z) is not solid."""
    out = np.zeros((len(ijk), 6), dtype=bool)
    for f in range(6):
        d = np.zeros(3, dtype=np.int64)
        d[f // 2] = 1 if f % 2 else -1
        out[:, f] = ~_solid_at(solid, ijk + d)
    return out


def _solid_at(solid: np.ndarray, ijk: np.ndarray) -> np.ndarray:
    """solid[ijk] for (..., 3) indices, False outside the grid."""
    shape = np.array(solid.shape)
    ok = np.all((ijk >= 0) & (ijk < shape), axis=-1)
    out = np.zeros(ijk.shape[:-1], dtype=bool)
    q = ijk[ok]
    out[ok] = solid[q[..., 0], q[..., 1], q[..., 2]]
    return out


def _voxel_faces(cv: L.Canvas, vox: L.Voxels, ijk: np.ndarray,
                 solid: np.ndarray) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """The faces of cells `ijk` that are exposed (neighbour not solid) and turned to the eye, as
    triangles (2K, 3, 3), their corners' light factors (2K, 3) and the faces' edges (4K, 2, 3).

    A face corner's factor is 1 − VOXEL_AO × (solid cells among the three that touch that corner in
    the layer in front of the face; 3 when both edge neighbours are solid), as voxel games do it; each
    quad is split along the diagonal that keeps the shading symmetric."""
    normals = np.repeat(np.eye(3), 2, axis=0) * np.tile([-1.0, 1.0], 3)[:, None]
    facing = normals @ cv.fwd < 0
    keep = _exposed(ijk, solid) & facing[None, :]
    cell, f = np.nonzero(keep)
    quads = L.sb.cube_quads(vox.centres(ijk), vox.h).reshape(len(ijk), 6, 4, 3)[cell, f]
    d = np.sign(quads - vox.centres(ijk[cell])[:, None, :]).astype(np.int64)  # (K, 4, 3), ±1
    a = f // 2
    eye = np.eye(3, dtype=np.int64)
    not_b, not_c = 1 - eye[(a + 2) % 3], 1 - eye[(a + 1) % 3]  # drop c (keep a, b); drop b
    base = ijk[cell][:, None, :]
    side1 = _solid_at(solid, base + d * not_b[:, None, :])
    side2 = _solid_at(solid, base + d * not_c[:, None, :])
    corner = _solid_at(solid, base + d)
    occ = np.where(side1 & side2, 3, side1.astype(int) + side2 + corner)
    lum = 1.0 - VOXEL_AO * occ  # (K, 4)
    flip = lum[:, 0] + lum[:, 2] < lum[:, 1] + lum[:, 3]
    split = np.where(flip[:, None, None], [[1, 2, 3], [1, 3, 0]], [[0, 1, 2], [0, 2, 3]])  # (K, 2, 3)
    rows = np.arange(len(quads))[:, None, None]
    tris = quads[rows, split].reshape(-1, 3, 3)
    ao = lum[rows, split].reshape(-1, 3)
    segs = np.concatenate([quads[:, [i, (i + 1) % 4]] for i in range(4)])
    return tris, ao, segs


def _exact(cv: L.Canvas, mesh: L.Mesh, y_cut: float | None) -> None:
    if y_cut is None:
        cv.triangles(mesh.tris, GREEN)
        return
    walls = [(1, -np.inf, y_cut)]
    polys, segs = [], []
    for t in mesh.tris:
        p = L._clip_to_box(t.tolist(), walls)
        if len(p) < 3:
            continue
        arr = np.array(p)
        polys.append(arr)
        q = np.roll(arr, -1, axis=0)
        for e in np.nonzero((arr[:, 1] == y_cut) & (q[:, 1] == y_cut))[0]:
            segs.append((q[e], arr[e]))
    cv.triangles(L.sb.fan(polys, 1.0), GREEN)
    centre = np.mean([s[0] for s in segs], axis=0)
    cap = np.array([[centre, a, b] for a, b in segs])
    cv.signed_region(cap, np.array([0.0, 1.0, 0.0]), GREEN)


def _cubes(cv: L.Canvas, vox: L.Voxels, removed: np.ndarray) -> None:
    """Cube voxels: a cell solid if it is interior or its centre is inside."""
    solid = np.zeros(tuple(int(x) for x in vox.n), dtype=bool)
    for ijk in (vox.interior, vox.ijk[vox.ctr_in > 0.5]):
        solid[ijk[:, 0], ijk[:, 1], ijk[:, 2]] = True
    solid &= ~removed
    tris, ao, segs = _voxel_faces(cv, vox, np.argwhere(solid), solid)
    cv.triangles(tris, SKY, ao=ao)
    if _cell_px(cv, vox.h) >= OUTLINE_PX:
        cv.lines(segs, BLUE, 0.5)


def _clipped(cv: L.Canvas, vox: L.Voxels, removed: np.ndarray) -> None:
    """Interior cubes (blue) and every crossed cell's exact piece (orange)."""
    solid = np.zeros(tuple(int(x) for x in vox.n), dtype=bool)
    for ijk in (vox.interior, vox.ijk):
        solid[ijk[:, 0], ijk[:, 1], ijk[:, 2]] = True
    solid &= ~removed
    inner = vox.interior[_solid_at(solid, vox.interior)]
    tris, ao, segs = _voxel_faces(cv, vox, inner, solid)
    cv.triangles(tris, SKY, ao=ao)
    keep = np.nonzero(_solid_at(solid, vox.ijk))[0]
    cv.triangles(L.sb.fan([p for b in keep for p in vox.polys[b]], vox.h), ORANGE)
    # Caps on the exposed walls (in practice the cut), one signed region per lattice wall.
    exposed = _exposed(vox.ijk, solid)
    walls: dict[tuple[int, int], list[np.ndarray]] = {}
    lines = []
    for b in keep:
        c_lo = vox.base + vox.ijk[b] * vox.h
        for f in range(6):
            segs_f = [(p0, p1) for f_i, p0, p1 in vox.caps[b] if f_i == f]
            if not segs_f:
                continue
            lines.append(np.array(segs_f))
            if not exposed[b, f]:
                continue
            a = f // 2
            ctr = c_lo + 0.5 * vox.h
            ctr[a] = c_lo[a] + (vox.h if f % 2 else 0.0)
            walls.setdefault((f, int(vox.ijk[b, a]) + f % 2), []).append(
                np.array([[ctr, p0, p1] for p0, p1 in segs_f]))
    for (f, _), fans in walls.items():
        nrm = np.zeros(3)
        nrm[f // 2] = 1.0 if f % 2 else -1.0
        cv.signed_region(np.concatenate(fans), nrm, ORANGE)
    if _cell_px(cv, vox.h) >= OUTLINE_PX:
        cv.lines(segs, BLUE, 0.5)
        edges = [np.stack([q, np.roll(q, -1, axis=0)], axis=1) for b in keep for q in map(np.asarray, vox.polys[b])]
        cv.lines(np.concatenate(edges), VERMILION, 0.3)
        if lines:
            cv.lines(np.concatenate(lines), VERMILION, 0.3)


def _section(mesh: L.Mesh, y: float, axis: int = 1) -> list[np.ndarray]:
    """Where the plane x[axis] = y cuts the mesh, as chained polylines (each (k, 3))."""
    t = mesh.tris
    s = t[:, :, axis] - y
    segs = []
    for tri, d in zip(t, s):
        pts = []
        for i in range(3):
            j = (i + 1) % 3
            if (d[i] < 0) != (d[j] < 0):
                pts.append(tri[i] + d[i] / (d[i] - d[j]) * (tri[j] - tri[i]))
        if len(pts) == 2:
            segs.append(pts)
    return _chain(segs)


def _chain(segs: list) -> list[np.ndarray]:
    """Segments (pairs of points) joined end to end into polylines."""
    key = lambda p: tuple(np.round(p, 9))  # noqa: E731
    ends: dict[tuple, list[int]] = {}
    for k, (a, b) in enumerate(segs):
        ends.setdefault(key(a), []).append(k)
        ends.setdefault(key(b), []).append(k)
    used = np.zeros(len(segs), dtype=bool)
    lines = []
    for k0 in range(len(segs)):
        if used[k0]:
            continue
        used[k0] = True
        chain = [segs[k0][0], segs[k0][1]]
        for grow in (1, 0):  # forwards from the end, then backwards from the start
            while True:
                tip = chain[-1] if grow else chain[0]
                nxt = [k for k in ends.get(key(tip), []) if not used[k]]
                if not nxt:
                    break
                k = nxt[0]
                used[k] = True
                a, b = segs[k]
                far = b if key(a) == key(tip) else a
                if grow:
                    chain.append(far)
                else:
                    chain.insert(0, far)
        lines.append(np.array(chain))
    return lines


def _outline(cv: L.Canvas, section: list[np.ndarray]) -> None:
    """The true surface's section on the cut as fine dots over the voxels (as the paper's diagrams),
    depth-tested so that anything in front covers them."""
    cv.dots(section, INK, pitch=SIZE / 150, radius=(SIZE / 620, SIZE / 480))


def _silhouette(mesh: L.Mesh, fwd: np.ndarray) -> list[np.ndarray]:
    """The mesh edges between a face turned to the eye and one turned away, chained."""
    v, f = mesh.verts, mesh.faces
    n = np.cross(v[f[:, 1]] - v[f[:, 0]], v[f[:, 2]] - v[f[:, 0]])
    toward = n @ fwd < 0
    e = np.concatenate([f[:, [0, 1]], f[:, [1, 2]], f[:, [2, 0]]])
    owner = np.tile(np.arange(len(f)), 3)
    key = np.sort(e, axis=1)
    order = np.lexsort((key[:, 1], key[:, 0]))
    k, o = key[order], owner[order]
    same = np.all(k[1:] == k[:-1], axis=1)
    i = np.nonzero(same)[0]
    sil = i[toward[o[i]] != toward[o[i + 1]]]
    return _chain([[v[k[j, 0]], v[k[j, 1]]] for j in sil])


def _rings(name: str) -> list[np.ndarray]:
    """The analytic shapes' cages, every RING_DEG degrees: latitude and longitude rings (sphere);
    rings around the tube and, every other step, around the axis (torus)."""
    t = np.radians(np.linspace(0.0, 360.0, 721))
    out = []
    if name == "sphere":
        for lat in np.radians(np.arange(-90 + RING_DEG, 90, RING_DEG)):
            out.append(np.column_stack([np.cos(lat) * np.cos(t), np.cos(lat) * np.sin(t),
                                        np.full_like(t, np.sin(lat))]))
        half = t[(t >= np.radians(RING_DEG)) & (t <= np.pi - np.radians(RING_DEG) + 1e-9)]  # not into the poles
        for lon in np.radians(np.arange(0, 360, RING_DEG)):
            out.append(np.column_stack([np.sin(half) * np.cos(lon), np.sin(half) * np.sin(lon),
                                        np.cos(half)]))
    else:  # torus, R = 1 m, r = 0.4 m
        big, small = 1.0, 0.4
        for u in np.radians(np.arange(0, 360, RING_DEG)):
            rho = big + small * np.cos(t)
            out.append(np.column_stack([rho * np.cos(u), rho * np.sin(u), small * np.sin(t)]))
        for w in np.radians(np.arange(0, 360, 2 * RING_DEG)):
            rho = big + small * np.cos(w)
            out.append(np.column_stack([rho * np.cos(t), rho * np.sin(t),
                                        np.full_like(t, small * np.sin(w))]))
    return out


def _cage(name: str, drawn: L.Mesh, vox: L.Voxels, fwd: np.ndarray,
          y_cut: float | None) -> list[np.ndarray]:
    """A sparse 3-D wireframe of the true surface: rings for the analytic shapes; for meshes,
    contour slices midway between lattice planes, about CAGE_LEVELS over the largest extent, and the
    silhouette. With a cut, only the part on the kept side
    y <= y_cut."""
    if name in ("sphere", "torus"):
        lines = _rings(name)
    else:
        v = drawn.verts
        z0, z1 = v[:, 2].min(), v[:, 2].max()
        size = float((v.max(axis=0) - v.min(axis=0)).max())
        step = max(int(round(size / (CAGE_LEVELS * vox.h))), 1)
        count = int((z1 - vox.base[2]) / (step * vox.h)) + 1
        levels = vox.base[2] + (np.arange(count) * step + step // 2 + 0.5) * vox.h
        lines = [ln for z in levels if z0 < z < z1 for ln in _section(drawn, float(z), axis=2)]
        lines += _silhouette(drawn, fwd)
    if y_cut is None:
        return lines
    out = []
    for ln in lines:
        keep = ln[:, 1] <= y_cut + 1e-12
        i = 0
        while i < len(ln):
            if not keep[i]:
                i += 1
                continue
            j = i
            while j < len(ln) and keep[j]:
                j += 1
            if j - i >= 2:
                out.append(ln[i:j])
            i = j
    return out


def _analytic_surface(name: str) -> L.Mesh:
    """A fine parametric tessellation used only to draw the analytic surface (256 × 128 quads)."""
    if name == "torus":
        return L.torus_mesh(1.0, 0.4, 256, 128)
    nu, nv = 256, 128
    u = 2 * np.pi * np.arange(nu) / nu
    w = np.pi * (np.arange(nv + 1) / nv)
    uu, ww = np.meshgrid(u, w, indexing="ij")
    v = np.stack([np.sin(ww) * np.cos(uu), np.sin(ww) * np.sin(uu), np.cos(ww)], -1)
    i, j = np.meshgrid(np.arange(nu), np.arange(nv), indexing="ij")
    a = i * (nv + 1) + j
    b = ((i + 1) % nu) * (nv + 1) + j
    c = ((i + 1) % nu) * (nv + 1) + j + 1
    d = i * (nv + 1) + j + 1
    faces = np.concatenate([np.stack([a, b, c], -1).reshape(-1, 3), np.stack([a, c, d], -1).reshape(-1, 3)])
    verts = v.reshape(-1, 3)
    t = verts[faces]
    area = np.linalg.norm(np.cross(t[:, 1] - t[:, 0], t[:, 2] - t[:, 0]), axis=1)
    faces = faces[area > 1e-14]
    n = np.cross(verts[faces[:, 1]] - verts[faces[:, 0]], verts[faces[:, 2]] - verts[faces[:, 0]])
    flip = np.einsum("nk,nk->n", n, verts[faces].mean(axis=1)) < 0
    faces[flip] = faces[flip][:, ::-1]
    return L.Mesh("sphere", verts, faces.astype(np.int64), "")


def _cut_of(mesh: L.Mesh, vox: L.Voxels) -> tuple[int, float]:
    """The lattice wall nearest the body's centre plane y = const: its index and y (m)."""
    centre_y = 0.5 * (mesh.verts[:, 1].min() + mesh.verts[:, 1].max())
    k_cut = int(round((centre_y - vox.base[1]) / vox.h))
    return k_cut, float(vox.base[1] + k_cut * vox.h)


def figure(name: str, across: int = ACROSS) -> None:
    mesh = L.mesh_by_name(name)
    analytic = name in ("sphere", "torus")
    drawn = _analytic_surface(name) if analytic else mesh
    size = float((mesh.verts.max(axis=0) - mesh.verts.min(axis=0)).max())  # as sweep.py
    t0 = time.perf_counter()
    fine = L.voxelise(mesh, size / across, keep_drawing=True)
    coarse = L.voxelise(mesh, size / COARSE, keep_drawing=True)
    t_vox = time.perf_counter() - t0
    n_on = {id(v): int((v.ctr_in > 0.5).sum()) for v in (fine, coarse)}
    for cut in (False, True):
        t1 = time.perf_counter()
        panels = []
        true_z: dict[int, np.ndarray] = {}
        for kind, vox in (("exact", fine), ("cubes", coarse), ("clipped", coarse), ("cubes", fine),
                          ("clipped", fine)):
            k_cut, y_cut = _cut_of(mesh, vox)
            removed = np.zeros(tuple(int(x) for x in vox.n), dtype=bool)
            if cut:
                removed[:, k_cut:, :] = True
            cv = _canvas(mesh)
            if kind == "exact":
                _exact(cv, drawn, y_cut if cut else None)
                true_z[id(fine)] = cv.z.copy()
                panels.append(cv.image())
                continue
            if id(vox) not in true_z:  # the true surface cut at this lattice's own wall
                ref = _canvas(mesh)
                _exact(ref, drawn, y_cut if cut else None)
                true_z[id(vox)] = ref.z
            (_cubes if kind == "cubes" else _clipped)(cv, vox, removed)
            # the true surface as a dotted cage, depth-tested against the voxels and against the
            # true surface's own render (so its far side stays hidden)
            cage = _cage(name, drawn, vox, cv.fwd, y_cut if cut else None)
            # A dot shows if it is within a hair of the voxels' depth: 2 supersampled pixels, or for
            # cubes too small to see weaving, half a cell (else the cage breaks into flecks).
            hair = 2.0 if _cell_px(cv, vox.h) >= OUTLINE_PX else max(2.0, 0.5 * vox.h / cv.depth_bias)
            cv.dots(cage, INK, pitch=SIZE / 150, radius=(SIZE / 1000, SIZE / 680), front=true_z[id(vox)],
                    hair=hair)
            if cut:
                _outline(cv, _section(drawn, y_cut))
            panels.append(cv.image())
        # Crop empty rows (the same rows in every panel, so the panels stay aligned).
        busy = np.any([np.abs(p - p[0, 0]).sum(axis=2).max(axis=1) > 0.02 for p in panels], axis=0)
        rows = np.nonzero(busy)[0]
        top, bottom = max(rows[0] - 12, 0), min(rows[-1] + 12, SIZE)
        busy_c = np.any([np.abs(p - p[0, 0]).sum(axis=2).max(axis=0) > 0.02 for p in panels], axis=0)
        cols = np.nonzero(busy_c)[0]
        left, right = max(cols[0] - 12, 0), min(cols[-1] + 12, SIZE)
        panels = [p[top:bottom, left:right] for p in panels]
        aspect = (bottom - top) / (right - left)
        # The model on the left, spanning two rows; beside it a 2 x 2 grid of voxel panels, rows
        # coarse / fine, columns cubes / clipped, so each pitch's two representations sit side by side.
        width, gap, margin, title_h, caption_h = 16.0, 0.05, 0.06, 0.5, 1.45
        pw = (width - 2 * margin - 2 * gap) / (2 + MODEL_W)
        ph = pw * aspect
        height = 2 * (title_h + ph) + caption_h + 0.1
        fig = plt.figure(figsize=(width, height), facecolor="#fcfcfb")

        def place(col: float, row: int, w: float, rows: int, img: np.ndarray, title: str) -> None:
            x = margin + (0.0 if col == 0 else MODEL_W * pw + gap + (col - 2) * (pw + gap))
            top_in = height - 0.05 - row * (title_h + ph)
            ax = fig.add_axes((x / width, (top_in - title_h - rows * ph - (rows - 1) * title_h) / height,
                               w / width, (rows * ph + (rows - 1) * title_h) / height))
            ax.imshow(img, interpolation="lanczos")
            ax.set_axis_off()
            fig.text((x + w / 2) / width, (top_in - 0.03) / height, title, fontsize=11,
                     color="#0b0b0b", ha="center", va="top")

        def counts(vox: L.Voxels, clipped: bool) -> str:
            return (f"{len(vox.interior)} cubes + {len(vox.frac)} pieces" if clipped else
                    f"{len(vox.interior)} interior + {n_on[id(vox)]} boundary")

        place(0, 0, MODEL_W * pw, 2, panels[0],
              "analytic" if analytic else "decimated, repaired mesh" if name in L.DECIMATE
              else "repaired mesh (original triangles)")
        for k, (vox, clipped) in enumerate([(coarse, False), (coarse, True), (fine, False), (fine, True)]):
            kind = "clipped polyhedra" if clipped else "cube voxels"
            n = COARSE if vox is coarse else across
            place(2 + k % 2, k // 2, pw, 1, panels[1 + k],
                  f"{kind}, {n} across (h = {vox.h:.3g} m)\n{counts(vox, clipped)}")
        what = {"sphere": "Sphere, radius 1 m", "torus": "Torus, R = 1 m, r = 0.4 m",
                "teapot": "Utah teapot, 1 m long", "suzanne": "Suzanne, 1 m wide",
                **{n: f"{f.title}, 1 m largest extent" for n, f in L.FAMOUS.items()}}[name]
        cut_txt = (f", cut away at the lattice wall nearest the centre plane (y = {_cut_of(mesh, fine)[1]:.3f} m "
                   f"at {across} across)") if cut else ""
        if analytic:
            left = ("Left: the analytic surface (drawn from a 256 × 128 parametric tessellation). The voxels "
                    f"and pieces are made from a mesh: {mesh.source}; the pieces are exact to that mesh.")
        else:
            chk = L.study_repair(name).check
            left = (f"Left: the repaired mesh. {mesh.source}. Check (pass if within 1e-6 m): closed, "
                    f"{chk['components']} piece; vertices to original triangles or caps, max "
                    f"{chk['max_vertex_to_source_m']:.1e} m; original vertices outside other parts to the "
                    f"result, max {chk['max_source_to_mesh_m']:.1e} m. The pieces are exact to this mesh.")
        cage_txt = (f"rings every {RING_DEG} degrees" if analytic else
                    "contours about a tenth of the size apart in height, and the silhouette")
        caption = (f"{what}{cut_txt}. Cells across = largest extent ({size:.3g} m) / pitch; the sweep's "
                   f"lattices, finest {ACROSS} across. {left} Right, rows are the pitches ({COARSE} across, to show a "
                   f"cell, and {across} across, as measured), columns the representations. Cube voxels: a cell "
                   f"is solid if its centre is inside (blue). Clipped polyhedra: the same interior cubes "
                   f"(blue), and every crossed cell cut to its inside piece (orange, thin vermilion piece "
                   f"outlines). At the same coarse pitch the pieces have exactly the mesh's shape and mass, "
                   f"while the cubes are off by first order in the pitch. Voxel faces flat-shaded with per-corner occlusion; cell edges drawn only where "
                   f"a cell is at least {OUTLINE_PX} px. Dotted: the true surface ({cage_txt}"
                   f"{'; and where the cut meets it' if cut else ''}), shown where it lies in front of the "
                   f"voxels or within a hair of them (half a cell at {across} across).")
        fig.text(0.01, 0.08 / height, "\n".join(textwrap.wrap(caption, 210)), fontsize=9, color="#52514e",
                 va="bottom", ha="left")
        tag = "" if across == ACROSS else f"-{across}"
        out = L.HERE / f"{name}{tag}{'-cutaway' if cut else ''}.png"
        fig.savefig(out, dpi=DPI, facecolor=fig.get_facecolor())
        plt.close(fig)
        print(f"wrote {out}: voxelise {t_vox:.1f} s (both lattices), draw {time.perf_counter() - t1:.1f} s",
              flush=True)


if __name__ == "__main__":
    for arg in sys.argv[1:] or ["sphere", "torus", "teapot", "suzanne"]:
        shape, _, n = arg.partition("@")
        figure(shape, int(n) if n else ACROSS)
