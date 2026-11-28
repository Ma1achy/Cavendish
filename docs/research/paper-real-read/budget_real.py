"""The error budget (paper-error-budget) with the real read: the flight integral of a moving body.

    python budget_real.py r4                         # data/budget-r4.json: the flight-node rule converged
    python budget_real.py truth cube|bunny|bracket|sphere   # data/truth-<body>.json
    python budget_real.py r6                         # data/budget-r6.json: Jacobian against differences
    python budget_real.py run cube|bunny|bracket <seed>     # data/run-<body>-s<seed>.json
    python budget_real.py analyse                    # data/budget.json, tables-budget.md, fig-budget-*.pdf

The scene is `../paper-error-budget/budget.py`'s, unchanged: body 1 m across, ρ = 2700 kg m⁻³, voxels
fixed in its frame, moving along +x at 1 m/s with its bounding-box centre 1 m below the lower cloud;
61 shots 2 s apart, one at closest approach (d = 1.5, 5, 20 m from the surface); σ = 5×10⁻⁵ rad per
shot; θ = (ln M, x₀, y₀, v); the same three random lattice placements per pitch (rng(1000 + seed)).

What changes is the read. Each shot is the flight integral with its π pulse at the old shot time,
the body moving during the 2T = 1.46 s (`realread.PassRead`, Gauss order 8 on panels ≤ 0.2 s, R4).
The Jacobian comes from g = ∇U at the flight nodes: ∂/∂x₀ is Σw·g_x terms, and ∂/∂v weights each
node by its own time (not the shot's), which the instant read could not see. The residuals of the
techniques are taken from the pieces' exact moments (criteria amendment, R7–R8):

    U_pieces = U_point(frac, centroid) + U_quadrupole(second) + U_3 + U_4 + U_5   (l ≥ 6 dropped)
    (d) U_point + U_quad − U_pieces = −(U_3 + U_4 + U_5)
    (c) U_point − U_pieces
    (a) U_cubes(0/1 at centre) − U_pieces;  (b) (s4 − 1) U_interior + s4 U_cubes(sampled) − U_pieces

with U_interior = U_mesh − U_pieces (interior cubes + pieces = the mesh), as `budget.py run`.
"""

from __future__ import annotations

import json
import sys
import time

import numpy as np

import moments as MO
import realread as R

L = R.L
sys.path.insert(0, str(R.RESEARCH / "paper-error-budget"))
import budget as B  # noqa: E402

sys.path.insert(0, str(R.RESEARCH / "exact-skin-and-partial-fill" / "shapes"))
import analytic as A  # noqa: E402

OUT = R.HERE / "data"
U_BODY = np.array([-B.SPEED, 0.0, 0.0])  # the tower's points move at −v in the body's frame
ORDER, PANEL = 8, 0.2


def reads(mesh: L.Mesh, y0s: list[float], order: int = ORDER, panel: float = PANEL) -> list[R.PassRead]:
    return [R.PassRead(B.TIMES, B.track_from(mesh, y0), U_BODY, order, panel) for y0 in y0s]


def all_points(prs: list[R.PassRead]) -> tuple[np.ndarray, list[slice]]:
    sl, start = [], 0
    for p in prs:
        sl.append(slice(start, start + len(p.points)))
        start += len(p.points)
    return np.concatenate([p.points for p in prs]), sl


def series_u(prs: list[R.PassRead], sl: list[slice], u: np.ndarray) -> np.ndarray:
    """Phase (rad) per distance and shot from U at all points: [D, S]."""
    return np.stack([p.phase(u[s]) for p, s in zip(prs, sl)])


def _mesh_fields(tris: np.ndarray, pts: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    u, g = R.tri_field(tris, np.ones((len(tris), 1)), pts)
    return u[0], g[0]


# ======================================= TRUTH =======================================


def truth(name: str) -> None:
    R.below_normal()
    t0 = time.perf_counter()
    if name == "sphere":
        return _sphere()
    mesh = B.mesh_for(name)
    old = json.loads((R.RESEARCH / "paper-error-budget" / f"truth-{name}.json").read_text(encoding = "utf-8"))
    y0s = [B.track(mesh, d)[0] for d in B.DISTANCES]
    prs = reads(mesh, y0s)
    pts, sl = all_points(prs)
    u, g = _mesh_fields(mesh.tris, pts)
    phi = series_u(prs, sl, u)
    der = {f"dphi_d{ax}": np.stack([p.derivative(g[s], k) for p, s in zip(prs, sl)]) for k, ax in enumerate("xyz")}
    der["dphi_dv"] = np.stack([p.derivative(g[s], 0, with_time = True) for p, s in zip(prs, sl)])
    lead_pts = np.concatenate([p.lead_points for p in prs])
    g_lead = R.tri_field(mesh.tris, np.ones((len(mesh.tris), 1)), lead_pts)[1][0]
    m2 = len(prs[0].lead_points)
    lead = np.stack([p.leading(g_lead[k * m2 : (k + 1) * m2]) for k, p in enumerate(prs)])
    vol, cen = L.sb.volume_centroid(mesh.tris, np.zeros(3))
    out: dict[str, object] = {
        "shape": name, "triangles": len(mesh.faces), "volume_m3": vol, "mass_kg": vol * L.RHO, "y0": y0s,
        "t": B.TIMES.tolist(), "rule": [ORDER, PANEL], "phi": phi, "phi_leading": lead,
        "r3_leading_vs_old_rel": float(np.abs(lead - np.array(old["phi"])).max() / np.abs(old["phi"]).max()),
        "r3_y0_vs_old": float(np.abs(np.array(y0s) - np.array(old["y0"])).max()), **der}
    phys: dict[str, np.ndarray] = {"density 1%": 0.01 * phi}
    for s, label in ((1e-3, "1 mm"), (1e-2, "1 cm")):
        for ax in "xyz":
            phys[f"position {label} {ax}"] = s * der[f"dphi_d{ax}"]
    for s in (1e-2, 0.3):
        phys[f"exact shift {s:g} m x"] = series_u(prs, sl, _mesh_fields(mesh.tris, pts - [s, 0.0, 0.0])[0]) - phi
    phys["exact shift 0.01 m y"] = series_u(prs, sl, _mesh_fields(mesh.tris, pts - [0.0, 0.01, 0.0])[0]) - phi
    phys["exact shift 0.3 m y"] = series_u(prs, sl, _mesh_fields(mesh.tris, pts - [0.0, 0.3, 0.0])[0]) - phi
    for axis, label in (((0, 0, 1), "z"), ((1, 0, 0), "x")):
        rot = L.rot_about(np.array(axis, dtype = float), 1.0)
        turned = (mesh.tris - cen) @ rot.T + cen
        phys[f"orientation 1 deg about {label}"] = series_u(prs, sl, _mesh_fields(turned, pts)[0]) - phi
    phys.update(_inhomogeneity(mesh, prs, sl, pts, cen))
    out["physical"] = phys
    out["seconds"] = time.perf_counter() - t0
    OUT.mkdir(exist_ok = True)
    (OUT / f"truth-{name}.json").write_text(json.dumps(out, default = R.json_safe), encoding = "utf-8")
    print(name, f"{out['seconds']:.0f} s", "peak real", np.abs(phi).max(axis = 1), "leading", np.abs(lead).max(axis = 1),
          "R3", out["r3_leading_vs_old_rel"], flush = True)


def _inhomogeneity(mesh: L.Mesh, prs: list[R.PassRead], sl: list[slice], pts: np.ndarray,
                   cen: np.ndarray) -> dict[str, np.ndarray]:
    """budget._inhomogeneity with U: 10 % of the volume 10 % denser, from cut-cell moments at 64."""
    size = float((mesh.verts.max(axis = 0) - mesh.verts.min(axis = 0)).max())
    h = size / 64
    vox = L.voxelise(mesh, h)
    p_in, p_b = vox.interior_centres, vox.centroid
    m_in, m_b = np.full(len(p_in), h**3), vox.frac * h**3
    pos, mass = np.concatenate([p_in, p_b]), np.concatenate([m_in, m_b])
    total = mass.sum()
    keys = {"near side (min y)": pos[:, 1], "far side (max y)": -pos[:, 1], "leading end (max x)": -pos[:, 0],
            "core (nearest centroid)": np.linalg.norm(pos - cen, axis = 1)}
    out = {}
    for label, key in keys.items():
        order = np.argsort(key, kind = "stable")
        take = order[: int(np.searchsorted(np.cumsum(mass[order]), 0.1 * total)) + 1]
        sel_in, sel_b = take[take < len(p_in)], take[take >= len(p_in)] - len(p_in)
        u = R.u_cubes(p_in[sel_in], h, np.ones(len(sel_in)), pts)
        up, _, uq, _ = R.point_field(p_b[sel_b], m_b[sel_b], vox.second[sel_b], pts)
        out[f"inhomogeneity 10%/10% {label}"] = 0.1 * series_u(prs, sl, u + up + uq)
    return out


def _sphere() -> None:
    """The 5120-triangle icosphere against the analytic unit sphere, passing (budget._sphere_facet)."""
    mesh, ball = L.mesh_by_name("sphere"), A.Ball(1.0)
    prs = []
    for d in B.DISTANCES:
        y0 = float(np.sqrt((1.0 + d) ** 2 - B.Z_BODY**2))
        pulses = -np.stack([B.SPEED * B.TIMES, np.full_like(B.TIMES, y0), np.full_like(B.TIMES, B.Z_BODY)], axis = 1)
        prs.append(R.PassRead(B.TIMES, pulses, U_BODY, ORDER, PANEL))
    pts, sl = all_points(prs)
    r = np.linalg.norm(pts, axis = 1)
    u_ball, g_ball = ball.volume / r, -ball.volume * pts / r[:, None] ** 3
    phi = series_u(prs, sl, u_ball)
    der = {f"dphi_d{ax}": np.stack([p.derivative(g_ball[s], k) for p, s in zip(prs, sl)]) for k, ax in enumerate("xyz")}
    der["dphi_dv"] = np.stack([p.derivative(g_ball[s], 0, with_time = True) for p, s in zip(prs, sl)])
    facet = series_u(prs, sl, _mesh_fields(mesh.tris, pts)[0]) - phi
    out = {"shape": "sphere", "triangles": len(mesh.faces), "volume_m3": ball.volume, "mass_kg": ball.volume * L.RHO,
           "y0": [float(np.sqrt((1.0 + d) ** 2 - B.Z_BODY**2)) for d in B.DISTANCES], "t": B.TIMES.tolist(),
           "phi": phi, **der, "physical": {"facet: 5120-triangle icosphere": facet}}
    OUT.mkdir(exist_ok = True)
    (OUT / "truth-sphere.json").write_text(json.dumps(out, default = R.json_safe), encoding = "utf-8")
    print("sphere facet", np.abs(facet).max(axis = 1), flush = True)


# ======================================= CHECKS =======================================


def r4() -> None:
    """The node rule against twice the order on half the panel: the bunny's signal and its
    residuals at 1/16 m (closed-form pieces: this is also R8 for a moving body)."""
    R.below_normal()
    mesh = B.mesh_for("bunny")
    y0s = [B.track(mesh, d)[0] for d in B.DISTANCES]
    vox = L.voxelise(mesh, 1 / 16)
    raw = MO.piece_moments(vox.piece_tris, vox.piece_of, vox.centroid)
    coef = MO.multipole_coefficients(raw)
    res = {}
    for order, panel in ((ORDER, PANEL), (2 * ORDER, PANEL / 2), (6, 0.1)):
        prs = reads(mesh, y0s, order, panel)
        pts, sl = all_points(prs)
        phi = series_u(prs, sl, _mesh_fields(mesh.tris, pts)[0])
        u_p = R.u_mesh(vox.piece_tris, pts)
        u0, _, uq, _ = R.point_field(vox.centroid, vox.frac * vox.h**3, vox.second, pts)
        d_exact = series_u(prs, sl, u0 + uq - u_p)
        d_mp = -series_u(prs, sl, R.mpole_field(vox.centroid, coef, pts).sum(axis = 1))
        c_exact = series_u(prs, sl, u0 - u_p)
        res[f"{order}/{panel}"] = {"phi": phi, "d_exact": d_exact, "d_mpole": d_mp, "c_exact": c_exact}
        print(order, panel, len(pts), flush = True)
    a, b = res[f"{ORDER}/{PANEL}"], res[f"{2 * ORDER}/{PANEL / 2}"]
    summary = {}
    for i, d in enumerate(B.DISTANCES):
        summary[str(d)] = {
            "signal_change_over_peak": float(np.abs(a["phi"][i] - b["phi"][i]).max() / np.abs(b["phi"][i]).max()),
            "d_change_over_max": float(np.abs(a["d_exact"][i] - b["d_exact"][i]).max() / np.abs(b["d_exact"][i]).max()),
            "c_change_over_max": float(np.abs(a["c_exact"][i] - b["c_exact"][i]).max() / np.abs(b["c_exact"][i]).max()),
            "r8_moving_mpole_vs_closed_form": float(np.abs(a["d_mpole"][i] - a["d_exact"][i]).max()
                                                    / np.abs(a["d_exact"][i]).max()),
            "engine_rule_6_0.1_vs_fine_signal": float(np.abs(res["6/0.1"]["phi"][i] - b["phi"][i]).max()
                                                      / np.abs(b["phi"][i]).max())}
    ok = all(v["signal_change_over_peak"] <= 1e-6 and v["d_change_over_max"] <= 1e-3 and v["c_change_over_max"] <= 1e-3
             for v in summary.values())
    out = {"summary": summary, "pass": ok, "rule": [ORDER, PANEL]}
    (OUT / "budget-r4.json").write_text(json.dumps(out, indent = 1, default = R.json_safe), encoding = "utf-8")
    print(json.dumps(out, indent = 1, default = R.json_safe))


def r4b() -> None:
    """Added after R4: at 20 m the closed-form residual is at its rounding floor (ε(R/s)² per piece),
    so R4's residual test there measured rounding. The same test on the moments' residual, which
    has no such floor (bunny, 1/16 m, every distance)."""
    R.below_normal()
    mesh = B.mesh_for("bunny")
    y0s = [B.track(mesh, d)[0] for d in B.DISTANCES]
    vox = L.voxelise(mesh, 1 / 16)
    coef = MO.multipole_coefficients(MO.piece_moments(vox.piece_tris, vox.piece_of, vox.centroid))
    got = []
    for order, panel in ((ORDER, PANEL), (2 * ORDER, PANEL / 2)):
        prs = reads(mesh, y0s, order, panel)
        pts, sl = all_points(prs)
        got.append(series_u(prs, sl, R.mpole_field(vox.centroid, coef, pts).sum(axis = 1)))
    out = {str(d): float(np.abs(got[0][i] - got[1][i]).max() / np.abs(got[1][i]).max())
           for i, d in enumerate(B.DISTANCES)}
    res = json.loads((OUT / "budget-r4.json").read_text(encoding = "utf-8"))
    res["r4b_moments_residual_change_over_max"] = out
    (OUT / "budget-r4.json").write_text(json.dumps(res, indent = 1), encoding = "utf-8")
    print("r4b", out, flush = True)


def r6() -> None:
    """The analytic Jacobian against 1 mm central differences (and a 30 cm control) on the cube and
    the CAD part, every distance."""
    R.below_normal()
    out = {}
    for name in ("cube", "bracket"):
        mesh = B.mesh_for(name)
        tr = json.loads((OUT / f"truth-{name}.json").read_text(encoding = "utf-8"))
        y0s = tr["y0"]
        prs = reads(mesh, y0s)
        pts, sl = all_points(prs)
        worst, worst_ctrl = 0.0, 0.0
        for step, tag in ((1e-3, "fd"), (0.3, "control")):
            cols = {}
            for k, ax in enumerate("xyz"):
                e = np.zeros(3)
                e[k] = step
                plus = series_u(prs, sl, _mesh_fields(mesh.tris, pts - e)[0])
                minus = series_u(prs, sl, _mesh_fields(mesh.tris, pts + e)[0])
                cols[ax] = (plus - minus) / (2 * step)
            dv = step  # speed changed by ±step m/s: the points move at −(v ± dv)
            pv = [R.PassRead(B.TIMES, B.track_from(mesh, y0) - np.outer(B.TIMES, [dv, 0, 0]), U_BODY - [dv, 0, 0],
                             ORDER, PANEL) for y0 in y0s]
            mv = [R.PassRead(B.TIMES, B.track_from(mesh, y0) + np.outer(B.TIMES, [dv, 0, 0]), U_BODY + [dv, 0, 0],
                             ORDER, PANEL) for y0 in y0s]
            p_pts, p_sl = all_points(pv)
            m_pts, m_sl = all_points(mv)
            cols["v"] = (series_u(pv, p_sl, _mesh_fields(mesh.tris, p_pts)[0])
                         - series_u(mv, m_sl, _mesh_fields(mesh.tris, m_pts)[0])) / (2 * dv)
            rel = max(float(np.abs(cols[ax] - np.array(tr[f"dphi_d{ax}"])).max() / np.abs(tr[f"dphi_d{ax}"]).max())
                      for ax in ("x", "y", "z", "v"))
            if tag == "fd":
                worst = rel
            else:
                worst_ctrl = rel
        out[name] = {"fd_1mm_rel": worst, "control_30cm_rel": worst_ctrl}
        print(name, out[name], flush = True)
    out["pass"] = all(v["fd_1mm_rel"] <= 1e-5 and v["control_30cm_rel"] > 1e-3 for v in out.values() if isinstance(v, dict))
    (OUT / "budget-r6.json").write_text(json.dumps(out, indent = 1), encoding = "utf-8")


# ======================================= VOXELS =======================================


def run(name: str, seed: int) -> None:
    R.below_normal()
    mesh = B.mesh_for(name)
    tr = json.loads((OUT / f"truth-{name}.json").read_text(encoding = "utf-8"))
    prs = reads(mesh, tr["y0"])
    pts, sl = all_points(prs)
    phi_mesh = np.array(tr["phi"])
    size = float((mesh.verts.max(axis = 0) - mesh.verts.min(axis = 0)).max())
    rng = np.random.default_rng(1000 + seed)
    path = OUT / f"run-{name}-s{seed}.json"
    rows: list[dict[str, object]] = json.loads(path.read_text())["rows"] if path.exists() else []
    for n in B.NS:
        offset = rng.random(3)  # drawn for every n: the same placements as budget.py run
        if any(r["n"] == n for r in rows):
            continue
        h = size / n
        t0 = time.perf_counter()
        vox = L.voxelise(mesh, h, origin = h * offset)
        t_vox = time.perf_counter() - t0
        raw = MO.piece_moments(vox.piece_tris, vox.piece_of, vox.centroid)
        mp = R.mpole_field(vox.centroid, MO.multipole_coefficients(raw), pts)
        mass_b = vox.frac * h**3
        u0, _, uq, _ = R.point_field(vox.centroid, mass_b, vox.second, pts)
        cen = vox.boundary_centres
        u_ctr = R.u_cubes(cen, h, vox.ctr_in, pts)
        u_ss = R.u_cubes(cen, h, vox.ss, pts)
        p_mp = series_u(prs, sl, mp.sum(axis = 1))
        p_l5 = series_u(prs, sl, mp[:, 2])
        p_pt, p_q = series_u(prs, sl, u0), series_u(prs, sl, uq)
        p_piece = p_pt + p_q + p_mp
        n_in = len(vox.interior)
        s4 = vox.mesh_volume / (h**3 * (n_in + vox.ss.sum()))
        resid = {
            B.TECH[0]: series_u(prs, sl, u_ctr) - p_piece,
            B.TECH[1]: (s4 - 1) * (phi_mesh - p_piece) + s4 * series_u(prs, sl, u_ss) - p_piece,
            B.TECH[2]: p_pt - p_piece,
            B.TECH[3]: -p_mp,
            "l5 term (truncation estimate)": p_l5,
        }
        rows.append({"n": n, "h": h, "offset": offset.tolist(), "interior": n_in, "boundary": len(vox.frac),
                     "seconds": time.perf_counter() - t0, "seconds_clip": t_vox,
                     "volume_check": vox.stats["volume_rel_error"],
                     "r7_volume": float(np.abs(raw[:, 0] - mass_b).max() / h**3),
                     "resid": {k: v.tolist() for k, v in resid.items()}})
        rows.sort(key = lambda r: int(r["n"]))  # type: ignore[arg-type]
        path.write_text(json.dumps({"shape": name, "seed": seed, "rows": rows}), encoding = "utf-8")
        print(name, seed, n, f"{rows[-1]['seconds']:.0f} s",
              {k[:3]: f"{np.abs(v).max():.1e}" for k, v in resid.items()}, flush = True)
        del vox


# ======================================= INFERENCE =======================================


def jacobian(tr: dict, i: int, with_z: bool = False) -> np.ndarray:
    """Columns d phase / d (ln M, x₀, y₀, v[, z₀]) at distance index i, from the flight read."""
    cols = [np.array(tr["phi"])[i], np.array(tr["dphi_dx"])[i], np.array(tr["dphi_dy"])[i], np.array(tr["dphi_dv"])[i]]
    if with_z:
        cols.append(np.array(tr["dphi_dz"])[i])
    return np.stack(cols, axis = 1)


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "truth":
        for nm in sys.argv[2].split(","):
            truth(nm)
    elif cmd == "run":
        for nm in sys.argv[2].split(","):
            run(nm, int(sys.argv[3]))
    elif cmd == "r4":
        r4()
    elif cmd == "r4b":
        r4b()
    elif cmd == "r6":
        r6()
    elif cmd == "analyse":
        import analyse_real

        analyse_real.budget()
