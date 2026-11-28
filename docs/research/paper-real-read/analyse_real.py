"""Tables, checks and figures from the study's JSON (data/), in the paper's style.

    python analyse_real.py sweep      # data/sweep-summary.json, tables-sweep.md, fig-sweep.pdf/.png
    python analyse_real.py conv       # tables-conv.md (Table 2 and Table 8's measured rows)
    python analyse_real.py regimes    # tables-regimes.md (Table 6, Q5, orientation, Table 7's column)
    python analyse_real.py budget     # data/budget.json, tables-budget.md, fig-bias.pdf/.png

Definitions are the paper's: an order is the least-squares slope of log|error| against log h over
size/h ≥ 8 (sweep) or h = 1/8 … 1/64 m (benchmark); a crossover is the smallest sampled distance
beyond which |error| stays below the tolerance; the budget's threshold is the pitch below which the
worst placement's max |bias|/σ stays < 0.1 (log–log interpolation, extrapolated beyond 1/128 m).
Over placements an error is the RMS (as the paper's Table 2); the spread is min–max.
"""

from __future__ import annotations

import json
import math
import sys

import numpy as np

import realread as R

sys.path.insert(0, str(R.RESEARCH / "paper-flight-phase"))
import style as S  # noqa: E402

DATA = R.HERE / "data"
HERE = R.HERE
SHAPES = ["sphere", "torus", "teapot", "suzanne", "bunny", "spot", "dragon", "armadillo", "buddha", "bracket"]
LABEL = {"suzanne": "Suzanne", "spot": "Spot", "armadillo": "Armadillo", "buddha": "Buddha", "bracket": "mk7 part"}
TECH = ["(a) cubes", "(b) sampled fill", "(c) fill at centroid", "(d) + second moment", "(e) clipped pieces",
        "(c★) analytic fill", "(d★) analytic + second"]
SHORT = dict(zip(TECH, ["(a)", "(b)", "(c)", "(d)", "(e)", "(c★)", "(d★)"]))
OLD_SHAPES = R.RESEARCH / "exact-skin-and-partial-fill" / "shapes"


def _j(path):  # noqa: ANN001, ANN202
    return json.loads(path.read_text(encoding = "utf-8"))


def order(h: np.ndarray, e: np.ndarray) -> float:
    """sweep.order: slope of log|e| on log h; nan if fewer than 3 values above 1e-13 rad."""
    ok = np.abs(e) > 1e-13
    if ok.sum() < 3 or np.abs(e).max() < 1e-10:
        return float("nan")
    return float(np.polyfit(np.log(h[ok]), np.log(np.abs(e[ok])), 1)[0])


def cost_to_reach(cost: np.ndarray, err: np.ndarray, tol: float) -> float:
    e = np.abs(err)
    below = np.nonzero(e < tol)[0]
    if len(below) == 0:
        return float("inf")
    k = int(below[0])
    if k == 0:
        return float(cost[0])
    lc, le = np.log(cost[k - 1 : k + 1]), np.log(e[k - 1 : k + 1])
    return float(np.exp(lc[0] + (np.log(tol) - le[0]) * (lc[1] - lc[0]) / (le[1] - le[0])))


def fmt(x: float, digits: int = 1) -> str:
    if x is None or not np.isfinite(x):
        return "—"
    if x == 0:
        return "0"
    e = int(math.floor(math.log10(abs(x))))
    m = x / 10**e
    if round(m, digits) >= 10:
        m, e = m / 10, e + 1
    return f"{m:.{digits}f}e{e:+d}".replace("e+", "e").replace("e-0", "e-").replace("e0", "e")


# ======================================= SWEEP =======================================


def _sweep_runs() -> dict[str, list[dict]]:
    out = {}
    for name in SHAPES:
        runs = [_j(p) for p in sorted(DATA.glob(f"sweep-{name}-s*.json"))]
        if runs:
            out[name] = runs
    return out


def sweep() -> None:
    runs = _sweep_runs()
    summ: dict[str, object] = {}
    md: list[str] = []
    checks = {"R3": {}, "R5": {}, "R8": {}}
    for name, rs in runs.items():
        ns = sorted({r["n"] for run in rs for r in run["rows"]})
        full = [n for n in ns if all(any(r["n"] == n for r in run["rows"]) for run in rs)]
        techs = [t for t in TECH if t in rs[0]["rows"][0]["err"]]
        ent: dict[str, object] = {"seeds": [run["seed"] for run in rs], "n": full, "clearance": rs[0]["clearance"],
                                  "signal": rs[0]["signal"], "size": rs[0]["size"]}
        if "facet_err" in rs[0]:
            ent["facet_err"] = rs[0]["facet_err"]
        hs = np.array([[r["h"] for r in run["rows"] if r["n"] in full] for run in rs][0])
        per_tech = {}
        for t in techs:
            # err[seed, n, read(6)]
            e = np.array([[next(r for r in run["rows"] if r["n"] == n)["err"][t] for n in full] for run in rs])
            rms = np.sqrt(np.mean(e**2, axis = 0))  # [n, 6]
            fit = hs >= 0  # every pitch from size/h = 8: drop n = 4
            sel = np.array([n >= 8 for n in full])
            orders = {k: order(hs[sel], rms[sel, k]) for k in range(6)}
            seed_orders = [[order(hs[sel], e[s, sel, k]) for k in range(6)] for s in range(len(rs))]
            per_tech[t] = {"rms": rms.tolist(), "min": np.abs(e).min(axis = 0).tolist(),
                           "max": np.abs(e).max(axis = 0).tolist(), "by_seed": e.tolist(),
                           "order_rms": [orders[k] for k in range(6)], "order_by_seed": seed_orders}
            del fit
        ent["tech"] = per_tech
        # costs (model) from seed 0, which differ little between placements
        ent["cost_ns"] = {t: [next(r for r in rs[0]["rows"] if r["n"] == n)["cost_ns"].get(t, np.nan) for n in full]
                          for t in techs if t in rs[0]["rows"][0]["cost_ns"]}
        ent["mesh_cost_ns"] = rs[0]["mesh_cost_ns"]
        summ[name] = ent
        # R3: seed 0's leading read against the committed sweep
        old = _j(OLD_SHAPES / f"sweep-{name}.json")
        s0 = next((run for run in rs if run["seed"] == 0), None)
        if s0 is not None:
            worst_abs, worst_rel_big = 0.0, 0.0
            for r in s0["rows"]:
                ro = next((x for x in old["rows"] if x["n"] == r["n"]), None)
                if ro is None:
                    continue
                for t in r["err"]:
                    if t not in ro["err"]:
                        continue
                    new, o = np.array(r["err"][t][3:5]), np.array(ro["err"][t])
                    diff = np.abs(new - o)
                    worst_abs = max(worst_abs, float(diff.max()))
                    big = np.abs(o) > 1e-7
                    if big.any():
                        worst_rel_big = max(worst_rel_big, float((diff[big] / np.abs(o[big])).max()))
            checks["R3"][name] = {"max_abs_rad": worst_abs, "max_rel_where_above_1e-7": worst_rel_big,
                                  "signal_rel": float(abs(s0["signal"][3] / old["signal"][0] - 1))}
        e_real = np.array([[abs(r["err"]["(e) clipped pieces"][0]) for r in run["rows"]] for run in rs])
        checks["R5"][name] = float(e_real.max())
        r8 = []
        for run in rs:
            for r in run["rows"]:
                if "r8" in r:
                    q = r["r8"]
                    mp = q["l3"] + q["l4"] + q["l5"]
                    r8.append({"seed": run["seed"], "n": r["n"], "closed": q["closed_form"], "mpole": mp,
                               "rel": abs(mp / q["closed_form"] - 1) if q["closed_form"] else None,
                               "l5_share": abs(q["l5"] / mp) if mp else None,
                               "axis_vs_nodes_rel": abs(q["axis_read_of_d"] / q["closed_form"] - 1)})
        checks["R8"][name] = r8
    summ["checks"] = checks
    (DATA / "sweep-summary.json").write_text(json.dumps(summ, indent = 1, default = R.json_safe), encoding = "utf-8")
    md += _sweep_tables(summ)
    (HERE / "tables-sweep.md").write_text("\n".join(md) + "\n", encoding = "utf-8")
    _sweep_figure(summ)


def _sweep_tables(s: dict) -> list[str]:
    names = [n for n in SHAPES if n in s]
    md = ["# Sweep with the real read (generated by `analyse_real.py sweep`)\n",
          "Error at 128 cells across: RMS over the placements [min–max], (order of the RMS over size/h ≥ 8; "
          "in brackets the per-placement orders' range). Leading = the paper's read, same placements.\n"]
    for di, d in enumerate((1.5, 5.0, 20.0)):
        md += [f"\n## {d:g} m\n", "| shape | read | signal (rad) | " + " | ".join(SHORT[t] for t in TECH) + " |",
               "|---|---|---|" + "---|" * len(TECH)]
        for name in names:
            e = s[name]
            for k, read in ((di, "real"), (di + 3, "leading")):
                cells = []
                for t in TECH:
                    if t not in e["tech"]:
                        cells.append("")
                        continue
                    q = e["tech"][t]
                    i = len(e["n"]) - 1
                    so = [o[k] for o in q["order_by_seed"] if np.isfinite(o[k])]
                    orng = f" [{min(so):.1f}–{max(so):.1f}]" if so else ""
                    o = q["order_rms"][k]
                    cells.append(f"{fmt(q['rms'][i][k])} [{fmt(q['min'][i][k], 0)}–{fmt(q['max'][i][k], 0)}]"
                                 + (f" ({o:.2f}{orng})" if np.isfinite(o) else ""))
                md.append(f"| {LABEL.get(name, name)} | {read} | {e['signal'][k]:.4g} | " + " | ".join(cells) + " |")
    # Q1 ranking at 128, 1.5 m, every placement, real read
    md += ["\n## Q1: ranking (d) < (c) < (b) < (a) at 128 across, 1.5 m, real read, per placement\n"]
    for name in names:
        e = s[name]
        bad = []
        for si, seed in enumerate(e["seeds"]):
            v = [abs(e["tech"][t]["by_seed"][si][-1][0]) for t in TECH[:4]]
            if not (v[3] < v[2] < v[1] < v[0]):
                bad.append(f"seed {seed}: " + ", ".join(f"{SHORT[t]} {fmt(x)}" for t, x in zip(TECH[:4], v)))
        md.append(f"- {LABEL.get(name, name)}: " + ("holds on every placement" if not bad else "; ".join(bad)))
    # Q2 orders
    md += ["\n## Q2: orders of the RMS, real against leading (1.5 m)\n",
           "| shape | (c) real | (c) leading | (d) real | (d) leading |", "|---|---|---|---|---|"]
    for name in names:
        q = s[name]["tech"]
        md.append(f"| {LABEL.get(name, name)} | {q[TECH[2]]['order_rms'][0]:.2f} | {q[TECH[2]]['order_rms'][3]:.2f} | "
                  f"{q[TECH[3]]['order_rms'][0]:.2f} | {q[TECH[3]]['order_rms'][3]:.2f} |")
    # (d)/(c) ratio at every pitch, real, RMS
    common = sorted(set.intersection(*[set(s[n]["n"]) for n in names]))
    md += ["\n## |(c)| / |(d)| at 1.5 m (RMS over placements), real read; nine shapes other than the sphere\n",
           "| n across | " + " | ".join(str(n) for n in common) + " |", "|---|" + "---|" * len(common)]
    for k, read in ((0, "real"), (3, "leading")):
        lo, hi = [], []
        for nn in common:
            rat = [s[n]["tech"][TECH[2]]["rms"][s[n]["n"].index(nn)][k] / s[n]["tech"][TECH[3]]["rms"][s[n]["n"].index(nn)][k]
                   for n in names if n != "sphere"]
            lo.append(min(rat))
            hi.append(max(rat))
        md.append(f"| {read} min–max | " + " | ".join(f"{a:.3g}–{b:.3g}" for a, b in zip(lo, hi)) + " |")
    # Q3 sphere anomaly
    if "sphere" in s:
        e = s["sphere"]
        md += ["\n## Q3: the sphere, (c) against (d), per placement (1.5 m, real / leading)\n",
               "| n | seed | (c) | (d) | (c★) | (d★) |", "|---|---|---|---|---|---|"]
        for i, n in enumerate(e["n"]):
            for si, seed in enumerate(e["seeds"]):
                cells = [f"{fmt(abs(e['tech'][t]['by_seed'][si][i][0]))} / {fmt(abs(e['tech'][t]['by_seed'][si][i][3]))}"
                         for t in (TECH[2], TECH[3], TECH[5], TECH[6])]
                md.append(f"| {n} | {seed} | " + " | ".join(cells) + " |")
        md.append("\nRMS over the random placements (seeds ≠ 0), real read, 1.5 m:\n")
        rnd = [si for si, seed in enumerate(e["seeds"]) if seed != 0]
        for i, n in enumerate(e["n"]):
            vals = {SHORT[t]: math.sqrt(np.mean([e["tech"][t]["by_seed"][si][i][0] ** 2 for si in rnd]))
                    for t in (TECH[2], TECH[3], TECH[5], TECH[6])}
            md.append(f"- {n} across: " + ", ".join(f"{k} {fmt(v)}" for k, v in vals.items()))
    # Table 5: cost to reach, real read, RMS over placements
    md += ["\n## Table 5 with the real read: cheapest modelled cost per point (µs) to bring the 1.5 m error "
           "(RMS over placements) below 1e-4 / 1e-6 / 1e-8 rad\n",
           "| technique | " + " | ".join(LABEL.get(n, n) for n in names) + " |", "|---|" + "---|" * len(names)]
    for t in TECH:
        cells = []
        for name in names:
            e = s[name]
            if t not in e["tech"] or t not in e["cost_ns"]:
                cells.append("n/a")
                continue
            c = np.array(e["cost_ns"][t]) * 1e-3
            err = np.array(e["tech"][t]["rms"])[:, 0]
            cells.append(" / ".join("—" if not np.isfinite(x) else f"{x:.3g}" for x in
                                    (cost_to_reach(c, err, tol) for tol in (1e-4, 1e-6, 1e-8))))
        md.append(f"| {t} | " + " | ".join(cells) + " |")
    md.append("| whole mesh, direct | " + " | ".join(f"{s[n]['mesh_cost_ns'] * 1e-3:.3g}" for n in names) + " |")
    # facet errors
    md += ["\n## Facet error (mesh against the analytic shape), real / leading, 1.5 / 5 / 20 m\n"]
    for name in ("sphere", "torus"):
        if name in s and "facet_err" in s[name]:
            f = s[name]["facet_err"]
            md.append(f"- {name}: real {fmt(f[0], 2)} / {fmt(f[1], 2)} / {fmt(f[2], 2)} rad; "
                      f"leading {fmt(f[3], 2)} / {fmt(f[4], 2)} / {fmt(f[5], 2)} rad")
    md += ["\n## Launch-axis clearance from the mesh (m) at 1.5 / 5 / 20 m\n"]
    for name in names:
        md.append(f"- {LABEL.get(name, name)}: " + " / ".join(f"{c:.2f}" for c in s[name]["clearance"]))
    ch = s["checks"]
    md += ["\n## Checks\n", "R3 (seed 0, leading, against the committed sweep): " + "; ".join(
        f"{LABEL.get(n, n)} max |Δ| {fmt(v['max_abs_rad'])} rad, rel where > 1e-7 rad {fmt(v['max_rel_where_above_1e-7'])}"
        for n, v in ch["R3"].items()),
        "\nR5 ((e) at 1.5 m, real, worst over pitches and placements): " + "; ".join(
        f"{LABEL.get(n, n)} {fmt(v)}" for n, v in ch["R5"].items())]
    worst8 = {n: [r for r in v if r["n"] >= 16 and r["rel"] is not None] for n, v in ch["R8"].items()}
    md.append("\nR8 (l ≤ 5 moments against the closed-form (d) residual, 1.5 m): " + "; ".join(
        f"{LABEL.get(n, n)} n=4 {fmt(max(r['rel'] for r in v if r['n'] == 4))}, n≥16 "
        f"{fmt(max((r['rel'] for r in worst8[n]), default = float('nan')))}" for n, v in ch["R8"].items() if v))
    return md


def _sweep_figure(s: dict) -> None:
    import matplotlib.pyplot as plt

    S.apply()
    names = [n for n in SHAPES if n in s]
    fig, axes = plt.subplots(2, 5, figsize = (7.2, 3.6), sharex = True, sharey = True)
    for ax, name in zip(axes.flat, names):
        e = s[name]
        ns = np.array(e["n"])
        for k, t in enumerate(TECH[:5]):
            q = np.array(e["tech"][t]["rms"])
            ax.loglog(ns, np.maximum(np.abs(q[:, 0]), 1e-15), color = S.SERIES[k], marker = S.MARKERS[k], ms = 2.5,
                      lw = 1.1, label = f"{SHORT[t]} real")
            ax.loglog(ns, np.maximum(np.abs(q[:, 3]), 1e-15), color = S.SERIES[k], ls = ":", lw = 0.9)
        ax.set_title(f"{LABEL.get(name, name)}", fontsize = 7)
        ax.set_ylim(1e-14, 1e2)
        ax.tick_params(labelsize = 6)
    for ax in axes[1]:
        ax.set_xlabel("cells across", fontsize = 7)
    for ax in axes[:, 0]:
        ax.set_ylabel("|phase error| at 1.5 m (rad)", fontsize = 7)
    h_, l_ = axes[0, 0].get_legend_handles_labels()
    from matplotlib.lines import Line2D
    h_.append(Line2D([], [], color = S.INK2, ls = ":", lw = 0.9))
    l_.append("leading read (dotted)")
    fig.legend(h_, l_, loc = "upper center", ncol = 6, fontsize = 6.5, bbox_to_anchor = (0.5, 1.04))
    for ext in ("pdf", "png"):
        fig.savefig(HERE / f"fig-sweep.{ext}", dpi = 200)
    plt.close(fig)


# ======================================= TABLE 2 AND TABLE 8 =======================================

REP_LABEL = {"zero_one_raw": "0/1", "zero_one": "0/1, renormalised", "ss4": "sampled fill 4³ (today)",
             "frac": "fill fraction", "frac_cen": "fill + centroid (cube)", "frac_pt": "fill + centroid (point)",
             "frac_cen_q": "fill + centroid + 2nd moment"}


def conv() -> None:
    c = _j(DATA / "conv.json")
    bodies = list(c)
    md = ["# Table 2 with the real read (generated by `analyse_real.py conv`)\n",
          "Phase error (rad) at h = 1/64 m, RMS over the placements of the worst of three directions; "
          "(order over 1/8 … 1/64 m). The lower launch point is the benchmark's lower point.\n"]
    for d in ("1.5", "10.0"):
        for read in ("real", "leading"):
            md += [f"\n## {d} m from the centre, {read} read\n", "| representation | " + " | ".join(bodies) + " |",
                   "|---|" + "---|" * len(bodies)]
            md.append("| *signal* | " + " | ".join(f"*{c[b][d][read]['signal']:.3g}*" for b in bodies) + " |")
            for rep in REP_LABEL:
                md.append(f"| {REP_LABEL[rep]} | " + " | ".join(
                    f"{fmt(c[b][d][read]['reps'][rep]['err'][-1])} ({c[b][d][read]['reps'][rep]['order']:.1f})"
                    for b in bodies) + " |")
    md += ["\n## Signals by direction (mean over placements), real / leading, 1.5 m\n"]
    for b in bodies:
        r, lg = c[b]["1.5"]["real"]["signal_by_direction"], c[b]["1.5"]["leading"]["signal_by_direction"]
        md.append(f"- {b}: " + "; ".join(f"dir {k}: {x:.3g} / {y:.3g}" for k, (x, y) in enumerate(zip(r, lg)))
                  + f"; axis clearance {', '.join(f'{x:.2f}' for x in c[b]['clearance'][:3])} m")
    # Table 8's measured rows (make.py: worst body at 1.5 m and 1/64 m; closed-form rounding × largest signal)
    ref = _j(R.RESEARCH.parent / "papers" / "polyhedral-gravity" / "data" / "mp_reference.json")
    rel = max(r["ours_g"] for rows in ref["bodies"].values() for r in rows if r["R"] <= 1.5 + 1e-9)
    md += ["\n## Table 8's measured rows (worst body, 1.5 m, h = 1/64 m)\n", "| row | real | leading (paper) |",
           "|---|---|---|"]
    for rep in ("ss4", "frac_cen", "frac_cen_q"):
        vals = {read: max(c[b]["1.5"][read]["reps"][rep]["err"][-1] for b in bodies) for read in ("real", "leading")}
        who = {read: max(bodies, key = lambda b: c[b]["1.5"][read]["reps"][rep]["err"][-1]) for read in vals}
        md.append(f"| {REP_LABEL[rep]} | {fmt(vals['real'])} ({who['real']}) | {fmt(vals['leading'])} ({who['leading']}) |")
    sig = {read: max(abs(c[b]["1.5"][read]["signal"]) for b in bodies) for read in ("real", "leading")}
    md.append(f"| closed form (rounding {rel:.1e} × largest signal) | {fmt(rel * sig['real'])} (signal {sig['real']:.3g}) | "
              f"{fmt(rel * sig['leading'])} (signal {sig['leading']:.3g}) |")
    (HERE / "tables-conv.md").write_text("\n".join(md) + "\n", encoding = "utf-8")


# ======================================= REGIMES, ORIENTATION, TABLE 7 =======================================


def crossover(d: np.ndarray, err: np.ndarray, tol: float) -> float:
    bad = np.nonzero(np.abs(err) >= tol)[0]
    if len(bad) == 0:
        return float(d[0])
    return float(d[bad[-1] + 1]) if bad[-1] + 1 < len(d) else float("inf")


def at(d: np.ndarray, y: np.ndarray, x: float) -> float:
    return float(np.exp(np.interp(np.log(x), np.log(d), np.log(np.maximum(y, 1e-300)))))


def regimes() -> None:
    md = ["# Regimes, orientation and Table 7 with the real read (generated by `analyse_real.py regimes`)\n"]
    rg = _j(DATA / "regimes-distance.json") if (DATA / "regimes-distance.json").exists() else {}
    techs = ["(a) cubes", "(b) sampled fill", "(c) fill at centroid", "(d) + second moment", "(f) body multipole",
             "(g) point mass"]
    if rg:
        names = [n for n in SHAPES if n in rg]
        md += ["\n## Table 6: distance from the surface (m) beyond which the error stays below 1e-6 / 1e-10 rad "
               "(real; leading in brackets). Grid: 18 log-spaced distances 0.5–1000 m.\n",
               "| technique | " + " | ".join(LABEL.get(n, n) for n in names) + " |", "|---|" + "---|" * len(names),
               "| pitch (m) | " + " | ".join(f"{rg[n]['h']:g}" for n in names) + " |"]
        for t in techs:
            cells = []
            for n in names:
                d = np.array(rg[n]["d"])
                re_ = [crossover(d, np.array(rg[n]["err"][t]), tol) for tol in (1e-6, 1e-10)]
                le_ = [crossover(d, np.array(rg[n]["err_leading"][t]), tol) for tol in (1e-6, 1e-10)]
                f = lambda x: "—" if not np.isfinite(x) else f"{x:.3g}"  # noqa: E731
                cells.append(f"{f(re_[0])} / {f(re_[1])} ({f(le_[0])} / {f(le_[1])})")
            md.append(f"| {t} | " + " | ".join(cells) + " |")
        md += ["\n## Q5: where the leading read agrees with the real one (the shapes' signals, the study's ray)\n",
               "| shape | |real/leading − 1| < 1 % beyond | < 10 % beyond | ratio at 0.5 / 5 / 20 / 100 m |", "|---|---|---|---|"]
        for n in names:
            d = np.array(rg[n]["d"])
            ratio = np.array(rg[n]["signal"]) / np.array(rg[n]["signal_leading"])
            dev = np.abs(ratio - 1)
            md.append(f"| {LABEL.get(n, n)} | {crossover(d, dev, 0.01):.3g} | {crossover(d, dev, 0.1):.3g} | "
                      + " / ".join(f"{np.interp(np.log(x), np.log(d), ratio):.3f}" for x in (0.5, 5, 20, 100)) + " |")
        md += ["\nClearance of the launch axis from the mesh at 0.5 m: " + ", ".join(
            f"{LABEL.get(n, n)} {rg[n]['clearance'][0]:.2f} m" for n in names)]
    if (DATA / "orientation.json").exists():
        o = _j(DATA / "orientation.json")
        md += ["\n## Orientation (h = 0.1 m, 1.5 m): range of |error| over the angles, real (leading)\n",
               "| body | (b) | (c) | (d) | signal range |", "|---|---|---|---|---|"]
        for name, r in o.items():
            cells = []
            for t in ("(b) sampled fill", "(c) fill at centroid", "(d) + second moment"):
                e = np.abs(np.array(r["err"][t]))
                cells.append(f"{fmt(e[:, 0].min())}–{fmt(e[:, 0].max())} ({fmt(e[:, 1].min())}–{fmt(e[:, 1].max())})")
            s = np.array(r["signal"])
            md.append(f"| {name} | " + " | ".join(cells) + f" | {s[:, 0].min():.3g}…{s[:, 0].max():.3g} |")
        b = o["box"]
        e = np.abs(np.array(b["err"]["(c) fill at centroid"]))
        md.append("\nBox, (c) by angle (real / leading): " + "; ".join(
            f"{a:g}°: {fmt(x[0])} / {fmt(x[1])}" for a, x in zip(b["angles"], e)))
    if (DATA / "exact-distance.json").exists():
        ex = _j(DATA / "exact-distance.json")
        md += ["\n## Table 7's last column: the point mass's relative departure from the mesh at 1.5 / 5 / 20 m "
               "(centre of mass to axis, level with the lower launch); real (leading)\n",
               "| mesh | point mass | + second moments | + third |", "|---|---|---|---|"]
        for n, r in ex.items():
            d = np.array(r["d"])
            cells = []
            for key in ("point", "l2", "l3"):
                re_ = np.abs(np.array(r[key]) - np.array(r["exact"])) / np.abs(r["exact"])
                le_ = np.abs(np.array(r[f"{key}_leading"]) - np.array(r["exact_leading"])) / np.abs(r["exact_leading"])
                cells.append(" / ".join(fmt(at(d, re_, x), 0) for x in (1.5, 5, 20)) + " ("
                             + " / ".join(fmt(at(d, le_, x), 0) for x in (1.5, 5, 20)) + ")")
            md.append(f"| {LABEL.get(n, n)} | " + " | ".join(cells) + " |")
    if (DATA / "walkpast.json").exists():
        wp = _j(DATA / "walkpast.json")
        md += ["\n## Table 7: walking past, real read (leading)\n",
               "| mesh | upright | turned | on side | point mass | poses differ by up to |", "|---|---|---|---|---|---|"]
        for n, r in wp.items():
            peaks, lead = [], []
            for p in ("upright", "turned 90° about z", "on its side (90° about x)", "point mass"):
                fl, le = np.array(r["poses"][p]["flight"]), np.array(r["poses"][p]["leading"])
                peaks.append(fl[np.argmax(np.abs(fl))])
                lead.append(le[np.argmax(np.abs(le))])
            ser = np.array([r["poses"][p]["flight"] for p in ("upright", "turned 90° about z", "on its side (90° about x)")])
            spread = float(np.max(ser.max(axis = 0) - ser.min(axis = 0)))
            sl = np.array([r["poses"][p]["leading"] for p in ("upright", "turned 90° about z", "on its side (90° about x)")])
            spread_l = float(np.max(sl.max(axis = 0) - sl.min(axis = 0)))
            md.append(f"| {LABEL.get(n, n)} | " + " | ".join(f"{a:+.2f} ({b:+.2f})" for a, b in zip(peaks, lead))
                      + f" | {spread:.2f} ({100 * spread / np.abs(peaks[:3]).max():.1f} %) "
                      f"[{spread_l:.2f} ({100 * spread_l / np.abs(lead[:3]).max():.0f} %)] |")
    (HERE / "tables-regimes.md").write_text("\n".join(md) + "\n", encoding = "utf-8")


# ======================================= BUDGET =======================================


def budget() -> None:
    import budget_real as BR

    B = BR.B
    shapes = ["cube", "bunny", "bracket"]
    old = _j(R.RESEARCH / "paper-error-budget" / "budget.json")
    out: dict[str, object] = {"sigma_per_shot": B.SIGMA, "shots": len(B.TIMES), "distances": B.DISTANCES}
    crbs, voxel = {}, {}
    for shape in shapes:
        tr = _j(DATA / f"truth-{shape}.json")
        runs = [_j(p) for p in sorted(DATA.glob(f"run-{shape}-s*.json"))]
        crbs[shape] = []
        for i, d in enumerate(B.DISTANCES):
            j = BR.jacobian(tr, i)
            phi = np.array(tr["phi"])[i]
            crbs[shape].append({"d": d, "peak_rad": float(phi[np.argmax(np.abs(phi))]),
                                "peak_leading": float(np.array(tr["phi_leading"])[i][np.argmax(np.abs(tr["phi_leading"][i]))]),
                                "crb": B.crb(j).tolist(), "crb_with_z0": B.crb(BR.jacobian(tr, i, True)).tolist()})
        per: dict[str, object] = {}
        for tech in B.TECH + ["l5 term (truncation estimate)"]:
            rows = []
            for n in B.NS:
                rs = [r for run in runs for r in run["rows"] if r["n"] == n]
                if not rs:
                    continue
                ent: dict[str, object] = {"n": n, "h": rs[0]["h"], "seeds": len(rs), "max_ratio": [], "norm": [], "peak": []}
                for i in range(len(B.DISTANCES)):
                    j = BR.jacobian(tr, i)
                    rat, nrm, pk = [], [], []
                    for r in rs:
                        res = np.array(r["resid"][tech])[i]
                        a, b = B.ratios(j, res)
                        assert np.all(a <= b * (1 + 1e-9) + 1e-12), "V3"
                        rat.append(float(a.max()))
                        nrm.append(b)
                        pk.append(float(np.abs(res).max()))
                    ent["max_ratio"].append(max(rat))
                    ent["norm"].append(max(nrm))
                    ent["peak"].append(max(pk))
                rows.append(ent)
            import analyse as OLD  # the earlier study's threshold rule
            thr = []
            for i in range(len(B.DISTANCES)):
                hs = np.array([e["h"] for e in rows])
                thr.append({"bias": OLD._threshold(hs, np.array([e["max_ratio"][i] for e in rows])),
                            "norm": OLD._threshold(hs, np.array([e["norm"][i] for e in rows]))})
            per[tech] = {"rows": rows, "threshold": thr}
        voxel[shape] = per
    out["crb"], out["voxel"] = crbs, voxel
    phys: dict[str, dict] = {}
    for shape in shapes + ["sphere"]:
        tr = _j(DATA / f"truth-{shape}.json")
        phys[shape] = {}
        for key, val in tr["physical"].items():
            if key.startswith("exact shift"):
                continue
            arr = np.array(val)
            ent = {"peak": [], "max_ratio": []}
            for i in range(len(B.DISTANCES)):
                a, _ = B.ratios(BR.jacobian(tr, i), arr[i])
                ent["peak"].append(float(np.abs(arr[i]).max()))
                ent["max_ratio"].append(float(a.max()))
            phys[shape][key] = ent
    out["physical"] = phys
    # V1-style known answers in the real read
    v1 = {}
    for shape in ("cube", "bunny"):
        tr = _j(DATA / f"truth-{shape}.json")
        ph = tr["physical"]
        rows = []
        for i, d in enumerate(B.DISTANCES):
            j = BR.jacobian(tr, i)
            rows.append({"d": d, "density_dlnM": float(B.bias(j, np.array(ph["density 1%"][i]))[0]),
                         "exact_1cm_x_over_1cm": float(B.bias(j, np.array(ph["exact shift 0.01 m x"][i]))[1] / 0.01),
                         "exact_1cm_y_over_1cm": float(B.bias(j, np.array(ph["exact shift 0.01 m y"][i]))[2] / 0.01),
                         "exact_30cm_y_over_30cm": float(B.bias(j, np.array(ph["exact shift 0.3 m y"][i]))[2] / 0.3)})
        v1[shape] = rows
    out["V1"] = v1
    (DATA / "budget.json").write_text(json.dumps(out, indent = 1), encoding = "utf-8")
    md = _budget_tables(out, old)
    (HERE / "tables-budget.md").write_text("\n".join(md) + "\n", encoding = "utf-8")
    _budget_figure(out, old)


def _fmt_h(h: float, how: str) -> str:
    if not np.isfinite(h):
        return "never"
    if how == "coarsest":
        return f"≥ {h * 1000:.0f} mm"
    return f"{h * 1000:.3g} mm" + ("" if how == "interpolated" else "*")


def _budget_tables(out: dict, old: dict) -> list[str]:
    import budget_real as BR

    B = BR.B
    md = ["# Error budget with the real read (generated by `analyse_real.py budget`)\n",
          f"σ = {B.SIGMA:.1e} rad per shot, {len(B.TIMES)} shots, v = {B.SPEED} m/s; θ = (ln M, x₀, y₀, v).\n",
          "## Cramér–Rao bounds, real read (leading read's in brackets, from the earlier study)\n",
          "| body | d (m) | peak, real (leading) | σ(M)/M | σ(x₀) | σ(y₀) | σ(v) |", "|---|---|---|---|---|---|---|"]
    for shape, rows in out["crb"].items():
        for i, r in enumerate(rows):
            c, oc = r["crb"], old["crb"][shape][i]["crb"]
            md.append(f"| {shape} | {r['d']} | {r['peak_rad']:+.3g} ({r['peak_leading']:+.3g}) | {c[0]:.2e} ({oc[0]:.1e}) | "
                      f"{c[1] * 1e6:.3g} µm ({oc[1] * 1e6:.2g}) | {c[2] * 1e6:.3g} µm ({oc[2] * 1e6:.2g}) | "
                      f"{c[3] * 1e6:.3g} µm/s ({oc[3] * 1e6:.2g}) |")
    md += ["\n## Pitch at which max |bias|/σ < 0.1, worst of the placements: real read (leading, earlier study); "
           "\\* extrapolated beyond 1/128 m\n", "| body | technique | 1.5 m | 5 m | 20 m |", "|---|---|---|---|---|"]
    for shape, per in out["voxel"].items():
        for tech in B.TECH:
            cells = []
            for i in range(3):
                t = per[tech]["threshold"][i]["bias"]
                ot = old["voxel"][shape][tech]["threshold"][i]["bias"]
                cells.append(f"{_fmt_h(*t)} ({_fmt_h(*ot)})")
            md.append(f"| {shape} | {tech} | " + " | ".join(cells) + " |")
    md += ["\n## max |bias|/σ by pitch (worst placement), real read — 1.5 / 5 / 20 m\n"]
    for shape, per in out["voxel"].items():
        md += [f"\n**{shape}**\n", "| n | h (mm) | " + " | ".join(B.TECH) + " | l5 share of (d) |",
               "|---|---|" + "---|" * (len(B.TECH) + 1)]
        for k, e in enumerate(per[B.TECH[0]]["rows"]):
            cells = [" / ".join(fmt(x) for x in per[t]["rows"][k]["max_ratio"]) for t in B.TECH]
            l5 = " / ".join(fmt(a / b) if b else "—" for a, b in zip(per["l5 term (truncation estimate)"]["rows"][k]["peak"],
                                                                      per[B.TECH[3]]["rows"][k]["peak"]))
            md.append(f"| {e['n']} | {e['h'] * 1e3:.3g} | " + " | ".join(cells) + f" | {l5} |")
    md += ["\n## Physical uncertainties: peak |Δφ| (rad) / max |bias|/σ, real read — 1.5 / 5 / 20 m "
           "(earlier study's leading-read ratio in brackets)\n", "| body | change | 1.5 m | 5 m | 20 m |", "|---|---|---|---|---|"]
    for shape, rows in out["physical"].items():
        for key, e in rows.items():
            o = old["physical"].get(shape, {}).get(key)
            cells = [f"{fmt(e['peak'][i])} / {e['max_ratio'][i]:.2g}" + (f" ({o['max_ratio'][i]:.2g})" if o else "")
                     for i in range(3)]
            md.append(f"| {shape} | {key} | " + " | ".join(cells) + " |")
    md += ["\n## V1 in the real read (known answers)\n"]
    for shape, rows in out["V1"].items():
        for r in rows:
            md.append(f"- {shape} {r['d']} m: density → δ ln M = {r['density_dlnM']:.6f}; exact 1 cm x → "
                      f"{r['exact_1cm_x_over_1cm']:.4f} cm; exact 1 cm y → {r['exact_1cm_y_over_1cm']:.4f} cm; "
                      f"exact 30 cm y (control) → {r['exact_30cm_y_over_30cm']:.3f} × 30 cm")
    return md


def _budget_figure(out: dict, old: dict) -> None:
    import matplotlib.pyplot as plt

    import budget_real as BR

    B = BR.B
    S.apply()
    fig, axes = plt.subplots(3, 3, figsize = (6.8, 6.0), sharex = True, sharey = True)
    for r, shape in enumerate(["cube", "bunny", "bracket"]):
        for c, d in enumerate(B.DISTANCES):
            ax = axes[r, c]
            for k, tech in enumerate(B.TECH):
                rows = out["voxel"][shape][tech]["rows"]
                orow = old["voxel"][shape][tech]["rows"]
                ax.loglog([e["h"] for e in rows], [max(e["max_ratio"][c], 1e-12) for e in rows], marker = S.MARKERS[k],
                          color = S.SERIES[k], label = f"{tech} (real)")
                ax.loglog([e["h"] for e in orow], [max(e["max_ratio"][c], 1e-12) for e in orow], ls = ":",
                          color = S.SERIES[k], lw = 0.9)
            ax.axhline(0.1, color = S.INK, lw = 0.8, ls = "--")
            if r == 0:
                ax.set_title(f"{d:g} m from the surface", fontsize = 8)
            if c == 0:
                ax.set_ylabel(f"{shape}\nmax |bias|/$\\sigma_{{\\rm CRB}}$")
            if r == 2:
                ax.set_xlabel("pitch $h$ (m)")
    axes[0, 0].invert_xaxis()
    axes[0, 0].set_ylim(1e-9, 1e6)
    h_, l_ = axes[0, 0].get_legend_handles_labels()
    from matplotlib.lines import Line2D
    h_.append(Line2D([], [], color = S.INK2, ls = ":", lw = 0.9))
    l_.append("leading read (earlier study)")
    fig.legend(h_, l_, loc = "upper center", ncol = 3, bbox_to_anchor = (0.5, 1.06), fontsize = 7)
    for ext in ("pdf", "png"):
        fig.savefig(HERE / f"fig-bias.{ext}", dpi = 200)
    plt.close(fig)


if __name__ == "__main__":
    sys.path.insert(0, str(R.RESEARCH / "paper-error-budget"))
    globals()[sys.argv[1]]()
