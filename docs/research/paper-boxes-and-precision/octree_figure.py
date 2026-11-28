"""The octree body, cut away: the paper's proposed figure for interior coalescing.

    python octree_figure.py [shape] [cells across]     # default: bunny at 32

Three panels on one lattice, camera and light (the shapes study's `figures.py` and `Canvas`, so the
style is the paper's Figure 8): (a) the true body, cut at the lattice wall nearest its centre plane;
(b) the octree body: interior merged into the largest aligned power-of-two blocks wholly inside
(sky blue, outlined in blue along block edges only), the surface at the finest pitch as its clipped
pieces (orange); (c) the same with the interior as free-size boxes (Darnell's greedy growth). Dotted:
the true surface's section on the cut and its sparse cage, depth-tested. Writes
figures/octree-cutaway.png and data/octree-figure.json (the counts in the titles).
"""

from __future__ import annotations

import json
import sys

import matplotlib.pyplot as plt
import numpy as np

import boxes as B
import common as C

sys.path.insert(0, str(C.STUDY))
import figures as F  # noqa: E402  (the shapes study's renderer helpers)

L = C.L


def _blocks(cv: L.Canvas, blocks: np.ndarray, vox: L.Voxels, k_cut: int) -> int:
    """Draw lattice boxes cut at y-index k_cut; returns how many are drawn."""
    b = blocks.copy()
    b = b[b[:, 1] < k_cut]
    b[:, 4] = np.minimum(b[:, 4], k_cut)
    lo, hi = vox.base + b[:, :3] * vox.h, vox.base + b[:, 3:] * vox.h
    c, s = 0.5 * (lo + hi), hi - lo
    q = c[:, None, None, :] + B._UNIT[None] * s[:, None, None, :]  # (K, 6, 4, 3)
    tris = np.concatenate([q[:, :, [0, 1, 2]], q[:, :, [0, 2, 3]]], axis=1).reshape(-1, 3, 3)
    cv.triangles(tris, F.SKY)
    segs = np.concatenate([q[:, :, [i, (i + 1) % 4]] for i in range(4)], axis=1).reshape(-1, 2, 3)
    cv.lines(segs, np.array([0.0, 0.25, 0.45]), 1.0)
    return len(b)


def _pieces(cv: L.Canvas, vox: L.Voxels, removed: np.ndarray) -> None:
    """The boundary cells' clipped pieces with their caps (as figures._clipped, without cubes)."""
    solid = np.zeros(tuple(int(x) for x in vox.n), dtype=bool)
    for ijk in (vox.interior, vox.ijk):
        solid[ijk[:, 0], ijk[:, 1], ijk[:, 2]] = True
    solid &= ~removed
    keep = solid[vox.ijk[:, 0], vox.ijk[:, 1], vox.ijk[:, 2]]
    exposed = F._exposed(vox.ijk, solid)
    lines = []
    for bi in np.nonzero(keep)[0]:
        cv.triangles(L.sb.fan(vox.polys[bi], vox.h), F.ORANGE)
        c_lo = vox.base + vox.ijk[bi] * vox.h
        for f in range(6):
            segs_f = [(p0, p1) for f_i, p0, p1 in vox.caps[bi] if f_i == f]
            if not segs_f:
                continue
            lines.append(np.array(segs_f))
            if not exposed[bi, f]:
                continue
            a = f // 2
            ctr = c_lo + 0.5 * vox.h
            ctr[a] = c_lo[a] + (vox.h if f % 2 else 0.0)
            nrm = np.zeros(3)
            nrm[a] = 1.0 if f % 2 else -1.0
            cv.signed_region(np.array([[ctr, p0, p1] for p0, p1 in segs_f]), nrm, F.ORANGE)
    if lines:
        cv.lines(np.concatenate(lines), F.VERMILION, 0.3)


def main(name: str = "bunny", n_across: int = 32) -> None:
    C.style()
    mesh = L.mesh_by_name(name)
    size = float((mesh.verts.max(axis=0) - mesh.verts.min(axis=0)).max())
    h = size / n_across
    analytic = name in ("sphere", "torus")
    drawn = F._analytic_surface(name) if analytic else mesh
    vox = L.voxelise(mesh, h, keep_drawing=True)
    g = np.zeros(tuple(int(x) for x in vox.n), dtype=bool)
    g[tuple(vox.interior.T)] = True
    octree = B.boxes_octree(g)
    free = B.boxes_darnell(g)
    centre_y = 0.5 * (mesh.verts[:, 1].min() + mesh.verts[:, 1].max())
    k_cut = int(round((centre_y - vox.base[1]) / h))
    y_cut = float(vox.base[1] + k_cut * h)
    removed = np.zeros(tuple(int(x) for x in vox.n), dtype=bool)
    removed[:, k_cut:, :] = True
    panels, counts = [], {}
    true_z = None
    for kind in ("exact", "octree", "free"):
        cv = F._canvas(mesh)
        if kind == "exact":
            F._exact(cv, drawn, y_cut)
            true_z = cv.z.copy()
        else:
            counts[kind] = _blocks(cv, octree if kind == "octree" else free, vox, k_cut)
            _pieces(cv, vox, removed)
            cage = F._cage(name, drawn, vox, cv.fwd, y_cut)
            cv.dots(cage, F.INK, pitch=F.SIZE / 170, radius=(F.SIZE / 900, F.SIZE / 600), front=true_z)
            F._outline(cv, F._section(drawn, y_cut))
        panels.append(cv.image())
    busy = np.any([np.abs(p - p[0, 0]).sum(axis=2).max(axis=1) > 0.02 for p in panels], axis=0)
    rows = np.nonzero(busy)[0]
    top, bottom = max(rows[0] - 12, 0), min(rows[-1] + 12, F.SIZE)
    panels = [p[top:bottom] for p in panels]
    aspect = (bottom - top) / F.SIZE
    fig, axes = plt.subplots(1, 3, figsize=(6.8, 6.8 / 3 * aspect + 0.3))
    sizes = np.unique(octree[:, 3] - octree[:, 0], return_counts=True)
    titles = ["(a) true body (mesh)",
              f"(b) octree: {len(octree)} blocks + {len(vox.frac)} pieces",
              f"(c) free boxes: {len(free)} + {len(vox.frac)} pieces"]
    for ax, img, t in zip(axes, panels, titles):
        ax.imshow(img, interpolation="lanczos")
        ax.set_axis_off()
        ax.set_title(t)
    fig.subplots_adjust(left=0.0, right=1.0, top=0.9, bottom=0.0, wspace=0.01)
    fig.savefig(C.FIG / "octree-cutaway.png", dpi=300, facecolor="white")
    plt.close(fig)
    info = {"shape": name, "n_across": n_across, "h": h, "interior_cells": int(g.sum()),
            "boundary_cells": len(vox.frac), "octree_blocks": len(octree), "free_boxes": len(free),
            "octree_block_sizes": {str(int(s)): int(c) for s, c in zip(*sizes)},
            "drawn_after_cut": counts, "y_cut": y_cut}
    (C.DATA / "octree-figure.json").write_text(json.dumps(info, indent=1))
    print(info, flush=True)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "bunny", int(sys.argv[2]) if len(sys.argv) > 2 else 32)
