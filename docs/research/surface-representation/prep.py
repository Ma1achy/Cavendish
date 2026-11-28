"""Voxelise the five shapes at four nested pitches and write what the Rust bench reads.

    python prep.py [shape ...]

Per shape, in `.work/<shape>/` (git-ignored; little-endian f64 throughout):
    mesh.f64        the closed mesh, 9 per triangle (m)
    towers.f64      per tower, rows (x, y, z, c): the flight read as Σ c·V/G (common.tower_points)
    L<k>/cells.f64  per boundary cell: lo corner (3), pitch, first piece triangle, piece triangles
    L<k>/tris.f64   the exact pieces' closed boundaries, 9 per triangle (m)
    L<k>/interior.f64  interior cell centres, 3 each (m)
and `data/prep-<shape>.json`: counts, volumes, tower sizes, voxelising time.

The lattices nest: every pitch uses the coarsest pitch's origin (h₀·GENERIC), so a cell at level
k + 1 lies in exactly one cell at level k.
"""

from __future__ import annotations

import sys
import time

import numpy as np

import common as C


def write(path: C.Path, arr: np.ndarray) -> None:
    path.parent.mkdir(parents = True, exist_ok = True)
    np.ascontiguousarray(arr, dtype = "<f8").tofile(path)


def prep(shape: str, towers_only: bool = False) -> None:
    mesh = C.L.mesh_by_name(shape)
    out = C.WORK / shape
    write(out / "mesh.f64", mesh.tris.reshape(-1, 9))
    tw = C.towers(mesh)
    write(out / "towers.f64", np.concatenate([np.column_stack([p, c]) for p, c in tw]))
    (out / "towers.count").write_text(str(len(tw)))
    if towers_only:
        return
    pitches = C.PITCHES[shape]
    origin = pitches[0] * C.L.GENERIC
    vol, _ = C.L.sb.volume_centroid(mesh.tris, np.zeros(3))
    info: dict[str, object] = {"shape": shape, "triangles": len(mesh.tris), "volume": float(vol),
                               "origin": origin.tolist(), "pitches": pitches,
                               "tower_points": [len(p) for p, _ in tw],
                               "tower_bases": C.L.tower(mesh, np.array(C.DISTANCES))[: len(C.DISTANCES)].tolist(),
                               "levels": []}
    for k, h in enumerate(pitches):
        t0 = time.time()
        v = C.L.voxelise(mesh, h, origin = origin)
        dt = time.time() - t0
        lo = v.base + v.ijk * h
        counts = np.bincount(v.piece_of, minlength = len(v.ijk))
        first = np.concatenate([[0], np.cumsum(counts)[:-1]])
        cells = np.column_stack([lo, np.full(len(lo), h), first, counts])
        d = out / f"L{k}"
        write(d / "cells.f64", cells)
        write(d / "tris.f64", v.piece_tris.reshape(-1, 9))
        write(d / "interior.f64", v.interior_centres)
        lev = {"level": k, "h": h, "boundary": len(v.ijk), "interior": len(v.interior),
               "piece_triangles": len(v.piece_tris), "seconds": dt, **v.stats}
        info["levels"].append(lev)  # type: ignore[union-attr]
        print(shape, lev, flush = True)
    C.save_json(f"prep-{shape}.json", info)


if __name__ == "__main__":
    args = sys.argv[1:]
    only = "--towers" in args
    for s in [a for a in args if a != "--towers"] or C.SHAPES:
        prep(s, only)
