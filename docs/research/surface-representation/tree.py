"""Parts 2–5: adaptive surface refinement, the representation chosen per node in the tree walk,
free-size boxes with the l = 4 and 6 series, and a BVH against the octree.

    python tree.py boxes <shape>     # interior decompositions -> .work/<shape>/{octree,sweep}.f64
                                     #   (then `srbench boxes` reads them at the towers)
    python tree.py run <shape>       # data/tree-<shape>.json
    python tree.py all

Everything is read from `srbench contrib` (per-cell phases of every representation at every tower,
and each cell's exact moments, box and |P Δ Q|) and `srbench boxes` (each interior box's closed form
and series). Costs are the measured CPU kernel times of `srbench time` (ns per element per point),
so every cost here is a model built from measured parts: counts × measured ns.

Budgets. A design budget B (rad) is shared out so that the measured or bounded per-element errors
sum to at most B: by surface area for adaptive refinement (each cell's error is measured against its
exact piece), by mass for the walk (each node's error is bounded a priori). The bounds:
- a multipole through order p of a node of volume M whose mass lies within b of its expansion
  centre, at distance d > b: |ΔU| ≤ M (b/d)^(p+1) / (d − b) (Salmon & Warren 1994, eq. for the
  Taylor remainder of 1/|x − y|; used per point and summed with the read's weights |c_i|);
- a box's series (even orders only) through l: the same with p = l + 1;
- the PLIC piece Q plus the multipole of P − Q through l = 3 about the cell centre: the same with
  p = 3 and M = |P Δ Q| (the residual's absolute volume), b the cell's half-diagonal.
"""

from __future__ import annotations

import json
import sys

import numpy as np

import common as C
import ladder as LD

BUDGETS = [1e-6, 1e-8, 1e-10]
# The check "measured error <= bound" allows for the reference's own rounding per element: the
# closed form's relative error grows as eps (R/s)^2 (skin note §1.1), read through the flight
# integral's weights, plus 1e-14 rad absolute. See slack().
FLOOR = 1e-14


def slack(vol: np.ndarray, cen: np.ndarray, size: np.ndarray | float, pts: np.ndarray, w: np.ndarray) -> np.ndarray:
    """Rounding allowance per element (rad): 3e-16 Σ|c_i| ρ vol d_i/size² + FLOOR."""
    size = np.broadcast_to(np.asarray(size, dtype = float), vol.shape)
    return 3e-16 * bound(vol, cen, size, -3, pts, w) + FLOOR
H3 = np.sqrt(3.0) / 2.0


# ------------------------------------------------------------------ data
def towers(shape: str) -> tuple[np.ndarray, np.ndarray, int]:
    t = np.fromfile(C.WORK / shape / "towers.f64").reshape(-1, 4)
    n = int((C.WORK / shape / "towers.count").read_text())
    return t[:, :3].reshape(n, -1, 3), t[:, 3].reshape(n, -1), n


def cells(shape: str, k: int) -> np.ndarray:
    return np.fromfile(C.WORK / shape / f"L{k}" / "cells.f64").reshape(-1, 6)


def interior(shape: str, k: int) -> np.ndarray:
    return np.fromfile(C.WORK / shape / f"L{k}" / "interior.f64").reshape(-1, 3)


def surface_area(shape: str, k: int) -> np.ndarray:
    """Each boundary cell's area of the body's surface (the piece's non-wall triangles), m²."""
    ce = cells(shape, k)
    tr = np.fromfile(C.WORK / shape / f"L{k}" / "tris.f64").reshape(-1, 3, 3)
    owner = np.repeat(np.arange(len(ce)), ce[:, 5].astype(int))
    lo, h = ce[owner, :3], ce[owner, 3]
    tol = 1e-10 * h
    wall = np.zeros(len(tr), dtype = bool)
    for a in range(3):
        x = tr[:, :, a]
        wall |= np.all(np.abs(x - lo[:, a, None]) < tol[:, None], axis = 1)
        wall |= np.all(np.abs(x - lo[:, a, None] - h[:, None]) < tol[:, None], axis = 1)
    area = 0.5 * np.linalg.norm(np.cross(tr[:, 1] - tr[:, 0], tr[:, 2] - tr[:, 0]), axis = 1)
    return np.bincount(owner, area * ~wall, minlength = len(ce))


def ijk(lo: np.ndarray, h: float, origin: np.ndarray) -> np.ndarray:
    return np.rint((lo - origin) / h).astype(np.int64)


def kernel_ns(shape: str) -> dict[str, float]:
    """Measured ns per element per point (srbench time, finest level timed)."""
    for k in (3, 1):
        p = C.DATA / f"time-{shape}-L{k}.json"
        if p.exists() and p.stat().st_size > 0:
            t = json.loads(p.read_text())["kernels"]
            return {n: v["ns_per_element_point"] for n, v in t.items()}
    raise FileNotFoundError(shape)


# ------------------------------------------------------------------ interior boxes
# boxes_octree, merge_along and boxes_sweep are paper-boxes-and-precision/boxes.py's, unchanged.
def boxes_octree(g: np.ndarray) -> np.ndarray:
    m = max(g.shape)
    top = int(np.ceil(np.log2(m)))
    p = 1 << top
    pad = np.zeros((p, p, p), dtype = bool)
    pad[: g.shape[0], : g.shape[1], : g.shape[2]] = g
    out = []
    parent_full = None
    for k in range(top, -1, -1):
        s = 1 << k
        q = p // s
        full = pad.reshape(q, s, q, s, q, s).all(axis = (1, 3, 5))
        leaf = full if parent_full is None else full & ~np.repeat(np.repeat(np.repeat(parent_full, 2, 0), 2, 1), 2, 2)
        ix = np.argwhere(leaf) * s
        out.append(np.hstack([ix, ix + s]))
        parent_full = full
    return np.vstack(out)


def merge_along(boxes: list[tuple[int, ...]], ax: int) -> tuple[list[tuple[int, ...]], bool]:
    o0, o1 = [a for a in range(3) if a != ax]
    start = {(b[o0], b[3 + o0], b[o1], b[3 + o1], b[ax]): i for i, b in enumerate(boxes)}
    alive = [True] * len(boxes)
    new = list(boxes)
    changed = False
    for i in sorted(range(len(boxes)), key = lambda i: boxes[i][ax]):
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


def boxes_sweep(g: np.ndarray) -> np.ndarray:
    runs = []
    for j in range(g.shape[1]):
        for k in range(g.shape[2]):
            col = g[:, j, k]
            if not col.any():
                continue
            d = np.diff(np.concatenate([[0], col.astype(np.int8), [0]]))
            for a, b in zip(np.nonzero(d == 1)[0], np.nonzero(d == -1)[0]):
                runs.append((int(a), j, k, int(b), j + 1, k + 1))
    b, _ = merge_along(runs, 1)
    b, _ = merge_along(b, 2)
    return np.array(b)


def make_boxes(shape: str) -> None:
    """The finest level's interior as octree blocks (aligned to the coarsest lattice) and as sweep
    boxes; written as rows (lo, hi) in metres for `srbench boxes`."""
    prep = C.load_json(f"prep-{shape}.json")
    origin = np.array(prep["origin"])
    h = C.PITCHES[shape][-1]
    cen = interior(shape, 3)
    ix = ijk(cen - h / 2, h, origin)
    align = 1 << 6
    off = (ix.min(axis = 0) // align) * align
    ix = ix - off
    g = np.zeros(ix.max(axis = 0) + 1, dtype = bool)
    g[tuple(ix.T)] = True
    out = {}
    for name, fn in (("octree", boxes_octree), ("sweep", boxes_sweep)):
        b = fn(g)
        lo = origin + (b[:, :3] + off) * h
        hi = origin + (b[:, 3:] + off) * h
        np.ascontiguousarray(np.hstack([lo, hi]), dtype = "<f8").tofile(C.WORK / shape / f"{name}.f64")
        vol = np.prod(hi - lo, axis = 1).sum()
        out[name] = {"boxes": len(b), "volume": float(vol), "cells": int(g.sum()),
                     "partition_ok": bool(abs(vol - g.sum() * h**3) < 1e-9 * vol)}
    C.save_json(f"boxes-{shape}.json", out)
    print(shape, out)


def box_phases(shape: str, name: str) -> tuple[np.ndarray, np.ndarray]:
    b = np.fromfile(C.WORK / shape / f"{name}.f64").reshape(-1, 6)
    _, _, nt = towers(shape)
    ph = np.fromfile(C.WORK / shape / f"{name}.f64.phases.f64").reshape(len(b), nt, 5)
    return b, ph  # ph[..., 0] closed form, 1..4: series l = 0, 2, 4, 6


# ------------------------------------------------------------------ bounds
def bound(vol: np.ndarray, centre: np.ndarray, b: np.ndarray, p: int | np.ndarray, pts: np.ndarray,
          w: np.ndarray) -> np.ndarray:
    """Σ_i |c_i| ρ vol (b/d_i)^(p+1)/(d_i − b), per element (rad); inf where a point is within b."""
    out = np.zeros(len(vol))
    step = max(1, 400_000 // len(pts))
    for s in range(0, len(vol), step):
        d = np.linalg.norm(pts[None, :, :] - centre[s:s + step, None, :], axis = 2)
        bb = b[s:s + step, None]
        pp = p if np.isscalar(p) else p[s:s + step, None]
        with np.errstate(divide = "ignore", invalid = "ignore"):
            t = np.where(d > bb, vol[s:s + step, None] * (bb / d) ** (pp + 1) / (d - bb), np.inf)
        out[s:s + step] = C.RHO * (np.abs(w)[None, :] * t).sum(axis = 1)
    return out


def mp_phase(vol: np.ndarray, cen: np.ndarray, m2: np.ndarray, m3: np.ndarray, order: int,
             pts: np.ndarray, w: np.ndarray) -> np.ndarray:
    """Phase (rad) of point masses with central moments (bench order xx yy zz xy xz yz; xxx ... xyz)."""
    out = np.zeros(len(vol))
    step = max(1, 300_000 // len(pts))
    for s in range(0, len(vol), step):
        d = pts[None, :, :] - cen[s:s + step, None, :]
        x, y, z = d[..., 0], d[..., 1], d[..., 2]
        r2 = x * x + y * y + z * z
        ir = 1 / np.sqrt(r2)
        ir2 = ir * ir
        u = vol[s:s + step, None] * ir
        if order >= 2:
            i = m2[s:s + step, :, None]
            tr = i[:, 0] + i[:, 1] + i[:, 2]
            rir = i[:, 0] * x * x + i[:, 1] * y * y + i[:, 2] * z * z + 2 * (i[:, 3] * x * y + i[:, 4] * x * z + i[:, 5] * y * z)
            u = u + 0.5 * (3 * rir * ir2 - tr) * ir2 * ir
        if order >= 3:
            t = m3[s:s + step, :, None]
            trrr = (t[:, 0] * x**3 + t[:, 1] * y**3 + t[:, 2] * z**3 + 3 * (t[:, 3] * x * x * y + t[:, 4] * x * x * z + t[:, 5] * x * y * y
                    + t[:, 6] * y * y * z + t[:, 7] * x * z * z + t[:, 8] * y * z * z) + 6 * t[:, 9] * x * y * z)
            rt = x * (t[:, 0] + t[:, 5] + t[:, 7]) + y * (t[:, 3] + t[:, 1] + t[:, 8]) + z * (t[:, 4] + t[:, 6] + t[:, 2])
            u = u + (2.5 * trrr * ir2 - 1.5 * rt) * ir2 * ir2 * ir
        out[s:s + step] = (w[None, :] * (-C.RHO * u)).sum(axis = 1)
    return out


I2 = [(0, 0), (1, 1), (2, 2), (0, 1), (0, 2), (1, 2)]
I3 = [(0, 0, 0), (1, 1, 1), (2, 2, 2), (0, 0, 1), (0, 0, 2), (0, 1, 1), (1, 1, 2), (0, 2, 2), (1, 2, 2), (0, 1, 2)]


def raw(vol: np.ndarray, cen: np.ndarray, m2: np.ndarray, m3: np.ndarray) -> np.ndarray:
    """Raw moments about the origin: [vol, S1 (3), S2 (6), S3 (10)] per element."""
    s1 = vol[:, None] * cen
    s2 = np.stack([m2[:, n] + vol * cen[:, i] * cen[:, j] for n, (i, j) in enumerate(I2)], 1)
    ix2 = {p: n for n, p in enumerate(I2)}

    def c2(i: int, j: int) -> np.ndarray:
        return m2[:, ix2[(min(i, j), max(i, j))]]
    s3 = np.stack([m3[:, n] + cen[:, i] * c2(j, k) + cen[:, j] * c2(i, k) + cen[:, k] * c2(i, j)
                   + vol * cen[:, i] * cen[:, j] * cen[:, k] for n, (i, j, k) in enumerate(I3)], 1)
    return np.hstack([vol[:, None], s1, s2, s3])


def central(r: np.ndarray) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
    vol = r[:, 0]
    cen = r[:, 1:4] / vol[:, None]
    s2, s3 = r[:, 4:10], r[:, 10:20]
    ix2 = {p: n for n, p in enumerate(I2)}

    def r2(i: int, j: int) -> np.ndarray:
        return s2[:, ix2[(min(i, j), max(i, j))]]
    m2 = np.stack([s2[:, n] - vol * cen[:, i] * cen[:, j] for n, (i, j) in enumerate(I2)], 1)
    m3 = np.stack([s3[:, n] - cen[:, i] * r2(j, k) - cen[:, j] * r2(i, k) - cen[:, k] * r2(i, j)
                   + 2 * vol * cen[:, i] * cen[:, j] * cen[:, k] for n, (i, j, k) in enumerate(I3)], 1)
    return vol, cen, m2, m3


def far_corner(cen: np.ndarray, lo: np.ndarray, hi: np.ndarray) -> np.ndarray:
    """Largest distance from cen to the box [lo, hi]."""
    return np.linalg.norm(np.maximum(np.abs(lo - cen), np.abs(hi - cen)), axis = 1)


# ------------------------------------------------------------------ elements of the finest level
class Elements:
    """The finest level as leaves: boundary cells (with every representation's phase) and interior
    blocks (octree or sweep boxes, with their closed form and series phases)."""

    def __init__(self, shape: str, interior_kind: str = "octree") -> None:
        k = 3
        dat = LD.load(shape, k)
        reps, st = dat["reps"], dat["stats"]
        s = dat["s"]
        col = {n: s[:, i] for i, n in enumerate(st)}
        ce = cells(shape, k)
        self.h = float(ce[0, 3])
        nb = len(ce)
        self.nb = nb
        self.lo = np.vstack([np.column_stack([col["lo_x"], col["lo_y"], col["lo_z"]])])
        self.hi = np.column_stack([col["hi_x"], col["hi_y"], col["hi_z"]])
        self.cell_lo = ce[:, :3]
        self.vol = col["vol"]
        self.cen = np.column_stack([col["cx"], col["cy"], col["cz"]])
        self.m2 = np.column_stack([col[n] for n in ("i_xx", "i_yy", "i_zz", "i_xy", "i_xz", "i_yz")])
        self.m3 = np.column_stack([col[n] for n in ("t_xxx", "t_yyy", "t_zzz", "t_xxy", "t_xxz", "t_xyy", "t_yyz", "t_xzz", "t_yzz", "t_xyz")])
        self.symdiff = np.maximum(col["f8_symdiff_over_h3"], 0.0) * self.h**3
        self.tris = col["triangles"]
        c = dat["c"]
        self.ph = {r: c[:, reps.index(r), :] for r in ("exact", "second", "third", "mof_fill8_residual_common")}
        b, bp = box_phases(shape, interior_kind)
        self.box_lo, self.box_hi = b[:, :3], b[:, 3:]
        self.box_ph = bp
        bv = np.prod(b[:, 3:] - b[:, :3], axis = 1)
        bc = 0.5 * (b[:, :3] + b[:, 3:])
        side = b[:, 3:] - b[:, :3]
        bm2 = np.zeros((len(b), 6))
        bm2[:, :3] = bv[:, None] * side**2 / 12
        # All leaves together: boundary first, then boxes.
        self.n = nb + len(b)
        self.all_vol = np.concatenate([self.vol, bv])
        self.all_cen = np.vstack([self.cen, bc])
        self.all_m2 = np.vstack([self.m2, bm2])
        self.all_m3 = np.vstack([self.m3, np.zeros((len(b), 10))])
        self.all_lo = np.vstack([self.lo, b[:, :3]])
        self.all_hi = np.vstack([self.hi, b[:, 3:]])
        self.all_exact = np.vstack([self.ph["exact"], bp[:, :, 0]])
        self.all_raw = raw(self.all_vol, self.all_cen, self.all_m2, self.all_m3)
        self.is_box = np.arange(self.n) >= nb


# ------------------------------------------------------------------ Part 2: adaptive refinement
def adaptive(shape: str) -> dict[str, object]:
    ns = kernel_ns(shape)
    tw_pts, tw_w, nt = towers(shape)
    prep = C.load_json(f"prep-{shape}.json")
    origin = np.array(prep["origin"])
    lv = []
    for k in range(4):
        dat = LD.load(shape, k)
        reps, st = dat["reps"], dat["stats"]
        c, s = dat["c"], dat["s"]
        col = {n: s[:, i] for i, n in enumerate(st)}
        ce = cells(shape, k)
        h = C.PITCHES[shape][k]
        ix = ijk(ce[:, :3], h, origin)
        icen = interior(shape, k)
        iix = ijk(icen - h / 2, h, origin)
        lv.append({"h": h, "ix": ix, "iix": iix, "c": c, "reps": reps, "tris": col["triangles"],
                   "wv": col["welded_vertices"], "wf": col["welded_faces"], "area": surface_area(shape, k)})
    atot = lv[-1]["area"].sum()
    # Parent links: boundary cell of level k+1 -> boundary cell of level k; interior cells of level
    # k+1 counted under their (boundary) parent.
    for k in range(3):
        key = {tuple(x): i for i, x in enumerate(lv[k]["ix"])}
        lv[k + 1]["parent"] = np.array([key[tuple(x)] for x in (lv[k + 1]["ix"] >> 1)])
        par = [key.get(tuple(x), -1) for x in (lv[k + 1]["iix"] >> 1)]
        lv[k]["full_children"] = np.bincount(np.array([p for p in par if p >= 0], dtype = int), minlength = len(lv[k]["ix"]))
    lv[3]["full_children"] = np.zeros(len(lv[3]["ix"]), dtype = int)
    ns_pst = ns["point_second_third"]
    menu = [("fill", ns["point"], 4.0), ("centroid", ns["point"], 16.0), ("second", ns["point_second"], 40.0),
            ("third", ns_pst, 80.0), ("mof_fill8_residual_common", ns["plic_prebuilt"] + ns_pst, 82.0)]
    ns_tri = ns["soup_triangle"]
    ns_block = ns["box_series_l6"]
    out: dict[str, object] = {}
    for t, dist in enumerate(C.DISTANCES):
        for budget in BUDGETS:
            # Bottom-up: cheapest acceptable representation, or refine.
            best: list[np.ndarray] = [None] * 4  # type: ignore[list-item]
            choice: list[np.ndarray] = [None] * 4  # type: ignore[list-item]
            for k in range(3, -1, -1):
                L = lv[k]
                n = len(L["ix"])
                ex = L["c"][:, L["reps"].index("exact"), t]
                share = budget * L["area"] / atot
                cost = np.full(n, np.inf)
                ch = np.full(n, -1)
                for m, (rep, nsr, _) in enumerate(menu):
                    err = np.abs(L["c"][:, L["reps"].index(rep), t] - ex)
                    ok = (err <= share) & (nsr < cost)
                    cost[ok] = nsr
                    ch[ok] = m
                exact_cost = L["tris"] * ns_tri
                ok = exact_cost < cost
                cost[ok], ch[ok] = exact_cost[ok], len(menu)
                if k < 3:
                    nxt = lv[k + 1]
                    sub = np.bincount(nxt["parent"], best[k + 1], minlength = n) + L["full_children"] * ns_block
                    ok = sub < cost
                    cost[ok], ch[ok] = sub[ok], len(menu) + 1
                best[k], choice[k] = cost, ch
            # Walk down from level 0 and collect the leaves.
            take = [np.ones(len(lv[0]["ix"]), dtype = bool)]
            counts: dict[str, int] = {}
            mem = 0.0
            err = np.zeros(nt)
            blocks = 0
            for k in range(4):
                L = lv[k]
                tk = take[k]
                ch = choice[k]
                ex = L["c"][:, L["reps"].index("exact"), :]
                for m, (rep, _, by) in enumerate(menu):
                    sel = tk & (ch == m)
                    if sel.any():
                        counts[f"L{k} {rep}"] = int(sel.sum())
                        mem += by * sel.sum()
                        err += (L["c"][sel, L["reps"].index(rep), :] - ex[sel]).sum(axis = 0)
                sel = tk & (ch == len(menu))
                if sel.any():
                    counts[f"L{k} exact"] = int(sel.sum())
                    mem += float((L["wv"][sel] * 12 + L["wf"][sel] * 6).sum())
                ref = tk & (ch == len(menu) + 1)
                blocks += int(L["full_children"][ref].sum())
                if k < 3:
                    take.append(ref[lv[k + 1]["parent"]])
            out[f"{dist}m/{budget:g}"] = {"cost_ns_per_point": float(best[0].sum()), "memory_bytes": mem,
                                          "leaves": counts, "new_full_blocks": blocks,
                                          "error_at_towers": err.tolist(), "design_tower": t}
    # Baselines: the finest level as exact pieces; the whole mesh.
    f = lv[3]
    out["uniform_finest_exact"] = {"cost_ns_per_point": float((f["tris"] * ns_tri).sum()),
                                   "memory_bytes": float((f["wv"] * 12 + f["wf"] * 6).sum()),
                                   "pieces": len(f["tris"]), "piece_triangles": float(f["tris"].sum())}
    out["whole_mesh"] = {"cost_ns_per_point": prep["triangles"] * ns_tri, "memory_bytes": prep["triangles"] * 36.0,
                         "triangles": prep["triangles"]}
    return out


# ------------------------------------------------------------------ Part 3 and 5: walks
def build_octree(el: Elements, origin: np.ndarray) -> dict[str, np.ndarray]:
    """Implicit octree over the leaves: a leaf sits at the level of its size (a cell at 0, a block
    of 2^q cells at q); parents up to a single root. Returns arrays per node."""
    h = el.h
    size = np.rint((el.all_hi - el.all_lo).max(axis = 1) / h).astype(int)
    size[: el.nb] = 1
    q_leaf = np.rint(np.log2(size)).astype(int)
    base = np.rint((np.vstack([el.cell_lo, el.box_lo]) - origin) / h).astype(np.int64)
    # Shift to non-negative indices on a 2^12 grid (keeps the alignment; negative keys would halve
    # towards -1 and never meet the positive ones at a single root).
    shift = (base.min(axis = 0) >> 12) << 12
    base = base - shift
    key = base >> q_leaf[:, None]
    nodes: dict[tuple[int, int, int, int], int] = {}
    par_list: list[int] = []
    q_list: list[int] = []
    k_list: list[tuple[int, int, int]] = []

    def node(q: int, kk: tuple[int, int, int]) -> int:
        i = nodes.get((q, *kk))
        if i is None:
            i = len(par_list)
            nodes[(q, *kk)] = i
            par_list.append(-1)
            q_list.append(q)
            k_list.append(kk)
        return i
    leaf_node = np.array([node(int(q_leaf[j]), (int(key[j, 0]), int(key[j, 1]), int(key[j, 2]))) for j in range(el.n)])
    q = 0
    while True:
        level = [i for i in range(len(par_list)) if q_list[i] == q]
        above = [i for i in range(len(par_list)) if q_list[i] > q]
        if len(level) + len(above) <= 1 and not above:
            break
        if len(level) == 1 and not above:
            break
        for i in level:
            kk = k_list[i]
            par_list[i] = node(q + 1, (kk[0] >> 1, kk[1] >> 1, kk[2] >> 1))
        q += 1
    qa = np.array(q_list)
    kk = np.array(k_list)
    lo = origin + (kk * (1 << qa)[:, None] + shift) * h
    hi = lo + (h * (1 << qa))[:, None]
    return {"parent": np.array(par_list), "leaf_node": leaf_node, "geo_lo": lo, "geo_hi": hi, "q": qa}


def sah_bvh(el: Elements, bins: int = 16) -> dict[str, np.ndarray]:
    """Binned SAH BVH (Wald 2007's binning) over the leaves' boxes, one leaf per element."""
    lo, hi = el.all_lo, el.all_hi
    cen = 0.5 * (lo + hi)
    parent = []
    leaf_node = np.zeros(el.n, dtype = int)

    def area(a: np.ndarray, b: np.ndarray) -> float:
        d = b - a
        return float(d[0] * d[1] + d[1] * d[2] + d[2] * d[0])
    stack = [(np.arange(el.n), -1)]
    while stack:
        idx, par = stack.pop()
        me = len(parent)
        parent.append(par)
        if len(idx) == 1:
            leaf_node[idx[0]] = me
            continue
        c = cen[idx]
        cl, ch = c.min(axis = 0), c.max(axis = 0)
        ax = int(np.argmax(ch - cl))
        if ch[ax] - cl[ax] <= 0:
            split = [idx[: len(idx) // 2], idx[len(idx) // 2:]]
        else:
            b = np.minimum(((c[:, ax] - cl[ax]) / (ch[ax] - cl[ax]) * bins).astype(int), bins - 1)
            blo = np.full((bins, 3), np.inf)
            bhi = np.full((bins, 3), -np.inf)
            cnt = np.bincount(b, minlength = bins)
            for a in range(3):
                np.minimum.at(blo[:, a], b, lo[idx, a])
                np.maximum.at(bhi[:, a], b, hi[idx, a])
            best, at = np.inf, 1
            for s in range(1, bins):
                nl, nr = cnt[:s].sum(), cnt[s:].sum()
                if nl == 0 or nr == 0:
                    continue
                cost = nl * area(blo[:s].min(0), bhi[:s].max(0)) + nr * area(blo[s:].min(0), bhi[s:].max(0))
                if cost < best:
                    best, at = cost, s
            if best == np.inf:
                split = [idx[: len(idx) // 2], idx[len(idx) // 2:]]
            else:
                split = [idx[b < at], idx[b >= at]]
        for part in split:
            stack.append((part, me))
    return {"parent": np.array(parent), "leaf_node": leaf_node}


def aggregate(el: Elements, tr: dict[str, np.ndarray]) -> dict[str, np.ndarray]:
    """Per node: raw moments, exact phase per tower, tight box, leaf count; children lists."""
    par = tr["parent"]
    nn = len(par)
    depth = np.zeros(nn, dtype = int)
    order = np.argsort(par)  # not needed for correctness; depth by iteration below
    for _ in range(400):
        d2 = np.where(par >= 0, depth[np.maximum(par, 0)] + 1, 0)
        if np.array_equal(d2, depth):
            break
        depth = d2
    r = np.zeros((nn, 20))
    ex = np.zeros((nn, el.all_exact.shape[1]))
    tlo = np.full((nn, 3), np.inf)
    thi = np.full((nn, 3), -np.inf)
    ln = tr["leaf_node"]
    r[ln] += el.all_raw
    ex[ln] += el.all_exact
    tlo[ln] = el.all_lo
    thi[ln] = el.all_hi
    for d in range(depth.max(), 0, -1):
        sel = np.nonzero(depth == d)[0]
        p = par[sel]
        np.add.at(r, p, r[sel])
        np.add.at(ex, p, ex[sel])
        for a in range(3):
            np.minimum.at(tlo[:, a], p, tlo[sel, a])
            np.maximum.at(thi[:, a], p, thi[sel, a])
    children = [[] for _ in range(nn)]
    for i in range(nn):
        if par[i] >= 0:
            children[par[i]].append(i)
    leaf_of = np.full(nn, -1)
    leaf_of[ln] = np.arange(el.n)
    del order
    return {"raw": r, "exact": ex, "tlo": tlo, "thi": thi, "children": children, "leaf_of": leaf_of,
            "root": int(np.nonzero(par < 0)[0][0])}


def walk(el: Elements, agg: dict[str, np.ndarray], t: int, budget: float, pts: np.ndarray, w: np.ndarray,
         ns: dict[str, float], radius: str, geo: tuple[np.ndarray, np.ndarray] | None = None,
         extra: int = 0) -> dict[str, object]:
    """The error-bounded walk for tower t. `radius`: "tight" (b from the contents' box) or
    "geometric" (b from the node's cube, geo). `extra` adds to every bound's exponent (the control)."""
    vol, cen, m2, m3 = central(agg["raw"])
    mtot = vol[agg["root"]]
    if radius == "tight":
        b = far_corner(cen, agg["tlo"], agg["thi"])
    else:
        assert geo is not None
        b = far_corner(cen, geo[0], geo[1])
    ns_tri, ns_pst = ns["soup_triangle"], ns["point_second_third"]
    frontier = np.array([agg["root"]])
    visits = 0
    acc_nodes: list[tuple[np.ndarray, int]] = []  # (nodes, order) accepted as multipoles
    cost = 0.0
    err = 0.0
    bsum = 0.0
    counts = {"node p2": 0, "node p3": 0, "cell second": 0, "cell third": 0, "cell plic": 0, "cell exact": 0,
              "box l2": 0, "box l4": 0, "box l6": 0, "box exact": 0}
    worst_ratio = 0.0
    viol = 0
    while len(frontier):
        visits += len(frontier)
        share = budget * vol[frontier] / mtot
        leaf = agg["leaf_of"][frontier]
        is_leaf = leaf >= 0
        nxt = []
        # Inner nodes: multipole p = 2 or 3, else open.
        inner = frontier[~is_leaf]
        if len(inner):
            b2 = bound(vol[inner], cen[inner], b[inner], 2 + extra, pts, w)
            b3 = bound(vol[inner], cen[inner], b[inner], 3 + extra, pts, w)
            s = share[~is_leaf]
            for p, bb, sel in ((2, b2, b2 <= s), (3, b3, (b2 > s) & (b3 <= s))):
                if sel.any():
                    nodes = inner[sel]
                    ap = mp_phase(vol[nodes], cen[nodes], m2[nodes], m3[nodes], p, pts, w)
                    e = ap - agg["exact"][nodes, t]
                    err += e.sum()
                    bsum += bb[sel].sum()
                    sl = slack(vol[nodes], cen[nodes], b[nodes], pts, w)
                    worst_ratio = max(worst_ratio, float((np.abs(e) / (bb[sel] + sl)).max()))
                    viol += int((np.abs(e) > bb[sel] + sl).sum())
                    cost += sel.sum() * (ns["point_second"] if p == 2 else ns_pst)
                    counts[f"node p{p}"] += int(sel.sum())
            opened = inner[(b2 > s) & (b3 > s)]
            for i in opened:
                nxt.extend(agg["children"][i])
        # Leaves.
        lf = frontier[is_leaf]
        j = leaf[is_leaf]
        s = share[is_leaf]
        cellm = j < el.nb
        if cellm.any():
            jc, sc, nc = j[cellm], s[cellm], lf[cellm]
            bpiece = far_corner(el.cen[jc], el.lo[jc], el.hi[jc])
            b2 = bound(el.vol[jc], el.cen[jc], bpiece, 2 + extra, pts, w)
            b3 = bound(el.vol[jc], el.cen[jc], bpiece, 3 + extra, pts, w)
            ccen = el.cell_lo[jc] + el.h / 2
            bp = bound(el.symdiff[jc], ccen, np.full(len(jc), H3 * el.h), 3 + extra, pts, w)
            done = np.zeros(len(jc), dtype = bool)
            for name, bb, rep, nsr in (("cell second", b2, "second", ns["point_second"]), ("cell third", b3, "third", ns_pst),
                                       ("cell plic", bp, "mof_fill8_residual_common", ns["plic_prebuilt"] + ns_pst)):
                sel = (~done) & (bb <= sc)
                if sel.any():
                    e = el.ph[rep][jc[sel], t] - el.ph["exact"][jc[sel], t]
                    err += e.sum()
                    bsum += bb[sel].sum()
                    sl = slack(el.vol[jc[sel]], el.cen[jc[sel]], el.h, pts, w)
                    worst_ratio = max(worst_ratio, float((np.abs(e) / (bb[sel] + sl)).max()))
                    viol += int((np.abs(e) > bb[sel] + sl).sum())
                    cost += sel.sum() * nsr
                    counts[name] += int(sel.sum())
                    done |= sel
            ex = ~done
            counts["cell exact"] += int(ex.sum())
            cost += float((el.tris[jc[ex]] * ns_tri).sum())
        boxm = ~cellm
        if boxm.any():
            jb, sb_ = j[boxm] - el.nb, s[boxm]
            bc = 0.5 * (el.box_lo[jb] + el.box_hi[jb])
            bb0 = np.linalg.norm(0.5 * (el.box_hi[jb] - el.box_lo[jb]), axis = 1)
            bv = np.prod(el.box_hi[jb] - el.box_lo[jb], axis = 1)
            done = np.zeros(len(jb), dtype = bool)
            for name, l, col, nsr in (("box l2", 2, 2, ns["box_series_l2"]), ("box l4", 4, 3, ns["box_series_l4"]),
                                      ("box l6", 6, 4, ns["box_series_l6"])):
                bb = bound(bv, bc, bb0, l + 1 + extra, pts, w)
                sel = (~done) & (bb <= sb_)
                if sel.any():
                    e = el.box_ph[jb[sel], t, col] - el.box_ph[jb[sel], t, 0]
                    err += e.sum()
                    bsum += bb[sel].sum()
                    sl = slack(bv[sel], bc[sel], bb0[sel], pts, w)
                    worst_ratio = max(worst_ratio, float((np.abs(e) / (bb[sel] + sl)).max()))
                    viol += int((np.abs(e) > bb[sel] + sl).sum())
                    cost += sel.sum() * nsr
                    counts[name] += int(sel.sum())
                    done |= sel
            counts["box exact"] += int((~done).sum())
            cost += (~done).sum() * ns["cube_closed_form_12_triangles"]
        frontier = np.array(nxt, dtype = int)
    return {"visits": visits, "cost_ns_per_point": cost, "error": float(err), "bound_sum": float(bsum),
            "accepted": counts, "worst_error_over_bound": worst_ratio, "nodes_over_bound": viol}


def walks(shape: str) -> dict[str, object]:
    ns = kernel_ns(shape)
    tw_pts, tw_w, nt = towers(shape)
    prep = C.load_json(f"prep-{shape}.json")
    origin = np.array(prep["origin"])
    out: dict[str, object] = {}
    for kind in ("octree", "sweep"):
        el = Elements(shape, kind)
        trees = {"bvh": sah_bvh(el)}
        if kind == "octree":
            trees["octree"] = build_octree(el, origin)
        for tname, tr in trees.items():
            agg = aggregate(el, tr)
            nnodes = len(tr["parent"])
            variants = [("tight", None)]
            if tname == "octree":
                variants.append(("geometric", (tr["geo_lo"], tr["geo_hi"])))
            for radius, geo in variants:
                for t, dist in enumerate(C.DISTANCES):
                    for budget in BUDGETS:
                        r = walk(el, agg, t, budget, tw_pts[t], tw_w[t], ns, radius, geo)
                        r["nodes"] = nnodes
                        out[f"{kind}/{tname}/{radius}/{dist}m/{budget:g}"] = r
                    if t == 1:  # control: one order too many in every bound, at 1.5 m
                        r = walk(el, agg, t, 1e-8, tw_pts[t], tw_w[t], ns, radius, geo, extra = 1)
                        out[f"{kind}/{tname}/{radius}/{dist}m/1e-08/control"] = r
            print(shape, kind, tname, "done", flush = True)
    return out


# ------------------------------------------------------------------ Part 4: free boxes vs blocks
def interior_switch(shape: str) -> dict[str, object]:
    """The interior alone, read with an error-bounded series switch (budget by volume): for octree
    blocks and sweep boxes, with the series allowed to l = 2 (the earlier study), 4 or 6."""
    ns = kernel_ns(shape)
    tw_pts, tw_w, nt = towers(shape)
    out: dict[str, object] = {}
    for kind in ("octree", "sweep"):
        b, ph = box_phases(shape, kind)
        lo, hi = b[:, :3], b[:, 3:]
        bc, bb0 = 0.5 * (lo + hi), np.linalg.norm(0.5 * (hi - lo), axis = 1)
        bv = np.prod(hi - lo, axis = 1)
        aspect = (hi - lo).max(axis = 1) / (hi - lo).min(axis = 1)
        for t, dist in enumerate(C.DISTANCES):
            bnd = {l: bound(bv, bc, bb0, l + 1, tw_pts[t], tw_w[t]) for l in (2, 4, 6)}
            for budget in BUDGETS:
                share = budget * bv / bv.sum()
                for lmax in (2, 4, 6):
                    done = np.zeros(len(b), dtype = bool)
                    cost, err = 0.0, 0.0
                    used = {}
                    for l, col in ((2, 2), (4, 3), (6, 4)):
                        if l > lmax:
                            break
                        sel = (~done) & (bnd[l] <= share)
                        cost += sel.sum() * ns[f"box_series_l{l}"]
                        err += (ph[sel, t, col] - ph[sel, t, 0]).sum()
                        used[f"l{l}"] = int(sel.sum())
                        done |= sel
                    used["closed"] = int((~done).sum())
                    cost += used["closed"] * ns["cube_closed_form_12_triangles"]
                    out[f"{kind}/{dist}m/{budget:g}/l{lmax}"] = {"boxes": len(b), "cost_ns_per_point": cost,
                                                                  "error": float(err), "used": used}
        out[f"{kind}/aspect"] = {"median": float(np.median(aspect)), "max": float(aspect.max())}
        # Series accuracy by itself: worst |series − closed| per tower and order.
        out[f"{kind}/series_error"] = {f"{d}m": {f"l{l}": float(np.abs(ph[:, t, col] - ph[:, t, 0]).sum())
                                                 for l, col in ((0, 1), (2, 2), (4, 3), (6, 4))}
                                       for t, d in enumerate(C.DISTANCES)}
    return out


def run(shape: str) -> None:
    res = {"adaptive": adaptive(shape)}
    print(shape, "adaptive done", flush = True)
    res["interior"] = interior_switch(shape)
    res["walks"] = walks(shape)
    C.save_json(f"tree-{shape}.json", res)


if __name__ == "__main__":
    cmd = sys.argv[1]
    names = sys.argv[2:] or C.SHAPES
    for s in names:
        {"boxes": make_boxes, "run": run}[cmd](s)
