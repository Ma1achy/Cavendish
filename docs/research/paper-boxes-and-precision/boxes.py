"""Question A: coalesce a body's interior cells into a few large axis-aligned boxes, each with its
exact closed-form field, and the boundary as cut-cell moments or clipped pieces.

    python boxes.py run <shape>      # writes data/boxes-<shape>.json (resumes by pitch)
    python boxes.py all              # every shape, one after another

For each of the paper's ten shapes and 4 to 128 cells across (the shapes study's lattice: the same
mesh, `voxelise`, `GENERIC` placement and tower), the interior cells are decomposed by five
strategies:

    cells     every interior cell alone (the study's cost model)
    octree    the largest aligned 2^k blocks wholly inside (an octree's interior leaves)
    octmerge  octree blocks merged pairwise wherever two share a whole face, until none do
    sweep     x-runs, merged along y, then along z ("greedy meshing")
    darnell   Darnell (2011) as Fenney, Allen & Aman (2025) summarise it: seeds in order of
              decreasing Manhattan distance to the outside; from each uncovered seed a box grows one
              layer at a time on all six faces in turn while the new layer is uncovered interior
    largest   largest-first: seeds by the largest centred cube of uncovered cells (chessboard
              distance, re-evaluated lazily as boxes are placed), each grown on whichever face adds
              the most cells; a greedy stand-in for the largest-empty-box rule

Each decomposition is checked to partition the interior (criterion A1), its field is the sum of its
boxes' closed forms (12 triangles each, h·2A numerator, `shapes_lib.g_mesh`), and its phase is
compared with the interior summed cell by cell (A2). The boundary is added as (c) fill + centroid,
(d) + second moment (cut-cell moments) or (e) the clipped pieces, against the whole mesh. Costs are
the paper's model (8.5 ns per cube series or point, 17 ns point + quadrupole, 55 ns per triangle)
plus a box at the measured box/triangle ratio (`kbench`, data/kbench-*.json). Criteria:
criteria.md.
"""

from __future__ import annotations

import heapq
import json
import sys
import time

import numpy as np
from scipy import ndimage

import common as C

L = C.L
sb = L.sb

NS = [4, 8, 16, 32, 64, 128]
DIST = np.array([0.1, 0.3, 1.5, 5.0, 20.0, 100.0, 1000.0])  # lower cloud from the surface, m
NS_SERIES, NS_QUAD, NS_TRI = 8.5, 17.0, 55.0
_kb = [json.loads((C.DATA / f"kbench-{i}.json").read_text()) for i in (1, 2, 3)]
BOX_PER_TRI = float(np.median([k["ns_box"] / k["ns_triangle"] for k in _kb]))
NS_BOX = NS_TRI * BOX_PER_TRI
SERIES_RATIO = 0.01  # a cube uses its l <= 4 series where (side/2)/distance < this (shapes study)
S3_RATIO = 1 / 6  # Cavendish S3's rule: the prism within 3 sides (PRISM_WITHIN = 3), series beyond
RATIOS = {"strict": SERIES_RATIO, "r0.05": 0.05, "s3": S3_RATIO}
STRATEGIES = ["cells", "octree", "octmerge", "sweep", "darnell", "largest"]


# ------------------------------------------------------------------ decompositions
# A box is (i0, j0, k0, i1, j1, k1): lattice indices, upper bounds exclusive.


def boxes_cells(g: np.ndarray) -> np.ndarray:
    ijk = np.argwhere(g)
    return np.hstack([ijk, ijk + 1])


def boxes_octree(g: np.ndarray) -> np.ndarray:
    m = max(g.shape)
    top = int(np.ceil(np.log2(m)))
    p = 1 << top
    pad = np.zeros((p, p, p), dtype=bool)
    pad[: g.shape[0], : g.shape[1], : g.shape[2]] = g
    out = []
    parent_full = None
    for k in range(top, -1, -1):
        s = 1 << k
        q = p // s
        full = pad.reshape(q, s, q, s, q, s).all(axis=(1, 3, 5))
        leaf = full if parent_full is None else full & ~np.repeat(np.repeat(np.repeat(
            parent_full, 2, 0), 2, 1), 2, 2)
        ijk = np.argwhere(leaf) * s
        out.append(np.hstack([ijk, ijk + s]))
        parent_full = full
    return np.vstack(out)


def merge_along(boxes: list[tuple[int, ...]], ax: int) -> tuple[list[tuple[int, ...]], bool]:
    """One pass: chain boxes that meet face to face along `ax` with the same cross-section."""
    o0, o1 = [a for a in range(3) if a != ax]
    start = {(b[o0], b[3 + o0], b[o1], b[3 + o1], b[ax]): i for i, b in enumerate(boxes)}
    alive = [True] * len(boxes)
    new = list(boxes)
    changed = False
    for i in sorted(range(len(boxes)), key=lambda i: boxes[i][ax]):
        if not alive[i]:
            continue
        b = list(new[i])
        while True:
            j = start.get((b[o0], b[3 + o0], b[o1], b[3 + o1], b[3 + ax]))
            if j is None or j == i or not alive[j]:
                break
            b[3 + ax] = new[j][3 + ax]
            alive[j] = False
            changed = True
        new[i] = tuple(b)
    return [new[i] for i in range(len(boxes)) if alive[i]], changed


def boxes_octmerge(g: np.ndarray) -> np.ndarray:
    b = [tuple(int(x) for x in r) for r in boxes_octree(g)]
    changed = True
    while changed:
        changed = False
        for ax in range(3):
            b, c = merge_along(b, ax)
            changed |= c
    return np.array(b)


def boxes_sweep(g: np.ndarray) -> np.ndarray:
    runs = []
    nx = g.shape[0]
    for j in range(g.shape[1]):
        for k in range(g.shape[2]):
            col = g[:, j, k]
            if not col.any():
                continue
            d = np.diff(np.concatenate([[0], col.astype(np.int8), [0]]))
            for a, b in zip(np.nonzero(d == 1)[0], np.nonzero(d == -1)[0]):
                runs.append((int(a), j, k, int(b), j + 1, k + 1))
    assert nx > 0
    b, _ = merge_along(runs, 1)
    b, _ = merge_along(b, 2)
    return np.array(b)


def _grow(free: np.ndarray, lo: list[int], hi: list[int], best_face: bool) -> None:
    """Grow [lo, hi) while a whole new layer of `free` cells is there; round robin over the six
    faces (Darnell) or, with `best_face`, always the face that adds the most cells."""
    faces = [(a, s) for a in range(3) for s in (-1, 1)]
    active = set(faces)
    shape = free.shape

    def layer(a: int, s: int) -> tuple[slice, ...] | None:
        idx = lo[a] - 1 if s < 0 else hi[a]
        if idx < 0 or idx >= shape[a]:
            return None
        sl = [slice(lo[0], hi[0]), slice(lo[1], hi[1]), slice(lo[2], hi[2])]
        sl[a] = slice(idx, idx + 1)
        return tuple(sl)

    def ok(a: int, s: int) -> bool:
        sl = layer(a, s)
        return sl is not None and bool(free[sl].all())

    def extend(a: int, s: int) -> None:
        if s < 0:
            lo[a] -= 1
        else:
            hi[a] += 1

    if not best_face:
        while active:
            for f in faces:
                if f in active:
                    if ok(*f):
                        extend(*f)
                    else:
                        active.discard(f)
        return
    while active:
        cand = [f for f in faces if f in active]
        size = [int(np.prod([hi[b] - lo[b] for b in range(3) if b != f[0]])) for f in cand]
        for i in np.argsort(size)[::-1]:
            f = cand[i]
            if ok(*f):
                extend(*f)
                break
            active.discard(f)


def boxes_darnell(g: np.ndarray) -> np.ndarray:
    free = g.copy()
    dist = ndimage.distance_transform_cdt(g, metric="taxicab")
    ijk = np.argwhere(g)
    order = np.argsort(-dist[g], kind="stable")
    out = []
    for i, j, k in ijk[order]:
        if not free[i, j, k]:
            continue
        lo, hi = [int(i), int(j), int(k)], [int(i) + 1, int(j) + 1, int(k) + 1]
        _grow(free, lo, hi, best_face=False)
        free[lo[0]:hi[0], lo[1]:hi[1], lo[2]:hi[2]] = False
        out.append((*lo, *hi))
    return np.array(out)


def boxes_largest(g: np.ndarray) -> np.ndarray:
    free = g.copy()
    dist = ndimage.distance_transform_cdt(g, metric="chessboard")
    ijk = np.argwhere(g)
    heap = [(-int(dist[i, j, k]), int(i), int(j), int(k)) for i, j, k in ijk]
    heapq.heapify(heap)
    out = []
    while heap:
        nd, i, j, k = heapq.heappop(heap)
        if not free[i, j, k]:
            continue
        d = -nd
        while d > 1 and not free[i - d + 1:i + d, j - d + 1:j + d, k - d + 1:k + d].all():
            d -= 1
        if d < -nd:
            heapq.heappush(heap, (-d, i, j, k))
            continue
        lo, hi = [i - d + 1, j - d + 1, k - d + 1], [i + d, j + d, k + d]
        _grow(free, lo, hi, best_face=True)
        free[lo[0]:hi[0], lo[1]:hi[1], lo[2]:hi[2]] = False
        out.append((*lo, *hi))
    return np.array(out)


MAKERS = {"cells": boxes_cells, "octree": boxes_octree, "octmerge": boxes_octmerge,
          "sweep": boxes_sweep, "darnell": boxes_darnell, "largest": boxes_largest}


def partition_ok(boxes: np.ndarray, g: np.ndarray) -> bool:
    """Criterion A1: every interior cell covered exactly once, nothing else covered."""
    count = np.zeros(g.shape, dtype=np.int32)
    for b in boxes:
        count[b[0]:b[3], b[1]:b[4], b[2]:b[5]] += 1
    return bool(np.array_equal(count, g.astype(np.int32)))


# ------------------------------------------------------------------ fields

_UNIT = sb.cube_quads(np.zeros((1, 3)), 1.0)  # (6, 4, 3), corners at ±0.5


def box_tris(lo: np.ndarray, hi: np.ndarray) -> np.ndarray:
    """12 outward triangles per axis-aligned box [lo, hi] (metres): (12K, 3, 3)."""
    c, s = 0.5 * (lo + hi), hi - lo
    q = c[:, None, None, :] + _UNIT[None] * s[:, None, None, :]
    return np.concatenate([q[:, :, [0, 1, 2]], q[:, :, [0, 2, 3]]], axis=1).reshape(-1, 3, 3)


def g_boxes(boxes: np.ndarray, base: np.ndarray, h: float, pts: np.ndarray) -> np.ndarray:
    out = np.zeros((len(pts), 3))
    for s in range(0, len(boxes), 20000):
        b = boxes[s:s + 20000]
        out += L.g_mesh(box_tris(base + b[:, :3] * h, base + b[:, 3:] * h), pts)
    return out


def _far(boxes: np.ndarray, base: np.ndarray, h: float, pair: np.ndarray, ratio: float) -> np.ndarray:
    """Which cubic elements take their series for the read at `pair` (lower, upper point):
    (side/2)/d < ratio at the nearer of the two."""
    lo, hi = base + boxes[:, :3] * h, base + boxes[:, 3:] * h
    c = 0.5 * (lo + hi)
    d = np.minimum(np.linalg.norm(c - pair[0], axis=1), np.linalg.norm(c - pair[1], axis=1))
    return 0.5 * (hi[:, 0] - lo[:, 0]) / d < ratio


def interior_cost(boxes: np.ndarray, base: np.ndarray, h: float, pair: np.ndarray, cubes: bool,
                  ratio: float = SERIES_RATIO) -> float:
    """Model cost (ns) per evaluation point for the read at `pair`. Cubic elements (cells, octree
    blocks) use the l <= 4 series where `_far`, their closed form otherwise; non-cubic boxes always
    their closed form (no box series is implemented here: pessimistic for far boxes)."""
    if not cubes:
        return float(len(boxes) * NS_BOX)
    far = _far(boxes, base, h, pair, ratio)
    return float(far.sum() * NS_SERIES + (~far).sum() * NS_BOX)


def g_cubic_hybrid(boxes: np.ndarray, base: np.ndarray, h: float, pts: np.ndarray,
                   ratio: float) -> np.ndarray:
    """g of cubic elements (cells or octree blocks) as read: per tower pair, the series where
    `_far` and the closed form elsewhere (`shapes_lib.g_cubes_fast` per block size)."""
    m = len(pts) // 2
    out = np.zeros((len(pts), 3))
    side = boxes[:, 3] - boxes[:, 0]
    for k in range(m):
        pair = pts[[k, m + k]]
        for s in np.unique(side):
            b = boxes[side == s]
            c = base + (b[:, :3] + 0.5 * s) * h
            out[[k, m + k]] += L.g_cubes_fast(c, s * h, np.ones(len(b)), pair, ratio=ratio)
    return out


def g_box_hybrid(boxes: np.ndarray, base: np.ndarray, h: float, pts: np.ndarray,
                 ratio: float) -> tuple[np.ndarray, list[float]]:
    """Free-size boxes as they would be read: per tower pair, a box whose half-diagonal over its
    distance (to the nearer point) is below `ratio` takes point + quadrupole (its exact second
    moment, V diag(a², b², c²)/12; a box has no dipole or octupole about its centre, so the
    remainder is l = 4), the others their closed form. Returns g and the model cost per pair (ns)."""
    m = len(pts) // 2
    out = np.zeros((len(pts), 3))
    costs = []
    lo, hi = base + boxes[:, :3] * h, base + boxes[:, 3:] * h
    c, s = 0.5 * (lo + hi), hi - lo
    rho = 0.5 * np.linalg.norm(s, axis=1)
    vol = s.prod(axis=1)
    second = np.zeros((len(boxes), 3, 3))
    for a in range(3):
        second[:, a, a] = vol * s[:, a] ** 2 / 12
    for k in range(m):
        idx = [k, m + k]
        pair = pts[idx]
        d = np.minimum(np.linalg.norm(c - pair[0], axis=1), np.linalg.norm(c - pair[1], axis=1))
        far = rho / d < ratio
        g = L.g_points(vol[far], c[far], pair) + L.g_quadrupole(second[far], c[far], pair)
        g += g_boxes(boxes[~far], base, h, pair)
        out[idx] = g
        costs.append(float(far.sum() * NS_QUAD + (~far).sum() * NS_BOX))
    return out, costs


def coverage(boxes: np.ndarray) -> list[int]:
    """Cell counts of the boxes, largest first."""
    v = np.prod(boxes[:, 3:] - boxes[:, :3], axis=1)
    return sorted((int(x) for x in v), reverse=True)


# ------------------------------------------------------------------ one shape


def run(name: str) -> None:
    mesh = L.mesh_by_name(name)
    size = float((mesh.verts.max(axis=0) - mesh.verts.min(axis=0)).max())
    pts = L.tower(mesh, DIST)
    m = len(DIST)
    ph = L.phase
    t0 = time.perf_counter()
    g_mesh = L.g_mesh(mesh.tris, pts)
    t_mesh = time.perf_counter() - t0
    path = C.DATA / f"boxes-{name}.json"
    out = json.loads(path.read_text()) if path.exists() else {
        "shape": name, "size": size, "triangles": len(mesh.faces), "distances": DIST.tolist(),
        "signal": ph(g_mesh).tolist(), "ns_box": NS_BOX, "box_per_triangle": BOX_PER_TRI,
        "mesh_cost_ns": len(mesh.faces) * NS_TRI, "seconds_mesh_numpy": t_mesh, "rows": []}
    sweep = json.loads((C.STUDY / f"sweep-{name}.json").read_text())
    for n_across in NS:
        if any(r["n"] == n_across for r in out["rows"]):
            continue
        h = size / n_across
        t0 = time.perf_counter()
        vox = L.voxelise(mesh, h)
        t_vox = time.perf_counter() - t0
        g = np.zeros(tuple(int(x) for x in vox.n), dtype=bool)
        g[tuple(vox.interior.T)] = True
        n_in, n_b = int(g.sum()), len(vox.frac)
        row: dict[str, object] = {"n": n_across, "h": h, "interior": n_in, "boundary": n_b,
                                  "piece_tris": len(vox.piece_tris), "seconds_voxelise": t_vox,
                                  "strategies": {}}
        # Boundary representations.
        mass_b = vox.frac * h**3
        g_c = L.g_points(mass_b, vox.centroid, pts)
        g_d = g_c + L.g_quadrupole(vox.second, vox.centroid, pts)
        g_e = L.g_mesh(vox.piece_tris, pts)
        # Interior: every cell (the reference), then each strategy.
        g_cells = L.g_cubes_fast(vox.interior_centres, h, np.ones(n_in), pts)
        ph_cells = ph(g_cells)
        row["interior_signal"] = ph_cells.tolist()
        row["err_boundary"] = {
            "c": (ph(g_cells + g_c) - ph(g_mesh)).tolist(),
            "d": (ph(g_cells + g_d) - ph(g_mesh)).tolist(),
            "e": (ph(g_cells + g_e) - ph(g_mesh)).tolist()}
        sw = next(r for r in sweep["rows"] if r["n"] == n_across)
        row["sweep_d_1.5m"] = sw["err"]["(d) + second moment"][0]
        row["sweep_e_1.5m"] = sw["err"]["(e) clipped pieces"][0]
        lower = pts[:m]
        row["cost_boundary_ns"] = {"c": n_b * NS_SERIES, "d": n_b * NS_QUAD,
                                   "e": len(vox.piece_tris) * NS_TRI}
        for strat in STRATEGIES:
            t0 = time.perf_counter()
            boxes = np.asarray(MAKERS[strat](g), dtype=np.int64).reshape(-1, 6)
            t_make = time.perf_counter() - t0
            st: dict[str, object] = {"boxes": len(boxes), "seconds": t_make}
            st["partition_ok"] = True if strat == "cells" else partition_ok(boxes, g)
            cubes = strat in ("cells", "octree")
            pairs = [pts[[k, m + k]] for k in range(m)]
            st["cost_interior_ns"] = [interior_cost(boxes, vox.base, h, p, cubes) for p in pairs]
            st["cost_interior_all_series_ns"] = len(boxes) * NS_SERIES if cubes else None
            if cubes:  # other switch distances, down to S3's rule (closed form within 3 sides)
                for key, ratio in RATIOS.items():
                    st[f"cost_interior_{key}_ns"] = [interior_cost(boxes, vox.base, h, p, True, ratio)
                                                    for p in pairs]
            if strat == "octree":  # the octree as it would be read: series far, closed form near
                for key, ratio in RATIOS.items():
                    g_o = g_cubic_hybrid(boxes, vox.base, h, pts, ratio)
                    st[f"diff_vs_cells_{key}"] = (ph(g_o) - ph_cells).tolist()
                    st[f"err_{key}"] = {k: (ph(g_o + gb) - ph(g_mesh)).tolist()
                                        for k, gb in (("c", g_c), ("d", g_d), ("e", g_e))}
                st["levels"] = {str(int(s)): int(c) for s, c in
                                zip(*np.unique(boxes[:, 3] - boxes[:, 0], return_counts=True))}
            if strat in ("darnell", "sweep"):  # free boxes read with a series switch too
                for key, ratio in RATIOS.items():
                    g_bh, c_bh = g_box_hybrid(boxes, vox.base, h, pts, ratio)
                    st[f"diff_vs_cells_{key}"] = (ph(g_bh) - ph_cells).tolist()
                    st[f"cost_interior_{key}_ns"] = c_bh
            if strat != "cells":
                g_b = g_boxes(boxes, vox.base, h, pts)
                diff = ph(g_b) - ph_cells
                st["diff_vs_cells"] = diff.tolist()
                tol = np.maximum(1e-11, 1e-14 * np.abs(ph_cells))
                st["rounding_ok"] = bool(np.all(np.abs(diff) <= tol))
                st["err"] = {k: (ph(g_b + gb) - ph(g_mesh)).tolist()
                             for k, gb in (("c", g_c), ("d", g_d), ("e", g_e))}
                if strat == "darnell" and len(boxes):  # the control arm: drop the largest box
                    vol = np.prod(boxes[:, 3:] - boxes[:, :3], axis=1)
                    keep = np.ones(len(boxes), dtype=bool)
                    keep[int(np.argmax(vol))] = False
                    st["control_partition_ok"] = partition_ok(boxes[keep], g)
                    st["control_diff_vs_cells"] = (ph(g_boxes(boxes[keep], vox.base, h, pts))
                                                   - ph_cells).tolist()
            if n_across == 128 or strat != "cells":
                st["coverage"] = coverage(boxes) if strat != "cells" else None
            row["strategies"][strat] = st  # type: ignore[index]
            print(name, n_across, strat, len(boxes), f"{t_make:.1f}s", st.get("partition_ok"),
                  st.get("rounding_ok"), flush=True)
        out["rows"].append(row)
        out["rows"].sort(key=lambda r: r["n"])
        path.write_text(json.dumps(out))
        del vox


if __name__ == "__main__":
    if sys.argv[1] == "run":
        if len(sys.argv) > 3:
            NS[:] = [n for n in NS if n <= int(sys.argv[3])]
        run(sys.argv[2])
    elif sys.argv[1] == "all":
        for s in C.SHAPES:
            run(s)
