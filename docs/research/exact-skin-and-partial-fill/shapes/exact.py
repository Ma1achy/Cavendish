"""Each famous mesh as one exact polyhedron (no voxels, no clipping): what an AION-10 gradiometer
reads from it, and its mass moments in closed form.

    python exact.py moments            # exact-moments.json: volume, centre of mass, inertia, per mesh
    python exact.py distance           # exact-distance.json: phase against distance (0.5–50 m)
    python exact.py passby             # exact-passby.json: phase as the body walks past, 3 poses
    python exact.py plot               # exact.png, exact.md

Frame and tower: the tower's lower cloud at the origin, the upper one BASELINE (5 m) above it; the
phase is k_eff T² Δg_z (upper − lower), ρ = 2700 kg/m³. Each body is the study's mesh (1 m largest
extent, decimated where `DECIMATE` says) and, where it is in `FULL`, the full-resolution mesh too.

What is exact and what is not:
- exact (to rounding): the field of the polyhedron at each point (Werner & Scheeres 1997 per face,
  with the stable solid-angle numerator, `skin_bench.polyhedron_field`), and the moments (volume,
  centroid, second and third moments) by the divergence theorem over the same triangles;
- not exact: the phase is taken from the field at the two clouds' mean positions at one instant
  (k_eff T² Δg_z), not integrated along the atoms' paths over the 2T of the sequence; the multipole
  curves are truncated series (l ≤ 3); and the mesh is itself an approximation of the real object
  (a scan, a decimation of it, capped holes).
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
import shapes_lib as L  # noqa: E402

NAMES = ["bunny", "spot", "dragon", "armadillo", "buddha", "bracket"]
# Full-resolution closed meshes (fullres.py dump): the bunny's holes capped; the Buddha's 32
# non-manifold edges dropped and capped; the dragon and Armadillo as downloaded (closed).
FULL = {"armadillo": "armadillo", "dragon": "dragon-closed", "buddha": "buddha-closed"}
DIST = np.geomspace(0.5, 50.0, 31)  # m, body centre of mass to the tower's axis, level with the lower cloud
SPEED = 1.4  # m/s, walking pace
OFFSET = 2.0  # m, closest approach of the centre of mass to the tower's axis
TIMES = np.linspace(-12.0, 12.0, 241)  # s
POSES = {"upright": np.eye(3), "turned 90° about z": L.rot_about(np.array([0, 0, 1.0]), 90.0),
         "on its side (90° about x)": L.rot_about(np.array([1.0, 0, 0]), 90.0)}


def _meshes() -> dict[str, L.Mesh]:
    out = {n: L.mesh_by_name(n) for n in NAMES}
    for n, tag in FULL.items():
        path = L.CACHE / "full" / f"{tag}.f64"
        if path.exists():
            t = np.fromfile(path, dtype="<f8").reshape(-1, 3, 3)
            v, inv = np.unique(t.reshape(-1, 3), axis=0, return_inverse=True)
            out[f"{n} (full)"] = L.Mesh(n, v, inv.reshape(-1, 3), f"{tag}: {len(t)} triangles")
    return out


def _centred(m: L.Mesh) -> tuple[L.Mesh, float, np.ndarray, np.ndarray, np.ndarray]:
    """The mesh moved so its centre of mass is at the origin, and its moments there."""
    vol, cen, m2, m3 = L.body_moments(m.tris)
    return L.Mesh(m.name, m.verts - cen, m.faces, m.source), vol, cen, m2, m3


def moments() -> None:
    res = {}
    for name, m in _meshes().items():
        vol, cen, m2, m3 = L.body_moments(m.tris)
        inertia = L.RHO * (np.trace(m2) * np.eye(3) - m2)  # kg m², about the centre of mass
        res[name] = {"triangles": len(m.faces), "volume_m3": vol, "mass_kg": L.RHO * vol, "centre_of_mass_m": cen.tolist(),
                     "inertia_kg_m2": inertia.tolist(), "principal_kg_m2": np.linalg.eigvalsh(inertia).tolist(),
                     "third_moment_norm_m6": float(np.linalg.norm(m3))}
        print(name, res[name]["mass_kg"], flush=True)
    (L.HERE / "exact-moments.json").write_text(json.dumps(res, indent=1))


def _phase_at(m: L.Mesh, centres: np.ndarray) -> np.ndarray:
    """Phase (rad) with the body's centre of mass at each of `centres` (N, 3): the tower is moved
    instead of the body, which is the same field."""
    lower = -centres
    pts = np.concatenate([lower, lower + [0.0, 0.0, L.BASELINE]])
    return L.phase(L.g_mesh(m.tris, pts))


def distance() -> None:
    res = {}
    for name, m in _meshes().items():
        t0 = time.perf_counter()
        c, vol, _, m2, m3 = _centred(m)
        centres = np.stack([DIST, np.zeros_like(DIST), np.zeros_like(DIST)], axis=1)
        exact = _phase_at(c, centres)
        lower = -centres
        pts = np.concatenate([lower, lower + [0.0, 0.0, L.BASELINE]])
        o = np.zeros(3)
        point = L.phase(L.g_multipole(vol, o, None, None, pts))
        quad = L.phase(L.g_multipole(vol, o, m2, None, pts))
        octu = L.phase(L.g_multipole(vol, o, m2, m3, pts))
        r_body = float(np.linalg.norm(c.verts, axis=1).max())
        res[name] = {"d": DIST.tolist(), "exact": exact.tolist(), "point": point.tolist(), "l2": quad.tolist(),
                     "l3": octu.tolist(), "radius_m": r_body, "seconds": time.perf_counter() - t0,
                     "triangles": len(m.faces)}
        print(name, f"{time.perf_counter() - t0:.1f}s", flush=True)
    (L.HERE / "exact-distance.json").write_text(json.dumps(res))


def passby() -> None:
    res = {}
    for name, m in _meshes().items():
        if "(full)" in name:
            continue  # the decimated mesh's pass-by differs from the full one's far below the plot
        t0 = time.perf_counter()
        c, vol, _, _, _ = _centred(m)
        centres = np.stack([np.full_like(TIMES, OFFSET), SPEED * TIMES, np.zeros_like(TIMES)], axis=1)
        out = {}
        for pose, rot in POSES.items():
            out[pose] = _phase_at(c.moved(rot), centres).tolist()
        lower = -centres
        pts = np.concatenate([lower, lower + [0.0, 0.0, L.BASELINE]])
        out["point mass"] = L.phase(L.g_multipole(vol, np.zeros(3), None, None, pts)).tolist()
        res[name] = {"t": TIMES.tolist(), "phase": out, "seconds": time.perf_counter() - t0}
        print(name, f"{time.perf_counter() - t0:.1f}s", flush=True)
    (L.HERE / "exact-passby.json").write_text(json.dumps(res))


def _style() -> None:
    plt.rcParams.update({"font.size": 10, "axes.edgecolor": "#c3c2b7", "axes.labelcolor": "#52514e",
                         "xtick.color": "#52514e", "ytick.color": "#52514e", "axes.titlecolor": "#0b0b0b",
                         "figure.facecolor": "#fcfcfb", "axes.facecolor": "#fcfcfb"})


def _clean(ax: plt.Axes) -> None:
    ax.grid(True, which="major", color="#e1e0d9", lw=0.6)
    for s in ("top", "right"):
        ax.spines[s].set_visible(False)


def plot() -> None:
    _style()
    dist = json.loads((L.HERE / "exact-distance.json").read_text())
    pas = json.loads((L.HERE / "exact-passby.json").read_text())
    mom = json.loads((L.HERE / "exact-moments.json").read_text())
    fig, axes = plt.subplots(3, 6, figsize=(24, 11.5), gridspec_kw={"height_ratios": [1.2, 0.8, 1.1]})
    for col, name in enumerate(NAMES):
        r = dist[name]
        d = np.array(r["d"])
        ex = np.array(r["exact"])
        ax = axes[0, col]
        ax.loglog(d, np.abs(ex), "-", color="#0b0b0b", lw=2.2, label="exact polyhedron")
        ax.loglog(d, np.abs(r["point"]), ":", color="#2a78d6", lw=2, label="point mass")
        ax.axvspan(d.min(), r["radius_m"], color="#e1e0d9", alpha=0.7, lw=0)
        ax.set_title(f"{name}: {mom[name]['mass_kg']:.0f} kg, {r['triangles']} triangles", fontsize=10)
        ax.set_xlabel("centre of mass to tower axis (m)")
        _clean(ax)
        ax2 = axes[1, col]
        for key, colour, label in (("point", "#2a78d6", "point mass"), ("l2", "#eb6834", "+ second moment"),
                                   ("l3", "#1baf7a", "+ third moment")):
            ax2.loglog(d, np.maximum(np.abs(np.array(r[key]) - ex) / np.abs(ex), 1e-16), "-", color=colour, lw=1.8,
                       label=label)
        full = dist.get(f"{name} (full)")
        if full is not None:
            ax2.loglog(d, np.maximum(np.abs(np.array(full["exact"]) - ex) / np.abs(ex), 1e-16), "--", color="#6250d6",
                       lw=1.5, label="decimated vs full mesh")
        ax2.axvspan(d.min(), r["radius_m"], color="#e1e0d9", alpha=0.7, lw=0)
        ax2.set_xlabel("centre of mass to tower axis (m)")
        _clean(ax2)
        ax3 = axes[2, col]
        t = np.array(pas[name]["t"])
        for (pose, ph), colour in zip(pas[name]["phase"].items(), ("#0b0b0b", "#eb6834", "#1baf7a", "#2a78d6")):
            ax3.plot(t, ph, ":" if pose == "point mass" else "-", color=colour, lw=1.8 if pose != "point mass" else 2,
                     label=pose)
        ax3.set_xlabel("time (s), walking at 1.4 m/s, 2 m off")
        _clean(ax3)
    axes[0, 0].set_ylabel("|phase| (rad)")
    axes[1, 0].set_ylabel("|series − exact| / |exact|")
    axes[2, 0].set_ylabel("phase (rad)")
    for row, loc in ((0, "upper right"), (1, "upper right"), (2, "upper left")):
        h_, l_ = [], []
        for ax in axes[row]:
            for hh, ll in zip(*ax.get_legend_handles_labels()):
                if ll not in l_:
                    h_.append(hh)
                    l_.append(ll)
        axes[row, -1].legend(h_, l_, frameon=False, fontsize=8.5, loc=loc)
    fig.text(0.01, 0.005, "Each body as one exact polyhedron (Werner & Scheeres, stable numerator), ρ = 2700 kg/m³, "
             "tower: lower cloud at the origin, upper 5 m above; phase = k_eff T² Δg_z at one instant (no "
             "quadrature along the atoms' paths). Grey: the tower axis would be within the body's reach "
             "(centre-of-mass distance below its largest radius). Top: the signal; middle: how far the multipole "
             "series (about the centre of mass) and the decimation are from it; bottom: walking past at three poses, "
             "same mass.", fontsize=8.5, color="#52514e", wrap=True)
    fig.subplots_adjust(left=0.04, right=0.995, top=0.97, bottom=0.07, wspace=0.28, hspace=0.32)
    fig.savefig(L.HERE / "exact.png", dpi=100)
    plt.close(fig)
    md = ["| mesh | triangles | volume (m³) | mass (kg) | centre of mass (m) | principal moments of inertia (kg m²) |",
          "|---|---|---|---|---|---|"]
    for name, r in mom.items():
        md.append(f"| {name} | {r['triangles']} | {r['volume_m3']:.6f} | {r['mass_kg']:.2f} | "
                  f"({', '.join(f'{x:+.4f}' for x in r['centre_of_mass_m'])}) | "
                  f"{', '.join(f'{x:.3f}' for x in r['principal_kg_m2'])} |")
    md += ["", "| mesh | peak phase walking past, upright (rad) | turned 90° about z | on its side | point mass | "
           "largest pose difference (rad) |", "|---|---|---|---|---|---|"]
    for name in NAMES:
        p = {k: np.array(v) for k, v in pas[name]["phase"].items()}
        poses = [p[k] for k in POSES]
        spread = max(np.abs(a - b).max() for a in poses for b in poses)
        md.append(f"| {name} | " + " | ".join(f"{np.abs(p[k]).max():.4f}" for k in [*POSES, "point mass"])
                  + f" | {spread:.2e} |")
    md += ["", "| mesh | departure of the point mass from exact: 1.5 m / 5 m / 20 m | + second moment | + third moment |",
           "|---|---|---|---|"]
    for name, r in dist.items():
        d = np.array(r["d"])
        ex = np.array(r["exact"])
        cells = []
        for key in ("point", "l2", "l3"):
            rel = np.abs(np.array(r[key]) - ex) / np.abs(ex)
            log_rel = np.log(np.maximum(rel, 1e-300))
            cells.append(" / ".join(f"{np.exp(np.interp(np.log(x), np.log(d), log_rel)):.1e}" for x in (1.5, 5.0, 20.0)))
        md.append(f"| {name} | " + " | ".join(cells) + " |")
    (L.HERE / "exact.md").write_text("\n".join(md) + "\n", encoding="utf-8")


if __name__ == "__main__":
    {"moments": moments, "distance": distance, "passby": passby, "plot": plot}[sys.argv[1]]()
