"""The shapes study's figures and tables for the paper (provisional until S5).

    python scripts/shapes.py          # called by `make.py shapes`

Reads the study's committed output in place, in docs/research/exact-skin-and-partial-fill/shapes/
(`sweep-<shape>.json`, `regimes-distance.json`, `translation.json`, `exact-*.json`,
`mesh-check.json`, and the rendered `<shape>-64-cutaway.png`), and writes:

- figures/shapes-<shape>.png   the study's representation renders, cropped to the panels
- figures/shapes-sweep.pdf     phase error against pitch, ten shapes
- figures/shapes-pareto.pdf    phase error against modelled cost, four shapes
- figures/shapes-regimes.pdf   phase error against distance, three shapes
- figures/shapes-motion.pdf    error spectrum of a torus passing a tower, body and world frame
- figures/shapes-exact.pdf     each mesh as one exact polyhedron: distance, series, walking past
- generated/shapes-*.tex       the tables

Nothing is recomputed here: the numbers are the study's. `make.py shapes-study` reruns the study's
own scripts (hours, and they download the meshes). The helpers `order`, `cost_to_reach` and
`crossover` repeat the study's definitions (sweep.py, regimes.py) so the paper does not import its
mesh library; `check()` compares a few results with the numbers the study's note quotes.
"""

from __future__ import annotations

import json
import math
import pathlib
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
from PIL import Image  # noqa: E402

HERE = pathlib.Path(__file__).resolve().parents[1]
REPO = HERE.parents[2]
STUDY = REPO / "docs" / "research" / "exact-skin-and-partial-fill" / "shapes"
FIG = HERE / "figures"
TEX = HERE / "generated"

# The study's technique keys, and the paper's names for them.
TECH = ["(a) cubes", "(b) sampled fill", "(c) fill at centroid", "(d) + second moment",
        "(e) clipped pieces", "(c★) analytic fill", "(d★) analytic + second"]
SHORT = {TECH[0]: "(a)", TECH[1]: "(b)", TECH[2]: "(c)", TECH[3]: "(d)", TECH[4]: "(e)",
         TECH[5]: "(c$^\\star$)", TECH[6]: "(d$^\\star$)"}
LABEL = {TECH[0]: "(a) cubes, centre inside", TECH[1]: "(b) sampled fill $4^3$",
         TECH[2]: "(c) fill + centroid", TECH[3]: "(d) cut-cell moments",
         TECH[4]: "(e) clipped pieces", TECH[5]: "(c$^\\star$) analytic (c)",
         TECH[6]: "(d$^\\star$) analytic (d)", "(f) body multipole": "(f) body multipole, $l\\leq3$",
         "(g) point mass": "(g) point mass"}
COLOUR = {TECH[0]: "#2a78d6", TECH[1]: "#eb6834", TECH[2]: "#1baf7a", TECH[3]: "#eda100",
          TECH[4]: "#e87ba4", TECH[5]: "#008300", TECH[6]: "#6250d6",
          "(f) body multipole": "#0b0b0b", "(g) point mass": "#898781"}
SHAPES = ["sphere", "torus", "teapot", "suzanne", "bunny", "spot", "dragon", "armadillo", "buddha",
          "bracket"]
NAME = {"sphere": "sphere", "torus": "torus", "teapot": "teapot", "suzanne": "Suzanne",
        "bunny": "bunny", "spot": "Spot", "dragon": "dragon", "armadillo": "Armadillo",
        "buddha": "Buddha", "bracket": "mk7 part"}
INK, INK2, GRID = "#0b0b0b", "#52514e", "#d9d8d4"
RENDERS = ("torus", "bunny")
RENDER_DPI = 150  # the renders' resolution at print size
RENDER_ACROSS = 64  # the renders' fine lattice, cells across (the study's own are 128, as measured)


def load(name: str) -> dict:
    return json.loads((STUDY / name).read_text(encoding="utf-8"))


def style() -> None:
    matplotlib.rcParams.update({
        "font.family": "serif", "font.size": 7, "axes.edgecolor": INK2, "axes.labelcolor": INK,
        "xtick.color": INK2, "ytick.color": INK2, "axes.grid": True, "grid.color": GRID,
        "grid.linewidth": 0.5, "axes.spines.top": False, "axes.spines.right": False,
        "lines.linewidth": 1.1, "lines.markersize": 2.8, "legend.frameon": False,
        "savefig.bbox": "tight", "pdf.fonttype": 42, "axes.titlesize": 7,
        "xtick.labelsize": 6, "ytick.labelsize": 6,
    })


# ------------------------------------------------------------------ the study's definitions

def order(h: np.ndarray, e: np.ndarray) -> float:
    """sweep.order: least-squares slope of log|e| against log h; nan when at rounding."""
    ok = np.abs(e) > 1e-13
    if ok.sum() < 3 or np.abs(e).max() < 1e-10:
        return float("nan")
    return float(np.polyfit(np.log(h[ok]), np.log(np.abs(e[ok])), 1)[0])


def cost_to_reach(cost: np.ndarray, err: np.ndarray, tol: float) -> float:
    """sweep.cost_to_reach: cost where |err| first falls below tol, log-log interpolated."""
    e = np.abs(err)
    below = np.nonzero(e < tol)[0]
    if len(below) == 0:
        return float("inf")
    k = int(below[0])
    if k == 0:
        return float(cost[0])
    lc, le = np.log(cost[k - 1:k + 1]), np.log(e[k - 1:k + 1])
    return float(np.exp(lc[0] + (np.log(tol) - le[0]) * (lc[1] - lc[0]) / (le[1] - le[0])))


def crossover(d: np.ndarray, err: np.ndarray, tol: float) -> float:
    """regimes.crossover: smallest sampled distance beyond which |err| stays below tol."""
    bad = np.nonzero(np.abs(err) >= tol)[0]
    if len(bad) == 0:
        return float(d[0])
    return float(d[bad[-1] + 1]) if bad[-1] + 1 < len(d) else float("inf")


# ------------------------------------------------------------------ formatting

def sci(x: float, digits: int = 1) -> str:
    if not math.isfinite(x):
        return "---"
    m, e = f"{x:.{digits}e}".split("e")
    return f"${m}\\times10^{{{int(e)}}}$"


def short(x: float) -> str:
    """A cost or distance in two or three significant figures, or a dash."""
    if not math.isfinite(x):
        return "---"
    if x >= 100:
        return f"{x:,.0f}".replace(",", "\\,")
    return f"{x:.2g}"


def write(name: str, rows: list[str]) -> None:
    TEX.mkdir(exist_ok=True)
    (TEX / name).write_text("\n".join(rows) + "\n", encoding="utf-8")


# ------------------------------------------------------------------ renders

def crop_render(shape: str) -> None:
    """The study's three-panel render without its caption (the paper gives its own)."""
    img = np.asarray(Image.open(STUDY / f"{shape}-{RENDER_ACROSS}-cutaway.png").convert("RGB")).astype(float)
    busy = np.abs(img - img[0, 0]).sum(axis=2).max(axis=1) > 30
    rows = np.nonzero(busy)[0]
    # From the bottom: the caption block (lines a few pixels apart), then the first blank run of
    # at least 15 rows, above which the panels end.
    k = rows[-1]
    while k > 0:
        if not busy[k]:
            run = k
            while run > 0 and not busy[run]:
                run -= 1
            if k - run >= 15:
                k = run
                break
            k = run
        else:
            k -= 1
    if k <= rows[0]:  # no clear gap: keep everything
        k = rows[-1]
    top, bottom = max(rows[0] - 6, 0), min(k + 8, len(img))
    out = Image.fromarray(img[top:bottom].astype(np.uint8))
    # Printed at \linewidth (A4, 2.2 cm margins: 16.6 cm), rasterised at RENDER_DPI.
    width = round(16.6 / 2.54 * RENDER_DPI)
    if out.width > width:
        out = out.resize((width, round(out.height * width / out.width)), Image.Resampling.LANCZOS)
    out.save(FIG / f"shapes-{shape}.png", optimize=True, dpi=(RENDER_DPI, RENDER_DPI))


# ------------------------------------------------------------------ sweeps

def sweep_data() -> dict[str, dict]:
    return {n: load(f"sweep-{n}.json") for n in SHAPES}


def fig_sweep(data: dict[str, dict]) -> None:
    fig, axes = plt.subplots(2, 5, figsize=(6.8, 3.5), sharex=True, sharey=True)
    for ax, (name, d) in zip(axes.flat, data.items()):
        ns = np.array([r["n"] for r in d["rows"]])
        for tech in TECH:
            if tech not in d["rows"][0]["err"]:
                continue
            e = np.abs([r["err"][tech][0] for r in d["rows"]])
            star = "★" in tech
            ax.loglog(ns, np.maximum(e, 1.2e-14), color=COLOUR[tech], marker="o",
                      ls="--" if star else "-", mfc="white" if star else COLOUR[tech],
                      label=LABEL[tech])
        if "facet_err" in d:
            ax.axhline(abs(d["facet_err"][0]), color=INK2, ls=":", lw=0.9, label="facet error of the mesh")
        ax.set_title(f"{NAME[name]} ({d['triangles']:,} tri.)".replace(",", "\u2009"))
        ax.set_ylim(1e-14, 3e2)
        ax.set_xticks([4, 16, 64], ["4", "16", "64"])
        ax.minorticks_off()
    for ax in axes[1]:
        ax.set_xlabel("body size / pitch")
    for ax in axes[:, 0]:
        ax.set_ylabel("phase error at 1.5 m (rad)")
    handles, labels = {}, {}
    for ax in axes.flat:
        for hdl, lab in zip(*ax.get_legend_handles_labels()):
            handles.setdefault(lab, hdl)
    fig.legend(list(handles.values()), list(handles.keys()), loc="upper center", ncol=4,
               bbox_to_anchor=(0.5, 1.09), fontsize=6.5)
    fig.subplots_adjust(wspace=0.08, hspace=0.3)
    fig.savefig(FIG / "shapes-sweep.pdf")
    plt.close(fig)


def table_sweep(data: dict[str, dict]) -> None:
    techs = TECH
    rows = [r"\begin{tabular}{l" + "r" * len(techs) + "}", r"\toprule",
            "shape (triangles) & " + " & ".join(SHORT[t] for t in techs) + r" \\", r"\midrule"]
    for name, d in data.items():
        hs = np.array([r["h"] for r in d["rows"]])
        cells = []
        for t in techs:
            if t not in d["rows"][0]["err"]:
                cells.append("")
                continue
            e = np.array([r["err"][t][0] for r in d["rows"]])
            if t == TECH[4]:
                cells.append(f"$\\leq${sci(float(np.abs(e).max()), 0)}")
                continue
            o = order(hs[1:], e[1:])
            cells.append(f"{sci(abs(float(e[-1])))} ({o:.1f})")
        tri = f"{d['triangles']:,}".replace(",", "\\,")
        rows.append(f"{NAME[name]} ({tri}) & " + " & ".join(cells) + r" \\")
        if name == "suzanne":
            rows.append(r"\midrule")
    rows += [r"\bottomrule", r"\end{tabular}"]
    write("shapes-sweep.tex", rows)


def fig_pareto(data: dict[str, dict]) -> None:
    shown = ("sphere", "teapot", "bunny", "bracket")
    fig, axes = plt.subplots(1, 4, figsize=(6.8, 2.2), sharey=True)
    for ax, name in zip(axes, shown):
        d = data[name]
        for tech in TECH:
            if tech not in d["rows"][0]["err"]:
                continue
            c = np.array([r["cost_ns"][tech] for r in d["rows"]]) * 1e-3
            e = np.abs([r["err"][tech][0] for r in d["rows"]])
            star = "★" in tech
            ax.loglog(c, np.maximum(e, 1e-16), color=COLOUR[tech], marker="o", ls="--" if star else "-",
                      mfc="white" if star else COLOUR[tech], label=LABEL[tech])
        ax.loglog([1e-3 * d["mesh_cost_ns"]], [1e-15], "*", color=INK, ms=7,
                  label="whole mesh, direct (exact to the mesh)")
        ax.set_title(f"{NAME[name]} ({d['triangles']:,} tri.)".replace(",", "\u2009"))
        ax.set_xlabel("cost per point ($\\mu$s, model)")
        ax.set_xticks([1e-1, 1e1, 1e3, 1e5])
        ax.minorticks_off()
    axes[0].set_ylabel("phase error at 1.5 m (rad)")
    handles = {}
    for ax in axes:
        for hdl, lab in zip(*ax.get_legend_handles_labels()):
            handles.setdefault(lab, hdl)
    fig.legend(list(handles.values()), list(handles.keys()), loc="upper center", ncol=4,
               bbox_to_anchor=(0.5, 1.18), fontsize=6.5)
    fig.subplots_adjust(wspace=0.08)
    fig.savefig(FIG / "shapes-pareto.pdf")
    plt.close(fig)


def table_cost(data: dict[str, dict]) -> None:
    techs = (TECH[1], TECH[2], TECH[3])
    rows = [r"\begin{tabular}{lrrrrr}", r"\toprule",
            r"shape & (b) sampled fill & (c) fill + centroid & (d) cut-cell moments & (e) clipped pieces"
            r" & whole mesh, direct \\", r"\midrule"]
    for name, d in data.items():
        cells = []
        for t in techs:
            c = np.array([r["cost_ns"][t] for r in d["rows"]]) * 1e-3
            e = np.array([r["err"][t][0] for r in d["rows"]])
            cells.append(" / ".join(short(cost_to_reach(c, e, tol)) for tol in (1e-4, 1e-6, 1e-8)))
        c_e = d["rows"][0]["cost_ns"][TECH[4]] * 1e-3
        cells.append(short(c_e))
        cells.append(short(d["mesh_cost_ns"] * 1e-3))
        rows.append(f"{NAME[name]} & " + " & ".join(cells) + r" \\")
        if name == "suzanne":
            rows.append(r"\midrule")
    rows += [r"\bottomrule", r"\end{tabular}"]
    write("shapes-cost.tex", rows)


# ------------------------------------------------------------------ distance

REG_TECH = [TECH[0], TECH[1], TECH[2], TECH[3], "(e) exact pieces", "(f) body multipole", "(g) point mass"]


def fig_regimes() -> None:
    res = load("regimes-distance.json")
    shown = ("torus", "teapot", "bunny")
    fig, axes = plt.subplots(1, 3, figsize=(6.8, 2.4), sharey=True)
    for ax, name in zip(axes, shown):
        r = res[name]
        d = np.array(r["d"])
        ax.loglog(d, np.abs(r["signal"]), ":", color=INK2, lw=1.0, label="signal")
        ax.axvspan(d.min() / 1.3, 3 * r["h"], color="#e1e0d9", alpha=0.7, lw=0)
        for tol in (1e-6, 1e-10):
            ax.axhline(tol, color=INK2, lw=0.6, ls="--")
        for tech in REG_TECH:
            key = TECH[4] if tech.startswith("(e)") else tech
            e = np.abs(np.array(r["err"][tech]))
            ax.loglog(d, np.maximum(e, 1e-17), color=COLOUR[key], marker="o", ms=1.8, lw=0.9,
                      label=LABEL[key])
        ax.set_title(f"{NAME[name]}, pitch {r['h']:g} m")
        ax.set_xlabel("distance from the surface (m)")
        ax.set_ylim(1e-16, 1e3)
        ax.minorticks_off()
    axes[0].set_ylabel("phase error (rad)")
    h_, l_ = axes[0].get_legend_handles_labels()
    fig.legend(h_, l_, loc="upper center", ncol=4, bbox_to_anchor=(0.5, 1.16), fontsize=6.5)
    fig.subplots_adjust(wspace=0.08)
    fig.savefig(FIG / "shapes-regimes.pdf")
    plt.close(fig)


def table_regimes() -> None:
    res = load("regimes-distance.json")
    names = list(res)
    rows = [r"\begin{tabular}{l" + "r" * len(names) + "}", r"\toprule",
            "technique & " + " & ".join(NAME[n] for n in names) + r" \\",
            "pitch (m) & " + " & ".join(f"{res[n]['h']:g}" for n in names) + r" \\", r"\midrule"]
    for tech in [REG_TECH[k] for k in (1, 2, 3, 5, 6)]:
        key = tech
        cells = []
        for n in names:
            d = np.array(res[n]["d"])
            e = np.array(res[n]["err"][tech])
            cells.append(" / ".join(short(crossover(d, e, tol)) for tol in (1e-6, 1e-10)))
        rows.append(f"{LABEL[key]} & " + " & ".join(cells) + r" \\")
    rows += [r"\bottomrule", r"\end{tabular}"]
    write("shapes-regimes.tex", rows)


# ------------------------------------------------------------------ motion

def spectrum(y: np.ndarray, dx: float) -> tuple[np.ndarray, np.ndarray]:
    """regimes._spectrum: Hann-windowed amplitude spectrum."""
    w = np.hanning(len(y))
    amp = np.abs(np.fft.rfft((y - y.mean()) * w)) * 2 / w.sum()
    return np.fft.rfftfreq(len(y), dx), amp


def fig_motion() -> None:
    r = load("translation.json")
    x, h = np.array(r["x"]), r["h"]
    dx = float(x[1] - x[0])
    fig, axes = plt.subplots(1, 2, figsize=(6.8, 2.2), sharey=True)
    titles = {"body": "(a) voxels fixed in the body", "world": "(b) voxels fixed in the world"}
    for ax, frame in zip(axes, ("body", "world")):
        for tech in TECH[:4]:
            f, a = spectrum(np.array(r[frame][tech]), dx)
            ax.loglog(f[1:], np.maximum(a[1:], 1e-18), color=COLOUR[tech], lw=0.8, label=LABEL[tech])
        for m in range(1, 8):
            ax.axvline(m / h, color="#c3c2b7", lw=0.6, ls=":")
        ax.set_title(titles[frame])
        ax.set_xlabel("spatial frequency (cycles per m; Hz at 1 m/s)")
        ax.set_ylim(1e-15, 2)
        ax.minorticks_off()
    axes[0].set_ylabel("error amplitude (rad)")
    h_, l_ = axes[0].get_legend_handles_labels()
    fig.legend(h_, l_, loc="upper center", ncol=4, bbox_to_anchor=(0.5, 1.1), fontsize=6.5)
    fig.subplots_adjust(wspace=0.06)
    fig.savefig(FIG / "shapes-motion.pdf")
    plt.close(fig)


# ------------------------------------------------------------------ exact polyhedron

EXACT = ["bunny", "spot", "dragon", "armadillo", "buddha", "bracket"]
BODY_COLOUR = dict(zip(EXACT, ["#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#6250d6", "#0b0b0b"]))
POSES = ["upright", "turned 90° about z", "on its side (90° about x)", "point mass"]


def at(d: np.ndarray, y: np.ndarray, x: float) -> float:
    """y at distance x, interpolated in log-log (y > 0)."""
    return float(np.exp(np.interp(np.log(x), np.log(d), np.log(np.maximum(y, 1e-300)))))


def fig_exact() -> None:
    dist, pas = load("exact-distance.json"), load("exact-passby.json")
    fig, axes = plt.subplots(2, 2, figsize=(6.8, 4.4))
    ax = axes[0, 0]
    for n in EXACT:
        r = dist[n]
        d = np.array(r["d"])
        ax.loglog(d, np.abs(r["exact"]), color=BODY_COLOUR[n], label=NAME[n])
        ax.loglog(d, np.abs(r["point"]), color=BODY_COLOUR[n], ls=":", lw=0.9)
    ax.set_xlabel("centre of mass to tower axis (m)")
    ax.set_ylabel("$|$phase$|$ (rad)")
    ax.set_title("(a) the mesh (solid) and a point mass (dotted)")
    ax.legend(fontsize=6, ncol=2, loc="lower left")
    ax = axes[0, 1]
    for key, colour, label in (("point", "#2a78d6", "point mass"), ("l2", "#eb6834", "+ second moments"),
                               ("l3", "#1baf7a", "+ third moments")):
        for k, n in enumerate(EXACT):
            r = dist[n]
            ex = np.array(r["exact"])
            ax.loglog(r["d"], np.maximum(np.abs(np.array(r[key]) - ex) / np.abs(ex), 1e-16), color=colour,
                      lw=0.7, alpha=0.8, label=label if k == 0 else None)
    for k, n in enumerate(("dragon", "armadillo", "buddha")):
        r, full = dist[n], dist[f"{n} (full)"]
        ex, fx = np.array(r["exact"]), np.array(full["exact"])
        ax.loglog(r["d"], np.abs(ex - fx) / np.abs(fx), color="#6250d6", ls="--", lw=0.9,
                  label="decimated against full scan" if k == 0 else None)
    ax.set_xlabel("centre of mass to tower axis (m)")
    ax.set_ylabel("departure from the mesh, relative")
    ax.set_title("(b) series about the centre of mass, six bodies")
    ax.set_ylim(1e-9, 1e2)
    ax.legend(fontsize=6, loc="lower left")
    for ax, n, tag in ((axes[1, 0], "spot", "(c)"), (axes[1, 1], "bracket", "(d)")):
        r = pas[n]
        for pose, colour, ls in zip(POSES, ["#0b0b0b", "#eb6834", "#1baf7a", "#2a78d6"],
                                    ["-", "-", "-", ":"]):
            label = pose.replace(" (90° about x)", "")
            ax.plot(r["t"], r["phase"][pose], color=colour, ls=ls, label=label)
        ax.set_xlabel("time (s), walking past at 1.4 m/s, 2 m off the axis")
        ax.set_title(f"{tag} {NAME[n]} walking past, three poses")
        ax.set_xlim(-10, 10)
        lo = min(min(r["phase"][p]) for p in POSES)
        ax.set_ylim(1.5 * lo, 0.1 * abs(lo))
        ax.legend(fontsize=6, loc="lower center", ncol=2)
    axes[1, 0].set_ylabel("phase (rad)")
    fig.subplots_adjust(hspace=0.45, wspace=0.28)
    fig.savefig(FIG / "shapes-exact.pdf")
    plt.close(fig)


def table_exact() -> None:
    dist, pas, mom = load("exact-distance.json"), load("exact-passby.json"), load("exact-moments.json")
    rows = [r"\begin{tabular}{lrrrrrrrr}", r"\toprule",
            r"mesh & triangles & mass (kg) & \multicolumn{4}{c}{peak phase walking past (rad)} & "
            r"poses differ & point mass off by \\",
            r"\cmidrule(lr){4-7}",
            r" & & & upright & turned & on side & point mass & by up to & 1.5 / 5 / 20 m \\", r"\midrule"]
    for n in EXACT:
        peaks = [float(np.max(np.abs(pas[n]["phase"][p]))) for p in POSES]
        # exact.md's "largest pose difference": the widest gap between poses at any one time.
        poses = np.array([pas[n]["phase"][p] for p in POSES[:3]])
        spread = float(np.max(poses.max(axis=0) - poses.min(axis=0)))
        r = dist[n]
        d, ex, pt = np.array(r["d"]), np.array(r["exact"]), np.array(r["point"])
        dep = np.abs(pt - ex) / np.abs(ex)
        far = " / ".join(sci(at(d, dep, x), 0) for x in (1.5, 5.0, 20.0))
        tri = f"{mom[n]['triangles']:,}".replace(",", "\\,")
        rows.append(f"{NAME[n]} & {tri} & {mom[n]['mass_kg']:.1f} & "
                    + " & ".join(f"{p:.2f}" for p in peaks)
                    + f" & {spread:.2f} ({100 * spread / max(peaks[:3]):.0f}\\%) & {far} \\\\")
    rows += [r"\bottomrule", r"\end{tabular}"]
    write("shapes-exact.tex", rows)


def table_decimation() -> None:
    chk, dist = load("mesh-check.json"), load("exact-distance.json")
    rows = [r"\begin{tabular}{lrrrrrrrl}", r"\toprule",
            r"mesh & original & used & \multicolumn{2}{c}{original $\to$ used (m)} & vertices & "
            r"volume & signal & verdict \\",
            r"\cmidrule(lr){4-5}",
            r" & triangles & triangles & max & RMS & beyond 3.9 mm & change & change, 1.5 m (rad) & \\",
            r"\midrule"]
    for n in ("dragon", "armadillo", "buddha"):
        dec = chk[n]["decimation"]
        r, full = dist[n], dist[f"{n} (full)"]
        diff = np.abs(np.array(r["exact"]) - np.array(full["exact"]))
        rows.append(
            f"{NAME[n]} & {dec['original_triangles']:,} & {dec['decimated_triangles']:,} & ".replace(",", "\\,")
            + f"{sci(dec['max_original_to_decimated_m'])} & {sci(dec['rms_original_to_decimated_m'])} & "
            f"{dec['original_vertices_beyond_tol']:,} & ".replace(",", "\\,")
            + f"{sci(dec['volume_rel'])} & {sci(at(np.array(r['d']), diff, 1.5))} & "
            + ("pass" if dec["passed"] else r"\textbf{fail}") + r" \\")
    rows += [r"\bottomrule", r"\end{tabular}"]
    write("shapes-decimation.tex", rows)


# ------------------------------------------------------------------ check against the note

def check(data: dict[str, dict]) -> None:
    """A few numbers the study's note quotes (§3.1-§3.4), recomputed here; stop if one differs."""
    def err(name: str, t: str) -> float:
        return abs(data[name]["rows"][-1]["err"][t][0])

    def cost(name: str, t: str, tol: float) -> float:
        d = data[name]
        return cost_to_reach(np.array([r["cost_ns"][t] for r in d["rows"]]) * 1e-3,
                             np.array([r["err"][t][0] for r in d["rows"]]), tol)

    res = load("regimes-distance.json")
    expect = [
        ("sphere (c) at 128", err("sphere", TECH[2]), 8.9e-8),
        ("sphere (d) at 128", err("sphere", TECH[3]), 9.6e-8),
        ("bracket (d) at 128", err("bracket", TECH[3]), 7.3e-11),
        ("teapot (d) to 1e-6 (us)", cost("teapot", TECH[3], 1e-6), 14.0),
        ("buddha (d) to 1e-8 (us)", cost("buddha", TECH[3], 1e-8), 116.0),
        ("teapot (d) beyond 1e-6 (m)", crossover(np.array(res["teapot"]["d"]),
                                                 np.array(res["teapot"]["err"][TECH[3]]), 1e-6), 1.2),
    ]
    bad = [(k, got, want) for k, got, want in expect if abs(got / want - 1) > 0.06]
    for k, got, want in expect:
        print(f"check {k}: {got:.3g} (note: {want:.3g})")
    if bad:
        sys.exit(f"shapes.py: numbers differ from the study's note: {bad}")


def main() -> None:
    FIG.mkdir(exist_ok=True)
    style()
    data = sweep_data()
    check(data)
    for s in RENDERS:
        crop_render(s)
    fig_sweep(data)
    table_sweep(data)
    fig_pareto(data)
    table_cost(data)
    fig_regimes()
    table_regimes()
    fig_motion()
    fig_exact()
    table_exact()
    table_decimation()


if __name__ == "__main__":
    main()
