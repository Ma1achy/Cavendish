"""Techniques across distance regimes, grid orientation and motion.

    python regimes.py distance      # regimes-distance.png, regimes-distance.json
    python regimes.py distance bunny dragon   # only these shapes (the rest kept from the json)
    python regimes.py orientation   # orientation.png, orientation.json
    python regimes.py translation   # translation.png, translation-spectrum.png, translation.json
    python regimes.py spin          # spin.json (body frame), then
    python regimes.py spinworld     # adds the world-fixed period to spin.json; spin.png

Techniques, all from the same voxelisation (interior cells are full cubes in (a)–(e); they cancel in
the error, so only boundary cells are summed, as in skin_bench.py):
    (a) cubes       a boundary cell is a full cube if its centre is inside, else empty (no renormalisation)
    (b) sampled     fill = 4³ samples inside / 64, cube at the centre, mass renormalised (Cavendish today)
    (c) moments     exact fill as a point mass at the piece's centroid
    (d) +second     (c) plus the piece's own second moment
    (e) exact       the clipped pieces themselves (error = rounding; plotted as a check)
    (f) multipole   the whole body: mass, second and third moments about its centroid (no voxels)
    (g) point       the whole body as a point mass at its centroid
Reference: the polyhedral closed form on the whole mesh. Phase = k_eff T² Δg_z, ρ = 2700 kg/m³.
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

TECH = ["(a) cubes", "(b) sampled fill", "(c) fill at centroid", "(d) + second moment",
        "(e) exact pieces", "(f) body multipole", "(g) point mass"]
COLOURS = ["#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4", "#008300", "#6250d6"]
TOLS = (1e-6, 1e-10)


def _style() -> None:
    plt.rcParams.update({"font.size": 10, "axes.edgecolor": "#c3c2b7", "axes.labelcolor": "#52514e",
                         "xtick.color": "#52514e", "ytick.color": "#52514e", "axes.titlecolor": "#0b0b0b",
                         "figure.facecolor": "#fcfcfb", "axes.facecolor": "#fcfcfb"})


def _clean(ax: plt.Axes) -> None:
    ax.grid(True, which="major", color="#e1e0d9", lw=0.6)
    for s in ("top", "right"):
        ax.spines[s].set_visible(False)


def technique_errors(vox: L.Voxels, pts: np.ndarray, g_mesh: np.ndarray, with_exact: bool = True,
                     body: tuple[float, np.ndarray, np.ndarray, np.ndarray] | None = None) -> dict[str, np.ndarray]:
    """Phase error (rad) of each technique at the tower points `pts` (2M, 3)."""
    h = vox.h
    g_p = L.g_mesh(vox.piece_tris, pts)
    cen = vox.boundary_centres
    g_ctr = L.g_cubes(cen, h, vox.ctr_in, pts)
    g_ss = L.g_cubes(cen, h, vox.ss, pts)
    n_in = len(vox.interior)
    s4 = vox.mesh_volume / (h**3 * (n_in + vox.ss.sum()))
    mass = vox.frac * h**3
    g_pt = L.g_points(mass, vox.centroid, pts)
    g_q = g_pt + L.g_quadrupole(vox.second, vox.centroid, pts)
    ph = L.phase
    out = {
        TECH[0]: ph(g_ctr) - ph(g_p),
        TECH[1]: (s4 - 1) * (ph(g_mesh) - ph(g_p)) + s4 * ph(g_ss) - ph(g_p),
        TECH[2]: ph(g_pt) - ph(g_p),
        TECH[3]: ph(g_q) - ph(g_p),
    }
    if with_exact:
        g_in = L.g_cubes(vox.interior_centres, h, np.ones(n_in), pts)
        out[TECH[4]] = ph(g_in + g_p) - ph(g_mesh)
    if body is not None:
        vol, c, m2, m3 = body
        out[TECH[5]] = ph(L.g_multipole(vol, c, m2, m3, pts)) - ph(g_mesh)
        out[TECH[6]] = ph(L.g_multipole(vol, c, None, None, pts)) - ph(g_mesh)
    return out


# ================================ DISTANCE ================================

DIST_SHAPES = {"sphere": 0.0625, "torus": 0.05, "teapot": 0.03125,
               **{n: 0.03125 for n in L.FAMOUS}}  # the famous meshes (26 Nov 2026), 1 m like the teapot


def distance(names: list[str] | None = None) -> None:
    """All shapes, or only `names` (the others are kept from regimes-distance.json)."""
    path = L.HERE / "regimes-distance.json"
    res: dict[str, object] = json.loads(path.read_text()) if names and path.exists() else {}
    for name, h in DIST_SHAPES.items():
        if names and name not in names:
            continue
        mesh = L.mesh_by_name(name)
        size = float((mesh.verts.max(axis=0) - mesh.verts.min(axis=0)).max())
        d = np.unique(np.concatenate([h * np.array([0.5, 1, 1.5, 2, 3, 5]), np.geomspace(0.5, 1000, 18)]))
        pts = L.tower(mesh, d)
        t0 = time.perf_counter()
        vox = L.voxelise(mesh, h)
        t1 = time.perf_counter()
        g_mesh = L.g_mesh(mesh.tris, pts)
        err = technique_errors(vox, pts, g_mesh, body=L.body_moments(mesh.tris))
        res[name] = {"h": h, "size": size, "d": d.tolist(), "signal": L.phase(g_mesh).tolist(),
                     "err": {k: v.tolist() for k, v in err.items()}, "seconds_voxelise": t1 - t0,
                     "boundary": len(vox.frac)}
        print(name, f"{t1 - t0:.1f}s", flush=True)
        res = {n: res[n] for n in DIST_SHAPES if n in res}
        path.write_text(json.dumps(res))
    plot_distance()


def crossover(d: np.ndarray, err: np.ndarray, tol: float) -> float:
    """Smallest sampled distance beyond which |err| stays below tol (inf if never)."""
    bad = np.nonzero(np.abs(err) >= tol)[0]
    if len(bad) == 0:
        return float(d[0])
    return float(d[bad[-1] + 1]) if bad[-1] + 1 < len(d) else float("inf")


def plot_distance() -> None:
    _style()
    res = json.loads((L.HERE / "regimes-distance.json").read_text())
    ncol = min(len(res), 3)
    nrow = -(-len(res) // ncol)
    height = 4.6 * nrow + 1.0
    fig, axes = plt.subplots(nrow, ncol, figsize=(16, height), sharey=True, squeeze=False)
    for k in range(len(res), nrow * ncol):
        axes.flat[k].set_visible(False)
    lines = ["| shape (pitch) | technique | good to 1e-6 rad beyond | good to 1e-10 rad beyond |",
             "|---|---|---|---|"]
    for ax, (name, r) in zip(axes.flat, res.items()):
        d = np.array(r["d"])
        size, h = r["size"], r["h"]
        x = d / size
        ax.loglog(x, np.abs(r["signal"]), ":", color="#898781", lw=1.5, label="signal")
        ax.axvspan(x.min() / 1.3, 3 * h / size, color="#e1e0d9", alpha=0.6, lw=0)
        for tol in TOLS:
            ax.axhline(tol, color="#52514e", lw=0.8, ls="--")
        for k, (tech, colour) in enumerate(zip(TECH, COLOURS)):
            e = np.abs(np.array(r["err"][tech]))
            ax.loglog(x, np.maximum(e, 1e-17), "-", color=colour, lw=2, marker="o", ms=3.5, label=tech)
            cells = []
            for tol in TOLS:
                c = crossover(d, e, tol)
                if np.isfinite(c):
                    ax.plot(c / size, tol, "v", color=colour, ms=8, mec="#fcfcfb", mew=1)
                cells.append("never (in range)" if not np.isfinite(c) else
                             f"{c:.3g} m = {c / size:.2g} sizes = {c / h:.3g} cells")
            lines.append(f"| {name} ({h} m) | {tech} | {cells[0]} | {cells[1]} |")
        ax.set_title(f"{name}: size {size:.2g} m, pitch {h} m, {r['boundary']} boundary cells", fontsize=10)
        ax.set_xlabel("distance from surface / body size")
        ax.set_ylim(1e-17, 1e3)
        ax.text(x.min(), 3e1, "≤ 3 cells", fontsize=8.5, color="#52514e")
        _clean(ax)
    for row in axes:
        row[0].set_ylabel("|phase error| (rad)")
    h_, l_ = axes[0, 0].get_legend_handles_labels()
    fig.legend(h_, l_, loc="upper center", ncol=8, frameon=False)
    fig.text(0.01, 0.1 / height, "Dashed: tolerances 1e-6 and 1e-10 rad; triangles: where each technique stays "
             "below them for every larger distance. Reference: polyhedral closed form on the mesh. "
             "Floor ~1e-14–1e-16 rad is rounding.", fontsize=8.5, color="#52514e")
    fig.subplots_adjust(left=0.05, right=0.99, top=1 - 0.7 / height, bottom=0.75 / height, wspace=0.06,
                        hspace=0.32)
    fig.savefig(L.HERE / "regimes-distance.png", dpi=110)
    (L.HERE / "regimes-distance.md").write_text("\n".join(lines) + "\n", encoding="utf-8")


# ================================ ORIENTATION ================================

ANGLES_SMALL = [0.0, 0.1, 0.3, 1.0, 3.0, 10.0, 20.0, 30.0, 45.0]
TORUS_SWEEP = sorted(set([0.1, 0.3, 1.0] + [1.5 * i for i in range(61)]))


def box_mesh(side: float = 0.8) -> L.Mesh:
    t = L.sb.cube_tris(np.zeros((1, 3)), side)
    v = t.reshape(-1, 3)
    return L.Mesh("box", v, np.arange(len(v)).reshape(-1, 3), f"cube of side {side} m")


def orientation() -> None:
    h = 0.1
    generic_axis = np.array([0.3, 0.5, 0.81])
    cases = [("torus", L.torus_mesh(), np.array([0.0, 0.0, 1.0]), TORUS_SWEEP),
             ("box", box_mesh(), generic_axis, ANGLES_SMALL),
             ("teapot", L.teapot_mesh(), generic_axis, ANGLES_SMALL)]
    res: dict[str, object] = {}
    for name, mesh, axis, angles in cases:
        pts = L.tower(mesh, np.array([1.5]))
        c = 0.5 * (mesh.verts.min(axis=0) + mesh.verts.max(axis=0))
        errs: dict[str, list[float]] = {t: [] for t in TECH[:4]}
        signal = []
        for ang in angles:
            m = L.Mesh(mesh.name, (mesh.verts - c) @ L.rot_about(axis, ang).T + c, mesh.faces, mesh.source)
            vox = L.voxelise(m, h)
            g_mesh = L.g_mesh(m.tris, pts)
            e = technique_errors(vox, pts, g_mesh, with_exact=False)
            for t in TECH[:4]:
                errs[t].append(float(e[t][0]))
            signal.append(float(L.phase(g_mesh)[0]))
            print(name, ang, {t: f"{v[-1]:.2e}" for t, v in errs.items()}, flush=True)
        res[name] = {"angles": angles, "axis": axis.tolist(), "err": errs, "signal": signal, "h": h}
    (L.HERE / "orientation.json").write_text(json.dumps(res))
    plot_orientation()


def plot_orientation() -> None:
    _style()
    res = json.loads((L.HERE / "orientation.json").read_text())
    fig, axes = plt.subplots(1, 3, figsize=(16, 5.2))
    for ax, (name, r) in zip(axes, res.items()):
        a = np.array(r["angles"])
        for tech, colour in zip(TECH[:4], COLOURS[:4]):
            ax.plot(a, np.abs(r["err"][tech]), "-", color=colour, lw=2, marker="o", ms=3, label=tech)
        ax.set_xscale("symlog", linthresh=0.1, linscale=0.5)
        ax.set_yscale("log")
        how = "about its axis (z)" if name == "torus" else "about a generic axis"
        ax.set_title(f"{name}, turned {how}\nh = {r['h']} m, 1.5 m away")
        ax.set_xlabel("rotation from the grid-aligned pose (degrees)")
        _clean(ax)
    axes[0].set_ylabel("|phase error| (rad)")
    h_, l_ = axes[0].get_legend_handles_labels()
    fig.legend(h_, l_, loc="upper center", ncol=4, frameon=False)
    fig.text(0.01, 0.01, "Static body, re-voxelised at each rotation on the same lattice. (e) is exact "
             "at every angle; (f) and (g) do not depend on the lattice. The torus's true signal does not "
             "change with the angle, so its curves are pure lattice effects.", fontsize=8.5, color="#52514e")
    fig.subplots_adjust(left=0.05, right=0.99, top=0.82, bottom=0.14, wspace=0.18)
    fig.savefig(L.HERE / "orientation.png", dpi=110)


# ================================ TRANSLATION ================================

Y0 = 2.9  # torus centre's offset from the tower: R + r + 1.5 m


def translation() -> None:
    h, phases = 0.1, 16
    mesh = L.torus_mesh()
    tower = np.array([[0.0, 0.0, 0.0], [0.0, 0.0, L.BASELINE]])
    xs = np.arange(-6.0, 6.0 + 1e-9, h / phases)
    k = np.rint((xs + 6.0) / (h / phases)).astype(int)
    body = L.body_moments(mesh.tris)
    o_world = h * L.GENERIC

    def points(x: np.ndarray) -> np.ndarray:
        c = np.stack([x, np.full_like(x, Y0), np.zeros_like(x)], axis=1)
        return np.concatenate([tower[0] - c, tower[1] - c])

    pts_all = points(xs)
    g_mesh = L.g_mesh(mesh.tris, pts_all)
    signal = L.phase(g_mesh)
    vox0 = L.voxelise(mesh, h, origin=o_world - np.array([0.0, Y0, 0.0]))
    body_frame = technique_errors(vox0, pts_all, g_mesh, with_exact=False, body=body)
    world = {t: np.zeros(len(xs)) for t in TECH[:4]}
    for p in range(phases):
        sel = np.nonzero(k % phases == p)[0]
        delta = float(xs[sel[0]])
        vox = L.voxelise(mesh, h, origin=o_world - np.array([delta, Y0, 0.0]))
        pts = points(xs[sel])
        e = technique_errors(vox, pts, L.g_mesh(mesh.tris, pts), with_exact=False)
        for t in TECH[:4]:
            world[t][sel] = e[t]
        print("phase", p, flush=True)
    res = {"x": xs.tolist(), "h": h, "signal": signal.tolist(),
           "body": {t: v.tolist() for t, v in body_frame.items()},
           "world": {t: v.tolist() for t, v in world.items()}}
    (L.HERE / "translation.json").write_text(json.dumps(res))
    plot_translation()


def _spectrum(y: np.ndarray, dx: float) -> tuple[np.ndarray, np.ndarray]:
    w = np.hanning(len(y))
    amp = np.abs(np.fft.rfft((y - y.mean()) * w)) * 2 / w.sum()
    return np.fft.rfftfreq(len(y), dx), amp


def plot_translation() -> None:
    _style()
    r = json.loads((L.HERE / "translation.json").read_text())
    x, h = np.array(r["x"]), r["h"]
    fig, axes = plt.subplots(3, 1, figsize=(12, 9.5), sharex=True)
    axes[0].plot(x, r["signal"], color="#0b0b0b", lw=1.5)
    axes[0].set_ylabel("phase (rad)")
    axes[0].set_title(f"Torus (R 1 m, r 0.4 m) passing a tower at 1.5 m closest approach, pitch {h} m")
    for ax, frame, label in ((axes[1], "body", "body-frame voxels (voxelised once, moved rigidly)"),
                             (axes[2], "world", "world-fixed voxels (re-voxelised on a fixed grid)")):
        for tech, colour in zip(TECH[:4], COLOURS[:4]):
            ax.plot(x, r[frame][tech], color=colour, lw=1.3, label=tech)
        ax.set_yscale("symlog", linthresh=1e-6)
        ax.set_ylabel("phase error (rad)")
        ax.set_title(label)
    for ax in axes:
        _clean(ax)
    axes[2].set_xlabel("body position along the track (m); at 1 m/s also the time (s)")
    h_, l_ = axes[1].get_legend_handles_labels()
    fig.legend(h_, l_, loc="upper center", ncol=4, frameon=False)
    fig.subplots_adjust(left=0.08, right=0.98, top=0.92, bottom=0.07, hspace=0.25)
    fig.savefig(L.HERE / "translation.png", dpi=110)

    fig, axes = plt.subplots(1, 2, figsize=(14, 5), sharey=True)
    dx = float(x[1] - x[0])
    for ax, frame in zip(axes, ("body", "world")):
        for tech, colour in zip(TECH[:4], COLOURS[:4]):
            f, a = _spectrum(np.array(r[frame][tech]), dx)
            ax.loglog(f[1:], np.maximum(a[1:], 1e-18), color=colour, lw=1.2, label=tech)
        for m in range(1, 8):
            ax.axvline(m / h, color="#c3c2b7", lw=0.8, ls=":")
        ax.set_title(f"{frame}-frame error spectrum (dotted: multiples of 1/h = {1 / h:g} per m)")
        ax.set_xlabel("spatial frequency (cycles per m; Hz at 1 m/s)")
        _clean(ax)
    axes[0].set_ylabel("amplitude (rad)")
    h_, l_ = axes[0].get_legend_handles_labels()
    fig.legend(h_, l_, loc="upper center", ncol=4, frameon=False)
    fig.subplots_adjust(left=0.07, right=0.99, top=0.86, bottom=0.12, wspace=0.05)
    fig.savefig(L.HERE / "translation-spectrum.png", dpi=110)


# ================================ SPIN ================================


def spin() -> None:
    h = 0.1
    mesh = L.torus_mesh()
    tower = np.array([[0.0, 0.0, 0.0], [0.0, 0.0, L.BASELINE]])
    centre = np.array([0.0, Y0, 0.0])
    theta = np.arange(0.0, 360.0, 0.5)
    rel = []
    for which in (0, 1):
        for t in theta:
            rel.append(L.rot_about(np.array([0.0, 0.0, 1.0]), -t) @ (tower[which] - centre))
    pts = np.array(rel)
    g_mesh = L.g_mesh(mesh.tris, pts)
    vox = L.voxelise(mesh, h, origin=h * L.GENERIC - centre)
    e = technique_errors(vox, pts, g_mesh, with_exact=False)
    res = {"theta": theta.tolist(), "h": h, "signal": L.phase(g_mesh).tolist(),
           "body": {t: v.tolist() for t, v in e.items()}}
    (L.HERE / "spin.json").write_text(json.dumps(res))  # spin_world adds the world-fixed part, then plots


NU_TORUS = 64  # segments round the torus mesh's axis (torus_mesh default)


def spin_world() -> None:
    """World-fixed voxels of the spinning torus over one period of the error. Turning the 64-segment
    mesh by 360/64 = 5.625° maps it onto itself and the lattice is unchanged, so the error repeats
    every 5.625°: its harmonics are multiples of 64 of the spin frequency. Sampled 48 times in that
    period (Nyquist: harmonic 64·24). Same procedure as `orientation` (tower of the unturned torus)."""
    h = 0.1
    mesh = L.torus_mesh()
    pts = L.tower(mesh, np.array([1.5]))
    c = 0.5 * (mesh.verts.min(axis=0) + mesh.verts.max(axis=0))
    period = 360.0 / NU_TORUS
    angles = np.arange(48) * period / 48
    errs: dict[str, list[float]] = {t: [] for t in TECH[:4]}
    for ang in angles:
        m = L.Mesh(mesh.name, (mesh.verts - c) @ L.rot_about(np.array([0.0, 0.0, 1.0]), ang).T + c, mesh.faces,
                   mesh.source)
        e = technique_errors(L.voxelise(m, h), pts, L.g_mesh(m.tris, pts), with_exact=False)
        for t in TECH[:4]:
            errs[t].append(float(e[t][0]))
    path = L.HERE / "spin.json"
    res = json.loads(path.read_text())
    res["world"] = {"theta": angles.tolist(), "period": period, "h": h, "err": errs}
    path.write_text(json.dumps(res))
    plot_spin()


def plot_spin() -> None:
    _style()
    r = json.loads((L.HERE / "spin.json").read_text())
    w = r["world"]
    th = np.array(r["theta"])
    fig, axes = plt.subplots(2, 2, figsize=(14, 8.5))
    for tech, colour in zip(TECH[:4], COLOURS[:4]):
        axes[0, 0].plot(th, r["body"][tech], color=colour, lw=1.4, label=tech)
        axes[0, 1].plot(w["theta"], w["err"][tech], color=colour, lw=1.4, marker="o", ms=2.5)
        f = np.fft.rfft(np.array(r["body"][tech]))
        amp = np.abs(f) * 2 / len(th)
        axes[1, 0].semilogy(np.arange(len(amp))[1:41], np.maximum(amp[1:41], 1e-18), "o-", color=colour,
                            ms=3, lw=1)
        amp_w = np.abs(np.fft.rfft(np.array(w["err"][tech]))) * 2 / len(w["theta"])
        harm = NU_TORUS * np.arange(len(amp_w))
        axes[1, 1].semilogy(harm[1:], np.maximum(amp_w[1:], 1e-18), "o-", color=colour, ms=3, lw=1)
    axes[0, 0].set_title(f"body-frame voxels, torus spinning about its axis, h = {r['h']} m")
    axes[0, 1].set_title(f"world-fixed voxels (re-voxelised each angle), one period 0–{w['period']:g}°")
    for ax in axes[0]:
        ax.set_yscale("symlog", linthresh=1e-6)
        ax.set_xlabel("spin angle (degrees)")
        ax.set_ylabel("phase error (rad); true signal is constant")
    for ax in axes[1]:
        ax.set_xlabel("harmonic of the spin frequency")
        ax.set_ylabel("amplitude (rad)")
    axes[1, 0].set_title("spectrum of the body-frame error (one turn)")
    axes[1, 1].set_title(f"spectrum of the world-fixed error (period {w['period']:g}°, so harmonics "
                         f"{NU_TORUS}k)")
    for ax in axes.ravel():
        _clean(ax)
    h_, l_ = axes[0, 0].get_legend_handles_labels()
    fig.legend(h_, l_, loc="upper center", ncol=4, frameon=False)
    fig.subplots_adjust(left=0.08, right=0.99, top=0.9, bottom=0.07, hspace=0.35, wspace=0.22)
    fig.savefig(L.HERE / "spin.png", dpi=110)


if __name__ == "__main__":
    if sys.argv[1] == "distance" and len(sys.argv) > 2:
        distance(sys.argv[2:])
        sys.exit(0)
    {"distance": distance, "orientation": orientation, "translation": translation, "spin": spin,
     "spinworld": spin_world,
     "plot": lambda: (plot_distance(), plot_orientation(), plot_translation(), plot_spin())}[sys.argv[1]]()
