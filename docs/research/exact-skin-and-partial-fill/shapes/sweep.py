"""Resolution sweeps: every technique's phase and mass error against pitch, with cell counts, memory,
cost per evaluation point and fitted orders; the clipped pieces against the mesh (rounding); facet
error against mesh fineness; and error against cost (the Pareto front).

    python sweep.py run sphere        # one shape per process: writes sweep-sphere.json
    python sweep.py run torus | teapot | suzanne
    python sweep.py facets            # facet error of the sphere and torus meshes: sweep-facets.json
    python sweep.py plot              # sweep.png, polyhedra.png, pareto.png, sweep.md

Pitches: body size / h = 4, 8, 16, 32, 64, 128 (size = the largest extent: sphere 2 m, torus 2.8 m,
teapot and Suzanne 1 m). The tower: lower cloud 1.5 m or 5 m from the surface along (1, 0.2, −0.3)
from the body's centre, the upper one 5 m above; phase = k_eff T² Δg_z, ρ = 2700 kg/m³.

Two families, so that two errors stay apart (owner's request):
- mesh techniques, against the mesh (the voxel error): (a) cubes, a cell solid if its centre is
  inside; (b) sampled fill, 4³ points, renormalised (Cavendish today); (c) exact fill as a point at
  the piece's centroid; (d) (c) + the piece's second moment; (e) the clipped pieces themselves.
- analytic techniques (sphere, torus), against the analytic shape, with no mesh at all: (c★) fill and
  centroid from the analytic surface (analytic.py), (d★) + its second moment.
The facet error (mesh against analytic shape) is the third, measured against triangle count.

Costs are a model of a direct sum per evaluation point with Cavendish's measured kernels (note §3):
cube series 8.5 ns per cell (also used for a point mass), 17 ns for point + quadrupole, 55 ns per
polyhedron triangle; a tree walk would cut every far-field term, so these are upper bounds that
compare techniques at equal accuracy, not engine timings. Memory is a model too: 7 bytes per leaf
(Cavendish's leaf today), plus per boundary leaf 16 bytes for fill + centroid (4 × f32), 24 more for
the second moment (6 × f32), or 36 bytes per piece triangle (9 × f32).
"""

from __future__ import annotations

import json
import sys
import time
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402

sys.path.insert(0, str(Path(__file__).resolve().parent))
import analytic as A  # noqa: E402
import shapes_lib as L  # noqa: E402

NS = [4, 8, 16, 32, 64, 128]
DIST = np.array([1.5, 5.0])
TECH = ["(a) cubes", "(b) sampled fill", "(c) fill at centroid", "(d) + second moment", "(e) clipped pieces",
        "(c★) analytic fill", "(d★) analytic + second"]
COLOURS = ["#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4", "#008300", "#6250d6"]
NS_SERIES, NS_QUAD, NS_TRI = 8.5, 17.0, 55.0
ANALYTIC = {"sphere": A.Ball(1.0), "torus": A.Torus(1.0, 0.4)}


def run(name: str) -> None:
    mesh = L.mesh_by_name(name)
    size = float((mesh.verts.max(axis=0) - mesh.verts.min(axis=0)).max())
    pts = L.tower(mesh, DIST)
    g_mesh = L.g_mesh(mesh.tris, pts)
    v_mesh = float(L.sb.volume_centroid(mesh.tris, np.zeros(3))[0])
    ph = L.phase
    shape = ANALYTIC.get(name)
    g_an = shape.g_ref(pts) if shape is not None else None
    rows: list[dict[str, object]] = []
    out: dict[str, object] = {"shape": name, "size": size, "mesh": mesh.source, "triangles": len(mesh.faces),
                              "signal": ph(g_mesh).tolist(), "volume_mesh": v_mesh, "rows": rows}
    if shape is not None:
        out["signal_analytic"] = ph(g_an).tolist()
        out["facet_err"] = (ph(g_mesh) - ph(g_an)).tolist()
        out["facet_mass"] = v_mesh / shape.volume - 1
    body = L.body_moments(mesh.tris)
    out["multipole_err"] = (ph(L.g_multipole(*body, pts)) - ph(g_mesh)).tolist()
    out["point_err"] = (ph(L.g_multipole(body[0], body[1], None, None, pts)) - ph(g_mesh)).tolist()
    out["mesh_cost_ns"] = len(mesh.faces) * NS_TRI
    path = L.HERE / f"sweep-{name}.json"
    if path.exists():  # resume: keep pitches already done
        rows.extend(json.loads(path.read_text())["rows"])
    for n_across in NS:
        if any(r["n"] == n_across for r in rows):
            continue
        h = size / n_across
        t0 = time.perf_counter()
        vox = L.voxelise(mesh, h)
        t_vox = time.perf_counter() - t0
        n_in, n_b = len(vox.interior), len(vox.frac)
        cen = vox.boundary_centres
        g_p = L.g_mesh(vox.piece_tris, pts)
        g_in = L.g_cubes_fast(vox.interior_centres, h, np.ones(n_in), pts)
        g_ctr = L.g_cubes(cen, h, vox.ctr_in, pts)
        g_ss = L.g_cubes(cen, h, vox.ss, pts)
        mass_b = vox.frac * h**3
        g_pt = L.g_points(mass_b, vox.centroid, pts)
        g_q = g_pt + L.g_quadrupole(vox.second, vox.centroid, pts)
        s4 = v_mesh / (h**3 * (n_in + vox.ss.sum()))
        err = {
            TECH[0]: ph(g_ctr) - ph(g_p),
            TECH[1]: (s4 - 1) * (ph(g_mesh) - ph(g_p)) + s4 * ph(g_ss) - ph(g_p),
            TECH[2]: ph(g_pt) - ph(g_p),
            TECH[3]: ph(g_q) - ph(g_p),
            TECH[4]: ph(g_in + g_p) - ph(g_mesh),
        }
        mass = {
            TECH[0]: (n_in + vox.ctr_in.sum()) * h**3 / v_mesh - 1,
            TECH[1]: (n_in + vox.ss.sum()) * h**3 / v_mesh - 1,  # before renormalisation (after: 0)
            TECH[2]: (n_in + vox.frac.sum()) * h**3 / v_mesh - 1,
            TECH[3]: (n_in + vox.frac.sum()) * h**3 / v_mesh - 1,
            TECH[4]: (n_in + vox.frac.sum()) * h**3 / v_mesh - 1,
        }
        tris_per_piece = len(vox.piece_tris) / max(n_b, 1)
        cost = {TECH[0]: (n_in + vox.ctr_in.sum()) * NS_SERIES, TECH[1]: (n_in + n_b) * NS_SERIES,
                TECH[2]: (n_in + n_b) * NS_SERIES, TECH[3]: n_in * NS_SERIES + n_b * NS_QUAD,
                TECH[4]: n_in * NS_SERIES + len(vox.piece_tris) * NS_TRI}
        memory = {TECH[0]: 7 * (n_in + vox.ctr_in.sum()), TECH[1]: 7 * (n_in + n_b),
                  TECH[2]: 7 * (n_in + n_b) + 16 * n_b, TECH[3]: 7 * (n_in + n_b) + 40 * n_b,
                  TECH[4]: 7 * (n_in + n_b) + 36 * len(vox.piece_tris)}
        row: dict[str, object] = {"n": n_across, "h": h, "interior": n_in, "boundary": n_b,
                                  "piece_tris": len(vox.piece_tris), "tris_per_piece": tris_per_piece,
                                  "seconds_clip": t_vox, "volume_check": vox.stats["volume_rel_error"]}
        if shape is not None:
            t0 = time.perf_counter()
            av = A.analytic_voxels(shape, h, h * L.GENERIC)
            t_an = time.perf_counter() - t0
            g_in_a = L.g_cubes_fast(av["interior"], h, np.ones(len(av["interior"])), pts)
            m_a = av["frac"] * h**3
            g_c = g_in_a + L.g_points(m_a, av["centroid"], pts)
            g_d = g_c + L.g_quadrupole(av["second"], av["centroid"], pts)
            err[TECH[5]] = ph(g_c) - ph(g_an)
            err[TECH[6]] = ph(g_d) - ph(g_an)
            v_a = (len(av["interior"]) + av["frac"].sum()) * h**3
            mass[TECH[5]] = mass[TECH[6]] = v_a / shape.volume - 1
            nb_a, ni_a = len(av["frac"]), len(av["interior"])
            cost[TECH[5]] = (ni_a + nb_a) * NS_SERIES
            cost[TECH[6]] = ni_a * NS_SERIES + nb_a * NS_QUAD
            memory[TECH[5]] = 7 * (ni_a + nb_a) + 16 * nb_a
            memory[TECH[6]] = 7 * (ni_a + nb_a) + 40 * nb_a
            row.update({"analytic_interior": ni_a, "analytic_boundary": nb_a, "seconds_analytic": t_an})
        row["err"] = {k: v.tolist() for k, v in err.items()}
        row["mass"] = {k: float(v) for k, v in mass.items()}
        row["cost_ns"] = {k: float(v) for k, v in cost.items()}
        row["memory_bytes"] = {k: float(v) for k, v in memory.items()}
        rows.append(row)
        rows.sort(key=lambda r: int(r["n"]))  # type: ignore[arg-type]
        print(name, n_across, f"h {h:.4g}", f"clip {t_vox:.1f}s", f"in {n_in} bnd {n_b}",
              {k: f"{abs(v[0]):.1e}" for k, v in err.items()}, flush=True)
        path.write_text(json.dumps(out, indent=1))
        del vox, g_p, g_in


def facets() -> None:
    res: dict[str, list[dict[str, float]]] = {}
    for name in ("sphere", "torus"):
        shape = ANALYTIC[name]
        pts = L.tower(L.mesh_by_name(name), DIST)
        g_an = shape.g_ref(pts)
        rows = []
        variants = ([(lev,) for lev in range(1, 7)] if name == "sphere"
                    else [(16, 8), (32, 16), (64, 32), (128, 64), (256, 128), (512, 256)])
        for var in variants:
            if name == "sphere":
                m = L.sphere_mesh(1.0) if var[0] == 4 else _icosphere(var[0])
            else:
                m = L.torus_mesh(1.0, 0.4, var[0], var[1])
            g = L.g_mesh(m.tris, pts)
            vol = float(L.sb.volume_centroid(m.tris, np.zeros(3))[0])
            rows.append({"triangles": len(m.faces), "phase": (L.phase(g) - L.phase(g_an)).tolist(),
                         "mass": vol / shape.volume - 1})
            print(name, var, len(m.faces), rows[-1], flush=True)
        res[name] = rows
    (L.HERE / "sweep-facets.json").write_text(json.dumps(res, indent=1))


def _icosphere(level: int) -> L.Mesh:
    v, f = L.sb.icosphere(level)
    faces = np.array(f, dtype=np.int64)
    n = np.cross(v[faces[:, 1]] - v[faces[:, 0]], v[faces[:, 2]] - v[faces[:, 0]])
    flip = np.einsum("nk,nk->n", n, v[faces].mean(axis=1)) < 0
    faces[flip] = faces[flip][:, ::-1]
    return L.Mesh("sphere", v, faces, "")


def order(h: np.ndarray, e: np.ndarray) -> float:
    """Least-squares slope of log|e| against log h; nan when every |e| is at rounding (< 1e-10 rad)."""
    ok = np.abs(e) > 1e-13
    if ok.sum() < 3 or np.abs(e).max() < 1e-10:
        return float("nan")
    return float(np.polyfit(np.log(h[ok]), np.log(np.abs(e[ok])), 1)[0])


def cost_to_reach(cost: np.ndarray, err: np.ndarray, tol: float) -> float:
    """Cost at which |err| first falls below `tol` along the pitch sweep, interpolated in log–log
    between the two pitches either side (inf if it never does; the first pitch's cost if it already
    does there). Errors that go back above `tol` at a finer pitch are not smoothed over."""
    e = np.abs(err)
    below = np.nonzero(e < tol)[0]
    if len(below) == 0:
        return float("inf")
    k = int(below[0])
    if k == 0:
        return float(cost[0])
    lc, le = np.log(cost[k - 1 : k + 1]), np.log(e[k - 1 : k + 1])
    return float(np.exp(lc[0] + (np.log(tol) - le[0]) * (lc[1] - lc[0]) / (le[1] - le[0])))


def _style() -> None:
    plt.rcParams.update({"font.size": 10, "axes.edgecolor": "#c3c2b7", "axes.labelcolor": "#52514e",
                         "xtick.color": "#52514e", "ytick.color": "#52514e", "axes.titlecolor": "#0b0b0b",
                         "figure.facecolor": "#fcfcfb", "axes.facecolor": "#fcfcfb"})


def _line_style(tech: str) -> dict[str, object]:
    """Analytic techniques (★) dashed with open markers, so they stay visible over their mesh twins."""
    if "★" in tech:
        return {"ls": "--", "marker": "o", "mfc": "#fcfcfb", "lw": 1.8, "ms": 6}
    return {"ls": "-", "marker": "o", "lw": 2, "ms": 4}


def _clean(ax: plt.Axes) -> None:
    ax.grid(True, which="major", color="#e1e0d9", lw=0.6)
    for s in ("top", "right"):
        ax.spines[s].set_visible(False)


FIRST = ("sphere", "torus", "teapot", "suzanne")
MORE = ("bunny", "spot", "dragon", "armadillo", "buddha", "bracket")  # added 26 Nov 2026: sweep-more.png


def plot() -> None:
    _style()
    data = {n: json.loads((L.HERE / f"sweep-{n}.json").read_text())
            for n in FIRST + MORE if (L.HERE / f"sweep-{n}.json").exists()}
    md: list[str] = []
    # ---- error against size / pitch, per shape, at 1.5 m (solid): sweep.png, sweep-more.png ----
    for group, out_name in ((FIRST, "sweep"), (MORE, "sweep-more")):
        sub = {n: d for n, d in data.items() if n in group}
        if sub:
            md += _sweep_figure(sub, out_name)
    md += _polyhedra_figure(data)
    md += _pareto_and_costs(data)
    (L.HERE / "sweep.md").write_text("\n".join(md) + "\n", encoding="utf-8")


def _sweep_figure(data: dict[str, dict], out_name: str) -> list[str]:
    md: list[str] = []
    fig, axes = plt.subplots(2, len(data), figsize=(4.2 * len(data), 8.4), squeeze=False)
    for col, (name, d) in enumerate(data.items()):
        ns = np.array([r["n"] for r in d["rows"]])
        hs = np.array([r["h"] for r in d["rows"]])
        md += [f"\n**{name}** ({d['mesh']}; size {d['size']:.3g} m; signal {d['signal'][0]:.4g} rad at 1.5 m, "
               f"{d['signal'][1]:.4g} rad at 5 m)\n",
               "| size/h | h (m) | interior | boundary | piece triangles | clip time (s) | "
               + " | ".join(TECH[:5] + (TECH[5:] if "facet_err" in d else [])) + " |",
               "|---|---|---|---|---|---|" + "---|" * (7 if "facet_err" in d else 5)]
        for r in d["rows"]:
            cells = [f"{abs(r['err'][t][0]):.1e} / {abs(r['err'][t][1]):.1e}" for t in TECH if t in r["err"]]
            md.append(f"| {r['n']} | {r['h']:.4g} | {r['interior']} | {r['boundary']} | {r['piece_tris']} | "
                      f"{r['seconds_clip']:.1f} | " + " | ".join(cells) + " |")
        orders = []
        for tech, colour in zip(TECH, COLOURS):
            if tech not in d["rows"][0]["err"]:
                continue
            e = np.array([r["err"][tech][0] for r in d["rows"]])
            m = np.array([r["mass"][tech] for r in d["rows"]])
            o = order(hs[1:], e[1:])
            orders.append(f"{tech}: {o:.2f}" if np.isfinite(o) else
                          f"{tech}: flat at rounding (max {np.abs(e).max():.0e} rad)")
            sty = _line_style(tech)
            axes[0, col].loglog(ns, np.maximum(np.abs(e), 1e-16), color=colour, label=tech, **sty)
            axes[1, col].loglog(ns, np.maximum(np.abs(m), 1e-17), color=colour, label=tech, **sty)
        if "facet_err" in d:
            axes[0, col].axhline(abs(d["facet_err"][0]), color="#898781", ls=":", lw=1.5)
            axes[0, col].text(ns[0], abs(d["facet_err"][0]) * 1.6, "facet error of the mesh", fontsize=8,
                              color="#52514e")
        md.append("\nFitted order in h (phase at 1.5 m, size/h ≥ 8): " + "; ".join(orders) + ".")
        axes[0, col].set_title(f"{name} ({d['triangles']} triangles)")
        axes[1, col].set_xlabel("body size / pitch")
        _clean(axes[0, col])
        _clean(axes[1, col])
    axes[0, 0].set_ylabel("|phase error| at 1.5 m (rad)")
    axes[1, 0].set_ylabel("|mass error| (relative)")
    h_, l_ = axes[0, 0].get_legend_handles_labels()
    fig.legend(h_, l_, loc="upper center", ncol=7, frameon=False, fontsize=9)
    fig.text(0.01, 0.005, "(a)–(e) against the mesh (voxel error); (c★), (d★) against the analytic shape. "
             "(b)'s mass is before its renormalisation (after it, 0). (e) and (c)–(d) carry exact mass, so "
             "their mass error is rounding.", fontsize=8.5, color="#52514e")
    fig.subplots_adjust(left=0.06, right=0.99, top=0.92, bottom=0.07, wspace=0.25, hspace=0.15)
    fig.savefig(L.HERE / f"{out_name}.png", dpi=110)
    plt.close(fig)
    return md


def _polyhedra_figure(data: dict[str, dict]) -> list[str]:
    md: list[str] = []
    # ---- polyhedra: clipped vs mesh, facet error, clip time ----
    fac = json.loads((L.HERE / "sweep-facets.json").read_text()) if (L.HERE / "sweep-facets.json").exists() else {}
    fig, axes = plt.subplots(1, 3, figsize=(16, 4.8))
    shape_col = {"sphere": "#2a78d6", "torus": "#eb6834", "teapot": "#1baf7a", "suzanne": "#eda100",
                 "bunny": "#e87ba4", "spot": "#008300", "dragon": "#6250d6", "armadillo": "#e34948",
                 "buddha": "#898781", "bracket": "#0b0b0b"}
    for name, d in data.items():
        ns = np.array([r["n"] for r in d["rows"]])
        e = np.array([r["err"][TECH[4]] for r in d["rows"]])
        axes[0].loglog(ns, np.maximum(np.abs(e[:, 0]), 1e-17), "-o", color=shape_col[name], lw=2, ms=4,
                       label=name)
        axes[2].loglog(ns, [r["seconds_clip"] / max(r["boundary"], 1) * 1e3 for r in d["rows"]], "-o",
                       color=shape_col[name], lw=2, ms=4, label=name)
    axes[0].set_title("clipped pieces + interior cubes against the mesh")
    axes[0].set_xlabel("body size / pitch")
    axes[0].set_ylabel("|phase error| at 1.5 m (rad)")
    axes[0].set_ylim(1e-17, 1e-8)
    for name, rows in fac.items():
        tr = np.array([r["triangles"] for r in rows])
        axes[1].loglog(tr, np.abs([r["phase"][0] for r in rows]), "-o", color=shape_col[name], lw=2, ms=4,
                       label=f"{name}: phase at 1.5 m")
        axes[1].loglog(tr, np.abs([r["mass"] for r in rows]) * 1e2, "--o", color=shape_col[name], lw=1.4,
                       ms=3, label=f"{name}: |mass error| × 100")
    axes[1].set_title("facet error: the mesh against the analytic shape")
    axes[1].set_xlabel("triangles")
    axes[1].set_ylabel("|phase error| (rad); |mass error| × 100")
    axes[2].set_title("clipping time per boundary cell (Python)")
    axes[2].set_xlabel("body size / pitch")
    axes[2].set_ylabel("ms per boundary cell")
    for ax in axes:
        _clean(ax)
        ax.legend(frameon=False, fontsize=8.5)
    fig.subplots_adjust(left=0.05, right=0.99, top=0.9, bottom=0.13, wspace=0.25)
    fig.savefig(L.HERE / "polyhedra.png", dpi=110)
    plt.close(fig)
    if fac:
        md += ["\n**Facet error** (mesh against analytic shape)\n",
               "| shape | triangles | phase error at 1.5 m (rad) | at 5 m (rad) | mass error |", "|---|---|---|---|---|"]
        for name, rows in fac.items():
            for r in rows:
                md.append(f"| {name} | {r['triangles']} | {r['phase'][0]:+.2e} | {r['phase'][1]:+.2e} | "
                          f"{r['mass']:+.2e} |")
        md.append("\nFacet error against triangle count (fitted slope of log|error| against log triangles): "
                  + "; ".join(f"{name}: phase {np.polyfit(np.log([r['triangles'] for r in rows]), np.log(np.abs([r['phase'][0] for r in rows])), 1)[0]:.2f}, "
                              f"mass {np.polyfit(np.log([r['triangles'] for r in rows]), np.log(np.abs([r['mass'] for r in rows])), 1)[0]:.2f}"
                              for name, rows in fac.items()) + " (−1 per triangle count is order 2 in the edge length).")
    return md


def _pareto_and_costs(data: dict[str, dict]) -> list[str]:
    md: list[str] = []
    # ---- Pareto: error against cost per point (rows of five shapes) ----
    ncol = min(len(data), 5)
    nrow = -(-len(data) // ncol)
    fig, axes = plt.subplots(nrow, ncol, figsize=(4.0 * ncol, 4.3 * nrow + 0.9), squeeze=False, sharey=True)
    for k in range(len(data), nrow * ncol):
        axes.flat[k].set_visible(False)
    for col, (name, d) in enumerate(data.items()):
        ax = axes.flat[col]
        for tech, colour in zip(TECH, COLOURS):
            if tech not in d["rows"][0]["err"]:
                continue
            c = np.array([r["cost_ns"][tech] for r in d["rows"]]) * 1e-3
            e = np.abs([r["err"][tech][0] for r in d["rows"]])
            ax.loglog(c, np.maximum(e, 1e-16), color=colour, label=tech, **_line_style(tech))
        ax.loglog([1e-3 * 30], [max(abs(d["multipole_err"][0]), 1e-16)], "s", color="#e34948", ms=8,
                  label="(f) body multipole")
        ax.loglog([1e-3 * NS_SERIES], [max(abs(d["point_err"][0]), 1e-16)], "D", color="#52514e", ms=7,
                  label="(g) point mass")
        ax.loglog([1e-3 * d["mesh_cost_ns"]], [1e-15], "*", color="#0b0b0b", ms=12, label="whole mesh (exact)")
        ax.set_title(f"{name} ({d['triangles']} triangles)")
        ax.set_xlabel("cost per evaluation point (µs, model)")
        _clean(ax)
    for r in range(nrow):
        axes[r, 0].set_ylabel("|phase error| at 1.5 m (rad)")
    h_, l_ = axes[0, 0].get_legend_handles_labels()
    height = 4.3 * nrow + 0.9
    fig.legend(h_, l_, loc="upper center", ncol=5, frameon=False, fontsize=9)
    fig.text(0.01, 0.12 / height, "Cost: a direct sum with Cavendish's measured kernels (8.5 ns per cube or point, "
             "17 ns point + quadrupole, 55 ns per triangle); points along each line are size/pitch 4…128. The whole "
             "mesh is exact by definition (plotted at 1e-15).", fontsize=8.5, color="#52514e")
    fig.subplots_adjust(left=0.06, right=0.99, top=1 - 0.75 / height, bottom=0.75 / height, wspace=0.08,
                        hspace=0.35)
    fig.savefig(L.HERE / "pareto.png", dpi=110)
    plt.close(fig)

    # ---- cost and memory, and the cost to reach a tolerance ----
    for name, d in data.items():
        techs = [t for t in TECH if t in d["rows"][0]["err"]]
        md += [f"\n**{name}: cost per evaluation point (µs, model) and memory (MB, model)**\n",
               "| size/h | " + " | ".join(techs) + " |", "|---|" + "---|" * len(techs)]
        for r in d["rows"]:
            md.append(f"| {r['n']} | " + " | ".join(
                f"{r['cost_ns'][t] * 1e-3:.3g} µs, {r['memory_bytes'][t] * 1e-6:.3g} MB" for t in techs) + " |")
    tols = (1e-4, 1e-6, 1e-8)
    md += ["\n**Cheapest cost per evaluation point (µs, model) to bring the 1.5 m phase error below "
           "1e-4 / 1e-6 / 1e-8 rad** (log–log interpolation between pitches; — never within size/h ≤ 128)\n",
           "| technique | " + " | ".join(data) + " |", "|---|" + "---|" * len(data)]
    for tech in TECH:
        cells = []
        for d in data.values():
            if tech not in d["rows"][0]["err"]:
                cells.append("n/a")
                continue
            c = np.array([r["cost_ns"][tech] for r in d["rows"]]) * 1e-3
            e = np.array([r["err"][tech][0] for r in d["rows"]])
            got = [cost_to_reach(c, e, t) for t in tols]
            cells.append(" / ".join("—" if not np.isfinite(g) else f"{g:.3g}" for g in got))
        md.append(f"| {tech} | " + " | ".join(cells) + " |")
    md.append("| whole mesh, direct | " + " | ".join(f"{d['mesh_cost_ns'] * 1e-3:.3g} (exact)"
                                                    for d in data.values()) + " |")
    return md


if __name__ == "__main__":
    if sys.argv[1] == "run":
        if len(sys.argv) > 3:  # a quick trial: only pitches up to this size/h
            NS[:] = [n for n in NS if n <= int(sys.argv[3])]
        run(sys.argv[2])
    elif sys.argv[1] == "facets":
        facets()
    else:
        plot()
