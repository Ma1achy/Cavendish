"""Turn results/*.txt into the markdown tables used in the note (bytes per occupied finest cell)."""

from __future__ import annotations

import sys
from pathlib import Path


def load(path: Path) -> dict[str, float]:
    out: dict[str, float] = {}
    for line in path.read_text().splitlines():
        parts = line.split()
        if line.startswith("#") or len(parts) != 2 or parts[0].startswith(("walk", "dag")):
            continue
        out[parts[0]] = float(parts[1])
    return out


def main() -> None:
    names = sys.argv[1:]
    rows = {
        "linear octree, topology only":              lambda r: r["octree_topology_B"],
        "linear octree + f32 mass sidecar":          lambda r: r["octree_topology_B"] + r["octree_values_B"],
        "  + moments, float-float mass, f32 rest":   lambda r: r["octree_topology_B"] + r["octree_values_B"] + r["leaves"] * 0 + r["internal"] * 88,
        "64-tree, topology only (12 B/node)":        lambda r: r["t64_topology_B"],
        "64-tree + f32 slot sidecar":                lambda r: r["t64_topology_B"] + r["t64_values_B"],
        "brickmap 8^3, 1 B palette":                 lambda r: r["brick8_B_1B"],
        "brickmap 8^3, 4 B per cell":                lambda r: r["brick8_B_4B"],
        "chunks 32^3, 1 B palette":                  lambda r: r["chunk32_B_1B"],
        "VDB 5-4-3 (dense node tables, estimate)":   lambda r: r["vdb_B_approx"],
    }
    data = {n: load(Path("results") / f"{n}.txt") for n in names}
    print("| structure | " + " | ".join(f"{n}: MB (B/cell)" for n in names) + " |")
    print("|---|" + "---|" * len(names))
    for label, fn in rows.items():
        cells = []
        for n in names:
            r = data[n]
            b = fn(r)
            cells.append(f"{b / 1e6:,.1f} ({b / r['occupied']:.4f})")
        print(f"| {label} | " + " | ".join(cells) + " |")
    print("| occupied cells / leaves / internal | " + " | ".join(
        f"{data[n]['occupied']:.3g} / {data[n]['leaves']:.3g} / {data[n]['internal']:.3g}" for n in names) + " |")


main()
