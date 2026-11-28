"""The machinery checks R1 (static read = flight integral) and R2 (compiled kernels = numpy).

    python checks.py        # checks.json

R1: `AxisRead` (g_z at Legendre heights, weights from the arms) against `flight.frozen_phase` (V at
the arms' own nodes, the C1-validated port of `cav run`), for point masses and the bunny mesh, with
the node count n = 16 ... 64 to show convergence; control arm: the leading read.
R2: the Rust kernels against skin_bench.polyhedron_field(stable=True) and shapes_lib's numpy point,
quadrupole and series forms; control arm: one vertex moved by 1e-6 m.
"""

from __future__ import annotations

import json
from dataclasses import replace

import numpy as np

import realread as R

L, FL = R.L, R.FL


def r2() -> dict[str, object]:
    rng = np.random.default_rng(7)
    mesh = L.mesh_by_name("bunny")
    vox = L.voxelise(mesh, 1 / 16)
    tris = vox.piece_tris[:3000]
    pts = rng.normal(size = (40, 3)) * 0.8 + np.array([1.6, 0.3, 0.5])
    w = rng.random((len(tris), 2))
    u_ref, g_ref = L.sb.polyhedron_field(tris, w, pts, stable = True)
    u, g = R.tri_field(tris, w, pts)
    tri_err = max(np.abs(u - u_ref).max() / np.abs(u_ref).max(), np.abs(g - g_ref).max() / np.abs(g_ref).max())
    moved = tris.copy()
    moved[0, 0, 0] += 1e-6
    u2, g2 = R.tri_field(moved, w, pts)
    control = max(np.abs(u2 - u).max() / np.abs(u).max(), np.abs(g2 - g).max() / np.abs(g).max())
    # points + quadrupole + series against the numpy forms (taken before patching)
    pos, mass, sec = vox.centroid, vox.frac * vox.h**3, vox.second
    up, gp, uq, gq = R.point_field(pos, mass, sec, pts)
    gp_ref = L.g_points(mass, pos, pts)
    gq_ref = L.g_quadrupole(sec, pos, pts)
    r = pos[None] - pts[:, None]
    rl = np.linalg.norm(r, axis = 2)
    up_ref = (mass[None] / rl).sum(axis = 1)
    pq_err = max(np.abs(gp - gp_ref).max() / np.abs(gp_ref).max(), np.abs(gq - gq_ref).max() / np.abs(gq_ref).max(),
                 np.abs(up - up_ref).max() / np.abs(up_ref).max())
    cen = vox.interior_centres
    us, gs = R.series_field(cen, np.full(len(cen), vox.h**3), vox.h, pts)
    gs_ref = L.g_series4(np.full(len(cen), vox.h**3), cen, vox.h, pts)
    # the series' U against the polyhedral cubes' U, at points >= 1 m from every cell (where the
    # read uses the series: its l = 6 remainder is ~(h/2d)^6)
    far = pts[np.linalg.norm(pts[:, None] - cen[None], axis = 2).min(axis = 1) > 1.0]
    us = R.series_field(cen, np.full(len(cen), vox.h**3), vox.h, far)[0]
    uc = L.sb.polyhedron_field(L.sb.cube_tris(cen, vox.h), np.ones((12 * len(cen), 1)), far, stable = True)[0][0]
    series_err = float(np.abs(gs - gs_ref).max() / np.abs(gs_ref).max())
    series_u = float(np.abs(us - uc).max() / np.abs(uc).max())
    out = {"tri_rel": float(tri_err), "tri_control_rel": float(control), "points_quadrupole_rel": float(pq_err),
           "series_g_rel": series_err, "series_u_vs_polyhedral_cubes_rel": series_u,
           "pass": bool(tri_err <= 1e-12 and pq_err <= 1e-12 and series_err <= 1e-12 and control > 1e-10)}
    print("R2", out, flush = True)
    return out


def r1() -> dict[str, object]:
    ins = FL.AION10
    ref_ins = replace(ins, order = 16, panel = 0.01)  # the flight integral, finer than the engine's rule
    tower = np.zeros(3)
    cases = []
    for d in (1.2, 1.5):
        for zc in (-1.0, 0.0, 2.5, 5.0, 7.5, 10.0, 11.0):
            src = np.array([d, 0.0, zc])
            q = 1.0  # m³ at rho: 2700 kg
            pot = FL.point_potential(q * R.RHO, lambda t, s = src: np.broadcast_to(s, (len(t), 3)))
            ref = FL.frozen_phase(pot, ref_ins, tower, 0.0)
            row = {"d": d, "z": zc, "flight": ref}
            for n in (16, 24, 32, 48, 64):
                read = R.AxisRead(n)
                pts = read.points(tower[None])
                r = src[None] - pts
                g = q * r / np.linalg.norm(r, axis = 1)[:, None] ** 3
                real, lead = read.phases(g)
                row[f"rel_n{n}"] = float(abs(real[0] / ref - 1))
                row["leading_rel"] = float(abs(lead[0] / ref - 1))
            cases.append(row)
    # the bunny at 1.5 m along the paper's ray
    mesh = L.mesh_by_name("bunny")
    lower = L.tower(mesh, np.array([1.5]))[:1]
    pot = lambda p, t: -R.RHO * R.u_mesh(mesh.tris, p)  # noqa: E731  (V/G)
    ref = FL.frozen_phase(pot, ref_ins, lower[0], 0.0)
    bunny = {"flight": ref}
    for n in (24, 48):
        read = R.AxisRead(n)
        g = R.tri_field(mesh.tris, np.ones((len(mesh.tris), 1)), read.points(lower))[1][0]
        real, lead = read.phases(g)
        bunny[f"rel_n{n}"] = float(abs(real[0] / ref - 1))
        bunny["leading"] = float(lead[0])
        bunny["leading_rel"] = float(abs(lead[0] / ref - 1))
    worst48 = max(max(c["rel_n48"] for c in cases), bunny["rel_n48"])
    lead_min = min(min(c["leading_rel"] for c in cases), bunny["leading_rel"])
    out = {"points": cases, "bunny": bunny, "worst_rel_n48": worst48, "control_leading_min_rel": lead_min,
           "area_check": R.AxisRead(48).area / (ins.kick * ins.half**2) - 1,
           "pass": bool(worst48 <= 1e-10 and lead_min > 0.1)}
    print("R1 worst n48", worst48, "bunny", bunny, "control", lead_min, flush = True)
    return out


if __name__ == "__main__":
    R.below_normal()
    res = {"R2": r2(), "R1": r1()}
    (R.HERE / "checks.json").write_text(json.dumps(res, indent = 1, default = R.json_safe), encoding = "utf-8")
