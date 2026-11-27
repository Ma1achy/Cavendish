"""Tables and figures from data/boxes-*.json and data/precision-*.json.

    python analyse.py        # writes data/tables.md, figures/boxes.pdf|png, figures/precision.pdf|png

Every number in the note comes from data/tables.md, which this script writes from the JSON.
"""

from __future__ import annotations

import json

import matplotlib.pyplot as plt
import numpy as np

import common as C

STRAT_LABEL = {"cells": "every cell", "octree": "octree blocks", "octmerge": "octree, merged",
               "sweep": "runs merged (sweep)", "darnell": "Darnell growth", "largest": "largest first"}
STRAT_COLOUR = {"cells": C.INK2, "octree": C.BLUE, "octmerge": C.SKY, "sweep": C.GREEN,
                "darnell": C.ORANGE, "largest": C.PURPLE}
NEAR, MID, FAR = 0, 2, 5  # indices into DIST: 0.1 m, 1.5 m, 100 m
TOLS = (1e-4, 1e-6, 1e-8)


def load() -> dict[str, dict]:
    out = {}
    for s in C.SHAPES:
        p = C.DATA / f"boxes-{s}.json"
        if p.exists():
            out[s] = json.loads(p.read_text())
    return out


def slope(n: np.ndarray, y: np.ndarray) -> float:
    ok = (n >= 16) & (y > 0)
    return float(np.polyfit(np.log(n[ok]), np.log(y[ok]), 1)[0]) if ok.sum() >= 3 else float("nan")


def cost_to_reach(cost: np.ndarray, err: np.ndarray, tol: float) -> float:
    """As the shapes study's sweep.cost_to_reach: first pitch below tol, log-log interpolated."""
    e = np.abs(err)
    below = np.nonzero(e < tol)[0]
    if len(below) == 0:
        return float("inf")
    k = int(below[0])
    if k == 0:
        return float(cost[0])
    lc, le = np.log(cost[k - 1:k + 1]), np.log(np.maximum(e[k - 1:k + 1], 1e-300))
    return float(np.exp(lc[0] + (np.log(tol) - le[0]) * (lc[1] - lc[0]) / (le[1] - le[0])))


def interior_costs(r: dict, i: int) -> dict[str, float]:
    """Interior cost (ns) per evaluation point at distance index i, by representation."""
    s = r["strategies"]
    return {
        "cells, all series (paper's model)": float(s["cells"]["cost_interior_all_series_ns"]),
        "cells, strict switch": s["cells"]["cost_interior_ns"][i],
        "octree, strict switch": s["octree"]["cost_interior_strict_ns"][i],
        "octree, S3 switch": s["octree"]["cost_interior_s3_ns"][i],
        "free boxes (Darnell), closed form": s["darnell"]["cost_interior_ns"][i],
        "free boxes (Darnell), strict switch": s["darnell"]["cost_interior_strict_ns"][i],
        "free boxes (sweep), strict switch": s["sweep"]["cost_interior_strict_ns"][i],
    }


def interior_extra_err(r: dict, key: str, i: int) -> float:
    """The interior's own error at distance i: 0 for exact ones (A2 rounding), the series
    truncation for the octree read with a switch."""
    o = r["strategies"]["octree"]
    if key == "octree, strict switch":
        return o["diff_vs_cells_strict"][i]
    if key == "octree, S3 switch":
        return o["diff_vs_cells_s3"][i]
    return 0.0


def tables(data: dict[str, dict]) -> list[str]:
    md: list[str] = []
    # ---- A5/A1: counts at 128 and growth exponents
    md += ["## A. Counts at 128 cells across, and growth with N (fitted exponent over 16-128)\n",
           "| shape | interior cells | boundary cells | " + " | ".join(STRAT_LABEL[s] for s in STRAT_LABEL if s != "cells")
           + " | exponent: cells / boundary / octree / Darnell |", "|---" * 9 + "|"]
    for name, d in data.items():
        r = next((x for x in d["rows"] if x["n"] == 128), None)
        if r is None:
            continue
        ns = np.array([x["n"] for x in d["rows"]])
        ex = [slope(ns, np.array([x["interior"] for x in d["rows"]], float)),
              slope(ns, np.array([x["boundary"] for x in d["rows"]], float)),
              slope(ns, np.array([x["strategies"]["octree"]["boxes"] for x in d["rows"]], float)),
              slope(ns, np.array([x["strategies"]["darnell"]["boxes"] for x in d["rows"]], float))]
        md.append(f"| {C.NAME[name]} | {r['interior']} | {r['boundary']} | "
                  + " | ".join(str(r["strategies"][s]["boxes"]) for s in STRAT_LABEL if s != "cells")
                  + " | " + " / ".join(f"{e:.2f}" for e in ex) + " |")
    # ---- coverage: boxes to cover 90 % and 99 % of the interior volume, at 128 across
    md += ["\n## Boxes (largest first) to cover 90 % / 99 % of the interior volume, 128 across\n",
           "| shape | " + " | ".join(STRAT_LABEL[s] for s in STRAT_LABEL if s != "cells") + " |", "|---" * 6 + "|"]
    for name, d in data.items():
        r = next((x for x in d["rows"] if x["n"] == 128), None)
        if r is None:
            continue
        cells = []
        for s in [x for x in STRAT_LABEL if x != "cells"]:
            v = np.cumsum(r["strategies"][s]["coverage"]) / r["interior"]
            cells.append(f"{int(np.searchsorted(v, 0.9)) + 1} / {int(np.searchsorted(v, 0.99)) + 1}")
        md.append(f"| {C.NAME[name]} | " + " | ".join(cells) + " |")
    # ---- A1, A2 verdicts
    worst = {s: 0.0 for s in STRAT_LABEL if s != "cells"}
    part_ok, round_ok, ctrl = True, True, []
    for d in data.values():
        for r in d["rows"]:
            for s in worst:
                st = r["strategies"][s]
                part_ok &= bool(st["partition_ok"])
                round_ok &= bool(st["rounding_ok"])
                worst[s] = max(worst[s], max(abs(x) for x in st["diff_vs_cells"]))
            dn = r["strategies"]["darnell"]
            if "control_partition_ok" in dn:
                ctrl.append((dn["control_partition_ok"], abs(dn["control_diff_vs_cells"][MID])))
    md += ["\n## A1, A2: partition and rounding\n",
           f"- A1 every decomposition partitions the interior: **{part_ok}**; control (largest box dropped) "
           f"fails the partition check in {sum(not c[0] for c in ctrl)} of {len(ctrl)} cases.",
           f"- A2 boxes against cells within max(1e-11 rad, 1e-14 of the interior phase) at every pitch and "
           f"distance: **{round_ok}**. Largest |difference| (rad): "
           + ", ".join(f"{STRAT_LABEL[s]} {v:.1e}" for s, v in worst.items())
           + f". Control at 1.5 m: smallest {min((c[1] for c in ctrl), default=float("nan")):.1e} rad (criterion > 1e-6)."]
    # Which side of an A2 miss is off: the box decompositions against each other.
    cross, where = 0.0, ("", 0, 0.0, 0.0)
    for name, d in data.items():
        for r in d["rows"]:
            dv = {s: np.array(r["strategies"][s]["diff_vs_cells"]) for s in worst}
            spread = np.max([np.abs(dv[a] - dv[b]) for a in dv for b in dv], axis=0)
            cross = max(cross, float(spread.max()))
            mean = np.mean([dv[s] for s in dv], axis=0)
            i = int(np.argmax(np.abs(mean)))
            if abs(mean[i]) > abs(where[3]):
                where = (name, r["n"], d["distances"][i], float(mean[i]))
    md.append(f"- The five box decompositions agree with one another to {cross:.1e} rad everywhere; their common "
              f"difference from the cell-by-cell sum is largest for {where[0]} at {where[1]} across, "
              f"{where[2]:g} m ({where[3]:.1e} rad).")
    # ---- A3 reproduction
    rep = [abs(r["err_boundary"]["d"][MID] - r["sweep_d_1.5m"]) for d in data.values() for r in d["rows"]
           if r["n"] == 128]
    md.append(f"- A3 (d) at 1.5 m, 128 across, against the shapes study: largest difference {max(rep, default=float("nan")):.1e} rad.")
    # ---- the octree's switch rules
    md += ["\n## A5: the octree read with a series switch, extra error at 1.5 m (rad), 128 across\n",
           "| shape | octree: strict (0.01) / 0.05 / S3 (1/6) | free boxes (sweep): strict / 0.05 / S3 | interior cost at 1.5 m (us), octree: strict / 0.05 / S3 | free boxes: strict / 0.05 / S3 |",
           "|---|---|---|---|---|"]
    for name, d in data.items():
        r = next((x for x in d["rows"] if x["n"] == 128), None)
        if r is None:
            continue
        o = r["strategies"]["octree"]
        sw = r["strategies"]["sweep"]
        keys = ("strict", "r0.05", "s3")
        md.append(f"| {C.NAME[name]} | " + " / ".join(f"{abs(o[f'diff_vs_cells_{k}'][MID]):.1e}" for k in keys)
                  + " | " + " / ".join(f"{abs(sw[f'diff_vs_cells_{k}'][MID]):.1e}" for k in keys)
                  + " | " + " / ".join(f"{o[f'cost_interior_{k}_ns'][MID] * 1e-3:.3g}" for k in keys)
                  + " | " + " / ".join(f"{sw[f'cost_interior_{k}_ns'][MID] * 1e-3:.3g}" for k in keys) + " |")
    # ---- error and cost near / mid / far at 128
    for i, label in ((NEAR, "0.1 m"), (MID, "1.5 m"), (FAR, "100 m")):
        md += [f"\n## Error (rad) and model cost per point (us) at {label} from the surface, 128 across\n",
               "| shape | signal | whole mesh | cells (paper's model) + (d) | octree strict + (d) | octree S3 + (d) "
               "| free boxes + (d) | free boxes + (e) | (d) error | (e) error |", "|---" * 10 + "|"]
        for name, d in data.items():
            r = next((x for x in d["rows"] if x["n"] == 128), None)
            if r is None:
                continue
            ic = interior_costs(r, i)
            cb = r["cost_boundary_ns"]
            ed, ee = r["err_boundary"]["d"][i], r["err_boundary"]["e"][i]
            es3 = abs(ed + interior_extra_err(r, "octree, S3 switch", i))
            md.append(f"| {C.NAME[name]} | {d['signal'][i]:.2e} | {d['mesh_cost_ns'] * 1e-3:.3g} | "
                      f"{(ic['cells, all series (paper' + chr(39) + 's model)'] + cb['d']) * 1e-3:.3g} | "
                      f"{(ic['octree, strict switch'] + cb['d']) * 1e-3:.3g} | "
                      f"{(ic['octree, S3 switch'] + cb['d']) * 1e-3:.3g} (err {es3:.1e}) | "
                      f"{(ic['free boxes (sweep), strict switch'] + cb['d']) * 1e-3:.3g} | "
                      f"{(ic['free boxes (sweep), strict switch'] + cb['e']) * 1e-3:.3g} | "
                      f"{abs(ed):.1e} | {abs(ee):.1e} |")
    # ---- corrected Table 5
    md += ["\n## Corrected cost to reach 1e-4 / 1e-6 / 1e-8 rad at 1.5 m (us per point, model), 4-128 across\n",
           "Interior as: every cell at 8.5 ns each (the paper's Table 5; the series' own error not "
           "included) / the octree's blocks / free boxes (sweep). For the octree and the boxes the series "
           "switch is the cheapest of strict, 0.05 and S3 that reaches the tolerance, the interior's own "
           "error (series truncation) added to the boundary's. Boundary: (a) 8.5 ns per cell whose centre "
           "is inside, (b) 8.5 ns per cut cell (errors from the shapes study's sweep), (c) 8.5 ns, (d) 17 ns "
           "per cut cell, (e) 55 ns per piece triangle. — not reached within 128 across.\n",
           "| shape | whole mesh | (d), cells (paper) | (d), octree | (d), free boxes | (c), octree | (b), octree "
           "| (a), octree | (e), free boxes |",
           "|---|---|---|---|---|---|---|---|---|"]
    rank: list[tuple[str, float, dict[str, float], float]] = []
    for name, d in data.items():
        rows = d["rows"]
        if not any(x["n"] == 128 for x in rows):
            continue
        sweep = json.loads((C.STUDY / f"sweep-{name}.json").read_text())
        srow = {r["n"]: r for r in sweep["rows"]}
        ns = [x["n"] for x in rows]
        cb = {k: np.array([x["cost_boundary_ns"][k] for x in rows]) for k in "cde"}
        cb["a"] = np.array([srow[n]["cost_ns"]["(a) cubes"] - srow[n]["interior"] * 8.5 for n in ns])
        cb["b"] = np.array([srow[n]["boundary"] * 8.5 for n in ns], float)
        err = {k: np.array([x["err_boundary"][k][MID] for x in rows]) for k in "cde"}
        err["a"] = np.array([srow[n]["err"]["(a) cubes"][0] for n in ns])
        err["b"] = np.array([srow[n]["err"]["(b) sampled fill"][0] for n in ns])
        old = np.array([x["strategies"]["cells"]["cost_interior_all_series_ns"] for x in rows], float)

        def best(strat: str, b: str, t: float) -> float:
            out = float("inf")
            for key in ("strict", "r0.05", "s3"):
                st = [x["strategies"][strat] for x in rows]
                c = np.array([s[f"cost_interior_{key}_ns"][MID] for s in st]) + cb[b]
                e = err[b] + np.array([s[f"diff_vs_cells_{key}"][MID] for s in st])
                out = min(out, cost_to_reach(c * 1e-3, e, t))
            return out

        fmt = lambda vals: " / ".join("—" if not np.isfinite(v) else f"{v:.3g}" for v in vals)  # noqa: E731
        paper = [cost_to_reach((old + cb["d"]) * 1e-3, err["d"], t) for t in TOLS]
        cols = {"d_oct": [best("octree", "d", t) for t in TOLS], "d_box": [best("sweep", "d", t) for t in TOLS],
                "c_oct": [best("octree", "c", t) for t in TOLS], "b_oct": [best("octree", "b", t) for t in TOLS],
                "a_oct": [best("octree", "a", t) for t in TOLS], "e_box": [best("sweep", "e", t) for t in TOLS]}
        mesh_us = d["mesh_cost_ns"] * 1e-3
        md.append(f"| {C.NAME[name]} | {mesh_us:.3g} | {fmt(paper)} | {fmt(cols['d_oct'])} | {fmt(cols['d_box'])} | "
                  f"{fmt(cols['c_oct'])} | {fmt(cols['b_oct'])} | {fmt(cols['a_oct'])} | {fmt(cols['e_box'])} |")
        for i, t in enumerate(TOLS):
            rank.append((name, t, {k: v[i] for k, v in cols.items()} | {"paper": paper[i]}, mesh_us))
    md.append("\nWhole mesh against the cheapest voxel route (d, octree or free boxes) at each tolerance: "
              + "; ".join(f"{C.NAME[n]} {t:.0e}: {'voxels' if min(c['d_oct'], c['d_box']) < m else 'mesh'}"
                          f" (paper's model: {'voxels' if c['paper'] < m else 'mesh'})" for n, t, c, m in rank) + ".")
    order_ok = [(n, t) for n, t, c, _ in rank
                if not (c["d_oct"] <= c["c_oct"] <= c["b_oct"] or not np.isfinite(c["c_oct"]))]
    md.append(f"\nCases where (d) is not the cheapest of (d), (c), (b) on the octree: {order_ok or 'none'}.")
    return md


def figure_boxes(data: dict[str, dict]) -> None:
    C.style()
    fig, axes = plt.subplots(1, 3, figsize=(6.8, 2.3))
    ax = axes[0]
    full = [d for d in data.values() if len(d["rows"]) == 6]
    ns = np.array([x["n"] for x in full[0]["rows"]]) if full else np.array([])
    series = {s: (lambda x, s=s: x["strategies"][s]["boxes"] if s != "cells" else x["interior"])
              for s in STRAT_LABEL}
    series["cut"] = lambda x: x["boundary"]
    for s, get in series.items():
        if not full:
            break
        y = np.array([[max(get(x), 0.5) for x in d["rows"]] for d in full], float)
        col = C.VERMILION if s == "cut" else STRAT_COLOUR[s]
        lab = "cut cells (boundary)" if s == "cut" else STRAT_LABEL[s]
        ax.loglog(ns, np.median(y, axis=0), "o" + (":" if s == "cut" else "-"), color=col, ms=2, lw=1, label=lab)
        ax.fill_between(ns, y.min(axis=0), y.max(axis=0), color=col, alpha=0.12, lw=0)
    if len(ns):
        ax.loglog(ns, 2.0 * ns**3, "--", color=C.INK, lw=0.5)
        ax.loglog(ns, 0.4 * ns**2, "--", color=C.INK, lw=0.5)
        ax.text(ns[-1], 2.0 * ns[-1] ** 3, r"$N^3$", fontsize=6, ha="right", va="bottom")
        ax.text(ns[-1], 0.4 * ns[-1] ** 2, r"$N^2$", fontsize=6, ha="left", va="top")
    ax.set_ylim(0.5, None)
    ax.set_xlabel("cells across, N")
    ax.set_ylabel("elements")
    ax.set_title(f"(a) median and range over {len(full)} shapes")
    ax.legend(fontsize=5, loc="upper left")
    ax = axes[1]
    for s in [x for x in STRAT_LABEL if x != "cells"]:
        for k, name in enumerate(data):
            r = next((x for x in data[name]["rows"] if x["n"] == 128), None)
            if r is None:
                continue
            v = np.array(r["strategies"][s]["coverage"], float)
            ax.semilogx(np.arange(1, len(v) + 1), np.cumsum(v) / v.sum(), color=STRAT_COLOUR[s], lw=0.7,
                        alpha=0.8, label=STRAT_LABEL[s] if k == 0 else None)
    ax.set_xlabel("boxes, largest first")
    ax.set_ylabel("share of interior volume")
    ax.set_title("(b) coverage at 128 across")
    ax = axes[2]
    name = "bunny" if "bunny" in data else next(iter(data))
    d = data[name]
    rows = d["rows"]
    err_d = np.abs([x["err_boundary"]["d"][MID] for x in rows])
    cb = np.array([x["cost_boundary_ns"]["d"] for x in rows])
    for lab, ic, col, mk in (
            ("(d), every cell (paper)", [x["strategies"]["cells"]["cost_interior_all_series_ns"] for x in rows], C.INK2, "o"),
            ("(d), octree", [x["strategies"]["octree"]["cost_interior_strict_ns"][MID] for x in rows], C.BLUE, "s"),
            ("(d), free boxes", [x["strategies"]["sweep"]["cost_interior_strict_ns"][MID] for x in rows], C.ORANGE, "^")):
        ax.loglog((np.array(ic, float) + cb) * 1e-3, np.maximum(err_d, 1e-15), "-" + mk, color=col, ms=2.5,
                  label=lab)
    err_e = np.abs([x["err_boundary"]["e"][MID] for x in rows])
    ce = np.array([x["cost_boundary_ns"]["e"] + x["strategies"]["sweep"]["cost_interior_strict_ns"][MID] for x in rows])
    ax.loglog(ce * 1e-3, np.maximum(err_e, 1e-15), "-v", color=C.VERMILION, ms=2.5, label="(e), free boxes")
    ax.loglog([d["mesh_cost_ns"] * 1e-3], [1e-15], "*", color=C.INK, ms=7, label="whole mesh")
    ax.set_xlabel("cost per point (us, model)")
    ax.set_ylabel("|phase error| at 1.5 m (rad)")
    ax.set_title(f"(c) {C.NAME[name]}: error against cost")
    ax.legend(fontsize=5, loc="lower left")
    fig.tight_layout()
    for ext in ("pdf", "png"):
        fig.savefig(C.FIG / f"boxes.{ext}", dpi=200)
    plt.close(fig)


def precision_tables() -> list[str]:
    md: list[str] = []
    k = json.loads((C.DATA / "precision-kernel.json").read_text())
    ratios = np.array(k["ratios"])
    eps = k["eps32"] / 2  # unit roundoff 6e-8
    md += ["\n## B1: one element, relative error of g_z against R/s (f64 reference)\n",
           "| element | variant | " + " | ".join(f"{q:.0e}" for q in ratios[::4]) + " |", "|---|---|" + "---|" * 6]
    for name, v in k["shapes"].items():
        for var, e in v["rel_err"].items():
            e = np.array(e)
            vals = e if "40" in var else e[::4]
            md.append(f"| {name} | {var} | " + " | ".join(f"{x:.0e}" for x in vals) + " |")
    md.append("| bound | 30 u (R/s)^2, u = 6e-8 | " + " | ".join(f"{30 * eps * q * q:.0e}" for q in ratios[::4]) + " |")
    for p in sorted(C.DATA.glob("precision-diagnose-*.json")):
        dg = json.loads(p.read_text())
        md += [f"\n## B: which f32 ingredient makes the error ({dg['shape']}, whole mesh, {dg['triangles']} "
               "triangles; terms summed exactly)\n",
               "| ingredient in f32 | " + " | ".join(f"{x:g} m" for x in dg["distances"]) + " |",
               "|---|" + "---|" * len(dg["distances"])]
        for part, v in dg["phase"].items():
            md.append(f"| {part if part != 'f64' else 'phase itself (f64)'} | " + " | ".join(f"{x:.1e}" for x in v) + " |")
    for p in sorted(C.DATA.glob("precision-*.json")):
        if "kernel" in p.name or "diagnose" in p.name:
            continue
        b = json.loads(p.read_text())
        for r in b["rows"]:
            md += [f"\n## B2-B4: {b['shape']} at {r['n']} cells across ({r['blocks']} octree blocks, "
                   f"{r['boundary']} cut cells, {r['piece_tris']} piece triangles): phase error of the f32 "
                   "evaluation (rad)\n",
                   f"f32 cut-cell moments: total mass relative error {r['mass_rel_err_f32_moments']:.1e}; "
                   f"largest centroid error {r.get('max_centroid_err_f32_over_h', float('nan')):.1e} h.\n",
                   "| part, frame, sum | " + " | ".join(f"{x:g} m" for x in b["distances"]) + " |",
                   "|---|" + "---|" * len(b["distances"])]
            for key in sorted(r["parts"]):
                v = r["parts"][key]
                md.append(f"| {key.replace('|', ', ')} | " + " | ".join(f"{x:.1e}" for x in v) + " |")
    return md


def figure_precision() -> None:
    C.style()
    k = json.loads((C.DATA / "precision-kernel.json").read_text())
    ratios = np.array(k["ratios"])
    u = k["eps32"] / 2
    fig, axes = plt.subplots(1, 2, figsize=(6.8, 2.4))
    ax = axes[0]
    style = {"f32 local h2A log1p": (C.BLUE, "-"), "f32 local triple log1p": (C.VERMILION, "-"),
             "f32 local h2A ratio-log": (C.ORANGE, "-"), "f32 world 1 km h2A log1p": (C.PURPLE, "-"),
             "f32 local h2A, WGSL worst case": (C.INK2, "-")}
    for var, (col, ls) in style.items():
        ax.loglog(ratios, k["shapes"]["box 1:1:1"]["rel_err"][var], ls, color=col, lw=0.9,
                  label=var.replace("f32 ", ""))
    f64 = k["shapes"]["box 1:1:1"]["rel_err"]["f64 vs 40 digits"]
    ax.loglog(ratios[::4], f64, "o-", color=C.GREEN, ms=2.5, lw=0.9, label="f64 against 40 digits")
    ax.loglog(ratios, 30 * u * ratios**2, "--", color=C.INK, lw=0.7, label=r"$30u(R/s)^2$, f32")
    ax.set_ylim(1e-17, 1e3)
    ax.set_xlabel("R/s")
    ax.set_ylabel("relative error of $g_z$")
    ax.set_title("(a) one cube, closed form")
    ax.legend(fontsize=4.8, loc="upper left")
    ax = axes[1]
    p = C.DATA / "precision-bunny.json"
    if p.exists():
        b = json.loads(p.read_text())
        r = b["rows"][-1]
        dist = np.array(b["distances"])
        parts = r["parts"]
        for key, col, lab in (("mesh|signal", C.INK, "phase (whole mesh)"),
                              ("mesh|local|ff", C.VERMILION, "closed form, whole mesh"),
                              ("pieces|local|ff", C.ORANGE, "closed form, clipped pieces"),
                              ("blocks|local|ff", C.PURPLE, "closed form, octree blocks"),
                              ("moments|local|ff", C.BLUE, "cut-cell moments"),
                              ("blockseries|local|ff", C.SKY, "octree blocks, series")):
            if key in parts:
                ax.loglog(dist, np.maximum(np.abs(parts[key]), 1e-16), "-o" if "signal" not in key else ":",
                          color=col, ms=2, lw=0.9, label=lab)
        ax.axhline(1e-10, color=C.GREEN, lw=0.7, ls="--")
        ax.text(dist[-1], 1.6e-10, "PHASE_TOLERANCE", fontsize=5, color=C.GREEN, ha="right")
        ax.axhline(1e-6, color=C.INK2, lw=0.7, ls="--")
        ax.text(dist[-1], 1.6e-6, "1e-6 rad (noise criterion)", fontsize=5, color=C.INK2, ha="right")
        ax.set_xlabel("distance from the surface (m)")
        ax.set_ylabel("|phase error| of f32 (rad)")
        ax.set_title(f"(b) bunny, {r['n']} across, f32 terms, float-float sums")
        ax.legend(fontsize=4.8, loc="lower left")
    fig.tight_layout()
    for ext in ("pdf", "png"):
        fig.savefig(C.FIG / f"precision.{ext}", dpi=200)
    plt.close(fig)


if __name__ == "__main__":
    data = load()
    md = tables(data) + precision_tables()
    (C.DATA / "tables.md").write_text("# Tables (written by analyse.py)\n\n" + "\n".join(md) + "\n", encoding="utf-8")
    figure_boxes(data)
    figure_precision()
    print("wrote data/tables.md and figures")
