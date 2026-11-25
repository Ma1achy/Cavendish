"""Tables from skin_bench.py's results: phase error by representation and pitch, and convergence orders.

    python tables.py results/sphere.json results/cube.json ...

For each body, representation and pitch: the phase error k_eff T² |Δg_z error| (rad, ρ = 2700 kg/m³),
worst over the three directions, RMS over the lattice placements; at a near distance (1.5 m) and a far
one (10 m). The order is the least-squares slope of log error against log h over the pitches.
"""

from __future__ import annotations

import json
import math
import sys

import numpy as np

K_EFF = 2.0 * math.pi * 1000.0 / 698e-9
T = 0.73
DISTANCES = (1.0, 1.5, 3.0, 5.0, 10.0, 20.0)
NDIR = 3
REPS = ("zero_one_raw", "zero_one", "ss4", "frac", "frac_cen", "frac_pt", "frac_cen_q")


def phase(values: list[float], d_index: int) -> float:
    v = np.abs(np.array(values[d_index * NDIR : (d_index + 1) * NDIR]))
    return float(K_EFF * T * T * v.max())


def summary(paths: list[str], dist: float) -> None:
    """One table: phase error at the finest pitch (and the order) for every body and representation."""
    di = DISTANCES.index(dist)
    bodies = [json.load(open(p)) for p in paths]
    names = [b[0]["body"] for b in bodies]
    print(f"\nD = {dist} m: phase error (rad) at h = 1/64 m, RMS over placements, worst direction; "
          "(order over h = 1/8 to 1/64)")
    print("| representation | " + " | ".join(names) + " |")
    print("|---|" + "---|" * len(names))
    sig = [np.mean([phase(r["signal_dgz"], di) for r in runs]) for runs in bodies]
    print("| *signal* | " + " | ".join(f"*{s:.3g}*" for s in sig) + " |")
    for rep in REPS:
        cells = []
        for runs in bodies:
            ns = sorted({r["n"] for r in runs})
            errs = [math.sqrt(np.mean(np.square([phase(r[rep]["dgz"], di) for r in runs if r["n"] == n])))
                    for n in ns]
            order = np.polyfit(np.log([1.0 / n for n in ns]), np.log(errs), 1)[0]
            cells.append(f"{errs[-1]:.1e} ({order:.1f})")
        print(f"| {rep} | " + " | ".join(cells) + " |")


def main() -> None:
    if sys.argv[1] == "summary":
        for dist in (1.5, 10.0):
            summary(sys.argv[2:], dist)
        return
    for path in sys.argv[1:]:
        runs = json.load(open(path))
        name = runs[0]["body"]
        ns = sorted({r["n"] for r in runs})
        for dist in (1.5, 10.0):
            di = DISTANCES.index(dist)
            sig = np.mean([phase(r["signal_dgz"], di) for r in runs])
            cells = {n: int(np.mean([r["boundary_cells"] for r in runs if r["n"] == n])) for n in ns}
            print(f"\n{name}, D = {dist} m from the centre, signal {sig:.3g} rad; boundary cells "
                  + ", ".join(f"{cells[n]}" for n in ns))
            print("| representation | " + " | ".join(f"h = 1/{n} m" for n in ns) + " | order |")
            print("|---|" + "---|" * (len(ns) + 1))
            for rep in REPS:
                errs = []
                for n in ns:
                    e = [phase(r[rep]["dgz"], di) for r in runs if r["n"] == n]
                    errs.append(math.sqrt(np.mean(np.square(e))))
                h = np.log([1.0 / n for n in ns])
                order = np.polyfit(h, np.log(errs), 1)[0]
                print(f"| {rep} | " + " | ".join(f"{e:.2e}" for e in errs) + f" | {order:.2f} |")


if __name__ == "__main__":
    main()
