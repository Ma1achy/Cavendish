"""What each form does to a gradiometer: mass error and phase error, by shape and pitch.

    python table.py          # writes table.md, table.json and convergence.png here

Forms: cube voxels (a cell is solid if its centre is inside; no renormalisation) and clipped
polyhedra (interior cubes + each boundary cell's exact piece). The tower is vertical, its lower atom
cloud 1.5 m or 5 m from the surface along (1, 0.2, −0.3) from the body's centre, the upper one 5 m
above; phase = k_eff T² Δg_z with k_eff T² = 4.80×10⁹ rad per m s⁻², ρ = 2700 kg/m³.

References: the sphere's closed form (a point mass, radius 1 m); for the torus, teapot and Suzanne the
polyhedral closed form on the full mesh (the 'exact' shape is the mesh). Two more rows say how far
those meshes are from the shapes they stand for: the icosphere against the sphere, and the torus mesh
against the smooth torus (product Gauss–Legendre × trapezoid quadrature, converged as printed).
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

sys.path.insert(0, str(Path(__file__).resolve().parent))
import shapes_lib as L  # noqa: E402

PITCHES = {"sphere": [0.25, 0.125, 0.0625, 0.03125], "torus": [0.2, 0.1, 0.05, 0.025],
           "teapot": [0.2, 0.1, 0.05, 0.025], "suzanne": [0.2, 0.1, 0.05, 0.025]}
DISTANCES = np.array([1.5, 5.0])
BLUE, ORANGE = "#2a78d6", "#eb6834"


def smooth_torus_g(pts: np.ndarray, big_r: float, small_r: float, ns: int, nt: int, nu: int) -> np.ndarray:
    """g per unit G·ρ of the smooth solid torus: Gauss–Legendre in the tube radius, trapezoid in the
    two angles (periodic, so spectrally accurate for points well outside)."""
    x, w = np.polynomial.legendre.leggauss(ns)
    s = 0.5 * small_r * (x + 1)
    ws = 0.5 * small_r * w
    t = 2 * np.pi * np.arange(nt) / nt
    u = 2 * np.pi * np.arange(nu) / nu
    out = np.zeros((len(pts), 3))
    for si, wsi in zip(s, ws):
        rho = big_r + si * np.cos(t)
        z = si * np.sin(t)
        jac = wsi * (2 * np.pi / nt) * (2 * np.pi / nu) * si * rho
        xs = (rho[:, None] * np.cos(u)[None]).ravel()
        ys = (rho[:, None] * np.sin(u)[None]).ravel()
        zs = np.repeat(z, nu)
        js = np.repeat(jac, nu)
        out += L.g_points(js, np.stack([xs, ys, zs], axis=1), pts)
    return out


def main() -> None:
    rows: list[dict[str, object]] = []
    for name in ("sphere", "torus", "teapot", "suzanne"):
        mesh = L.mesh_by_name(name)
        pts = L.tower(mesh, DISTANCES)
        g_mesh = L.g_mesh(mesh.tris, pts)
        v_mesh = float(L.sb.volume_centroid(mesh.tris, np.zeros(3))[0])
        if name == "sphere":
            v_ref = 4.0 / 3.0 * math.pi
            g_ref = L.g_points(np.array([v_ref]), np.zeros((1, 3)), pts)
            ref = "closed form (point mass)"
        else:
            v_ref, g_ref, ref = v_mesh, g_mesh, "polyhedral closed form on the mesh"
        sig = L.phase(g_ref)
        if name == "sphere":
            rows.append({"shape": name, "form": "icosphere mesh vs sphere", "h": None,
                         "mass": v_mesh / v_ref - 1, "phase": (L.phase(g_mesh) - sig).tolist()})
        if name == "torus":
            fine = smooth_torus_g(pts, 1.0, 0.4, 24, 96, 384)
            finer = smooth_torus_g(pts, 1.0, 0.4, 32, 128, 512)
            conv = float(np.abs(L.phase(fine) - L.phase(finer)).max())
            v_smooth = 2 * math.pi**2 * 1.0 * 0.4**2
            rows.append({"shape": name, "form": "torus mesh vs smooth torus", "h": None,
                         "mass": v_mesh / v_smooth - 1, "phase": (L.phase(g_mesh) - L.phase(finer)).tolist(),
                         "quadrature_change": conv})
        for h in PITCHES[name]:
            t0 = time.perf_counter()
            vox = L.voxelise(mesh, h)
            t1 = time.perf_counter()
            g_in = L.g_cubes(vox.interior_centres, h, np.ones(len(vox.interior)), pts)
            g_p = L.g_mesh(vox.piece_tris, pts)
            g_b = L.g_cubes(vox.boundary_centres, h, vox.ctr_in, pts)
            n_in = len(vox.interior)
            forms = {
                "cube voxels": ((n_in + vox.ctr_in.sum()) * h**3, g_in + g_b),
                "clipped polyhedra": ((n_in + vox.frac.sum()) * h**3, g_in + g_p),
            }
            for form, (vol, g) in forms.items():
                rows.append({"shape": name, "form": form, "h": h, "mass": vol / v_ref - 1,
                             "phase": (L.phase(g) - sig).tolist(), "boundary": len(vox.frac),
                             "interior": n_in, "seconds": t1 - t0})
            rows.append({"shape": name, "form": "clipped vs mesh (rounding)", "h": h,
                         "mass": (n_in + vox.frac.sum()) * h**3 / v_mesh - 1,
                         "phase": (L.phase(g_in + g_p) - L.phase(g_mesh)).tolist()})
            print(name, h, f"{t1 - t0:.1f}s", vox.stats, flush=True)
        rows.append({"shape": name, "form": "reference", "h": None, "mass": 0.0, "phase": sig.tolist(),
                     "ref": ref, "mass_kg": v_ref * L.RHO})
    (L.HERE / "table.json").write_text(json.dumps(rows, indent=1))
    write_md(rows)
    plot(rows)


def write_md(rows: list[dict[str, object]]) -> None:
    out = ["| shape | form | pitch h (m) | mass error (relative) | phase error at 1.5 m (rad) | "
           "phase error at 5 m (rad) |", "|---|---|---|---|---|---|"]
    for r in rows:
        ph = r["phase"]
        assert isinstance(ph, list)
        if r["form"] == "reference":
            out.append(f"| {r['shape']} | *signal* ({r['ref']}, {r['mass_kg']:.0f} kg) | | | "
                       f"*{ph[0]:.4g}* | *{ph[1]:.4g}* |")
            continue
        h = "" if r["h"] is None else f"{r['h']:g}"
        out.append(f"| {r['shape']} | {r['form']} | {h} | {r['mass']:+.2e} | {ph[0]:+.2e} | {ph[1]:+.2e} |")
    (L.HERE / "table.md").write_text("\n".join(out) + "\n", encoding="utf-8")


def plot(rows: list[dict[str, object]]) -> None:
    plt.rcParams.update({"font.size": 10, "axes.edgecolor": "#c3c2b7", "axes.labelcolor": "#52514e",
                         "xtick.color": "#52514e", "ytick.color": "#52514e"})
    shapes = ["sphere", "torus", "teapot", "suzanne"]
    fig, axes = plt.subplots(2, 4, figsize=(15, 7.4), facecolor="#fcfcfb")
    for col, name in enumerate(shapes):
        for row, what in enumerate(("phase", "mass")):
            ax = axes[row, col]
            ax.set_facecolor("#fcfcfb")
            for form, colour in (("cube voxels", BLUE), ("clipped polyhedra", ORANGE)):
                sel = [r for r in rows if r["shape"] == name and r["form"] == form]
                h = np.array([r["h"] for r in sel], dtype=float)
                if what == "phase":
                    for di, style in ((0, "-"), (1, "--")):
                        y = np.abs([r["phase"][di] for r in sel])  # type: ignore[index]
                        ax.loglog(h, np.maximum(y, 1e-16), style, color=colour, lw=2, marker="o", ms=5)
                else:
                    y = np.abs([r["mass"] for r in sel])
                    ax.loglog(h, np.maximum(np.asarray(y, dtype=float), 1e-17), "-", color=colour, lw=2,
                              marker="o", ms=5)
            ax.grid(True, which="major", color="#e1e0d9", lw=0.6)
            for s in ("top", "right"):
                ax.spines[s].set_visible(False)
            if row == 0:
                ax.set_title(name, color="#0b0b0b")
            if col == 0:
                ax.set_ylabel("|phase error| (rad)" if what == "phase" else "|mass error| (relative)")
            if row == 1:
                ax.set_xlabel("pitch h (m)")
    axes[0, 0].plot([], [], "-", color="#52514e", label="1.5 m from surface")
    axes[0, 0].plot([], [], "--", color="#52514e", label="5 m from surface")
    axes[0, 0].plot([], [], "-", color=BLUE, lw=2, label="cube voxels (centre inside)")
    axes[0, 0].plot([], [], "-", color=ORANGE, lw=2, label="clipped polyhedra")
    fig.legend(*axes[0, 0].get_legend_handles_labels(), loc="upper center", ncol=4, frameon=False)
    fig.text(0.01, 0.01, "Reference: sphere, closed form (the clipped sphere's error is the 5120-face "
             "icosphere's own, so it does not shrink with h); torus, teapot, Suzanne, the polyhedral closed "
             "form on the mesh (the clipped form matches it to rounding, 1e-12 to 1e-14 rad, plotted).",
             fontsize=8.5, color="#52514e")
    fig.subplots_adjust(left=0.06, right=0.99, top=0.88, bottom=0.11, wspace=0.28, hspace=0.12)
    fig.savefig(L.HERE / "convergence.png", dpi=110, facecolor=fig.get_facecolor())


if __name__ == "__main__":
    main()
