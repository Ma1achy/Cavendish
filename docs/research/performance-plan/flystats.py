"""Median and 99th percentile of each column of a flythrough trace."""
import csv
import math
import sys

for path in sys.argv[1:]:
    rows = [r for r in csv.DictReader(l for l in open(path) if not l.startswith("#"))]
    print(f"== {path.rsplit(chr(92), 1)[-1]} ({len(rows)} frames)")
    for col in ["ms", "poll_ms", "draw_ms", "mesh_ms", "encode_ms", "gpu_cull_ms", "gpu_bodies_ms", "gpu_chunks_ms", "gpu_bricks_ms", "meshes", "bricks_on_gpu", "vertices", "marched", "resident_mb"]:
        v = sorted(float(r[col]) for r in rows if r[col] not in ("NaN", "nan"))
        if not v:
            continue
        at = lambda q: v[min(len(v) - 1, round((len(v) - 1) * q))]
        print(f"  {col:14} median {at(0.5):9.3f}  p99 {at(0.99):9.3f}  max {v[-1]:9.3f}")
