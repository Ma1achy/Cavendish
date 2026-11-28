"""What the boundary representation does to inference, next to the physical uncertainties.

    python budget.py truth   cube | bunny | bracket | sphere   # truth-<shape>.json: signal, Jacobian, physical rows
    python budget.py run     cube | bunny | bracket  <seed>    # run-<shape>-s<seed>.json: voxel residuals by pitch
    python budget.py checks                                    # checks.json: the validation cases (V1-V5)
    python budget.py analyse                                   # budget.json, tables.md, figures

The scene (single-instant phase, as the shapes study; see the note §1 for what that leaves out):

    body (1 m, rho = 2700 kg/m^3, its own frame, voxels fixed in it) moves along +x at v = 1 m/s,
    bounding-box centre 1 m below the lower cloud (level with it, a point mass's M, range and speed
    are exactly degenerate for one tower: the lower cloud then sees no g_z, and the upper cloud's
    M a^-3 g(vt/a) fixes only M/a^3 and v/a); the tower's lower cloud at the world origin, the
    upper one 5 m above; closest approach at t = 0 puts the lower cloud d = 1.5, 5 or 20 m from the
    body's surface; shots every 2 s for t = -60 ... 60 s (61 shots), one at closest approach.

    phase(t) = k_eff T^2 [g_z(upper) - g_z(lower)]            (shapes_lib.phase, 4.80e9 rad per m s^-2)

Inference (Kay 1993 eq. 3.31-3.33; Cutler & Vallisneri 2007 eq. 12-13):

    theta = (ln M, x0, y0, v)     J = d phase / d theta  (central differences of the exact mesh, 1 mm)
    F     = J^T J / sigma^2        sigma = 1e-4 rad/sqrt(Hz) / sqrt(2 x 2 s) = 5e-5 rad per shot
    bias  = F^-1 J^T (phi_model - phi_true) / sigma^2       (the representation's systematic error)
    |bias_i| / sigma_i <= ||phi_model - phi_true|| / sigma  (Cauchy-Schwarz; the whitened residual norm)

Every residual is against the exact polyhedron (the mesh): interior cubes are common to every
technique, so a technique's residual is a sum over boundary cells against their exact clipped pieces,
as in the shapes study (`sweep.py`), whose voxeliser and kernels this reuses unchanged.
"""

from __future__ import annotations

import ctypes
import json
import sys
import time
from pathlib import Path

import numpy as np
import numpy.typing as npt

HERE = Path(__file__).resolve().parent
SHAPES = HERE.parent / "exact-skin-and-partial-fill" / "shapes"
sys.path.insert(0, str(SHAPES))
import analytic as A  # noqa: E402
import shapes_lib as L  # noqa: E402

F64 = npt.NDArray[np.float64]

# ======================================= SCENE =======================================

NS = [4, 8, 16, 32, 64, 128]            # cells across the body's largest extent (1 m)
DISTANCES = [1.5, 5.0, 20.0]            # m, lower cloud to the body's surface at closest approach
SPEED = 1.0                             # m/s along +x
CYCLE = 2.0                             # s between shots (plan/VALIDATION-AND-RESEARCH.md §1)
TIMES = np.arange(-60.0, 60.0 + 1e-9, CYCLE)
ASD = 1e-4                              # rad/sqrt(Hz), AION-10 goal (Badurina et al. 2020, Table 1)
SIGMA = ASD / np.sqrt(2.0 * CYCLE)      # rad per shot, white noise sampled every CYCLE s: 5e-5
Z_BODY = -1.0                           # m, body's bounding-box centre relative to the lower cloud
STEP = 1e-3                             # m, central-difference step for the Jacobian
TECH = ["(a) cubes", "(b) sampled fill", "(c) fill + centroid", "(d) + second moment"]
PARAMS = ["ln M", "x0", "y0", "v"]


def below_normal() -> None:
    """Lower this process to below-normal priority (Windows), so the desktop stays usable."""
    if sys.platform == "win32":
        k32 = ctypes.windll.kernel32  # type: ignore[attr-defined]
        k32.SetPriorityClass(k32.GetCurrentProcess(), 0x4000)


def cube_mesh() -> L.Mesh:
    """A 1 m cube centred on the origin, faces on the lattice's axes (the paper's `cube_aligned`)."""
    tris = L.sb.cube_tris(np.zeros((1, 3)), 1.0)
    return L.Mesh("cube", tris.reshape(-1, 3), np.arange(len(tris) * 3).reshape(-1, 3), "1 m cube")


def mesh_for(name: str) -> L.Mesh:
    if name == "cube":
        return cube_mesh()
    return L.mesh_by_name(name)


def track(mesh: L.Mesh, d: float) -> tuple[float, F64]:
    """Offset y0 of the body's bounding-box centre from the tower (the tower on the body's -y side)
    so that the lower cloud is `d` from the surface at t = 0, and the body-frame lower-cloud points
    for every shot, (len(TIMES), 3)."""
    v = mesh.verts
    c = 0.5 * (v.min(axis = 0) + v.max(axis = 0))
    lo, hi = float(c[1] - v[:, 1].min()), float(c[1] - v[:, 1].min()) + d + 2.0

    def gap(y0: float) -> float:
        return L.distance_to_mesh(mesh.tris, c - np.array([0.0, y0, Z_BODY])) - d

    assert gap(lo) < 0 < gap(hi), (gap(lo), gap(hi))
    for _ in range(60):
        mid = 0.5 * (lo + hi)
        lo, hi = (mid, hi) if gap(mid) < 0 else (lo, mid)
    y0 = 0.5 * (lo + hi)
    return y0, track_from(mesh, y0)


def tower_points(lowers: list[F64]) -> F64:
    """[all lower clouds; all upper clouds] for `shapes_lib.phase`, distances stacked in order."""
    lo = np.concatenate(lowers)
    return np.concatenate([lo, lo + np.array([0.0, 0.0, L.BASELINE])])


def series(g: F64) -> F64:
    """Phase (rad) from g at `tower_points`, as (distances, shots)."""
    return L.phase(g).reshape(len(DISTANCES), len(TIMES))


# ======================================= TRUTH =======================================


def truth(name: str) -> None:
    """The exact polyhedron's pass at each distance, its Jacobian, and the physical rows (part 2)."""
    below_normal()
    t0 = time.perf_counter()
    if name == "sphere":
        return _sphere_facet()
    mesh = mesh_for(name)
    tracks = [track(mesh, d) for d in DISTANCES]
    pts = tower_points([lw for _, lw in tracks])
    vol, cen = L.sb.volume_centroid(mesh.tris, np.zeros(3))
    phi = series(L.g_mesh(mesh.tris, pts))
    # Moving the body by +s along axis k moves the points by -s in its frame.
    grad = []
    for k in range(3):
        e = np.zeros(3)
        e[k] = STEP
        plus, minus = series(L.g_mesh(mesh.tris, pts - e)), series(L.g_mesh(mesh.tris, pts + e))
        grad.append((plus - minus) / (2 * STEP))
    out: dict[str, object] = {
        "shape": name, "triangles": len(mesh.faces), "volume_m3": vol, "mass_kg": vol * L.RHO,
        "centroid": cen.tolist(), "y0": [y for y, _ in tracks], "t": TIMES.tolist(), "phi": phi.tolist(),
        "dphi_dx": grad[0].tolist(), "dphi_dy": grad[1].tolist(), "dphi_dz": grad[2].tolist(),
    }
    phys: dict[str, list[list[float]]] = {}
    phys["density 1%"] = (0.01 * phi).tolist()
    for s, label in ((1e-3, "1 mm"), (1e-2, "1 cm")):
        for k, ax in enumerate("xyz"):
            phys[f"position {label} {ax}"] = (s * grad[k]).tolist()
    # Exact (non-linear) shifts: the check that the linear rows hold, and the 30 cm control arm.
    for s in (1e-2, 0.3):
        e = np.array([s, 0.0, 0.0])
        phys[f"exact shift {s:g} m x"] = (series(L.g_mesh(mesh.tris, pts - e)) - phi).tolist()
    for axis, label in (((0, 0, 1), "z"), ((1, 0, 0), "x")):
        rot = L.rot_about(np.array(axis, dtype = float), 1.0)
        turned = (mesh.tris - cen) @ rot.T + cen
        phys[f"orientation 1 deg about {label}"] = (series(L.g_mesh(turned, pts)) - phi).tolist()
    phys.update(_inhomogeneity(mesh, pts, cen))
    out["physical"] = phys
    out["seconds"] = time.perf_counter() - t0
    (HERE / f"truth-{name}.json").write_text(json.dumps(out), encoding = "utf-8")
    print(name, f"{out['seconds']:.0f} s", "peak", np.abs(phi).max(axis = 1), flush = True)


def _cells(mesh: L.Mesh, n: int) -> tuple[L.Voxels, float]:
    size = float((mesh.verts.max(axis = 0) - mesh.verts.min(axis = 0)).max())
    return L.voxelise(mesh, size / n), size / n


def _inhomogeneity(mesh: L.Mesh, pts: F64, cen: F64) -> dict[str, list[list[float]]]:
    """A region of 10% of the volume made 10% denser: its extra field, from cut-cell moments at
    64 cells across (their error there is ~1e-7 of the region's own field, far below the row)."""
    vox, h = _cells(mesh, 64)
    p_in, p_b = vox.interior_centres, vox.centroid
    m_in, m_b = np.full(len(p_in), h**3), vox.frac * h**3
    pos, mass = np.concatenate([p_in, p_b]), np.concatenate([m_in, m_b])
    total = mass.sum()
    out: dict[str, list[list[float]]] = {}
    # The tower is on the body's -y side, the track along +x.
    keys = {"near side (min y)": pos[:, 1], "far side (max y)": -pos[:, 1], "leading end (max x)": -pos[:, 0],
            "core (nearest centroid)": np.linalg.norm(pos - cen, axis = 1)}
    for label, key in keys.items():
        order = np.argsort(key, kind = "stable")
        take = order[: int(np.searchsorted(np.cumsum(mass[order]), 0.1 * total)) + 1]
        sel_in = take[take < len(p_in)]
        sel_b = take[take >= len(p_in)] - len(p_in)
        g = L.g_cubes_fast(p_in[sel_in], h, np.ones(len(sel_in)), pts)
        g += L.g_points(m_b[sel_b], p_b[sel_b], pts) + L.g_quadrupole(vox.second[sel_b], p_b[sel_b], pts)
        out[f"inhomogeneity 10%/10% {label}"] = (0.1 * series(g)).tolist()
    return out


def _sphere_facet() -> None:
    """The facet error of the 5120-triangle icosphere (the shapes study's sphere) against the
    analytic unit sphere, passing at each distance: a mesh-fidelity row with its own Jacobian."""
    mesh, ball = L.mesh_by_name("sphere"), A.Ball(1.0)
    lowers = []
    for d in DISTANCES:
        y0 = float(np.sqrt((1.0 + d) ** 2 - Z_BODY**2))
        lowers.append(-np.stack([SPEED * TIMES, np.full_like(TIMES, y0), np.full_like(TIMES, Z_BODY)], axis = 1))
    pts = tower_points(lowers)
    exact = series(ball.g_ref(pts))
    grad = []
    for k in range(3):
        e = np.zeros(3)
        e[k] = STEP
        grad.append((series(ball.g_ref(pts - e)) - series(ball.g_ref(pts + e))) / (2 * STEP))
    out = {"shape": "sphere", "triangles": len(mesh.faces), "volume_m3": ball.volume,
           "mass_kg": ball.volume * L.RHO, "y0": [float(np.sqrt((1.0 + d) ** 2 - Z_BODY**2)) for d in DISTANCES], "t": TIMES.tolist(),
           "phi": exact.tolist(), "dphi_dx": grad[0].tolist(), "dphi_dy": grad[1].tolist(),
           "dphi_dz": grad[2].tolist(),
           "physical": {"facet: 5120-triangle icosphere": (series(L.g_mesh(mesh.tris, pts)) - exact).tolist()}}
    (HERE / "truth-sphere.json").write_text(json.dumps(out), encoding = "utf-8")
    print("sphere facet", np.abs(np.array(out["physical"]["facet: 5120-triangle icosphere"])).max(axis = 1))


# ======================================= VOXELS =======================================


def run(name: str, seed: int) -> None:
    """Every technique's residual series against the mesh, at each pitch, one random lattice placement."""
    below_normal()
    mesh = mesh_for(name)
    tr = json.loads((HERE / f"truth-{name}.json").read_text(encoding = "utf-8"))
    pts = tower_points([track_from(mesh, y0) for y0 in tr["y0"]])
    phi_mesh = np.array(tr["phi"])
    size = float((mesh.verts.max(axis = 0) - mesh.verts.min(axis = 0)).max())
    rng = np.random.default_rng(1000 + seed)
    path = HERE / f"run-{name}-s{seed}.json"
    rows: list[dict[str, object]] = json.loads(path.read_text())["rows"] if path.exists() else []
    for n in NS:
        offset = rng.random(3)  # drawn for every n, so a resumed run places the lattice the same way
        if any(r["n"] == n for r in rows):
            continue
        h = size / n
        t0 = time.perf_counter()
        vox = L.voxelise(mesh, h, origin = h * offset)
        t_vox = time.perf_counter() - t0
        cen = vox.boundary_centres
        p_piece = series(L.g_mesh(vox.piece_tris, pts))
        p_ctr = series(L.g_cubes_fast(cen, h, vox.ctr_in, pts))
        p_ss = series(L.g_cubes_fast(cen, h, vox.ss, pts))
        mass_b = vox.frac * h**3
        p_pt = series(L.g_points(mass_b, vox.centroid, pts))
        p_q = p_pt + series(L.g_quadrupole(vox.second, vox.centroid, pts))
        p_in = series(L.g_cubes_fast(vox.interior_centres, h, np.ones(len(vox.interior)), pts))
        n_in = len(vox.interior)
        s4 = vox.mesh_volume / (h**3 * (n_in + vox.ss.sum()))
        resid = {
            TECH[0]: p_ctr - p_piece,
            TECH[1]: (s4 - 1) * p_in + s4 * p_ss - p_piece,
            TECH[2]: p_pt - p_piece,
            TECH[3]: p_q - p_piece,
            "(e) check: interior + pieces - mesh": p_in + p_piece - phi_mesh,
        }
        rows.append({"n": n, "h": h, "offset": offset.tolist(), "interior": n_in, "boundary": len(vox.frac),
                     "seconds": time.perf_counter() - t0, "seconds_clip": t_vox,
                     "volume_check": vox.stats["volume_rel_error"],
                     "mass_rel": {TECH[0]: (n_in + vox.ctr_in.sum()) * h**3 / vox.mesh_volume - 1,
                                  TECH[1]: 0.0, TECH[2]: (n_in + vox.frac.sum()) * h**3 / vox.mesh_volume - 1},
                     "resid": {k: v.tolist() for k, v in resid.items()}})
        rows.sort(key = lambda r: int(r["n"]))  # type: ignore[arg-type]
        path.write_text(json.dumps({"shape": name, "seed": seed, "rows": rows}), encoding = "utf-8")
        print(name, seed, n, f"{rows[-1]['seconds']:.0f} s",
              {k: f"{np.abs(v).max():.1e}" for k, v in resid.items()}, flush = True)
        del vox


def track_from(mesh: L.Mesh, y0: float) -> F64:
    v = mesh.verts
    c = 0.5 * (v.min(axis = 0) + v.max(axis = 0))
    return c[None] - np.stack([SPEED * TIMES, np.full_like(TIMES, y0), np.full_like(TIMES, Z_BODY)], axis = 1)


# ======================================= INFERENCE =======================================


def jacobian(tr: dict[str, object], i: int, with_z: bool = False) -> F64:
    """Columns d phase / d (ln M, x0, y0, v[, z0]) at distance index i, (shots, P)."""
    phi = np.array(tr["phi"])[i]
    dx, dy, dz = (np.array(tr[k])[i] for k in ("dphi_dx", "dphi_dy", "dphi_dz"))
    t = np.array(tr["t"])
    cols = [phi, dx, dy, t * dx] + ([dz] if with_z else [])
    return np.stack(cols, axis = 1)


def _svd(j: F64) -> tuple[F64, F64, F64, F64]:
    """SVD of J with its columns scaled to unit norm (the columns differ by ~10^6 in size)."""
    s = np.linalg.norm(j, axis = 0)
    u, w, vt = np.linalg.svd(j / s, full_matrices = False)
    return s, u, w, vt


def crb(j: F64, sigma: float = SIGMA) -> F64:
    """sqrt(diag(F^-1)), F = J^T J / sigma^2 (white noise), via the SVD for conditioning."""
    s, _, w, vt = _svd(j)
    return sigma * np.sqrt(((vt.T / w) ** 2).sum(axis = 1)) / s


def bias(j: F64, resid: F64) -> F64:
    """F^-1 J^T Sigma^-1 resid for white noise: the least-squares projection (sigma cancels)."""
    s, u, w, vt = _svd(j)
    return (vt.T @ ((u.T @ resid) / w)) / s


def condition(j: F64) -> float:
    """Condition number of J with unit-norm columns."""
    w = _svd(j)[2]
    return float(w[0] / w[-1])


def ratios(j: F64, resid: F64, sigma: float = SIGMA) -> tuple[F64, float]:
    """(|bias_i| / sigma_i, ||resid|| / sigma)."""
    return np.abs(bias(j, resid)) / crb(j, sigma), float(np.linalg.norm(resid) / sigma)


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "truth":
        truth(sys.argv[2])
    elif cmd == "run":
        run(sys.argv[2], int(sys.argv[3]))
    else:
        import analyse

        getattr(analyse, cmd)()
