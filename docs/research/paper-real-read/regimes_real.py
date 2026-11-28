"""The shapes study's distance regimes (Table 6), orientation numbers and exact-mesh series (Table 7)
with the real read.

    python regimes_real.py distance [shape ...]   # data/regimes-distance.json
    python regimes_real.py orientation            # data/orientation.json
    python regimes_real.py exact                  # data/exact-distance.json

Each is the shapes study's function (`regimes.py distance`, `regimes.py orientation`,
`exact.py distance`) with the same meshes, pitches, voxels and techniques; every phase is the pair
(real, leading) from `realread.AxisRead` at the study's lower launch point. Two departures:
- distance: only the study's log grid from 0.5 m (not its points within 5 cells of the surface):
  the real read's arms rise 10 m from the lower launch point, and next to the surface they would pass
  through the body. Each distance records the launch axis's clearance from the mesh.
- the kernels are the compiled ones (R2).
"""

from __future__ import annotations

import json
import sys
import time

import numpy as np

import realread as R

L = R.L
sys.path.insert(0, str(R.RESEARCH / "exact-skin-and-partial-fill" / "shapes"))
import exact as EX  # noqa: E402
import regimes as RG  # noqa: E402

OUT = R.HERE / "data"


def _clearance(mesh: L.Mesh, lowers: np.ndarray, top: float) -> list[float]:
    zs = np.linspace(0.0, R.INS.separation + top, 41)
    return [min(L.distance_to_mesh(mesh.tris, p + np.array([0.0, 0.0, z])) for z in zs) for p in lowers]


def distance(names: list[str]) -> None:
    R.below_normal()
    R.patch_shapes_lib()
    read = R.AxisRead(48)
    L.phase = R.both(read)  # technique_errors reads L.phase at call time
    path = OUT / "regimes-distance.json"
    res = json.loads(path.read_text()) if path.exists() else {}
    for name, h in RG.DIST_SHAPES.items():
        if names and name not in names:
            continue
        mesh = L.mesh_by_name(name)
        size = float((mesh.verts.max(axis = 0) - mesh.verts.min(axis = 0)).max())
        d = np.geomspace(0.5, 1000, 18)
        lowers = L.tower(mesh, d)[: len(d)]
        pts = read.points(lowers)
        t0 = time.perf_counter()
        vox = L.voxelise(mesh, h)
        g_mesh = L.g_mesh(mesh.tris, pts)
        err = RG.technique_errors(vox, pts, g_mesh, body = L.body_moments(mesh.tris))
        k = len(d)
        res[name] = {"h": h, "size": size, "d": d.tolist(), "clearance": _clearance(mesh, lowers, read.top),
                     "signal": L.phase(g_mesh)[:k].tolist(), "signal_leading": L.phase(g_mesh)[k:].tolist(),
                     "err": {t: v[:k].tolist() for t, v in err.items()},
                     "err_leading": {t: v[k:].tolist() for t, v in err.items()},
                     "boundary": len(vox.frac), "seconds": time.perf_counter() - t0}
        print(name, f"{res[name]['seconds']:.0f}s", flush = True)
        OUT.mkdir(exist_ok = True)
        path.write_text(json.dumps(res), encoding = "utf-8")


def orientation() -> None:
    """regimes.orientation: torus about its axis, box and teapot about a generic axis, h = 0.1 m, 1.5 m."""
    R.below_normal()
    R.patch_shapes_lib()
    read = R.AxisRead(48)
    L.phase = R.both(read)
    h = 0.1
    generic_axis = np.array([0.3, 0.5, 0.81])
    cases = [("torus", L.torus_mesh(), np.array([0.0, 0.0, 1.0]), RG.TORUS_SWEEP),
             ("box", RG.box_mesh(), generic_axis, RG.ANGLES_SMALL),
             ("teapot", L.teapot_mesh(), generic_axis, RG.ANGLES_SMALL)]
    res: dict[str, object] = {}
    for name, mesh, axis, angles in cases:
        pts = read.points(L.tower(mesh, np.array([1.5]))[:1])
        c = 0.5 * (mesh.verts.min(axis = 0) + mesh.verts.max(axis = 0))
        errs: dict[str, list[list[float]]] = {t: [] for t in RG.TECH[:4]}
        signal = []
        for ang in angles:
            m = L.Mesh(mesh.name, (mesh.verts - c) @ L.rot_about(axis, ang).T + c, mesh.faces, mesh.source)
            vox = L.voxelise(m, h)
            g_mesh = L.g_mesh(m.tris, pts)
            e = RG.technique_errors(vox, pts, g_mesh, with_exact = False)
            for t in errs:
                errs[t].append(e[t].tolist())  # [real, leading]
            signal.append(L.phase(g_mesh).tolist())
        res[name] = {"angles": list(angles), "axis": axis.tolist(), "signal": signal, "err": errs}
        print(name, flush = True)
    (OUT / "orientation.json").write_text(json.dumps(res), encoding = "utf-8")


def exact() -> None:
    """exact.distance: each mesh whole, centre of mass level with the lower launch at 0.5–50 m from
    the axis, against its point-mass, l ≤ 2 and l ≤ 3 series."""
    R.below_normal()
    R.patch_shapes_lib()
    read = R.AxisRead(48)
    ph = R.both(read)
    res = {}
    for name, m in EX._meshes().items():
        t0 = time.perf_counter()
        c, vol, _, m2, m3 = EX._centred(m)
        lowers = -np.stack([EX.DIST, np.zeros_like(EX.DIST), np.zeros_like(EX.DIST)], axis = 1)
        pts = read.points(lowers)
        k = len(EX.DIST)
        o = np.zeros(3)
        ex = ph(L.g_mesh(c.tris, pts))
        out = {"d": EX.DIST.tolist(), "exact": ex[:k].tolist(), "exact_leading": ex[k:].tolist(),
               "triangles": len(m.faces), "radius_m": float(np.linalg.norm(c.verts, axis = 1).max())}
        for key, args in (("point", (None, None)), ("l2", (m2, None)), ("l3", (m2, m3))):
            v = ph(L.g_multipole(vol, o, args[0], args[1], pts))
            out[key], out[f"{key}_leading"] = v[:k].tolist(), v[k:].tolist()
        out["seconds"] = time.perf_counter() - t0
        res[name] = out
        print(name, f"{out['seconds']:.0f}s", flush = True)
    (OUT / "exact-distance.json").write_text(json.dumps(res), encoding = "utf-8")


def walkpast(names: list[str]) -> None:
    """Table 7's walk past (exact.passby's scene, as the flight-phase study's walkpast.py): each mesh
    whole, centre of mass 2 m from the axis level with the lower launch, walking along y at 1.4 m/s,
    three poses; the real read with the engine's rule (Gauss order 6, panels ≤ 0.1 s), every 0.1 s
    over ±6 s. Spot, the CAD part and the bunny repeat the flight-phase study (a cross-check)."""
    R.below_normal()
    speed, offset = 1.4, 2.0
    times = np.round(np.arange(-6.0, 6.0 + 1e-9, 0.1), 10)
    poses = {"upright": np.eye(3), "turned 90° about z": L.rot_about(np.array([0, 0, 1.0]), 90.0),
             "on its side (90° about x)": L.rot_about(np.array([1.0, 0, 0]), 90.0)}
    path = OUT / "walkpast.json"
    res = json.loads(path.read_text()) if path.exists() else {}
    pulses = -np.stack([np.full_like(times, offset), speed * times, np.zeros_like(times)], axis = 1)
    pr = R.PassRead(times, pulses, np.array([0.0, -speed, 0.0]), order = 6, panel = 0.1)
    for name in names or EX.NAMES:
        t0 = time.perf_counter()
        mesh = L.mesh_by_name(name)
        vol, cen, _, _ = L.body_moments(mesh.tris)
        tris = mesh.tris - cen[None, None, :]
        out: dict[str, object] = {"t": times.tolist(), "mass_kg": L.RHO * vol, "poses": {}}
        for pose, rot in poses.items():
            t_rot = tris @ rot.T
            u = R.tri_field(t_rot, np.ones((len(t_rot), 1)), pr.points)[0][0]
            g_lead = R.tri_field(t_rot, np.ones((len(t_rot), 1)), pr.lead_points)[1][0]
            out["poses"][pose] = {"flight": pr.phase(u).tolist(), "leading": pr.leading(g_lead).tolist()}
        r = np.linalg.norm(pr.points, axis = 1)
        rl = np.linalg.norm(pr.lead_points, axis = 1)
        out["poses"]["point mass"] = {"flight": pr.phase(vol / r).tolist(),
                                      "leading": pr.leading(-vol * pr.lead_points / rl[:, None] ** 3).tolist()}
        out["seconds"] = time.perf_counter() - t0
        res[name] = out
        print(name, f"{out['seconds']:.0f}s", {p: f"{np.abs(v['flight']).max():.3f}" for p, v in out["poses"].items()},
              flush = True)
        path.write_text(json.dumps(res), encoding = "utf-8")


if __name__ == "__main__":
    OUT.mkdir(exist_ok = True)
    if sys.argv[1] == "distance":
        distance(sys.argv[2:])
    elif sys.argv[1] == "orientation":
        orientation()
    elif sys.argv[1] == "walkpast":
        walkpast(sys.argv[2:])
    else:
        exact()
