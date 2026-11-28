"""The shapes study's representation sweep (paper Table 4, Figs. 9–10) with the real read.

    python sweep_real.py run <shape> <seed>     # data/sweep-<shape>-s<seed>.json (resumable by pitch)
    python sweep_real.py facets                 # data/facets.json: the meshes against the analytic shapes

Everything is `exact-skin-and-partial-fill/shapes/sweep.py`'s `run`, unchanged in substance: the
same meshes, voxeliser, techniques (a)–(e), (c★), (d★) and cost model. Three things differ:
- the read: every error is a pair (real, leading) at 1.5, 5 and 20 m (`realread.AxisRead`, 48
  heights per launch axis, from the same lower launch point as the study's `tower`);
- the lattice placement: seed 0 is the study's (h·GENERIC, the near-symmetric one the referee's S1
  questions); seeds 1–3 are random sub-cell offsets, rng(1000 + seed) drawn once per pitch;
- R8: at 1.5 m and size/h ≤ 64, (d)'s residual is also computed from the pieces' l = 3–5 moments,
  for the budget's method, against the closed-form residual, both read at the arms' nodes.
The kernels are the compiled ones (`realread.patch_shapes_lib`).
"""

from __future__ import annotations

import json
import sys
import time

import numpy as np

import moments as MO
import realread as R

L = R.L
sys.path.insert(0, str(R.RESEARCH / "exact-skin-and-partial-fill" / "shapes"))
import analytic as A  # noqa: E402

NS = [4, 8, 16, 32, 64, 128]
DIST = np.array([1.5, 5.0, 20.0])
TECH = ["(a) cubes", "(b) sampled fill", "(c) fill at centroid", "(d) + second moment", "(e) clipped pieces",
        "(c★) analytic fill", "(d★) analytic + second"]
NS_SERIES, NS_QUAD, NS_TRI = 8.5, 17.0, 55.0
ANALYTIC = {"sphere": A.Ball(1.0), "torus": A.Torus(1.0, 0.4)}
SHAPES = ["sphere", "torus", "teapot", "suzanne", "bunny", "spot", "dragon", "armadillo", "buddha", "bracket"]
OUT = R.HERE / "data"


def axis_clearance(mesh: L.Mesh, lowers: np.ndarray, top: float) -> list[float]:
    """Closest distance (m) from each launch axis (the lower launch point up to Δr + the arms' top)
    to the mesh."""
    out = []
    for p in lowers:
        zs = np.linspace(0.0, R.INS.separation + top, 41)
        out.append(min(L.distance_to_mesh(mesh.tris, p + np.array([0.0, 0.0, z])) for z in zs))
    return out


def run(name: str, seed: int) -> None:
    R.below_normal()
    R.patch_shapes_lib()
    OUT.mkdir(exist_ok = True)
    mesh = L.mesh_by_name(name)
    size = float((mesh.verts.max(axis = 0) - mesh.verts.min(axis = 0)).max())
    read = R.AxisRead(48)
    lowers = L.tower(mesh, DIST)[: len(DIST)]
    pts = read.points(lowers)
    ph = R.both(read)
    g_mesh = L.g_mesh(mesh.tris, pts)
    v_mesh = float(L.sb.volume_centroid(mesh.tris, np.zeros(3))[0])
    shape = ANALYTIC.get(name)
    g_an = shape.g_ref(pts) if shape is not None else None
    path = OUT / f"sweep-{name}-s{seed}.json"
    if path.exists():
        out = json.loads(path.read_text(encoding = "utf-8"))
    else:
        body = L.body_moments(mesh.tris)
        out = {"shape": name, "seed": seed, "size": size, "triangles": len(mesh.faces), "distances": DIST.tolist(),
               "reads": ["real 1.5", "real 5", "real 20", "leading 1.5", "leading 5", "leading 20"],
               "lowers": lowers.tolist(), "clearance": axis_clearance(mesh, lowers, read.top),
               "signal": ph(g_mesh).tolist(), "volume_mesh": v_mesh,
               "multipole_err": (ph(L.g_multipole(*body, pts)) - ph(g_mesh)).tolist(),
               "quadrupole_err": (ph(L.g_multipole(body[0], body[1], body[2], None, pts)) - ph(g_mesh)).tolist(),
               "point_err": (ph(L.g_multipole(body[0], body[1], None, None, pts)) - ph(g_mesh)).tolist(),
               "mesh_cost_ns": len(mesh.faces) * NS_TRI, "rows": []}
        if shape is not None:
            out["signal_analytic"] = ph(g_an).tolist()
            out["facet_err"] = (ph(g_mesh) - ph(g_an)).tolist()
    rows = out["rows"]
    rng = np.random.default_rng(1000 + seed)
    pr = R.PassRead(np.zeros(1), lowers[:1], np.zeros(3))  # R8: the static read at the arms' nodes, 1.5 m
    for n_across in NS:
        offset = L.GENERIC if seed == 0 else rng.random(3)  # drawn for every pitch, so resuming is stable
        if any(r["n"] == n_across for r in rows):
            continue
        h = size / n_across
        t0 = time.perf_counter()
        vox = L.voxelise(mesh, h, origin = h * offset)
        t_vox = time.perf_counter() - t0
        n_in, n_b = len(vox.interior), len(vox.frac)
        cen = vox.boundary_centres
        g_p = L.g_mesh(vox.piece_tris, pts)
        g_in = L.g_cubes_fast(vox.interior_centres, h, np.ones(n_in), pts)
        g_ctr = L.g_cubes(cen, h, vox.ctr_in, pts)
        g_ss = L.g_cubes(cen, h, vox.ss, pts)
        mass_b = vox.frac * h**3
        g_pt = L.g_points(mass_b, vox.centroid, pts)
        g_q = g_pt + L.g_quadrupole(vox.second, vox.centroid, pts)
        s4 = v_mesh / (h**3 * (n_in + vox.ss.sum()))
        err = {
            TECH[0]: ph(g_ctr) - ph(g_p),
            TECH[1]: (s4 - 1) * (ph(g_mesh) - ph(g_p)) + s4 * ph(g_ss) - ph(g_p),
            TECH[2]: ph(g_pt) - ph(g_p),
            TECH[3]: ph(g_q) - ph(g_p),
            TECH[4]: ph(g_in + g_p) - ph(g_mesh),
        }
        mass = {TECH[0]: (n_in + vox.ctr_in.sum()) * h**3 / v_mesh - 1, TECH[1]: (n_in + vox.ss.sum()) * h**3 / v_mesh - 1,
                TECH[2]: (n_in + vox.frac.sum()) * h**3 / v_mesh - 1}
        cost = {TECH[0]: (n_in + vox.ctr_in.sum()) * NS_SERIES, TECH[1]: (n_in + n_b) * NS_SERIES,
                TECH[2]: (n_in + n_b) * NS_SERIES, TECH[3]: n_in * NS_SERIES + n_b * NS_QUAD,
                TECH[4]: n_in * NS_SERIES + len(vox.piece_tris) * NS_TRI}
        row: dict[str, object] = {"n": n_across, "h": h, "offset": list(map(float, offset)), "interior": n_in,
                                  "boundary": n_b, "piece_tris": len(vox.piece_tris), "seconds_clip": t_vox,
                                  "volume_check": vox.stats["volume_rel_error"]}
        if n_across <= 64:
            t1 = time.perf_counter()
            raw = MO.piece_moments(vox.piece_tris, vox.piece_of, vox.centroid)
            mp = R.mpole_field(vox.centroid, MO.multipole_coefficients(raw), pr.points)
            u_p = R.u_mesh(vox.piece_tris, pr.points)
            u0, _, uq, _ = R.point_field(vox.centroid, mass_b, vox.second, pr.points)
            exact_d = float(pr.phase(u_p - u0 - uq)[0])  # residual of the pieces against (d): −(d)'s error
            row["r8"] = {"closed_form": exact_d, "l3": float(pr.phase(mp[:, 0])[0]), "l4": float(pr.phase(mp[:, 1])[0]),
                         "l5": float(pr.phase(mp[:, 2])[0]), "axis_read_of_d": float(-err[TECH[3]][0]),
                         "seconds": time.perf_counter() - t1}
        if shape is not None:
            t1 = time.perf_counter()
            av = A.analytic_voxels(shape, h, h * offset)
            g_in_a = L.g_cubes_fast(av["interior"], h, np.ones(len(av["interior"])), pts)
            g_c = g_in_a + L.g_points(av["frac"] * h**3, av["centroid"], pts)
            g_d = g_c + L.g_quadrupole(av["second"], av["centroid"], pts)
            err[TECH[5]] = ph(g_c) - ph(g_an)
            err[TECH[6]] = ph(g_d) - ph(g_an)
            nb_a, ni_a = len(av["frac"]), len(av["interior"])
            cost[TECH[5]] = (ni_a + nb_a) * NS_SERIES
            cost[TECH[6]] = ni_a * NS_SERIES + nb_a * NS_QUAD
            row.update({"analytic_interior": ni_a, "analytic_boundary": nb_a, "seconds_analytic": time.perf_counter() - t1})
        row["err"] = {k: v.tolist() for k, v in err.items()}
        row["mass"] = {k: float(v) for k, v in mass.items()}
        row["cost_ns"] = {k: float(v) for k, v in cost.items()}
        row["seconds"] = time.perf_counter() - t0
        rows.append(row)
        rows.sort(key = lambda r: int(r["n"]))  # type: ignore[arg-type]
        print(name, seed, n_across, f"clip {t_vox:.0f}s all {row['seconds']:.0f}s",
              {k[:3]: f"{abs(v[0]):.1e}/{abs(v[3]):.1e}" for k, v in err.items()}, flush = True)
        path.write_text(json.dumps(out, default = R.json_safe), encoding = "utf-8")
        del vox, g_p, g_in


def facets() -> None:
    """The sphere and torus meshes against their analytic shapes at every fineness (sweep.facets)."""
    R.below_normal()
    R.patch_shapes_lib()
    read = R.AxisRead(48)
    ph = R.both(read)
    res: dict[str, list[dict[str, object]]] = {}
    for name in ("sphere", "torus"):
        shape = ANALYTIC[name]
        pts = read.points(L.tower(L.mesh_by_name(name), DIST)[: len(DIST)])
        g_an = shape.g_ref(pts)
        rows = []
        variants = ([(lev,) for lev in range(1, 7)] if name == "sphere"
                    else [(16, 8), (32, 16), (64, 32), (128, 64), (256, 128), (512, 256)])
        for var in variants:
            if name == "sphere":
                v, f = L.sb.icosphere(var[0])
                faces = np.array(f, dtype = np.int64)
                nrm = np.cross(v[faces[:, 1]] - v[faces[:, 0]], v[faces[:, 2]] - v[faces[:, 0]])
                flip = np.einsum("nk,nk->n", nrm, v[faces].mean(axis = 1)) < 0
                faces[flip] = faces[flip][:, ::-1]
                m = L.Mesh("sphere", v, faces, "")
            else:
                m = L.torus_mesh(1.0, 0.4, var[0], var[1])
            rows.append({"triangles": len(m.faces), "phase": (ph(L.g_mesh(m.tris, pts)) - ph(g_an)).tolist()})
            print(name, rows[-1], flush = True)
        res[name] = rows
    OUT.mkdir(exist_ok = True)
    (OUT / "facets.json").write_text(json.dumps(res, indent = 1), encoding = "utf-8")


if __name__ == "__main__":
    if sys.argv[1] == "run":
        if len(sys.argv) > 4:  # a trial: only pitches up to this size/h (a later full run resumes)
            NS[:] = [n for n in NS if n <= int(sys.argv[4])]
        for nm in sys.argv[2].split(","):
            run(nm, int(sys.argv[3]))
    elif sys.argv[1] == "facets":
        facets()
