"""Full resolution: the original scans (and near-cap decimations) as files for `cav` and for the
direct-sum timing, and the Python clipper's cost at full resolution.

    python fullres.py dump <name>          # .cache/full/<name>.stl (binary) and <name>.f64 (triangles)
    python fullres.py dump <name> closed   # the full-resolution mesh after `repair` (holes capped)
    python fullres.py dump <name> study    # the repaired (decimated) mesh the sweeps use
    python fullres.py dump <name> <target> # a quadric decimation to <target> triangles, welded
    python fullres.py clip <name> <n>      # time the Python voxeliser at size/h = n on the full mesh

Every file is in the study's frame (largest extent 1 m, `_raw`). `dump` writes the original download
as it is (welded at 8 decimals, degenerate triangles dropped: what an importer sees). Files go to
`.cache/full/` (ignored by git); the numbers are in the note's "Full resolution" section.
"""

from __future__ import annotations

import json
import sys
import time
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import shapes_lib as L  # noqa: E402

OUT = L.CACHE / "full"


def _welded(v: L.F64, f: L.I64) -> tuple[L.F64, L.I64]:
    q = np.round(v, 8)
    _, first, inv = np.unique(q, axis=0, return_index=True, return_inverse=True)
    v, f = v[first], inv.reshape(-1)[f]
    return v, f[(f[:, 0] != f[:, 1]) & (f[:, 1] != f[:, 2]) & (f[:, 2] != f[:, 0])]


def _write(tag: str, v: L.F64, f: L.I64) -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    t = v[f]
    n = np.cross(t[:, 1] - t[:, 0], t[:, 2] - t[:, 0])
    n /= np.maximum(np.linalg.norm(n, axis=1), 1e-300)[:, None]
    rec = np.zeros(len(f), dtype=[("n", "<f4", 3), ("v", "<f4", (3, 3)), ("a", "<u2")])
    rec["n"], rec["v"] = n, t
    with open(OUT / f"{tag}.stl", "wb") as fh:
        fh.write(b"Cavendish shapes study: full-resolution test mesh".ljust(80, b" "))
        fh.write(np.uint32(len(f)).tobytes())
        fh.write(rec.tobytes())
    t.astype("<f8").tofile(OUT / f"{tag}.f64")
    e = np.sort(np.concatenate([f[:, [0, 1]], f[:, [1, 2]], f[:, [2, 0]]]), axis=1)
    _, cnt = np.unique(e, axis=0, return_counts=True)
    info = {"triangles": int(len(f)), "vertices": int(len(v)), "open_edges": int((cnt == 1).sum()),
            "edges_in_more_than_two": int((cnt > 2).sum()), "stl_bytes": 84 + 50 * len(f)}
    (OUT / f"{tag}.json").write_text(json.dumps(info))
    print(tag, info, flush=True)


def dump(name: str, variant: str | None) -> None:
    up = L.FAMOUS[name].up
    if variant == "study":  # the repaired mesh the study uses (decimated where it is)
        m = L.famous_mesh(name)
        _write(f"{name}-study", m.verts, m.faces)
        return
    v, f, _, _ = L._raw(name, up, original=True)
    if variant is None:
        _write(name, *_welded(v, f))
    elif variant == "closed":
        rep = L.repair(name, up, ear=True, source=(v, f))  # the full-resolution download
        _write(f"{name}-closed", rep.mesh.verts, rep.mesh.faces)
    else:
        target = int(variant)
        rep = L.repair(name, up, source=L.decimated(name, v, f, target), ear=True)
        _write(f"{name}-{target}", rep.mesh.verts, rep.mesh.faces)


def clip(name: str, n_across: int) -> None:
    up = L.FAMOUS[name].up
    v, f, _, _ = L._raw(name, up, original=True)
    if name == "bunny":
        rep = L.repair(name, up)
        v, f = rep.mesh.verts, rep.mesh.faces
    else:
        v, f = _welded(v, f)
    mesh = L.Mesh(name, v, f, "")
    h = float((v.max(0) - v.min(0)).max()) / n_across
    t0 = time.perf_counter()
    vox = L.voxelise(mesh, h)
    s = time.perf_counter() - t0
    print(json.dumps({"name": name, "triangles": len(f), "n": n_across, "seconds": s, "boundary": len(vox.frac),
                      "interior": len(vox.interior), "piece_tris": len(vox.piece_tris),
                      "volume_check": vox.stats["volume_rel_error"]}), flush=True)


if __name__ == "__main__":
    if sys.argv[1] == "dump":
        dump(sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else None)
    else:
        clip(sys.argv[2], int(sys.argv[3]))
