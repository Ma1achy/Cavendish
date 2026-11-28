"""The paper's moving torus (Fig. 12, §5 "Orientation and motion") with the real phase (C6, C7).

    python torus.py body            # torus-body.json: body-frame voxels, (c) and (d) errors over the pass
    python torus.py world           # torus-world.json: world-fixed voxels, the lines at k/h
    python torus.py plot            # torus.pdf/.png

Geometry as the paper's (`exact-skin-and-partial-fill/shapes/regimes.py translation`): the torus
(R 1 m, r 0.4 m, 4096 triangles, axis vertical, centre level with the lower launch point) passes the
tower along x with its centre at y = 2.9 m (1.5 m closest approach of the surface), pitch h = 0.1 m,
lattice origin h·GENERIC. Techniques: (c) each boundary cell's exact fill as a point at the piece's
centroid; (d) (c) plus the piece's second moment; error = technique − exact clipped pieces (interior
cubes cancel). Phase reads as in flight.py: flight (moving), frozen (at the π pulse), leading (paper).

World-fixed voxels: the lattice phase δ = x mod h of the body sweeps through a cell every h/v
seconds. The error field is sampled at 16 phases (as the paper did) and trigonometrically
interpolated in δ, E(q, δ) = Σ_k A_k(q) e^{2πikδ/h}; harmonic k is a line at f = k v/h. Its flight
read is integrated with e^{2πik x(τ)/h} inside the flight (Gauss order 10, panels ≤ 0.01 s). For the
lines only the (c) − (d) part is computed (the cells' quadrupoles, cheap); the paper's (d) line is
1/36 of its (c) line, so this is (c)'s line to about 3 %.
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

H = 0.1
Y0 = 2.9
PHASES = 16
TOWER = np.zeros(3)
XS_BODY = np.round(np.arange(-4.0, 4.0 + 1e-9, 0.25), 10)
# (speed m/s, harmonics, track positions of the π pulse m): the 1 m/s case needs panels of 0.01 s
# to follow the 10 Hz line inside the flight, so it is read at k = 1 near closest approach only.
WORLD_CASES = ((0.01, (1, 2, 3), np.round(np.arange(-3.0, 3.0 + 1e-9, 0.5), 10)),
               (0.1, (1, 2, 3), np.round(np.arange(-3.0, 3.0 + 1e-9, 0.5), 10)),
               (1.0, (1,), np.array([-1.0, -0.5, 0.0, 0.5, 1.0])))
HARMONICS = (1, 2, 3)


def _u_points(mass: np.ndarray, pos: np.ndarray, q: np.ndarray) -> np.ndarray:
    """Σ m/|q − c|, kg/m per unit ρ (U > 0)."""
    out = np.zeros(len(q))
    step = max(1, 2_000_000 // max(len(q), 1))
    for s in range(0, len(mass), step):
        r = np.linalg.norm(q[:, None, :] - pos[None, s:s + step], axis = 2)
        out += (mass[None, s:s + step] / r).sum(axis = 1)
    return out


def _u_quadrupole(second: np.ndarray, pos: np.ndarray, q: np.ndarray) -> np.ndarray:
    """Σ ½(3 rᵀIr − r² tr I)/r⁵ (the potential whose gradient is shapes_lib.g_quadrupole)."""
    out = np.zeros(len(q))
    step = max(1, 1_000_000 // max(len(q), 1))
    for s in range(0, len(pos), step):
        big_i = second[s:s + step]
        r = q[:, None, :] - pos[None, s:s + step]
        r2 = np.einsum("pci,pci->pc", r, r)
        rir = np.einsum("pci,cij,pcj->pc", r, big_i, r)
        tr = np.trace(big_i, axis1 = 1, axis2 = 2)
        out += (0.5 * (3 * rir - r2 * tr[None, :]) / r2**2.5).sum(axis = 1)
    return out


def _u_mesh(tris: np.ndarray, q: np.ndarray) -> np.ndarray:
    u, _ = L.sb.polyhedron_field(tris, np.ones((len(tris), 1)), q, chunk = max(8, 300_000 // len(q)), stable = True)
    return u[0]


# ======================================= BODY FRAME =======================================


def body(speed: float = 1.0) -> None:
    t0 = time.perf_counter()
    mesh = L.torus_mesh()
    vox = L.voxelise(mesh, H, origin = H * L.GENERIC - np.array([0.0, Y0, 0.0]))
    mass = vox.frac * H**3
    path = F.straight(np.array([0.0, Y0, 0.0]), np.array([speed, 0.0, 0.0]))  # centre at x = v t
    ins = replace(F.AION10, order = 6, panel = 0.1)

    # Each read is linear in the source, so each part is read once and the techniques are sums:
    # (c) = points − pieces, (d) = (c) + quadrupoles.
    parts = {
        "mesh": (lambda q: _u_mesh(mesh.tris, q), lambda q: L.g_mesh(mesh.tris, q)[:, 2]),
        "pieces": (lambda q: _u_mesh(vox.piece_tris, q), lambda q: L.g_mesh(vox.piece_tris, q)[:, 2]),
        "points": (lambda q: _u_points(mass, vox.centroid, q), lambda q: L.g_points(mass, vox.centroid, q)[:, 2]),
        "quad": (lambda q: _u_quadrupole(vox.second, vox.centroid, q),
                 lambda q: L.g_quadrupole(vox.second, vox.centroid, q)[:, 2]),
    }
    times = XS_BODY / speed
    res: dict[str, object] = {"x": XS_BODY.tolist(), "speed": speed, "h": H, "cells": len(mass),
                              "piece_triangles": len(vox.piece_tris)}
    reads: dict[str, dict[str, np.ndarray]] = {}
    for name, (u, g) in parts.items():
        def pot(pts: np.ndarray, ts: np.ndarray, u=u) -> np.ndarray:
            return -L.RHO * u(pts - path(ts))

        def fld(pts: np.ndarray, ts: np.ndarray, g=g) -> np.ndarray:
            return L.RHO * g(pts - path(ts))
        reads[name] = {
            "flight": np.array([F.flight_phase(pot, ins, TOWER, t - ins.half) for t in times]),
            "frozen": np.array([F.frozen_phase(pot, ins, TOWER, t - ins.half) for t in times]),
            "leading": np.array([F.leading_phase(fld, ins, TOWER, t) for t in times]),
        }
        print(f"{name} read ({time.perf_counter() - t0:.0f} s)", flush = True)
    for kind, combo in (("signal", {"mesh": 1}), ("c", {"points": 1, "pieces": -1}),
                        ("d", {"points": 1, "pieces": -1, "quad": 1})):
        res[kind] = {r: sum(s * reads[p][r] for p, s in combo.items()).tolist() for r in ("flight", "frozen", "leading")}
        print(f"{kind}: max |flight| {np.abs(res[kind]['flight']).max():.3e}, |frozen| "
              f"{np.abs(res[kind]['frozen']).max():.3e}, |leading| {np.abs(res[kind]['leading']).max():.3e}")
    (HERE / "torus-body.json").write_text(json.dumps(res))


# ======================================= WORLD FRAME =======================================


def world() -> None:
    t0 = time.perf_counter()
    mesh = L.torus_mesh()
    o_world = H * L.GENERIC
    voxes = []
    for p in range(PHASES):
        delta = p * H / PHASES
        v = L.voxelise(mesh, H, origin = o_world - np.array([delta, Y0, 0.0]))
        voxes.append((v.second, v.centroid))
    print(f"16 voxelisations in {time.perf_counter() - t0:.0f} s", flush = True)

    def a_k(q: np.ndarray) -> np.ndarray:
        """A_k(q) for k = 0..15 (FFT over the 16 phases) of the (d) − (c) potential, per unit ρ."""
        e = np.stack([_u_quadrupole(sec, cen, q) for sec, cen in voxes])  # [16, N]
        return np.fft.fft(e, axis = 0) / PHASES

    def g_k(q: np.ndarray) -> np.ndarray:
        e = np.stack([L.g_quadrupole(sec, cen, q)[:, 2] for sec, cen in voxes])
        return np.fft.fft(e, axis = 0) / PHASES

    res: dict[str, object] = {"h": H, "phases": PHASES, "speeds": {}}
    coarse = replace(F.AION10, order = 6, panel = 0.1)
    tau_c, w_c, zl_c, zu_c = F.nodes(coarse)
    n_c = len(tau_c)
    heights_c = np.concatenate([zu_c, zl_c, coarse.separation + zu_c, coarse.separation + zl_c])
    for speed, harmonics, xs in WORLD_CASES:
        ins = replace(F.AION10, order = 10, panel = min(0.1, 0.1 * H / speed / max(harmonics)))
        tau, w, z_lo, z_up = F.nodes(ins)
        n = len(tau)
        flight = {k: [] for k in harmonics}
        lead = {k: [] for k in harmonics}
        frozen = {k: [] for k in harmonics}
        for x_mid in xs:
            t_mid = x_mid / speed
            launch = t_mid - ins.half
            x_nodes = speed * (launch + tau)
            heights = np.concatenate([z_up, z_lo, ins.separation + z_up, ins.separation + z_lo])
            body = np.stack([np.tile(x_nodes, 4), np.full(4 * n, Y0), np.zeros(4 * n)], axis = 1)
            q = TOWER[None, :] + np.stack([np.zeros(4 * n), np.zeros(4 * n), heights], axis = 1) - body
            a = a_k(q).reshape(PHASES, 4, n)
            diff = (a[:, 0] - a[:, 1]) - (a[:, 2] - a[:, 3])  # [16, n]
            # frozen: body at x_mid for every node (a static source: the engine's coarse rule suffices)
            body_f = np.array([x_mid, Y0, 0.0])
            q_f = np.stack([np.zeros(4 * n_c), np.zeros(4 * n_c), heights_c], axis = 1) - body_f
            a_f = a_k(q_f).reshape(PHASES, 4, n_c)
            diff_f = (a_f[:, 0] - a_f[:, 1]) - (a_f[:, 2] - a_f[:, 3])
            pts_lead = np.array([[0.0, 0.0, 0.0], [0.0, 0.0, ins.separation]]) - body_f
            gk = g_k(pts_lead)
            for k in harmonics:
                theta = 2 * np.pi * k * x_nodes / H
                # V/G = −ρU, ×2 for the ±k pair, as the amplitude of a real line.
                flight[k].append(float(2 * abs(ins.scale * np.sum(w * diff[k] * np.exp(1j * theta)) * L.RHO)))
                frozen[k].append(float(2 * abs(ins.scale * np.sum(w_c * diff_f[k]) * L.RHO)))
                lead[k].append(float(2 * abs(ins.k_eff * ins.half**2 * F.BIG_G * L.RHO * (gk[k, 1] - gk[k, 0]))))
            print(f"v={speed} x={x_mid:+.2f}: k=1 flight {flight[1][-1]:.2e} frozen {frozen[1][-1]:.2e} "
                  f"leading {lead[1][-1]:.2e} ({time.perf_counter() - t0:.0f} s)", flush = True)
        res["speeds"][f"{speed:g}"] = {"x": xs.tolist(), "nodes": n,
                                       "flight": {str(k): v for k, v in flight.items()},
                                       "frozen": {str(k): v for k, v in frozen.items()},
                                       "leading": {str(k): v for k, v in lead.items()}}
    (HERE / "torus-world.json").write_text(json.dumps(res))


# ======================================= PLOT =======================================


def plot() -> None:
    S.apply()
    fig, (a, b) = plt.subplots(1, 2, figsize = (6.8, 2.6))
    if (HERE / "torus-body.json").exists():
        r = json.loads((HERE / "torus-body.json").read_text(encoding = "utf-8"))
        x = np.array(r["x"])
        for kind, colour in (("c", S.GREEN), ("d", S.VERMILION)):
            for read, ls in (("leading", ":"), ("frozen", "--"), ("flight", "-")):
                a.semilogy(x, np.maximum(np.abs(r[kind][read]), 1e-14), color = colour, ls = ls, lw = 1.0,
                           label = f"({kind}) {read}")
        a.set_xlabel("torus centre along the track (m), 1 m/s")
        a.set_ylabel("|phase error| (rad)")
        a.set_title("(a) body-frame voxels, h = 0.1 m", fontsize = 8)
        a.legend(fontsize = 6, ncol = 2)
    if (HERE / "torus-world.json").exists():
        w = json.loads((HERE / "torus-world.json").read_text(encoding = "utf-8"))
        for j, (speed, r) in enumerate(w["speeds"].items()):
            for k in [int(x) for x in r["flight"]]:
                f = k * float(speed) / H
                fl, le = max(r["flight"][str(k)]), max(r["leading"][str(k)])
                b.loglog([f], [fl], marker = S.MARKERS[j], color = S.BLUE, ls = "none",
                         label = f"flight, {speed} m/s" if k == 1 else None)
                b.loglog([f], [le], marker = S.MARKERS[j], color = S.ORANGE, ls = "none", mfc = "none",
                         label = f"leading, {speed} m/s" if k == 1 else None)
        ff = np.geomspace(0.05, 40, 400)
        lead1 = max(w["speeds"]["1"]["leading"]["1"])
        b.loglog(ff, lead1 * np.sinc(ff * 0.73) ** 2, color = S.INK2, lw = 0.7, ls = ":",
                 label = r"leading$\times$sinc$^2$ (k = 1, 1 m/s)")
        b.set_xlabel("line frequency k v/h (Hz)")
        b.set_ylabel("line amplitude, envelope peak (rad)")
        b.set_title("(b) world-fixed voxels: (c) − (d) lines", fontsize = 8)
        b.legend(fontsize = 6)
    fig.tight_layout()
    fig.savefig(HERE / "torus.pdf")
    fig.savefig(HERE / "torus.png", dpi = 200)


if __name__ == "__main__":
    {"body": body, "world": world, "plot": plot}[sys.argv[1]]()
