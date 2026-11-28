"""Figures in the paper's style (paper-flight-phase/style.py), from data/*.json.

    python figures.py     # figures/ladder.{pdf,png}, figures/cost.{pdf,png}, figures/gpu.{pdf,png}
"""

from __future__ import annotations

import json

import numpy as np

import common as C

C.style()
import matplotlib.pyplot as plt  # noqa: E402
import style as st  # noqa: E402

LADDER = [("fill", "fill"), ("centroid", "+ centroid"), ("second", "+ second"), ("third", "+ third"),
          ("plic_mof", "PLIC (MOF)"), ("mof_fill8", "PLIC, 8-bit normal"),
          ("mof_fill8_residual", "PLIC 8-bit + residual l ≤ 3")]


def ladder() -> None:
    res = json.loads((C.DATA / "ladder.json").read_text())
    fig, axes = plt.subplots(1, 5, figsize = (8.0, 2.4), sharey = True)
    for ax, shape in zip(axes, C.SHAPES):
        top = res[shape]["levels"][-1]
        for (key, lab), col, mk in zip(LADDER, st.SERIES + [st.SKY], st.MARKERS + ["*"]):
            ax.loglog(C.DISTANCES, np.maximum(top["errors"][key], 1e-16), marker = mk, color = col, label = lab)
        ax.loglog(C.DISTANCES, np.maximum(top["errors"]["_floor"], 1e-16), ls = ":", color = st.INK2, label = "reference floor")
        ax.axhline(1e-10, color = st.INK2, lw = 0.6, ls = "--")
        ax.set_title(f"{C.NAME[shape]}\nh = 1/{round(1 / top['h'])} m", fontsize = 7)
    fig.supxlabel("tower base from the surface (m)", fontsize = 7)
    axes[0].set_ylabel("phase error (rad)")
    h, l = axes[0].get_legend_handles_labels()
    fig.legend(h, l, loc = "lower center", ncol = 4, fontsize = 6, bbox_to_anchor = (0.5, -0.2))
    fig.savefig(C.FIG / "ladder.pdf")
    fig.savefig(C.FIG / "ladder.png", dpi = 200)


def cost() -> None:
    """Adaptive refinement: cost per point against budget, per design tower, against the baselines."""
    fig, axes = plt.subplots(1, 5, figsize = (7.2, 2.2), sharey = True)
    for ax, shape in zip(axes, C.SHAPES):
        p = C.DATA / f"tree-{shape}.json"
        if not p.exists():
            continue
        a = json.loads(p.read_text())["adaptive"]
        for (t, d), col, mk in zip(enumerate(C.DISTANCES), st.SERIES, st.MARKERS):
            b = [1e-6, 1e-8, 1e-10]
            ax.loglog(b, [a[f"{d}m/{x:g}"]["cost_ns_per_point"] / 1e3 for x in b], marker = mk, color = col, label = f"adaptive, {d} m")
        ax.axhline(a["uniform_finest_exact"]["cost_ns_per_point"] / 1e3, color = st.INK, ls = "--", lw = 0.8, label = "finest exact pieces")
        ax.axhline(a["whole_mesh"]["cost_ns_per_point"] / 1e3, color = st.INK2, ls = ":", lw = 0.8, label = "whole mesh")
        ax.invert_xaxis()
        ax.set_title(C.NAME[shape], fontsize = 7)
    fig.supxlabel("budget B (rad)", fontsize = 7)
    axes[0].set_ylabel("surface cost per point (µs)")
    h, l = axes[0].get_legend_handles_labels()
    fig.legend(h, l, loc = "lower center", ncol = 6, fontsize = 6, bbox_to_anchor = (0.5, -0.2))
    fig.savefig(C.FIG / "cost.pdf")
    fig.savefig(C.FIG / "cost.png", dpi = 200)


if __name__ == "__main__":
    ladder()
    cost()
