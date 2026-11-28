"""The paper's walk-past (Fig. 13c–d, Table 7) with the real phase, and where the instantaneous read holds.

    python walkpast.py map               # map.json: point-mass passes over speed × distance (C4)
    python walkpast.py bodies [names]    # walkpast-<name>.json: Spot, the CAD part, the bunny, 3 poses
    python walkpast.py pose <name> <i>   # one pose into walkpast-<name>-pose<i>.json; then
    python walkpast.py merge <name>      # folds the three into walkpast-<name>.json
    python walkpast.py effheight [names] # adds the read at the arms' effective height to those jsons
    python walkpast.py plot              # walkpast.pdf/.png, map.pdf/.png, walkpast.md

Geometry as the paper's (`exact-skin-and-partial-fill/shapes/exact.py passby`): each mesh (1 m largest
extent, ρ = 2700 kg/m³) with its centre of mass level with the lower launch point, 2 m from the
tower's axis, walking at 1.4 m/s along y; three poses. For each shot whose π pulse is at time t:

    flight   Cavendish's flight integral, body moving during the shot (crosschecked: C1)
    frozen   the same integral, body frozen where it is at t
    leading  k_eff T² [g_z(0, 0, Δr) − g_z(0, 0, 0)] with the body where it is at t (the paper's)

Quadrature: Gauss order 6, panels ≤ 0.1 s, the engine's default rule (against order 16 at 0.01 s it
is off by 3e-13 relative on the point-mass walk). The bunny (69 664 triangles) is read every 0.2 s
over ±5 s, the others every 0.1 s over ±6 s.
"""

from __future__ import annotations

import json
import sys
import time
from dataclasses import replace
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np

import flight as F
import style as S

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "exact-skin-and-partial-fill" / "shapes"))
import shapes_lib as L  # noqa: E402

INS = replace(F.AION10, order = 6, panel = 0.1)
SPEED, OFFSET = 1.4, 2.0
TIMES = np.round(np.arange(-6.0, 6.0 + 1e-9, 0.1), 10)
TIMES_FOR = {"bunny": np.round(np.arange(-5.0, 5.0 + 1e-9, 0.2), 10)}
POSES = {"upright": np.eye(3), "turned 90° about z": L.rot_about(np.array([0, 0, 1.0]), 90.0),
         "on its side (90° about x)": L.rot_about(np.array([1.0, 0, 0]), 90.0)}
BODIES = ["spot", "bracket", "bunny"]
Z_EFF = F.effective_height(INS)
TOWER = np.zeros(3)


# ======================================= POINT-MASS MAP =======================================


def _three_reads(pot: F.Potential, fld: F.Field, times: np.ndarray, ins: F.Instrument = INS
                 ) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    fl = np.array([F.flight_phase(pot, ins, TOWER, t - ins.half) for t in times])
    fr = np.array([F.frozen_phase(pot, ins, TOWER, t - ins.half) for t in times])
    le = np.array([F.leading_phase(fld, ins, TOWER, t) for t in times])
    return fl, fr, le


def point_map() -> None:
    """A point mass (mass cancels in every ratio) passing along y at speed v, closest approach d
    horizontally, level with the lower launch point (z = 0) or midway (z = 5 m)."""
    speeds = np.geomspace(0.1, 30.0, 16)
    dists = np.array([1.0, 1.5, 2.0, 3.0, 5.0, 7.0, 10.0, 20.0])
    out: dict[str, object] = {"speeds_m_s": speeds.tolist(), "distances_m": dists.tolist(), "z": {}}
    law_checks = []
    for z in (0.0, 5.0):
        temporal = np.zeros((len(dists), len(speeds)))
        temporal_peak = np.zeros_like(temporal)
        spatial = np.zeros(len(dists))
        spatial_eff = np.zeros(len(dists))
        for i, d in enumerate(dists):
            for j, v in enumerate(speeds):
                span = 6.0 * max(d, 2.0) / v  # the pass, six closest-approach times either side
                dt = min(span / 120.0, 0.25 * max(d, 2.0) / v)
                ts = np.arange(-span, span + 1e-12, dt)
                path = F.straight(np.array([d, 0.0, z]), np.array([0.0, v, 0.0]))
                fl, fr, le = _three_reads(F.point_potential(1.0, path), F.point_field(1.0, path), ts)
                peak = int(np.argmax(np.abs(fr)))
                scale = float(np.abs(fr).max())
                temporal[i, j] = float(np.abs(fl - fr).max() / scale)
                temporal_peak[i, j] = float((fl[peak] - fr[peak]) / fr[peak])
                if j == 0:
                    spatial[i] = float(np.abs(fr - le).max() / scale)
                    le_eff = np.array([F.leading_phase(F.point_field(1.0, path), INS, TOWER, t, Z_EFF) for t in ts])
                    spatial_eff[i] = float(np.abs(fr - le_eff).max() / scale)
                # C4: predicted −φ''T²/12 at the peak from the frozen series (central difference).
                if 0 < peak < len(ts) - 1 and abs(temporal_peak[i, j]) > 1e-3:
                    second = (fr[peak + 1] - 2 * fr[peak] + fr[peak - 1]) / dt**2
                    pred = second * INS.half**2 / 12.0 / fr[peak]
                    law_checks.append({"z": z, "d": float(d), "v": float(v), "measured": temporal_peak[i, j],
                                       "predicted": float(pred), "ratio": float(temporal_peak[i, j] / pred)})
            print(f"z={z:g} d={d:g}: temporal {np.array2string(temporal[i], precision = 2)}", flush = True)
        out["z"][f"{z:g}"] = {"temporal_max_over_peak": temporal.tolist(), "temporal_at_peak": temporal_peak.tolist(),
                              "spatial_max_over_peak": spatial.tolist(),
                              "spatial_eff_max_over_peak": spatial_eff.tolist()}
    ratios = np.array([c["ratio"] for c in law_checks])
    out["C4"] = {"checks": law_checks, "ratio_min": float(ratios.min()), "ratio_max": float(ratios.max()),
                 "pass": bool(np.all((ratios > 1 / 1.5) & (ratios < 1.5)))}
    print(f"C4: measured/predicted in [{ratios.min():.3f}, {ratios.max():.3f}] over {len(ratios)} cases")
    (HERE / "map.json").write_text(json.dumps(out))


# ======================================= BODIES =======================================


def _mesh_sources(tris: np.ndarray, rot: np.ndarray, path) -> tuple[F.Potential, F.Field]:
    t_rot = tris @ rot.T

    def potential(pts: np.ndarray, times: np.ndarray) -> np.ndarray:
        rel = pts - path(times)
        u, _ = L.sb.polyhedron_field(t_rot, np.ones((len(t_rot), 1)), rel, chunk = max(8, 300_000 // len(rel)),
                                     stable = True)
        return -L.RHO * u[0]

    def field(pts: np.ndarray, times: np.ndarray) -> np.ndarray:
        rel = pts - path(times)
        return L.RHO * L.g_mesh(t_rot, rel)[:, 2]
    return potential, field


def bodies(names: list[str]) -> None:
    for name in names:
        t0 = time.perf_counter()
        mesh = L.mesh_by_name(name)
        vol, cen, _, _ = L.body_moments(mesh.tris)
        tris = mesh.tris - cen[None, None, :]
        path = F.straight(np.array([OFFSET, 0.0, 0.0]), np.array([0.0, SPEED, 0.0]))
        times = TIMES_FOR.get(name, TIMES)
        res: dict[str, object] = {"t": times.tolist(), "mass_kg": L.RHO * vol, "triangles": len(mesh.faces),
                                  "poses": {}}
        for pose, rot in POSES.items():
            pot, fld = _mesh_sources(tris, rot, path)
            fl, fr, le = _three_reads(pot, fld, times)
            res["poses"][pose] = {"flight": fl.tolist(), "frozen": fr.tolist(), "leading": le.tolist()}
            print(f"{name} {pose}: peak flight {np.abs(fl).max():.4f}, frozen {np.abs(fr).max():.4f}, "
                  f"leading {np.abs(le).max():.4f} rad ({time.perf_counter() - t0:.0f} s)", flush = True)
        pot, fld = F.point_potential(L.RHO * vol, path), F.point_field(L.RHO * vol, path)
        fl, fr, le = _three_reads(pot, fld, times)
        res["poses"]["point mass"] = {"flight": fl.tolist(), "frozen": fr.tolist(), "leading": le.tolist()}
        res["seconds"] = time.perf_counter() - t0
        (HERE / f"walkpast-{name}.json").write_text(json.dumps(res))


def one_pose(name: str, index: int) -> None:
    """One pose of one body into walkpast-<name>-pose<index>.json (to run poses in parallel);
    `merge` folds them into walkpast-<name>.json."""
    mesh = L.mesh_by_name(name)
    vol, cen, _, _ = L.body_moments(mesh.tris)
    tris = mesh.tris - cen[None, None, :]
    path = F.straight(np.array([OFFSET, 0.0, 0.0]), np.array([0.0, SPEED, 0.0]))
    times = TIMES_FOR.get(name, TIMES)
    pose, rot = list(POSES.items())[index]
    pot, fld = _mesh_sources(tris, rot, path)
    fl, fr, le = _three_reads(pot, fld, times)
    out = {"t": times.tolist(), "mass_kg": L.RHO * vol, "triangles": len(mesh.faces), "pose": pose,
           "reads": {"flight": fl.tolist(), "frozen": fr.tolist(), "leading": le.tolist()}}
    (HERE / f"walkpast-{name}-pose{index}.json").write_text(json.dumps(out))
    print(f"{name} {pose}: peak flight {np.abs(fl).max():.4f}, frozen {np.abs(fr).max():.4f}, "
          f"leading {np.abs(le).max():.4f} rad", flush = True)


def merge(name: str) -> None:
    """walkpast-<name>.json from its per-pose files (and the point mass, computed here)."""
    parts = [json.loads((HERE / f"walkpast-{name}-pose{i}.json").read_text(encoding = "utf-8"))
             for i in range(len(POSES))]
    times = np.array(parts[0]["t"])
    res: dict[str, object] = {"t": parts[0]["t"], "mass_kg": parts[0]["mass_kg"],
                              "triangles": parts[0]["triangles"], "poses": {p["pose"]: p["reads"] for p in parts}}
    path = F.straight(np.array([OFFSET, 0.0, 0.0]), np.array([0.0, SPEED, 0.0]))
    pot, fld = F.point_potential(parts[0]["mass_kg"], path), F.point_field(parts[0]["mass_kg"], path)
    fl, fr, le = _three_reads(pot, fld, times)
    res["poses"]["point mass"] = {"flight": fl.tolist(), "frozen": fr.tolist(), "leading": le.tolist()}
    (HERE / f"walkpast-{name}.json").write_text(json.dumps(res))
    for i in range(len(POSES)):
        (HERE / f"walkpast-{name}-pose{i}.json").unlink()


def effheight(names: list[str]) -> None:
    """Adds `leading_eff` to each walkpast-<name>.json: the paper's read at the arms' effective
    height (flight.effective_height), cheap (two points per shot)."""
    for name in names:
        path_json = HERE / f"walkpast-{name}.json"
        res = json.loads(path_json.read_text(encoding = "utf-8"))
        times = np.array(res["t"])
        mesh = L.mesh_by_name(name)
        vol, cen, _, _ = L.body_moments(mesh.tris)
        tris = mesh.tris - cen[None, None, :]
        path = F.straight(np.array([OFFSET, 0.0, 0.0]), np.array([0.0, SPEED, 0.0]))
        for pose, rot in POSES.items():
            _, fld = _mesh_sources(tris, rot, path)
            res["poses"][pose]["leading_eff"] = [F.leading_phase(fld, INS, TOWER, t, Z_EFF) for t in times]
        fld = F.point_field(L.RHO * vol, path)
        res["poses"]["point mass"]["leading_eff"] = [F.leading_phase(fld, INS, TOWER, t, Z_EFF) for t in times]
        res["z_eff_m"] = Z_EFF
        path_json.write_text(json.dumps(res))
        print(name, "leading_eff added", flush = True)


# ======================================= PLOTS AND TABLE =======================================


def _summary(res: dict[str, object]) -> dict[str, dict[str, float]]:
    poses = {k: {r: np.array(v) for r, v in p.items()} for k, p in res["poses"].items()}
    out: dict[str, dict[str, float]] = {}
    for read in ("flight", "frozen", "leading", "leading_eff"):
        if read not in poses["upright"]:
            continue
        series = [poses[k][read] for k in POSES]
        peaks = [float(np.abs(s).max()) for s in series]
        spread = max(float(np.abs(a - b).max()) for a in series for b in series)
        out[read] = {"peaks": peaks, "point_peak": float(np.abs(poses["point mass"][read]).max()),
                     "spread": spread, "spread_over_peak": spread / max(peaks)}
    up = poses["upright"]
    out["upright_errors"] = {"temporal_max_over_peak": float(np.abs(up["flight"] - up["frozen"]).max()
                                                             / np.abs(up["frozen"]).max()),
                             "paper_total_max_over_peak": float(np.abs(up["flight"] - up["leading"]).max()
                                                                / np.abs(up["flight"]).max())}
    return out


def plot() -> None:
    S.apply()
    have = [n for n in BODIES if (HERE / f"walkpast-{n}.json").exists()]
    md = ["| body | read | peak upright / turned / on side (rad) | point mass (rad) | largest pose difference (rad) "
          "| over peak |", "|---|---|---|---|---|---|"]
    fig, axes = plt.subplots(2, len(have), figsize = (6.8, 4.2), squeeze = False, sharex = True)
    summary = {}
    for col, name in enumerate(have):
        res = json.loads((HERE / f"walkpast-{name}.json").read_text(encoding = "utf-8"))
        t = np.array(res["t"])
        s = _summary(res)
        summary[name] = s
        for read in ("leading", "leading_eff", "frozen", "flight"):
            if read not in s:
                continue
            r = s[read]
            md.append(f"| {name} | {read} | " + " / ".join(f"{p:.4f}" for p in r["peaks"])
                      + f" | {r['point_peak']:.4f} | {r['spread']:.3e} | {100 * r['spread_over_peak']:.1f} % |")
        ax, bx = axes[0, col], axes[1, col]
        for k, pose in enumerate(POSES):
            p = res["poses"][pose]
            ax.plot(t, p["flight"], color = S.SERIES[k], lw = 1.1, label = f"{pose}, flight")
            ax.plot(t, p["leading"], color = S.SERIES[k], lw = 0.7, ls = ":", label = f"{pose}, leading")
            bx.plot(t, np.array(p["flight"]) - np.array(p["frozen"]), color = S.SERIES[k], lw = 1.0)
            bx.plot(t, np.array(p["frozen"]) - np.array(p["leading"]), color = S.SERIES[k], lw = 0.7, ls = "--")
        ax.set_title(f"{name} ({res['mass_kg']:.0f} kg)", fontsize = 8)
        bx.set_xlabel("time of the π pulse (s)")
    axes[0, 0].set_ylabel("phase (rad)")
    axes[1, 0].set_ylabel("error of the read (rad)")
    h, lab = axes[0, 0].get_legend_handles_labels()
    fig.legend(h, lab, loc = "upper center", ncol = 3, fontsize = 6, bbox_to_anchor = (0.5, 1.07))
    fig.text(0.5, -0.01, "bottom: solid, temporal (flight − frozen); dashed, spatial (frozen − leading)",
             ha = "center", fontsize = 6.5, color = S.INK2)
    fig.tight_layout()
    fig.savefig(HERE / "walkpast.pdf")
    fig.savefig(HERE / "walkpast.png", dpi = 200)
    (HERE / "walkpast-summary.json").write_text(json.dumps(summary, indent = 1))
    if (HERE / "map.json").exists():
        plot_map(md)
    (HERE / "walkpast.md").write_text("\n".join(md) + "\n", encoding = "utf-8")


def plot_map(md: list[str]) -> None:
    m = json.loads((HERE / "map.json").read_text(encoding = "utf-8"))
    v, d = np.array(m["speeds_m_s"]), np.array(m["distances_m"])
    fig, axes = plt.subplots(1, 2, figsize = (6.8, 2.7), sharey = True)
    for ax, z in zip(axes, ("0", "5")):
        err = np.array(m["z"][z]["temporal_max_over_peak"])
        cs = ax.contourf(v, d, np.log10(np.maximum(err, 1e-8)), levels = np.arange(-6, 1.01, 0.5), cmap = "cividis")
        c = ax.contour(v, d, err, levels = [0.01, 0.1], colors = [S.ORANGE, S.VERMILION], linewidths = 1.2)
        ax.clabel(c, fmt = {0.01: "1 %", 0.1: "10 %"}, fontsize = 6.5)
        vv = np.geomspace(v.min(), v.max(), 50)
        ax.plot(vv, vv * 0.73, color = S.SKY, lw = 0.8, ls = "--", label = "d = vT")
        ax.set_xscale("log")
        ax.set_yscale("log")
        ax.set_ylim(d.min(), d.max())
        ax.set_xlabel("speed (m/s)")
        ax.set_title(f"point source {'level with the lower launch' if z == '0' else 'at mid-tower (5 m)'}",
                     fontsize = 8)
    axes[0].set_ylabel("closest approach (m)")
    cb = fig.colorbar(cs, ax = axes, shrink = 0.9)
    cb.set_label(r"$\log_{10}$ max|flight − frozen| / peak")
    fig.savefig(HERE / "map.pdf")
    fig.savefig(HERE / "map.png", dpi = 200)
    md += ["", "Point-mass passes: speed at which the temporal error (max over the pass, over the peak) reaches "
           "1 % and 10 %, by closest approach (z = 0 / z = 5 m).", "", "| d (m) | 1 % at v (m/s) | 10 % at v (m/s) |",
           "|---|---|---|"]
    for i, dd in enumerate(d):
        cells = []
        for lvl in (0.01, 0.1):
            vals = []
            for z in ("0", "5"):
                e = np.array(m["z"][z]["temporal_max_over_peak"])[i]
                k = np.nonzero(e >= lvl)[0]
                if len(k) == 0:
                    vals.append("> 30")
                elif k[0] == 0:
                    vals.append("< 0.1")
                else:
                    a, b = k[0] - 1, k[0]
                    x = (np.log(lvl) - np.log(e[a])) / (np.log(e[b]) - np.log(e[a]))
                    vals.append(f"{np.exp(np.log(v[a]) + x * (np.log(v[b]) - np.log(v[a]))):.2g}")
            cells.append(" / ".join(vals))
        md.append(f"| {dd:g} | " + " | ".join(cells) + " |")


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "map":
        point_map()
    elif cmd == "pose":
        one_pose(sys.argv[2], int(sys.argv[3]))
    elif cmd == "merge":
        merge(sys.argv[2])
    elif cmd == "effheight":
        effheight(sys.argv[2:] or BODIES)
    elif cmd == "bodies":
        bodies(sys.argv[2:] or BODIES)
    else:
        plot()
