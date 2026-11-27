"""Pictures: each shape analytic or repaired mesh | as cube voxels | as clipped polyhedra, whole and cut.

    python figures.py                 # the first four shapes
    python figures.py sphere bunny    # any (also spot, dragon, armadillo, buddha, bracket)
    python figures.py torus@0.1       # at another pitch than the study's (m)

Writes `<shape>.png` and `<shape>-cutaway.png` here. Same lattice, camera and light in every panel.
Colours (the paper's): the shape green; cube voxels blue; clipped polyhedra blue (interior cubes)
and orange (boundary pieces). Cell edges are drawn faintly. Over the voxels the true surface is a
sparse dotted 3-D cage (rings for the analytic shapes; for meshes, contours every few cells in height
and the silhouette), each dot depth-tested against the voxels and the true surface, so it shows where
the true surface is in front of the voxels or within a hair of them. Lit as the paper's diagrams
are: key, fill and ambient light with ambient occlusion (see `shapes_lib.Canvas`). The cutaway removes every cell whose centre lies beyond the lattice wall
nearest the body's centre plane y = const, and cuts the shape at the same wall; the true surface's
section on that wall is dotted over the voxels.
"""

from __future__ import annotations

import sys
import textwrap

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402

sys.path.insert(0, str(__import__("pathlib").Path(__file__).resolve().parent))
import shapes_lib as L  # noqa: E402

# The teapot and Suzanne are ~1 m across: at 0.2 m they have no interior cell, so they are drawn at 0.1 m.
# The famous meshes (1 m largest extent) likewise, at 0.0625 m so the thin parts get some cells. The
# torus and the bunny, which the paper shows, are at 32 cells across their largest extent.
PITCH = {"sphere": 0.25, "torus": 0.0875, "teapot": 0.1, "suzanne": 0.1, **{n: 0.0625 for n in L.FAMOUS},
         "bunny": 0.03125}
# The paper's palette (Okabe-Ito): the true body green, whole cells sky blue with blue edges,
# clipped pieces orange with vermilion edges; ink for the true surface's dotted cage.
GREEN = np.array([0x7F, 0xCF, 0xB9]) / 255   # Okabe-Ito green lightened halfway to white
SKY = np.array([0x56, 0xB4, 0xE9]) / 255
BLUE = np.array([0x00, 0x72, 0xB2]) / 255
ORANGE = np.array([0xE6, 0x9F, 0x00]) / 255
VERMILION = np.array([0xD5, 0x5E, 0x00]) / 255
INK = np.array([0x11, 0x11, 0x11]) / 255
CAGE_CELLS = 3  # the mesh cage's contour spacing, cells
RING_DEG = 15   # the analytic cages' ring spacing, degrees
SIZE = 800  # pixels per panel, rendered (supersampled 2x); the PNG is drawn at DPI
DPI = {"torus": 160, "bunny": 160}  # the paper's two at print resolution; the rest at 110
AZIMUTH, ELEVATION = 58.0, 24.0


def _canvas(mesh: L.Mesh) -> L.Canvas:
    v = mesh.verts
    c = 0.5 * (v.min(axis=0) + v.max(axis=0))
    probe = L.Canvas(10, AZIMUTH, ELEVATION, c, 1.0)
    corners = np.array([[x, y, z] for x in (v.min(0)[0], v.max(0)[0]) for y in (v.min(0)[1], v.max(0)[1])
                        for z in (v.min(0)[2], v.max(0)[2])])
    q = corners - c
    half = 1.12 * max(np.abs(q @ probe.right).max(), np.abs(q @ probe.up).max())
    return L.Canvas(SIZE, AZIMUTH, ELEVATION, c, half)


def _cube_faces(centres: np.ndarray, h: float, keep: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """Triangles and edge segments of the cube faces flagged in `keep` (N, 6)."""
    quads = L.sb.cube_quads(centres, h).reshape(len(centres), 6, 4, 3)[keep]
    tris = np.concatenate([quads[:, [0, 1, 2]], quads[:, [0, 2, 3]]])
    segs = np.concatenate([quads[:, [i, (i + 1) % 4]] for i in range(4)])
    return tris, segs


def _exposed(ijk: np.ndarray, solid: np.ndarray) -> np.ndarray:
    """(N, 6) faces of cells `ijk` whose neighbour (−x, +x, −y, +y, −z, +z) is not solid."""
    out = np.zeros((len(ijk), 6), dtype=bool)
    shape = np.array(solid.shape)
    for f in range(6):
        d = np.zeros(3, dtype=np.int64)
        d[f // 2] = 1 if f % 2 else -1
        nb = ijk + d
        ok = np.all((nb >= 0) & (nb < shape), axis=1)
        nbs = np.zeros(len(ijk), dtype=bool)
        nbs[ok] = solid[nb[ok, 0], nb[ok, 1], nb[ok, 2]]
        out[:, f] = ~nbs
    return out


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
    solid = np.zeros(tuple(int(x) for x in vox.n), dtype=bool)
    b_on = vox.ijk[vox.ctr_in > 0.5]
    for ijk in (vox.interior, b_on):
        solid[ijk[:, 0], ijk[:, 1], ijk[:, 2]] = True
    solid &= ~removed
    segs_all = []
    for ijk in (vox.interior, b_on):
        ijk = ijk[solid[ijk[:, 0], ijk[:, 1], ijk[:, 2]]]
        tris, segs = _cube_faces(vox.centres(ijk), vox.h, _exposed(ijk, solid))
        cv.triangles(tris, SKY)
        segs_all.append(segs)
    cv.lines(np.concatenate(segs_all), BLUE, 0.5)


def _clipped(cv: L.Canvas, vox: L.Voxels, removed: np.ndarray) -> None:
    solid = np.zeros(tuple(int(x) for x in vox.n), dtype=bool)
    for ijk in (vox.interior, vox.ijk):
        solid[ijk[:, 0], ijk[:, 1], ijk[:, 2]] = True
    solid &= ~removed
    inner = vox.interior[solid[vox.interior[:, 0], vox.interior[:, 1], vox.interior[:, 2]]]
    tris, segs = _cube_faces(vox.centres(inner), vox.h, _exposed(inner, solid))
    cv.triangles(tris, SKY)
    lines = []
    keep = solid[vox.ijk[:, 0], vox.ijk[:, 1], vox.ijk[:, 2]]
    exposed = _exposed(vox.ijk, solid)
    for b in np.nonzero(keep)[0]:
        cv.triangles(L.sb.fan(vox.polys[b], vox.h), ORANGE)
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
            nrm = np.zeros(3)
            nrm[a] = 1.0 if f % 2 else -1.0
            cv.signed_region(np.array([[ctr, p0, p1] for p0, p1 in segs_f]), nrm, ORANGE)
    cv.lines(segs, BLUE, 0.5)
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
    contour slices every CAGE_CELLS cells in height (midway between lattice planes) and the
    silhouette. With a cut, only the part on the kept side y <= y_cut."""
    if name in ("sphere", "torus"):
        lines = _rings(name)
    else:
        z0, z1 = drawn.verts[:, 2].min(), drawn.verts[:, 2].max()
        count = int((z1 - vox.base[2]) / (CAGE_CELLS * vox.h)) + 1
        levels = vox.base[2] + (np.arange(count) * CAGE_CELLS + 1.5) * vox.h
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


def figure(name: str, pitch: float | None = None) -> None:
    mesh = L.mesh_by_name(name)
    analytic = name in ("sphere", "torus")
    drawn = _analytic_surface(name) if analytic else mesh
    h = PITCH[name] if pitch is None else pitch
    vox = L.voxelise(mesh, h, keep_drawing=True)
    centre_y = 0.5 * (mesh.verts[:, 1].min() + mesh.verts[:, 1].max())
    k_cut = int(round((centre_y - vox.base[1]) / h))
    y_cut = float(vox.base[1] + k_cut * h)
    n_on = int((vox.ctr_in > 0.5).sum())
    for cut in (False, True):
        removed = np.zeros(tuple(int(x) for x in vox.n), dtype=bool)
        if cut:
            removed[:, k_cut:, :] = True
        panels = []
        true_z = None
        for kind in ("exact", "cubes", "clipped"):
            cv = _canvas(mesh)
            if kind == "exact":
                _exact(cv, drawn, y_cut if cut else None)
                true_z = cv.z.copy()
            else:
                if kind == "cubes":
                    _cubes(cv, vox, removed)
                else:
                    _clipped(cv, vox, removed)
                # the true surface as a dotted cage, depth-tested against the voxels and against
                # the true surface's own render (so its far side stays hidden)
                cage = _cage(name, drawn, vox, cv.fwd, y_cut if cut else None)
                cv.dots(cage, INK, pitch=SIZE / 170, radius=(SIZE / 900, SIZE / 600), front=true_z)
                if cut:
                    _outline(cv, _section(drawn, y_cut))
            panels.append(cv.image())
        # Crop empty rows (the same rows in every panel, so the panels stay aligned).
        busy = np.any([np.abs(p - p[0, 0]).sum(axis=2).max(axis=1) > 0.02 for p in panels], axis=0)
        rows = np.nonzero(busy)[0]
        top, bottom = max(rows[0] - 12, 0), min(rows[-1] + 12, SIZE)
        panels = [p[top:bottom] for p in panels]
        aspect = (bottom - top) / SIZE
        fig, axes = plt.subplots(1, 3, figsize=(15, 5.0 * aspect + 1.6), facecolor="#fcfcfb")
        titles = ["analytic" if analytic else "decimated, repaired mesh" if name in L.DECIMATE
                  else "repaired mesh (original triangles)",
                  f"cube voxels: {len(vox.interior)} interior + {n_on} boundary",
                  f"clipped polyhedra: {len(vox.interior)} cubes + {len(vox.frac)} pieces"]
        for ax, img, title in zip(axes, panels, titles):
            ax.imshow(img, interpolation="lanczos")
            ax.set_axis_off()
            ax.set_title(title, fontsize=12, color="#0b0b0b")
        what = {"sphere": "Sphere, radius 1 m", "torus": "Torus, R = 1 m, r = 0.4 m",
                "teapot": "Utah teapot, 1 m long", "suzanne": "Suzanne, 1 m wide",
                **{n: f"{f.title}, 1 m largest extent" for n, f in L.FAMOUS.items()}}[name]
        cut_txt = f", cut away at the lattice wall y = {y_cut:.3f} m" if cut else ""
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
                    f"contours every {CAGE_CELLS} cells in height and the silhouette")
        caption = (f"{what}, pitch {h} m{cut_txt}. {left} Cube voxels: a cell is solid if its centre is "
                   f"inside (blue). Clipped polyhedra: the same interior cubes (blue), and every crossed cell "
                   f"cut to its inside piece (orange). Dotted: the true surface ({cage_txt}"
                   f"{'; and where the cut meets it' if cut else ''}), shown where it lies in front of the "
                   f"voxels or within a hair of them.")
        height = 5.0 * aspect + 1.6
        fig.text(0.02, 0.08 / height, "\n".join(textwrap.wrap(caption, 190)), fontsize=9, color="#52514e",
                 va="bottom", ha="left")
        fig.subplots_adjust(left=0.01, right=0.99, top=1 - 0.42 / height, bottom=0.72 / height, wspace=0.02)
        out = L.HERE / f"{name}{'-cutaway' if cut else ''}.png"
        fig.savefig(out, dpi=DPI.get(name, 110), facecolor=fig.get_facecolor())
        plt.close(fig)
        print(f"wrote {out}", flush=True)


if __name__ == "__main__":
    for arg in sys.argv[1:] or ["sphere", "torus", "teapot", "suzanne"]:
        shape, _, pitch = arg.partition("@")
        figure(shape, float(pitch) if pitch else None)
