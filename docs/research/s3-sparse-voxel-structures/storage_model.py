"""Storage and walk-cost accounting for sparse voxel structures on two synthetic scenes.

Builds a conservative top-down octree of an analytic scene (a sphere, and terrain with buildings),
merges uniform children bottom-up, then counts what each candidate structure would store and how
many nodes a bounded top-down walk touches. Nothing here is a GPU implementation: it is an
accounting model, with every byte size stated in ``BYTES`` so it can be argued with.

    python3 -I storage_model.py sphere 10        # N = 2**10 cells per axis (1.07e9 cells)
    python3 -I storage_model.py terrain 11 --dag # adds the DAG count (needs N <= 2**11)

Leaf values: a finest cell is Outside, Inside, or Partial (fractional occupancy in (0, 1)), which is
what Cavendish's voxeliser produces on a body's boundary (mass is fractional there).
"""

from __future__ import annotations

import argparse
import math
import time
from dataclasses import dataclass
from typing import Callable

import numpy as np
import numpy.typing as npt

I64 = npt.NDArray[np.int64]
F64 = npt.NDArray[np.float64]

OUTSIDE, INSIDE, MIXED = 0, 1, 2
MASK = (1 << 21) - 1


# ======================================= KEYS =======================================

def pack(i: I64, j: I64, k: I64) -> I64:
    """Pack integer cell coordinates into one sortable int64 (21 bits per axis)."""
    return (i << 42) | (j << 21) | k


def unpack(key: I64) -> tuple[I64, I64, I64]:
    return key >> 42, (key >> 21) & MASK, key & MASK


def parent(key: I64) -> I64:
    i, j, k = unpack(key)
    return pack(i >> 1, j >> 1, k >> 1)


# ======================================= SCENES =======================================

@dataclass(frozen = True)
class Scene:
    name:     str
    classify: Callable[[int, I64, I64, I64], npt.NDArray[np.int8]]   # level > 0, conservative
    cells:    Callable[[I64, I64, I64], F64]                         # fractional occupancy


def sphere(depth: int) -> Scene:
    n = float(1 << depth)
    c = n / 2
    r = 0.4 * n

    def classify(level: int, i: I64, j: I64, k: I64) -> npt.NDArray[np.int8]:
        s = n / (1 << level)
        d = np.stack([(i + 0.5) * s - c, (j + 0.5) * s - c, (k + 0.5) * s - c])
        a = np.abs(d)
        far = np.sqrt(((a + s / 2) ** 2).sum(0))
        near = np.sqrt((np.maximum(a - s / 2, 0.0) ** 2).sum(0))
        out = np.full(i.shape, MIXED, np.int8)
        out[far <= r] = INSIDE
        out[near >= r] = OUTSIDE
        return out

    def cells(i: I64, j: I64, k: I64) -> F64:
        d = np.sqrt((i + 0.5 - c) ** 2 + (j + 0.5 - c) ** 2 + (k + 0.5 - c) ** 2) - r
        return np.clip(0.5 - d, 0.0, 1.0)

    return Scene("sphere", classify, cells)


def terrain(depth: int, buildings: int = 48, seed: int = 7) -> Scene:
    """A height field with a rolling surface and random axis-aligned buildings on integer cells."""
    n = float(1 << depth)
    rng = np.random.default_rng(seed)
    # (amplitude, kx, ky) in units of n; every term is A sin(2 pi (kx x + ky y) / n + phase)
    terms = [(0.06, 1.4, 0.9, 0.3), (0.02, 7.0, 5.0, 1.1), (0.004, 19.0, 23.0, 2.0)]
    lip = sum(a * 2 * math.pi * math.hypot(kx, ky) for a, kx, ky, _ in terms)   # slope bound

    def height(x: F64, y: F64) -> F64:
        h = np.full(x.shape, 0.25 * n)
        for a, kx, ky, ph in terms:
            h = h + a * n * np.sin(2 * math.pi * (kx * x + ky * y) / n + ph)
        return h

    foot = rng.integers(max(depth * 2, 4), max(int(n / 14), 8), size = (buildings, 2))
    lo_xy = rng.integers(0, int(n) - foot.max(), size = (buildings, 2))
    tall = rng.integers(int(n / 60) + 2, int(n / 10) + 3, size = buildings)
    base = np.floor(height(lo_xy[:, 0] + foot[:, 0] / 2.0, lo_xy[:, 1] + foot[:, 1] / 2.0) - 2.0)
    blo = np.concatenate([lo_xy, base[:, None]], 1).astype(np.float64)
    bhi = blo + np.concatenate([foot, tall[:, None] + 4], 1)

    def classify(level: int, i: I64, j: I64, k: I64) -> npt.NDArray[np.int8]:
        s = n / (1 << level)
        lo = np.stack([i * s, j * s, k * s], 1).astype(np.float64)
        hi = lo + s
        cx, cy = lo[:, 0] + s / 2, lo[:, 1] + s / 2
        h = height(cx, cy)
        reach = lip * s * math.sqrt(0.5)
        inside = hi[:, 2] <= h - reach
        outside = lo[:, 2] >= h + reach
        for b in range(buildings):
            within = np.all((lo >= blo[b]) & (hi <= bhi[b]), 1)
            touch = np.all((lo < bhi[b]) & (hi > blo[b]), 1)
            inside |= within
            outside &= ~touch
        out = np.full(i.shape, MIXED, np.int8)
        out[inside] = INSIDE
        out[outside & ~inside] = OUTSIDE
        return out

    def cells(i: I64, j: I64, k: I64) -> F64:
        frac = np.clip(height(i + 0.5, j + 0.5) - k, 0.0, 1.0)
        for b in range(buildings):
            hit = np.all(np.stack([i, j, k], 1) >= blo[b], 1) & np.all(np.stack([i, j, k], 1) < bhi[b], 1)
            frac = np.maximum(frac, hit.astype(np.float64))
        return frac

    return Scene("terrain", classify, cells)


# ======================================= BUILD =======================================

@dataclass
class Tree:
    depth:   int
    inside:  list[I64]       # per level, keys of uniform-inside nodes (level depth = full cells)
    mixed:   list[I64]       # per level < depth, keys of non-uniform nodes
    partial: I64             # finest cells with fractional occupancy
    values:  F64             # their occupancy


def build(scene: Scene, depth: int, chunk: int = 400_000) -> Tree:
    offs = np.array([(a, b, c) for a in (0, 1) for b in (0, 1) for c in (0, 1)], np.int64)
    inside: list[list[I64]] = [[] for _ in range(depth + 1)]
    mixed: list[list[I64]] = [[] for _ in range(depth + 1)]
    part_k: list[I64] = []
    part_v: list[F64] = []
    fi = np.zeros(1, np.int64)
    fj = np.zeros(1, np.int64)
    fk = np.zeros(1, np.int64)
    mixed[0].append(pack(fi, fj, fk))
    for level in range(depth):
        ni: list[I64] = []
        nj: list[I64] = []
        nk: list[I64] = []
        for s in range(0, len(fi), chunk):
            ci = (2 * fi[s:s + chunk, None] + offs[None, :, 0]).ravel()
            cj = (2 * fj[s:s + chunk, None] + offs[None, :, 1]).ravel()
            ck = (2 * fk[s:s + chunk, None] + offs[None, :, 2]).ravel()
            if level + 1 < depth:
                st = scene.classify(level + 1, ci, cj, ck)
                m = st == INSIDE
                inside[level + 1].append(pack(ci[m], cj[m], ck[m]))
                m = st == MIXED
                ni.append(ci[m]); nj.append(cj[m]); nk.append(ck[m])
            else:
                v = scene.cells(ci, cj, ck)
                m = v >= 1.0
                inside[depth].append(pack(ci[m], cj[m], ck[m]))
                m = (v > 0.0) & (v < 1.0)
                part_k.append(pack(ci[m], cj[m], ck[m]))
                part_v.append(v[m])
        if level + 1 < depth:
            fi, fj, fk = np.concatenate(ni), np.concatenate(nj), np.concatenate(nk)
            mixed[level + 1].append(pack(fi, fj, fk))
    cat = lambda xs: np.concatenate(xs) if xs else np.zeros(0, np.int64)   # noqa: E731
    tree = Tree(depth, [cat(x) for x in inside], [cat(x) for x in mixed], cat(part_k),
                np.concatenate(part_v) if part_v else np.zeros(0))
    return merge(tree)


def merge(t: Tree) -> Tree:
    """Lossless merge: eight Inside children become one Inside node; prune empty Mixed nodes."""
    d = t.depth
    for level in range(d - 1, -1, -1):
        pk, cnt = np.unique(parent(t.inside[level + 1]), return_counts = True)
        full = pk[cnt == 8]
        keep = ~np.isin(parent(t.inside[level + 1]), full)
        t.inside[level + 1] = t.inside[level + 1][keep]
        t.mixed[level] = t.mixed[level][~np.isin(t.mixed[level], full)]
        t.inside[level] = np.concatenate([t.inside[level], full])
        below = t.partial if level + 1 == d else t.mixed[level + 1]
        occupied = np.unique(parent(np.concatenate([t.inside[level + 1], below])))
        t.mixed[level] = t.mixed[level][np.isin(t.mixed[level], occupied)]
    return t


# ======================================= ACCOUNTING =======================================

# Bytes, stated so they can be argued with. Topology and sidecar are separate on purpose.
BYTES = {
    "oct_leaf":      8,     # key u32 + level u8, padded
    "oct_node":      8,     # child_mask u8 + first_child u32 + flags u8, padded
    "side_cell":     4,     # f32 mass (or occupancy x density) per leaf
    "node64":        12,    # u64 child mask + u32 first_child
    "moments_f32":   80,    # mass, com(3), quad(6), oct(10) = 20 f32
    "moments_mono":  16,    # mass + com
    "brick_cell":    1,     # palette index; fractional occupancy would be extra (see side_cell)
    "hash_entry":    8,     # sparse top-level map: key u32 + brick index u32
}


def report(t: Tree, scene: str) -> dict[str, float]:
    d = t.depth
    n_in = [len(x) for x in t.inside]
    n_mx = [len(x) for x in t.mixed]
    leaves = sum(n_in) + len(t.partial)
    internal = sum(n_mx)
    volume = sum(c * 8.0 ** (d - lvl) for lvl, c in enumerate(n_in)) + float(len(t.partial))
    out: dict[str, float] = {"depth": d, "cells_total": float(8 ** d), "occupied": volume,
                             "leaves": leaves, "internal": internal}
    # linear octree, sidecars
    topo = leaves * BYTES["oct_leaf"] + internal * BYTES["oct_node"]
    out["octree_topology_B"] = topo
    out["octree_values_B"] = leaves * BYTES["side_cell"]
    out["octree_moments_f32_B"] = internal * BYTES["moments_f32"]
    out["octree_moments_mono_B"] = internal * BYTES["moments_mono"]
    # 64-tree (levels in pairs): a node exists for each Mixed octree node at an even level
    even = sum(n_mx[lvl] for lvl in range(0, d, 2))
    slots = sum(n_in[lvl] for lvl in range(2, d + 1, 2)) + 8 * sum(n_in[lvl] for lvl in range(1, d, 2)) \
        + len(t.partial)
    out["t64_nodes"] = even
    out["t64_topology_B"] = even * BYTES["node64"]
    out["t64_values_B"] = slots * BYTES["side_cell"]
    out["t64_moments_f32_B"] = even * BYTES["moments_f32"]
    # brickmap 8^3: full bricks where Mixed at level d-3; uniform bricks are index flags
    if d >= 3:
        full_bricks = n_mx[d - 3]
        solid = sum(n_in[lvl] * 8 ** (d - 3 - lvl) for lvl in range(0, d - 2))
        out["brick8_full"] = full_bricks
        out["brick8_solid_entries"] = solid
        out["brick8_B_1B"] = full_bricks * 512 * BYTES["brick_cell"] + (full_bricks + solid) * BYTES["hash_entry"]
        out["brick8_B_4B"] = full_bricks * 512 * BYTES["side_cell"] + (full_bricks + solid) * BYTES["hash_entry"]
    # chunk 32^3 with palette (the world volume): full chunks where Mixed at level d-5
    if d >= 5:
        full_chunks = n_mx[d - 5]
        solid = sum(n_in[lvl] * 8 ** (d - 5 - lvl) for lvl in range(0, d - 4))
        out["chunk32_full"] = full_chunks
        out["chunk32_solid_entries"] = solid
        out["chunk32_B_1B"] = full_chunks * 32768 + (full_chunks + solid) * BYTES["hash_entry"]
    # VDB 5-4-3: leaf 8^3, lower 16^3, upper 32^3 (node tables are dense; see note in the report)
    if d >= 3:
        leaf_nodes = n_mx[d - 3]
        lower = len(np.unique(parent_n(t.mixed[d - 3], 4)))
        upper = len(np.unique(parent_n(t.mixed[d - 3], 9)))
        vdb_leaf = leaf_nodes * (512 * 4 + 64 + 32)         # f32 values + value mask + header
        vdb_lower = lower * (4096 * 8 + 2 * 512 + 64)
        vdb_upper = upper * (32768 * 8 + 2 * 4096 + 64)
        out["vdb_leaf_nodes"] = leaf_nodes
        out["vdb_lower_nodes"] = lower
        out["vdb_upper_nodes"] = upper
        out["vdb_B_approx"] = vdb_leaf + vdb_lower + vdb_upper
    return out


def parent_n(key: I64, n: int) -> I64:
    i, j, k = unpack(key)
    return pack(i >> n, j >> n, k >> n)


def dag(t: Tree, fractional: bool) -> dict[str, float]:
    """Unique-subtree count by bottom-up hashing. ``fractional`` keeps partial cells distinct (8-bit)."""
    d = t.depth
    offs = np.array([(a, b, c) for a in (0, 1) for b in (0, 1) for c in (0, 1)], np.int64)
    FULL0 = 1                                   # id of the Full leaf value
    if fractional:
        q = np.clip(np.rint(t.values * 255).astype(np.int64), 1, 254) + 1     # ids 2..255
    else:
        q = np.where(t.values >= 0.5, FULL0, 0).astype(np.int64)
    cell_keys = np.concatenate([t.inside[d], t.partial])
    cell_ids = np.concatenate([np.full(len(t.inside[d]), FULL0, np.int64), q])
    order = np.argsort(cell_keys)
    keys_below, ids_below = cell_keys[order], cell_ids[order]
    next_id = 300
    full_id = {d: FULL0}
    total_nodes = 0
    total_bytes = 0.0
    for level in range(d - 1, -1, -1):
        full_id[level] = next_id
        next_id += 1
        mk = t.mixed[level]
        ik = t.inside[level]
        rows = np.zeros((len(mk), 8), np.int64)
        i, j, k = unpack(mk)
        for c in range(8):
            ck = pack(2 * i + offs[c, 0], 2 * j + offs[c, 1], 2 * k + offs[c, 2])
            pos = np.searchsorted(keys_below, ck)
            pos[pos >= len(keys_below)] = 0
            hit = keys_below[pos] == ck
            rows[:, c] = np.where(hit, ids_below[pos], 0)
        # a child that is an Inside node at level+1 has the Full id of that level
        uniq, inv = np.unique(rows, axis = 0, return_inverse = True)
        inv = inv.ravel()
        ids = next_id + inv
        next_id += len(uniq)
        nz = (uniq != 0).sum(1)
        if level == d - 1:
            total_bytes += len(uniq) * (1 + (nz.sum() / max(len(uniq), 1) if fractional else 0))
        else:
            total_bytes += float((1 + 4 * nz).sum())
        total_nodes += len(uniq)
        # ids of this level's nodes, for the next iteration up
        all_keys = np.concatenate([mk, ik])
        all_ids = np.concatenate([ids, np.full(len(ik), full_id[level], np.int64)])
        order = np.argsort(all_keys)
        keys_below, ids_below = all_keys[order], all_ids[order]
    total_nodes += d + 1
    total_bytes += (d + 1) * 33
    return {"dag_nodes": total_nodes, "dag_B": total_bytes}


# ======================================= WALK =======================================

def walk(t: Tree, theta: float, query: tuple[float, float, float]) -> dict[str, float]:
    """Top-down walk with accept-if size/distance < theta, octree and 64-tree granularity.

    ``tests`` is the number of child records read; ``accepted`` is the number of elements the
    kernel then evaluates (accepted internal nodes plus leaves reached). Distance is to the node
    centre; leaves (Inside nodes and finest cells) are always evaluated when reached.
    """
    d = t.depth
    assert d % 2 == 0, "the 64-tree model pairs levels: use an even depth"
    n = float(1 << d)
    q = np.array(query) * n

    def dist(key: I64, level: int) -> F64:
        i, j, k = unpack(key)
        s = n / (1 << level)
        c = np.stack([(i + 0.5) * s, (j + 0.5) * s, (k + 0.5) * s], 1)
        return np.sqrt(((c - q) ** 2).sum(1))

    res: dict[str, float] = {}
    # octree: opened[l] = Mixed nodes at l whose parent was opened and which fail the criterion
    opened = t.mixed[0]
    tests = 0.0
    accepted = 0.0
    for level in range(0, d):
        s = n / (1 << level)
        fail = (s / dist(opened, level)) >= theta
        acc_mask = ~fail
        accepted += float(acc_mask.sum())                  # accepted Mixed node: one multipole eval
        op = opened[fail]
        tests += 8.0 * len(op)
        # children: Inside leaves are evaluated when reached, Mixed children continue
        ins_c = np.isin(parent(t.inside[level + 1]), op) if level + 1 <= d else np.zeros(0, bool)
        accepted += float(ins_c.sum())
        if level + 1 < d:
            opened = t.mixed[level + 1][np.isin(parent(t.mixed[level + 1]), op)]
        else:
            accepted += float(np.isin(parent(t.partial), op).sum())
    res["oct_tests"] = tests
    res["oct_evals"] = accepted
    # 64-tree: criterion only at even levels; opening a node reads 64 children at level+2
    tests = 0.0
    accepted = 0.0
    opened = t.mixed[0]
    for level in range(0, d, 2):
        s = n / (1 << level)
        fail = (s / dist(opened, level)) >= theta
        accepted += float((~fail).sum())
        op = opened[fail]
        tests += 64.0 * len(op)
        lvl2 = min(level + 2, d)
        mid = level + 1
        anc2 = (lambda kk: parent(parent(kk)))
        # Inside leaves at level+1 and level+2 under an opened node are evaluated when reached
        accepted += float(np.isin(parent(t.inside[mid]), op).sum()) if mid <= d else 0.0
        accepted += float(np.isin(anc2(t.inside[lvl2]), op).sum()) if lvl2 > mid else 0.0
        if lvl2 < d:
            opened = t.mixed[lvl2][np.isin(anc2(t.mixed[lvl2]), op)]
        else:
            accepted += float(np.isin(anc2(t.partial), op).sum())
    res["t64_tests"] = tests
    res["t64_evals"] = accepted
    return res


# ======================================= MAIN =======================================

def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("scene", choices = ["sphere", "terrain"])
    ap.add_argument("depth", type = int)
    ap.add_argument("--dag", action = "store_true")
    ap.add_argument("--walk", action = "store_true")
    a = ap.parse_args()
    scene = sphere(a.depth) if a.scene == "sphere" else terrain(a.depth)
    t0 = time.time()
    tree = build(scene, a.depth)
    print(f"# {a.scene} depth {a.depth} (N = {1 << a.depth}); build {time.time() - t0:.1f} s")
    for key, value in report(tree, a.scene).items():
        print(f"{key:24s} {value:.6g}")
    if a.dag:
        for frac in (False, True):
            r = dag(tree, frac)
            print(f"dag_{'fractional' if frac else 'binary':10s} nodes {r['dag_nodes']:.6g}  B {r['dag_B']:.6g}")
    if a.walk:
        for theta in (0.5, 0.1, 0.02, 0.004):
            r = walk(tree, theta, (0.5, 0.5, 0.12) if a.scene == "terrain" else (0.5, 0.5, 0.98))
            print(f"walk theta {theta:<6} " + "  ".join(f"{k} {v:.6g}" for k, v in r.items()))


if __name__ == "__main__":
    main()
