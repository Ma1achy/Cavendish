"""Validation cases, the bias/σ tables, the thresholds and the figures, from budget.py's JSON.

    python budget.py checks      # checks.json  (V1-V5 of the note's pass criteria)
    python budget.py analyse     # budget.json, tables.md, fig-bias.pdf/.png, fig-budget.pdf/.png
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np

import budget as B

HERE = Path(__file__).resolve().parent
L = B.L
SHAPES = ["cube", "bunny", "bracket"]
TITLE = {"cube": "1 m cube, faces on the lattice (2.7 t)", "bunny": "Stanford bunny (540 kg)",
         "bracket": "CAD part, mk7 drive (184 kg)"}

# Okabe-Ito, and the paper's plot style (make.py `style`)
BLUE, ORANGE, GREEN, VERMILION, SKY, YELLOW, PURPLE = (
    "#0072B2", "#E69F00", "#009E73", "#D55E00", "#56B4E9", "#F0E442", "#CC79A7")
INK, INK2, GRID = "#0b0b0b", "#52514e", "#d9d8d4"
TECH_COLOUR = dict(zip(B.TECH, (BLUE, ORANGE, GREEN, VERMILION)))
MARKERS = ["o", "s", "^", "D"]


def _style() -> None:
    import matplotlib as mpl

    mpl.use("Agg")
    mpl.rcParams.update({
        "font.family": "serif", "font.size": 8, "axes.edgecolor": INK2, "axes.labelcolor": INK,
        "xtick.color": INK2, "ytick.color": INK2, "axes.grid": True, "grid.color": GRID,
        "grid.linewidth": 0.5, "axes.spines.top": False, "axes.spines.right": False,
        "lines.linewidth": 1.4, "lines.markersize": 4, "legend.frameon": False,
        "savefig.bbox": "tight", "pdf.fonttype": 42,
    })


def _load(name: str) -> dict:
    return json.loads((HERE / name).read_text(encoding = "utf-8"))


def _runs(shape: str) -> list[dict]:
    return [_load(p.name) for p in sorted(HERE.glob(f"run-{shape}-s*.json"))]


# ======================================= CHECKS =======================================


def checks() -> None:
    out: dict[str, object] = {}
    # ---- V1: known answers, on the bunny and the cube
    v1 = {}
    for shape in ("cube", "bunny"):
        tr = _load(f"truth-{shape}.json")
        ph = tr["physical"]
        rows = []
        for i, d in enumerate(B.DISTANCES):
            j = B.jacobian(tr, i)
            sig = B.crb(j)
            b_rho = B.bias(j, np.array(ph["density 1%"][i]))
            b_lin = B.bias(j, np.array(ph["position 1 cm x"][i]))
            b_ex = B.bias(j, np.array(ph["exact shift 0.01 m x"][i]))
            b_30 = B.bias(j, np.array(ph["exact shift 0.3 m x"][i]))
            rows.append({
                "d": d,
                "density: dlnM": b_rho[0], "density: others / sigma": float(np.max(np.abs(b_rho[1:]) / sig[1:])),
                "1 cm linear: dx0 (m)": b_lin[1],
                "1 cm linear: others / sigma": float(np.max(np.abs(b_lin[[0, 2, 3]]) / sig[[0, 2, 3]])),
                "1 cm exact: dx0 / 1 cm - 1": b_ex[1] / 0.01 - 1,
                "30 cm exact (control): dx0 / 30 cm - 1": b_30[1] / 0.3 - 1,
            })
        v1[shape] = rows
    ok = all(abs(r["density: dlnM"] - 0.01) < 1e-9 and r["density: others / sigma"] < 1e-6
             and abs(r["1 cm linear: dx0 (m)"] - 0.01) < 1e-9 and r["1 cm linear: others / sigma"] < 1e-6
             and abs(r["1 cm exact: dx0 / 1 cm - 1"]) < 0.01 for rows in v1.values() for r in rows)
    control = any(abs(r["30 cm exact (control): dx0 / 30 cm - 1"]) > 0.01
                  for r in v1["cube"] + v1["bunny"] if r["d"] == 1.5)
    out["V1"] = {"rows": v1, "pass": bool(ok), "control fails as it must": bool(control)}
    # Added after the written control did not fail: an along-track shift is nearly a time shift, so
    # it stays linear at 30 cm. Across the track (range) it should not: 1 cm must pass, 30 cm miss.
    across = {}
    for shape in ("cube", "bunny"):
        tr = _load(f"truth-{shape}.json")
        mesh = B.mesh_for(shape)
        pts = B.tower_points([B.track_from(mesh, tr["y0"][0])])
        base = L.phase(L.g_mesh(mesh.tris, pts))
        j = B.jacobian(tr, 0)
        got = {}
        for s in (0.01, 0.3):
            moved = L.phase(L.g_mesh(mesh.tris, pts - np.array([0.0, s, 0.0])))
            got[f"{s:g} m: dy0 / shift - 1"] = float(B.bias(j, moved - base)[2] / s - 1)
        across[shape] = got
    out["V1"]["across-track control (added)"] = {
        "rows": across,
        "1 cm within 1%": all(abs(g["0.01 m: dy0 / shift - 1"]) < 0.01 for g in across.values()),
        "30 cm misses by more than 1%": all(abs(g["0.3 m: dy0 / shift - 1"]) > 0.01 for g in across.values())}

    # ---- V2: Fisher matrix of the analytic sphere (point mass) against the analytic derivative
    tr = _load("truth-sphere.json")
    vol = tr["volume_m3"]
    v2 = []
    worst_fd, worst_ctrl = 0.0, 0.0
    for i, d in enumerate(B.DISTANCES):
        y0 = tr["y0"][i]
        t = np.array(tr["t"])
        cen = np.stack([B.SPEED * t, np.full_like(t, y0), np.full_like(t, B.Z_BODY)], axis = 1)  # body, world
        exact = []
        for k in range(3):
            exact.append(_dphase_point(vol, cen, k))
        j_an = np.stack([np.array(tr["phi"])[i], exact[0], exact[1], t * exact[0]], axis = 1)
        j_fd = B.jacobian(tr, i)
        rel = np.abs(B.crb(j_fd) / B.crb(j_an) - 1)
        # control: a 30 cm step
        j_bad = _fd_point(vol, cen, 0.3, np.array(tr["phi"])[i], t)
        rel_bad = np.abs(B.crb(j_bad) / B.crb(j_an) - 1)
        worst_fd, worst_ctrl = max(worst_fd, float(rel.max())), max(worst_ctrl, float(rel_bad.max()))
        v2.append({"d": d, "crb": B.crb(j_an).tolist(), "rel diff (1 mm step)": rel.tolist(),
                   "rel diff (30 cm step, control)": rel_bad.tolist()})
    out["V2"] = {"rows": v2, "pass": worst_fd < 1e-5, "control fails as it must": worst_ctrl > 1e-3}
    out["V4"] = _v4()
    (HERE / "checks.json").write_text(json.dumps(out, indent = 1), encoding = "utf-8")
    print(json.dumps({k: {kk: vv for kk, vv in v.items() if kk != "rows"} for k, v in out.items()}, indent = 1))


def _v4(shape: str = "bunny", n: int = 32, i: int = 0) -> dict[str, object]:
    """Gauss-Newton fit of a technique's own model (its voxels moved with the body) to the noiseless
    exact pass, against the linear bias formula."""
    tr = _load(f"truth-{shape}.json")
    row = [r for r in _load(f"run-{shape}-s0.json")["rows"] if r["n"] == n][0]
    mesh = B.mesh_for(shape)
    vox = L.voxelise(mesh, row["h"], origin = row["h"] * np.array(row["offset"]))
    h = row["h"]
    v = mesh.verts
    c = 0.5 * (v.min(axis = 0) + v.max(axis = 0))
    t = B.TIMES
    phi_t = np.array(tr["phi"])[i]
    y_true = tr["y0"][i]
    theta0 = np.array([0.0, 0.0, y_true, B.SPEED])
    s4 = vox.mesh_volume / (h**3 * (len(vox.interior) + vox.ss.sum()))

    def pts_of(th: np.ndarray) -> np.ndarray:
        lower = c[None] - np.stack([th[1] + th[3] * t, np.full_like(t, th[2]), np.full_like(t, B.Z_BODY)], axis = 1)
        return np.concatenate([lower, lower + [0.0, 0.0, L.BASELINE]])

    def model(tech: str, th: np.ndarray) -> np.ndarray:
        p = pts_of(th)
        g_in = L.g_cubes_fast(vox.interior_centres, h, np.ones(len(vox.interior)), p)
        if tech == B.TECH[1]:
            g = s4 * (g_in + L.g_cubes_fast(vox.boundary_centres, h, vox.ss, p))
        else:
            g = g_in + L.g_points(vox.frac * h**3, vox.centroid, p)
        return np.exp(th[0]) * L.phase(g)

    res: dict[str, object] = {}
    worst = 0.0
    for tech in (B.TECH[1], B.TECH[2]):
        th = theta0.copy()
        for _ in range(5):
            f0 = model(tech, th)
            cols = [f0]
            for k in (1, 2, 3):
                e = np.zeros(4)
                e[k] = B.STEP
                cols.append((model(tech, th + e) - model(tech, th - e)) / (2 * B.STEP))
            step = np.linalg.lstsq(np.stack(cols, axis = 1), phi_t - f0, rcond = None)[0]
            th = th + step
        # The estimate's bias is th - theta0. With resid = model - truth at theta0, the linear bias of
        # fitting `model` to the truth is -F^-1 J^T resid; B.bias gives +F^-1 J^T resid.
        j = B.jacobian(tr, i)
        lin = -B.bias(j, np.array(row["resid"][tech])[i])
        sig = B.crb(j)
        dev = np.abs((th - theta0) - lin) / sig
        rel = np.abs((th - theta0) - lin) / np.maximum(np.abs(lin), 1e-300)
        ok = bool(np.all((rel < 0.1) | (dev < 0.02)))
        worst = max(worst, float(np.max(np.minimum(rel, dev / 0.02 * 0.1))))
        res[tech] = {"nonlinear_bias": (th - theta0).tolist(), "linear_bias": lin.tolist(),
                     "linear_bias_over_sigma": (lin / sig).tolist(), "rel_diff": rel.tolist(),
                     "diff_over_sigma": dev.tolist(), "pass": ok}
    res["pass"] = all(v["pass"] for k, v in res.items() if isinstance(v, dict))
    return res


def _gz_point(vol: float, src: np.ndarray, pts: np.ndarray) -> np.ndarray:
    r = src - pts
    return vol * r[:, 2] / np.linalg.norm(r, axis = 1) ** 3


def _phase_point(vol: float, cen: np.ndarray) -> np.ndarray:
    lower = np.zeros_like(cen)
    upper = lower + [0.0, 0.0, L.BASELINE]
    return L.PHASE_PER_DG * L.G * L.RHO * (_gz_point(vol, cen, upper) - _gz_point(vol, cen, lower))


def _dphase_point(vol: float, cen: np.ndarray, k: int) -> np.ndarray:
    """Analytic d phase / d (source position)_k for a point mass: d/ds_k of s_z-p_z over |s-p|^3."""
    out = np.zeros(len(cen))
    for zc, sign in ((L.BASELINE, 1.0), (0.0, -1.0)):
        r = cen - np.array([0.0, 0.0, zc])
        rl = np.linalg.norm(r, axis = 1)
        d = (1.0 if k == 2 else 0.0) / rl**3 - 3 * r[:, 2] * r[:, k] / rl**5
        out += sign * vol * d
    return L.PHASE_PER_DG * L.G * L.RHO * out


def _fd_point(vol: float, cen: np.ndarray, step: float, phi: np.ndarray, t: np.ndarray) -> np.ndarray:
    cols = []
    for k in range(2):
        e = np.zeros(3)
        e[k] = step
        cols.append((_phase_point(vol, cen + e) - _phase_point(vol, cen - e)) / (2 * step))
    return np.stack([phi, cols[0], cols[1], t * cols[0]], axis = 1)


# ======================================= ANALYSIS =======================================


def _threshold(hs: np.ndarray, metric: np.ndarray, tol: float = 0.1) -> tuple[float, str]:
    """The pitch below which `metric` stays < tol (log-log interpolation; extrapolated by the order
    fitted over the last three pitches if never reached). Returns (h, how)."""
    order = np.argsort(-hs)  # coarse to fine
    hs, m = hs[order], metric[order]
    bad = np.nonzero(m >= tol)[0]
    if len(bad) == 0:
        return float(hs[0]), "coarsest"
    k = int(bad[-1])
    if k + 1 < len(hs):
        lh, lm = np.log(hs[k : k + 2]), np.log(np.maximum(m[k : k + 2], 1e-300))
        return float(np.exp(lh[0] + (np.log(tol) - lm[0]) * (lh[1] - lh[0]) / (lm[1] - lm[0]))), "interpolated"
    p = np.polyfit(np.log(hs[-3:]), np.log(np.maximum(m[-3:], 1e-300)), 1)
    if p[0] <= 0.2:
        return float("nan"), f"not converging (order {p[0]:.2f})"
    return float(np.exp((np.log(tol) - p[1]) / p[0])), f"extrapolated (order {p[0]:.1f})"


def analyse() -> None:
    out: dict[str, object] = {"sigma_per_shot": B.SIGMA, "shots": len(B.TIMES), "distances": B.DISTANCES}
    md: list[str] = []
    crbs, voxel = {}, {}
    for shape in SHAPES:
        tr = _load(f"truth-{shape}.json")
        runs = _runs(shape)
        crbs[shape] = []
        for i, d in enumerate(B.DISTANCES):
            j = B.jacobian(tr, i)
            j5 = B.jacobian(tr, i, with_z = True)
            phi = np.array(tr["phi"])[i]
            crbs[shape].append({"d": d, "y0": tr["y0"][i], "peak_rad": float(np.abs(phi).max()),
                                "snr": float(np.linalg.norm(phi) / B.SIGMA), "crb": B.crb(j).tolist(),
                                "crb_with_z0": B.crb(j5).tolist()})
        per: dict[str, object] = {}
        for tech in B.TECH + ["(e) check: interior + pieces - mesh"]:
            rows = []
            for n in B.NS:
                rs = [r for run in runs for r in run["rows"] if r["n"] == n]
                if not rs:
                    continue
                ent = {"n": n, "h": rs[0]["h"], "seeds": len(rs), "max_ratio": [], "norm": [], "peak": [],
                       "max_ratio_z0": []}
                for i in range(len(B.DISTANCES)):
                    j, j5 = B.jacobian(tr, i), B.jacobian(tr, i, with_z = True)
                    rat, nrm, pk, rat5 = [], [], [], []
                    for r in rs:
                        res = np.array(r["resid"][tech])[i]
                        a, b = B.ratios(j, res)
                        assert np.all(a <= b * (1 + 1e-9) + 1e-12), "V3: Cauchy-Schwarz violated"
                        rat.append(float(a.max()))
                        nrm.append(b)
                        pk.append(float(np.abs(res).max()))
                        rat5.append(float(B.ratios(j5, res)[0].max()))
                    ent["max_ratio"].append(max(rat))
                    ent["norm"].append(max(nrm))
                    ent["peak"].append(max(pk))
                    ent["max_ratio_z0"].append(max(rat5))
                    ent.setdefault("per_param", []).append(
                        np.max([np.abs(B.bias(j, np.array(r["resid"][tech])[i])) / B.crb(j) for r in rs],
                               axis = 0).tolist())
                rows.append(ent)
            thr = []
            for i in range(len(B.DISTANCES)):
                hs = np.array([e["h"] for e in rows])
                thr.append({"bias": _threshold(hs, np.array([e["max_ratio"][i] for e in rows])),
                            "norm": _threshold(hs, np.array([e["norm"][i] for e in rows]))})
            per[tech] = {"rows": rows, "threshold": thr}
        voxel[shape] = per
    out["crb"], out["voxel"] = crbs, voxel

    # ---- physical rows
    phys: dict[str, dict] = {}
    for shape in SHAPES + ["sphere"]:
        tr = _load(f"truth-{shape}.json")
        phys[shape] = {}
        for key, val in tr["physical"].items():
            if key.startswith("exact shift"):
                continue
            arr = np.array(val)
            ent = {"peak": [], "norm": [], "max_ratio": [], "per_param": []}
            for i in range(len(B.DISTANCES)):
                j = B.jacobian(tr, i)
                a, b = B.ratios(j, arr[i])
                ent["peak"].append(float(np.abs(arr[i]).max()))
                ent["norm"].append(b)
                ent["max_ratio"].append(float(a.max()))
                ent["per_param"].append(a.tolist())
            phys[shape][key] = ent
    out["physical"] = phys
    out["decimation"] = _decimation()
    (HERE / "budget.json").write_text(json.dumps(out, indent = 1), encoding = "utf-8")
    md += _tables(out)
    (HERE / "tables.md").write_text("\n".join(md) + "\n", encoding = "utf-8")
    _figures(out)


def _decimation() -> dict[str, object]:
    """Decimated against full scan, from the shapes study's exact-distance.json (static, the centre
    of mass at distance d from the tower's axis, level with the lower cloud)."""
    ex = json.loads((B.SHAPES / "exact-distance.json").read_text(encoding = "utf-8"))
    res = {}
    for name in ("dragon", "buddha", "armadillo"):
        d = np.array(ex[name]["d"])
        a, b = np.array(ex[name]["exact"]), np.array(ex[f"{name} (full)"]["exact"])
        diff = a - b
        at = {}
        for x in B.DISTANCES:
            at[str(x)] = {"diff_rad": float(np.interp(np.log(x), np.log(d), diff)),
                          "rel": float(np.interp(np.log(x), np.log(d), diff / b))}
        res[name] = at
    return res


def _fmt_h(h: float, how: str) -> str:
    if not np.isfinite(h):
        return "never (" + how + ")"
    star = "" if how in ("interpolated", "coarsest") else "*"
    if how == "coarsest":
        return f"≥ {h * 1000:.0f} mm"
    return f"{h * 1000:.3g} mm{star}"


def _tables(out: dict) -> list[str]:
    md = [f"Noise σ = {B.SIGMA:.1e} rad per shot (10⁻⁴ rad/√Hz, 2 s cycle), {len(B.TIMES)} shots, "
          f"v = {B.SPEED} m/s. Parameters (ln M, x₀, y₀, v).\n", "## Cramér–Rao bounds\n",
          "| body | d (m) | peak (rad) | SNR | σ(M)/M | σ(x₀) | σ(y₀) | σ(v) | σ(M)/M with z₀ free |",
          "|---|---|---|---|---|---|---|---|---|"]
    for shape, rows in out["crb"].items():
        for r in rows:
            c = r["crb"]
            md.append(f"| {shape} | {r['d']} | {r['peak_rad']:.3g} | {r['snr']:.3g} | {c[0]:.2e} | "
                      f"{c[1] * 1e3:.3g} mm | {c[2] * 1e3:.3g} mm | {c[3] * 1e3:.3g} mm/s | {r['crb_with_z0'][0]:.2e} |")
    md += ["\n## Pitch at which max |bias|/σ < 0.1 (worst of the placements); in brackets, ‖Δφ‖/σ < 0.1\n",
           "\\* extrapolated beyond 1/128 m with the fitted order of the last three pitches.\n",
           "| body | technique | 1.5 m | 5 m | 20 m |", "|---|---|---|---|---|"]
    for shape, per in out["voxel"].items():
        for tech in B.TECH:
            cells = [f"{_fmt_h(*t['bias'])} ({_fmt_h(*t['norm'])})" for t in per[tech]["threshold"]]
            md.append(f"| {shape} | {tech} | " + " | ".join(cells) + " |")
    md += ["\n## max |bias|/σ by pitch (worst placement) — 1.5 / 5 / 20 m\n"]
    for shape, per in out["voxel"].items():
        md += [f"\n**{shape}**\n", "| n across | h (mm) | " + " | ".join(B.TECH) + " | (e) ‖Δφ‖/σ |",
               "|---|---|" + "---|" * (len(B.TECH) + 1)]
        for k, e in enumerate(per[B.TECH[0]]["rows"]):
            cells = [" / ".join(f"{x:.1e}" for x in per[t]["rows"][k]["max_ratio"]) for t in B.TECH]
            chk = " / ".join(f"{x:.0e}" for x in per["(e) check: interior + pieces - mesh"]["rows"][k]["norm"])
            md.append(f"| {e['n']} | {e['h'] * 1e3:.3g} | " + " | ".join(cells) + f" | {chk} |")
    md += ["\n## Physical uncertainties: peak |Δφ| (rad) / max |bias|/σ — 1.5 / 5 / 20 m\n",
           "| body | change | 1.5 m | 5 m | 20 m |", "|---|---|---|---|---|"]
    for shape, rows in out["physical"].items():
        for key, e in rows.items():
            cells = [f"{e['peak'][i]:.1e} / {e['max_ratio'][i]:.2g}" for i in range(3)]
            md.append(f"| {shape} | {key} | " + " | ".join(cells) + " |")
    md += ["\n## Decimation (static, centre of mass to axis; from the shapes study)\n",
           "| scan | 1.5 m | 5 m | 20 m |", "|---|---|---|---|"]
    for name, at in out["decimation"].items():
        md.append(f"| {name} | " + " | ".join(f"{v['diff_rad']:.1e} rad ({v['rel']:.1e})" for v in at.values()) + " |")
    return md


def _figures(out: dict) -> None:
    import matplotlib.pyplot as plt

    _style()
    # ---- bias/σ against pitch: shapes x distances
    fig, axes = plt.subplots(3, 3, figsize = (6.8, 6.0), sharex = True, sharey = True)
    for r, shape in enumerate(SHAPES):
        per = out["voxel"][shape]
        for c, d in enumerate(B.DISTANCES):
            ax = axes[r, c]
            for k, tech in enumerate(B.TECH):
                rows = per[tech]["rows"]
                ax.loglog([e["h"] for e in rows], [max(e["max_ratio"][c], 1e-12) for e in rows],
                          marker = MARKERS[k], color = TECH_COLOUR[tech], label = tech)
            chk = per["(e) check: interior + pieces - mesh"]["rows"]
            ax.loglog([e["h"] for e in chk], [max(e["norm"][c], 1e-12) for e in chk], ls = ":", color = INK2,
                      lw = 1.0, label = "reference floor (e)")
            ax.axhline(0.1, color = INK, lw = 0.8, ls = "--")
            ax.invert_xaxis() if (r == 0 and c == 0) else None
            if r == 0:
                ax.set_title(f"{d:g} m from the surface", fontsize = 8)
            if c == 0:
                ax.set_ylabel(f"{shape}\nmax |bias|/$\\sigma_{{\\rm CRB}}$")
            if r == 2:
                ax.set_xlabel("pitch $h$ (m)")
    axes[0, 0].set_ylim(1e-9, 1e6)
    h_, l_ = axes[0, 0].get_legend_handles_labels()
    fig.legend(h_, l_, loc = "upper center", ncol = 5, bbox_to_anchor = (0.5, 1.04), fontsize = 7)
    for ext in ("pdf", "png"):
        fig.savefig(HERE / f"fig-bias.{ext}", dpi = 200)
    plt.close(fig)

    # ---- the budget: voxel rows at 1/64 m next to physical rows, bunny, peak |Δφ| at three distances
    shape = "bunny"
    labels, vals, cols = [], [], []
    i64 = [k for k, e in enumerate(out["voxel"][shape][B.TECH[0]]["rows"]) if e["n"] == 64][0]
    for tech in B.TECH:
        labels.append(f"{tech}, h = 1/64 m")
        vals.append(out["voxel"][shape][tech]["rows"][i64]["peak"])
        cols.append(TECH_COLOUR[tech])
    ph = out["physical"][shape]
    for key, colour in (("density 1%", PURPLE), ("inhomogeneity 10%/10% near side (min y)", PURPLE),
                        ("position 1 mm y", SKY), ("position 1 cm y", SKY),
                        ("orientation 1 deg about z", YELLOW)):
        labels.append(key.replace("inhomogeneity 10%/10% ", "10% denser 10%, "))
        vals.append(ph[key]["peak"])
        cols.append(colour)
    dec = out["decimation"]["dragon"]
    labels.append("decimation, dragon (static)")
    vals.append([abs(dec[str(d)]["diff_rad"]) for d in B.DISTANCES])
    cols.append(INK2)
    fig, ax = plt.subplots(figsize = (6.8, 3.0))
    y = np.arange(len(labels))
    for c, (d, mk) in enumerate(zip(B.DISTANCES, ("o", "s", "^"))):
        ax.scatter([max(v[c], 1e-14) for v in vals], y, marker = mk, c = cols, s = 18,
                   label = f"{d:g} m", edgecolors = INK, linewidths = 0.3)
    ax.axvline(B.SIGMA, color = INK, ls = "--", lw = 0.8)
    ax.text(B.SIGMA * 1.3, -0.55, "one shot's noise", fontsize = 7, color = INK)
    ax.set_xscale("log")
    ax.set_yticks(y, labels, fontsize = 7)
    ax.invert_yaxis()
    ax.set_xlabel("largest change of the phase over the pass (rad), bunny")
    ax.legend(loc = "upper left", bbox_to_anchor = (1.0, 1.0), fontsize = 7, title = "closest approach", title_fontsize = 7)
    for ext in ("pdf", "png"):
        fig.savefig(HERE / f"fig-budget.{ext}", dpi = 200)
    plt.close(fig)
