"""The primitive each voxel cell is read as, for the same cell masses.

    python primitives.py     # primitives.png, primitives.md, primitives.json

Every cell carries the mass of its exact piece (interior cells ρh³, boundary cells ρ·fill·h³); only
the shape the read gives that mass changes:
    (1) point at the cell centre             (= the cube's series to order 2: a cube has no quadrupole)
    (2) point at the piece's centroid
    (3) uniform sphere of the cell's volume at the centre (radius 0.620 h; neighbours overlap)
    (4) uniform cube of side h at the centre (Nagy prism / polyhedral form; Cavendish's near kernel)
    (5) the cube's series to order 4 (monopole + l = 4 hexadecapole; Cavendish's far kernel)
    (6) Gaussian blob, σ = h/√12 (the cube's second moment), at the centre
    (7) trilinear density: cell values as samples of a continuous field (each cell a tent of
        half-width h), field = the cube's field averaged over a cube of offsets, 3×3×3 Gauss–Legendre
    (8) the exact clipped piece (interior cubes + pieces)
Reference: the polyhedral closed form on the mesh. The arm point is 0.5–100 cells from the surface.
Each primitive's cost is timed here, numpy, per (cell, point) pair.
"""

from __future__ import annotations

import json
import math
import sys
import time
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
from scipy.special import erf  # noqa: E402

sys.path.insert(0, str(Path(__file__).resolve().parent))
import shapes_lib as L  # noqa: E402

SHAPES = {"sphere": 0.125, "torus": 0.1, "suzanne": 0.1}
CELLS = np.array([0.5, 0.75, 1, 1.5, 2, 3, 5, 7, 10, 15, 20, 30, 50, 70, 100])
NAMES = ["(1) point at centre", "(2) point at centroid", "(3) sphere", "(4) cube (prism)",
         "(5) cube series l≤4", "(6) Gaussian σ=h/√12", "(7) trilinear density", "(8) exact piece"]
COLOURS = ["#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4", "#008300", "#6250d6", "#e34948"]


def g_spheres(m: np.ndarray, c: np.ndarray, a: float, pts: np.ndarray) -> np.ndarray:
    out = np.zeros((len(pts), 3))
    step = max(1, 2_000_000 // len(pts))
    for s in range(0, len(m), step):
        r = c[None, s : s + step] - pts[:, None]
        rl = np.linalg.norm(r, axis=2)
        k = np.where(rl >= a, 1 / np.maximum(rl, 1e-300) ** 3, 1 / a**3)
        out += (m[None, s : s + step, None] * r * k[..., None]).sum(axis=1)
    return out


def g_gauss(m: np.ndarray, c: np.ndarray, sigma: float, pts: np.ndarray) -> np.ndarray:
    out = np.zeros((len(pts), 3))
    step = max(1, 2_000_000 // len(pts))
    for s in range(0, len(m), step):
        r = c[None, s : s + step] - pts[:, None]
        rl = np.maximum(np.linalg.norm(r, axis=2), 1e-12)
        q = rl / (math.sqrt(2) * sigma)
        enc = erf(q) - 2 / math.sqrt(math.pi) * q * np.exp(-q * q)
        out += (m[None, s : s + step, None] * r * (enc / rl**3)[..., None]).sum(axis=1)
    return out


def g_series4(m: np.ndarray, c: np.ndarray, h: float, pts: np.ndarray) -> np.ndarray:
    """Point mass + the cube's l = 4 term, U₄ = A(5Σxᵢ⁴/r⁹ − 3/r⁵), A = −(7/60)(h/2)⁴ m, x = p − c."""
    out = L.g_points(m, c, pts)
    step = max(1, 2_000_000 // len(pts))
    for s in range(0, len(m), step):
        x = pts[:, None] - c[None, s : s + step]
        r = np.linalg.norm(x, axis=2)
        big_s = (x**4).sum(axis=2)
        a = -(7 / 60) * (h / 2) ** 4 * m[None, s : s + step]
        g = a[..., None] * (20 * x**3 / r[..., None] ** 9 - 45 * (big_s / r**11)[..., None] * x
                            + 15 * x / r[..., None] ** 7)
        out += g.sum(axis=1)
    return out


def g_trilinear(fill: np.ndarray, c: np.ndarray, h: float, pts: np.ndarray) -> np.ndarray:
    x, w = np.polynomial.legendre.leggauss(3)
    u = 0.5 * h * x
    w = 0.5 * w
    offs = np.array([[a, b, d] for a in u for b in u for d in u])
    wts = np.array([wa * wb * wd for wa in w for wb in w for wd in w])
    shifted = np.concatenate([pts - o for o in offs])
    g = L.g_cubes(c, h, fill, shifted).reshape(len(offs), len(pts), 3)
    return np.einsum("q,qpk->pk", wts, g)


def main() -> None:
    res: dict[str, object] = {}
    timing: dict[str, float] = {}
    for name, h in SHAPES.items():
        mesh = L.mesh_by_name(name)
        pts = L.tower(mesh, CELLS * h)
        g_ref = L.g_mesh(mesh.tris, pts)
        vox = L.voxelise(mesh, h)
        n_in = len(vox.interior)
        c = np.concatenate([vox.interior_centres, vox.boundary_centres])
        cen = np.concatenate([vox.interior_centres, vox.centroid])
        fill = np.concatenate([np.ones(n_in), vox.frac])
        m = fill * h**3
        a = h * (3 / (4 * math.pi)) ** (1 / 3)
        runs = {
            NAMES[0]: lambda: L.g_points(m, c, pts),
            NAMES[1]: lambda: L.g_points(m, cen, pts),
            NAMES[2]: lambda: g_spheres(m, c, a, pts),
            NAMES[3]: lambda: L.g_cubes(c, h, fill, pts),
            NAMES[4]: lambda: g_series4(m, c, h, pts),
            NAMES[5]: lambda: g_gauss(m, c, h / math.sqrt(12), pts),
            NAMES[6]: lambda: g_trilinear(fill, c, h, pts),
            NAMES[7]: lambda: L.g_cubes(vox.interior_centres, h, np.ones(n_in), pts)
            + L.g_mesh(vox.piece_tris, pts),
        }
        errs = {}
        for key, fn in runs.items():
            t0 = time.perf_counter()
            g = fn()
            dt = time.perf_counter() - t0
            errs[key] = (L.phase(g) - L.phase(g_ref)).tolist()
            timing.setdefault(key, 0.0)
            timing[key] = max(timing[key], 0.0) + dt / (len(m) * len(pts)) * 1e9 / len(SHAPES)
        # The cube series against the cube itself, one cell, far: a check on g_series4.
        far = np.array([[7.3 * h, 2.9 * h, -4.1 * h]]) + c[0]
        one = np.array([1.0])
        rel = np.linalg.norm(g_series4(one * h**3, c[:1], h, far) - L.g_cubes(c[:1], h, one, far)) \
            / np.linalg.norm(L.g_cubes(c[:1], h, one, far))
        res[name] = {"h": h, "cells": CELLS.tolist(), "signal": L.phase(g_ref).tolist(), "err": errs,
                     "n_cells": len(m), "n_piece_tris": len(vox.piece_tris), "n_boundary": len(vox.frac),
                     "series_check_at_9_cells": float(rel)}
        print(name, {k: f"{abs(v[0]):.1e}" for k, v in errs.items()}, f"series check {rel:.1e}", flush=True)
    res["timing_ns_per_cell_point"] = timing
    (L.HERE / "primitives.json").write_text(json.dumps(res, indent=1))
    plot()


def plot() -> None:
    res = json.loads((L.HERE / "primitives.json").read_text())
    timing = res.pop("timing_ns_per_cell_point")
    plt.rcParams.update({"font.size": 10, "axes.edgecolor": "#c3c2b7", "axes.labelcolor": "#52514e",
                         "xtick.color": "#52514e", "ytick.color": "#52514e", "axes.titlecolor": "#0b0b0b",
                         "figure.facecolor": "#fcfcfb", "axes.facecolor": "#fcfcfb"})
    fig, axes = plt.subplots(1, 3, figsize=(16, 5.6), sharey=True)
    lines = ["| primitive | " + " | ".join(f"{n} at 0.5 / 1 / 3 / 10 / 100 cells" for n in res) + " | numpy ns per (cell, point) |",
             "|---|" + "---|" * len(res) + "---|"]
    picks = [0.5, 1, 3, 10, 100]
    table: dict[str, list[str]] = {k: [] for k in NAMES}
    for ax, (name, r) in zip(axes, res.items()):
        x = np.array(r["cells"])
        ax.loglog(x, np.abs(r["signal"]), ":", color="#898781", lw=1.5, label="signal")
        for tol in (1e-6, 1e-10):
            ax.axhline(tol, color="#52514e", lw=0.8, ls="--")
        for key, colour in zip(NAMES, COLOURS):
            e = np.abs(np.array(r["err"][key]))
            ax.loglog(x, np.maximum(e, 1e-17), "-", color=colour, lw=2, marker="o", ms=3.5, label=key)
            table[key].append(" / ".join(f"{e[list(x).index(p)]:.0e}" for p in picks))
        ax.set_title(f"{name}, h = {r['h']} m, {r['n_cells']} cells ({r['n_boundary']} boundary)")
        ax.set_xlabel("distance of the arm point from the surface (cells)")
        ax.set_ylim(1e-17, 1e3)
        ax.grid(True, which="major", color="#e1e0d9", lw=0.6)
        for s in ("top", "right"):
            ax.spines[s].set_visible(False)
    axes[0].set_ylabel("|phase error| (rad)")
    h_, l_ = axes[0].get_legend_handles_labels()
    fig.legend(h_, l_, loc="upper center", ncol=9, frameon=False, fontsize=9)
    fig.text(0.01, 0.01, "Same cell masses in every primitive (each cell's exact piece mass). Dashed: 1e-6 "
             "and 1e-10 rad. (1) is also the cube's series to order 2. Reference: polyhedral closed form on "
             "the mesh.", fontsize=8.5, color="#52514e")
    fig.subplots_adjust(left=0.05, right=0.99, top=0.85, bottom=0.13, wspace=0.06)
    fig.savefig(L.HERE / "primitives.png", dpi=110)
    for key in NAMES:
        lines.append(f"| {key} | " + " | ".join(table[key]) + f" | {timing[key]:.0f} |")
    (L.HERE / "primitives.md").write_text("\n".join(lines) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main() if len(sys.argv) < 2 else plot()
