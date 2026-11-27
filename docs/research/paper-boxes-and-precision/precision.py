"""Question B: the polyhedral closed form and the cut-cell moment sums in single precision.

    python precision.py kernel          # B1, A4: one triangle / box, f32 variants and f64 against 40 digits
    python precision.py body <shape>    # B2-B4: a voxelised body summed in f32 variants, 0.1 m - 1 km

Everything a GPU would do is emulated with numpy float32 (IEEE single, round to nearest: numpy's f32
ufuncs are within an ulp or two, better than WGSL requires). WGSL's permitted worst case is injected
separately (`wgsl=True`): atan2 off by up to 4096 ULP, log off by up to 2^-21 absolute on [0.5, 2]
and 3 ULP elsewhere, and no log1p (WGSL 15.7.4; https://www.w3.org/TR/WGSL/#floating-point-accuracy),
as uniform random perturbations of that size.

Variants of one evaluation (per element, per field point):
    frame   'local'  offsets element - point formed in f64, then rounded (plan §3.4: tower frame)
            'body'   vertices and point both rounded to f32 in the body's frame, subtracted in f32
            'world'  the same with the world origin 1 km away (a town-scale world in f32)
    numer   'h2A'    the solid angle's numerator as h_f * 2A_f (the paper's rule)
            'triple' r1 . (r2 x r3)
    edge    'log1p'  L_e = log1p(2l/(a + b - l))
            'ratio'  L_e = log((a + b + l)/(a + b - l))
Element geometry (unit normal, 2A, edge normals, lengths) is computed in f64 and stored rounded.

Sums over elements, as a GPU reduces them (LANES threads, each summing a strided share in order, then
a pairwise tree across threads): 'seq32' one thread, plain f32; 'lane32' plain f32 lanes and tree;
'kahan' Kahan lanes, f32 tree; 'ff' float-float (two-sum) lanes and tree; 'f64acc' f32 terms, f64
accumulators. The reference is the f64 expression with every term summed exactly (math.fsum).
"""

from __future__ import annotations

import json
import math
import sys
import time

import numpy as np

import common as C

L = C.L
sb = L.sb
F32, F64 = np.float32, np.float64
LANES = 1024
WORLD = np.array([1000.0, 0.0, 0.0])
DIST = np.array([0.1, 0.3, 1.5, 5.0, 20.0, 100.0, 1000.0])
PHASE = L.PHASE_PER_DG * L.G * L.RHO  # rad per (m of g per unit G rho)
RNG = np.random.default_rng(20261127)


# ------------------------------------------------------------------ geometry and kernels


def geometry(tris: np.ndarray) -> dict[str, np.ndarray]:
    """Per triangle (f64): unit normal, twice the area, in-plane outward edge normals, lengths."""
    a, b, c = tris[:, 0], tris[:, 1], tris[:, 2]
    n = np.cross(b - a, c - a)
    a2 = np.linalg.norm(n, axis=1)
    n = n / a2[:, None]
    ne, el = [], []
    for i in range(3):
        e = tris[:, (i + 1) % 3] - tris[:, i]
        ln = np.linalg.norm(e, axis=1)
        ne.append(np.cross(e, n) / ln[:, None])
        el.append(ln)
    return {"n": n, "a2": a2, "ne": np.stack(ne, 1), "el": np.stack(el, 1)}


def offsets(tris: np.ndarray, p: np.ndarray, frame: str, dt: type) -> np.ndarray:
    if frame == "local" or dt is F64:
        return (tris - p).astype(dt)
    shift = WORLD if frame == "world" else np.zeros(3)
    return (tris + shift).astype(dt) - (p + shift).astype(dt)


def _wgsl_atan2(y: np.ndarray, x: np.ndarray) -> np.ndarray:
    v = np.arctan2(y, x)
    return (v * (1 + RNG.uniform(-1, 1, v.shape) * 4096 * 2.0**-23)).astype(v.dtype)


def _wgsl_log(u: np.ndarray) -> np.ndarray:
    v = np.log(u)
    near = (u >= 0.5) & (u <= 2.0)
    pert = np.where(near, RNG.uniform(-1, 1, v.shape) * 2.0**-21,
                    v * RNG.uniform(-1, 1, v.shape) * 3 * 2.0**-23)
    return (v + pert).astype(v.dtype)


def tri_gz(r: np.ndarray, geo: dict[str, np.ndarray], dt: type, numer: str = "h2A",
           edge: str = "log1p", wgsl: bool = False) -> np.ndarray:
    """Each triangle's share of g_z (per unit G rho, pointing at the mass) at one point, from its
    corner offsets r (N, 3, 3) in precision dt; every operation in dt."""
    n = geo["n"].astype(dt)
    l = np.sqrt((r * r).sum(axis=2))  # (N, 3)
    h = (n * r[:, 0]).sum(axis=1)
    if numer == "h2A":
        num = h * geo["a2"].astype(dt)
    else:
        num = (r[:, 0] * np.cross(r[:, 1], r[:, 2])).sum(axis=1).astype(dt)
    dot = lambda i, j: (r[:, i] * r[:, j]).sum(axis=1)  # noqa: E731
    den = l[:, 0] * l[:, 1] * l[:, 2] + l[:, 0] * dot(1, 2) + l[:, 1] * dot(2, 0) + l[:, 2] * dot(0, 1)
    om = (_wgsl_atan2 if wgsl else np.arctan2)(num, den) * dt(2)
    e = np.zeros(len(r), dtype=dt)
    el = geo["el"].astype(dt)
    ne = geo["ne"].astype(dt)
    for i in range(3):
        j = (i + 1) % 3
        ab = l[:, i] + l[:, j]
        if wgsl:  # no log1p in WGSL: log(1 + x) with WGSL's log
            lg = _wgsl_log(dt(1) + dt(2) * el[:, i] / (ab - el[:, i]))
        elif edge == "log1p":
            lg = np.log1p(dt(2) * el[:, i] / (ab - el[:, i]))
        else:
            lg = np.log((ab + el[:, i]) / (ab - el[:, i]))
        e = e + (ne[:, i] * r[:, i]).sum(axis=1) * lg
    return -n[:, 2] * (e - h * om)


def quad_gz(mass: np.ndarray, rel: np.ndarray, second: np.ndarray, dt: type) -> np.ndarray:
    """Point + quadrupole g_z per element (`shapes_lib.g_quadrupole`'s algebra), rel = position -
    point, all in dt."""
    m, r, big_i = mass.astype(dt), rel.astype(dt), second.astype(dt)
    r2 = (r * r).sum(axis=1)
    rl = np.sqrt(r2)
    tr = np.trace(big_i, axis1=1, axis2=2)
    ir = np.einsum("cij,cj->ci", big_i, r)
    rir = (r * ir).sum(axis=1)
    i5 = dt(1) / (r2 * r2 * rl)
    i7 = i5 / r2
    du = dt(0.5) * (dt(6) * ir[:, 2] * i5 - dt(2) * tr * r[:, 2] * i5
                    - dt(5) * (dt(3) * rir - r2 * tr) * i7 * r[:, 2])
    return m * r[:, 2] * i5 * r2 - du


def series_gz(mass: np.ndarray, rel: np.ndarray, side: np.ndarray, dt: type) -> np.ndarray:
    """A uniform cube's monopole + l = 4 series, g_z per element; rel = centre - point."""
    m, x, s = mass.astype(dt), (-rel).astype(dt), side.astype(dt)
    r2 = (x * x).sum(axis=1)
    r = np.sqrt(r2)
    s4 = (x**4).sum(axis=1)
    a = dt(-7 / 60) * (s / dt(2)) ** 4 * m
    inv = dt(1) / r
    i3, i7 = inv**3, inv**7
    i9, i11 = i7 / r2, i7 / (r2 * r2)
    return -m * x[:, 2] * i3 + a * (dt(20) * x[:, 2] ** 3 * i9 - dt(45) * s4 * i11 * x[:, 2]
                                    + dt(15) * x[:, 2] * i7)


# ------------------------------------------------------------------ sums


def _two_sum(a: np.ndarray, b: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    s = a + b
    bb = s - a
    return s, (a - (s - bb)) + (b - bb)


def gpu_sum(x: np.ndarray, how: str) -> float:
    """Sum f32 terms `x` the way a GPU reduction would (see the module docstring)."""
    x = x.astype(F32)
    if how == "seq32":
        return float(np.cumsum(x, dtype=F32)[-1]) if len(x) else 0.0
    pad = (-len(x)) % LANES
    lanes = np.concatenate([x, np.zeros(pad, F32)]).reshape(-1, LANES)  # row k: elements k*LANES..
    if how == "f64acc":
        return float(lanes.astype(F64).sum())
    hi = np.zeros(LANES, F32)
    lo = np.zeros(LANES, F32)
    for row in lanes:
        if how == "lane32":
            hi = hi + row
        elif how == "kahan":
            y = row - lo
            t = hi + y
            lo = (t - hi) - y
            hi = t
        else:  # ff
            hi, e = _two_sum(hi, row)
            lo = lo + e
            hi, lo = _two_sum(hi, lo)
    if how == "kahan":
        hi = hi - lo  # fold the compensation in before the tree
        lo = np.zeros(LANES, F32)
    while len(hi) > 1:
        a, b = hi[0::2], hi[1::2]
        if how == "ff":
            al, bl = lo[0::2], lo[1::2]
            s, e = _two_sum(a, b)
            e = e + al + bl
            hi, lo = _two_sum(s, e)
        else:
            hi = a + b
            lo = lo[0::2]
    return float(hi[0]) + (float(lo[0]) if how == "ff" else 0.0)


SUMS = ["seq32", "lane32", "kahan", "ff", "f64acc"]


# ------------------------------------------------------------------ B1 and A4: one element


def mp_tri_g(tris: np.ndarray, p: np.ndarray, dps: int = 40) -> np.ndarray:
    """g (per unit G rho) of a triangle soup at p, the same face form at `dps` digits."""
    import mpmath as mp

    mp.mp.dps = dps
    g = [mp.mpf(0)] * 3
    pm = [mp.mpf(float(x)) for x in p]
    for t in tris:
        v = [[mp.mpf(float(x)) for x in q] for q in t]
        r = [[v[i][k] - pm[k] for k in range(3)] for i in range(3)]
        cr = lambda a, b: [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]  # noqa: E731
        dt_ = lambda a, b: a[0] * b[0] + a[1] * b[1] + a[2] * b[2]  # noqa: E731
        e1 = [v[1][k] - v[0][k] for k in range(3)]
        e2 = [v[2][k] - v[0][k] for k in range(3)]
        nn = cr(e1, e2)
        a2 = mp.sqrt(dt_(nn, nn))
        n = [x / a2 for x in nn]
        ln = [mp.sqrt(dt_(x, x)) for x in r]
        h = dt_(n, r[0])
        den = ln[0] * ln[1] * ln[2] + ln[0] * dt_(r[1], r[2]) + ln[1] * dt_(r[2], r[0]) + ln[2] * dt_(r[0], r[1])
        om = 2 * mp.atan2(dt_(r[0], cr(r[1], r[2])), den)
        e = mp.mpf(0)
        for i in range(3):
            j = (i + 1) % 3
            ed = [v[j][k] - v[i][k] for k in range(3)]
            el = mp.sqrt(dt_(ed, ed))
            ne = [x / el for x in cr(ed, n)]
            e += dt_(ne, r[i]) * mp.log((ln[i] + ln[j] + el) / (ln[i] + ln[j] - el))
        b = e - h * om
        g = [g[k] - n[k] * b for k in range(3)]
    return np.array([float(x) for x in g])


def kernel() -> None:
    """B1: relative error of g_z for one cut-piece-like triangle and for boxes of several aspect
    ratios, against R/s, in each f32 variant; A4: the f64 box against 40 digits."""
    direction = np.array([0.37, -0.52, 0.77])
    direction /= np.linalg.norm(direction)
    centre = np.array([0.3141, -0.2718, 0.1618])  # body-frame position of the element
    s = 1 / 64
    tri = centre + s * np.array([[[0.5, -0.2, 0.1], [-0.3, 0.45, -0.05], [0.1, 0.05, 0.6]]])
    shapes = {"triangle": tri}
    for ar in (1, 16, 256):
        size = s * np.array([1.0, 1.0, float(ar)]) / ar ** (1 / 3)
        shapes[f"box 1:1:{ar}"] = box_tris_one(centre - size / 2, centre + size / 2)
    ratios = np.logspace(0, 5, 21)
    variants = {"f32 local h2A log1p": ("local", "h2A", "log1p", False),
                "f32 local triple log1p": ("local", "triple", "log1p", False),
                "f32 local h2A ratio-log": ("local", "h2A", "ratio", False),
                "f32 body frame h2A log1p": ("body", "h2A", "log1p", False),
                "f32 world 1 km h2A log1p": ("world", "h2A", "log1p", False),
                "f32 local h2A, WGSL worst case": ("local", "h2A", "log1p", True)}
    out: dict[str, object] = {"ratios": ratios.tolist(), "eps32": float(np.finfo(F32).eps), "shapes": {}}
    for name, tris in shapes.items():
        geo = geometry(tris)
        size = float(np.ptp(tris.reshape(-1, 3), axis=0).max())
        rows: dict[str, list[float]] = {k: [] for k in variants}
        rows["f64 vs 40 digits"] = []
        for q in ratios:
            p = centre + q * size * direction
            ref64 = tri_gz(offsets(tris, p, "local", F64), geo, F64).sum()
            if name != "triangle" and q in ratios[::4]:
                mp_ref = mp_tri_g(tris, p)[2]
                rows["f64 vs 40 digits"].append(abs(ref64 / mp_ref - 1))
            for k, (frame, numer, edge, wg) in variants.items():
                v = tri_gz(offsets(tris, p, frame, F32), geo, F32, numer, edge, wg)
                rows[k].append(abs(float(v.astype(F64).sum()) / ref64 - 1))
        out["shapes"][name] = {"size": size, "rel_err": rows}  # type: ignore[index]
        print(name, {k: f"{v[len(v) // 2]:.1e}" for k, v in rows.items() if v}, flush=True)
    (C.DATA / "precision-kernel.json").write_text(json.dumps(out, indent=1))


def box_tris_one(lo: np.ndarray, hi: np.ndarray) -> np.ndarray:
    import boxes as B

    return B.box_tris(lo[None], hi[None])


# ------------------------------------------------------------------ B2-B4: a body


def body(name: str, ns: tuple[int, ...] = (32, 128)) -> None:
    import boxes as B

    mesh = L.mesh_by_name(name)
    size = float((mesh.verts.max(axis=0) - mesh.verts.min(axis=0)).max())
    pts = L.tower(mesh, DIST)
    m = len(DIST)
    out: dict[str, object] = {"shape": name, "distances": DIST.tolist(), "sums": SUMS, "rows": []}
    mesh_geo = geometry(mesh.tris)
    for n_across in ns:
        h = size / n_across
        t0 = time.perf_counter()
        vox = L.voxelise(mesh, h)
        g = np.zeros(tuple(int(x) for x in vox.n), dtype=bool)
        g[tuple(vox.interior.T)] = True
        blocks = B.boxes_octree(g)
        btris = B.box_tris(vox.base + blocks[:, :3] * h, vox.base + blocks[:, 3:] * h)
        bgeo = geometry(btris)
        bside = (blocks[:, 3] - blocks[:, 0]) * h
        bctr = vox.base + (blocks[:, :3] + 0.5 * (blocks[:, 3:] - blocks[:, :3])) * h
        bmass = bside**3
        pgeo = geometry(vox.piece_tris)
        # Cut-cell moments recomputed in f32 from the pieces, about each cell's centre (A GPU or an
        # f32 voxeliser would do this; the CPU f64 values are the reference.)
        ctr = vox.boundary_centres
        rel = (vox.piece_tris - ctr[vox.piece_of][:, None, :]).astype(F32)
        a, b_, c = rel[:, 0], rel[:, 1], rel[:, 2]
        vol_t = (a * np.cross(b_, c)).sum(axis=1) / F32(6)
        vol32 = np.zeros(len(ctr), F32)
        np.add.at(vol32, vox.piece_of, vol_t)
        mom32 = np.zeros((len(ctr), 3), F32)
        np.add.at(mom32, vox.piece_of, vol_t[:, None] * (a + b_ + c) / F32(4))
        cen32 = mom32 / np.maximum(vol32, F32(1e-30))[:, None]  # centroid offset from the cell centre
        mass64 = vox.frac * h**3
        row: dict[str, object] = {
            "n": n_across, "h": h, "blocks": len(blocks), "boundary": len(ctr),
            "piece_tris": len(vox.piece_tris), "mass_rel_err_f32_moments":
                float(abs(vol32.astype(F64).sum() / mass64.sum() - 1)),
            "max_centroid_err_f32_over_h": float(np.abs(ctr + cen32.astype(F64) - vox.centroid)[
                vox.frac > 1e-6].max() / h),
            "parts": {}}
        sec = vox.second
        parts: dict[str, dict[str, list]] = {}
        for k in range(len(DIST)):
            pair = (pts[k], pts[m + k])
            # f64 reference terms of each part, at both points.
            ref = {}
            terms: dict[str, list[np.ndarray]] = {}
            for side, p in enumerate(pair):
                cases = {
                    "mesh": lambda dt, fr="local", wg=False: tri_gz(offsets(mesh.tris, p, fr, dt), mesh_geo, dt, wgsl=wg),
                    "blocks": lambda dt, fr="local", wg=False: tri_gz(offsets(btris, p, fr, dt), bgeo, dt, wgsl=wg),
                    "pieces": lambda dt, fr="local", wg=False: tri_gz(offsets(vox.piece_tris, p, fr, dt), pgeo, dt, wgsl=wg),
                    # The octree's blocks read by their l <= 4 series (whatever its truncation: this
                    # measures only the rounding of the series), offsets formed in f64 and rounded.
                    "blockseries": lambda dt, fr="local", wg=False: series_gz(
                        bmass, (bctr - p).astype(dt), bside, dt),
                    # f32: the moments recomputed in f32 above; the cell centre's offset from the
                    # point formed in f64 and rounded, plus the f32 centroid offset.
                    "moments": lambda dt, fr="local", wg=False: (
                        quad_gz(mass64, vox.centroid - p, sec, F64) if dt is F64 else
                        quad_gz(vol32, (ctr - p).astype(F32) + cen32, sec, F32)),
                }
                for part, fn in cases.items():
                    r64 = fn(F64)
                    ref.setdefault(part, []).append(math.fsum(r64))
                    for var, (fr, wg) in {"local": ("local", False), "body": ("body", False),
                                          "world": ("world", False), "wgsl": ("local", True)}.items():
                        if part in ("moments", "blockseries") and var in ("body", "world", "wgsl"):
                            continue
                        terms.setdefault(f"{part}|{var}", []).append(fn(F32, var if not wg else "local", wg))
            for key, (lo_t, up_t) in terms.items():
                part, var = key.split("|")
                d_ref = ref[part][1] - ref[part][0]
                for how in (SUMS if var == "local" else ["ff"]):
                    d = gpu_sum(up_t, how) - gpu_sum(lo_t, how)
                    parts.setdefault(f"{key}|{how}", []).append(PHASE * (d - d_ref))
            for part in ref:
                parts.setdefault(f"{part}|signal", []).append(PHASE * (ref[part][1] - ref[part][0]))
            print(name, n_across, DIST[k], {k_: f"{v[-1]:.1e}" for k_, v in parts.items()
                                            if k_.endswith("|ff") or k_.endswith("signal")}, flush=True)
        row["parts"] = parts
        row["seconds"] = time.perf_counter() - t0
        out["rows"].append(row)  # type: ignore[union-attr]
        (C.DATA / f"precision-{name}.json").write_text(json.dumps(out, indent=1))
        del vox


def tri_gz_mixed(r64: np.ndarray, geo: dict[str, np.ndarray], part: str) -> np.ndarray:
    """g_z per triangle with one ingredient in f32 and the rest in f64, to find what makes the f32
    error: 'offsets' (r rounded to f32), 'functions' (atan2 and log1p evaluated in f32 on f64
    arguments), 'geometry' (normals, areas, edge data rounded to f32), 'products' (the edge products
    (n_e . r_e) L_e and h*omega rounded to f32 before they are added)."""
    r = r64.astype(F32).astype(F64) if part == "offsets" else r64
    g = {k: v.astype(F32).astype(F64) for k, v in geo.items()} if part == "geometry" else geo
    rnd = (lambda x: x.astype(F32).astype(F64)) if part == "products" else (lambda x: x)  # noqa: E731
    fn32 = part == "functions"
    n = g["n"]
    l = np.sqrt((r * r).sum(axis=2))
    h = (n * r[:, 0]).sum(axis=1)
    num = h * g["a2"]
    dot = lambda i, j: (r[:, i] * r[:, j]).sum(axis=1)  # noqa: E731
    den = l[:, 0] * l[:, 1] * l[:, 2] + l[:, 0] * dot(1, 2) + l[:, 1] * dot(2, 0) + l[:, 2] * dot(0, 1)
    om = (np.arctan2(num.astype(F32), den.astype(F32)).astype(F64) if fn32 else np.arctan2(num, den)) * 2
    e = np.zeros(len(r))
    for i in range(3):
        j = (i + 1) % 3
        x = 2 * g["el"][:, i] / (l[:, i] + l[:, j] - g["el"][:, i])
        lg = np.log1p(x.astype(F32)).astype(F64) if fn32 else np.log1p(x)
        e = e + rnd((g["ne"][:, i] * r[:, i]).sum(axis=1) * lg)
    return -n[:, 2] * (e - rnd(h * om))


def diagnose(name: str) -> None:
    """The whole mesh at every tower distance, one ingredient in f32 at a time (terms summed by
    math.fsum, so only the kernel is tested)."""
    mesh = L.mesh_by_name(name)
    pts = L.tower(mesh, DIST)
    m = len(DIST)
    geo = geometry(mesh.tris)
    out: dict[str, list[float]] = {}
    for k in range(m):
        ph = {}
        for part in ("f64", "offsets", "functions", "geometry", "products", "all f32"):
            v = []
            for p in (pts[k], pts[m + k]):
                r64 = mesh.tris - p
                if part == "f64":
                    v.append(math.fsum(tri_gz(r64, geo, F64)))
                elif part == "all f32":
                    v.append(math.fsum(tri_gz(r64.astype(F32), geo, F32).astype(F64)))
                else:
                    v.append(math.fsum(tri_gz_mixed(r64, geo, part)))
            ph[part] = PHASE * (v[1] - v[0])
        for part in ph:
            out.setdefault(part, []).append(ph[part] - ph["f64"] if part != "f64" else ph["f64"])
        print(name, DIST[k], {p_: f"{x[-1]:.1e}" for p_, x in out.items()}, flush=True)
    (C.DATA / f"precision-diagnose-{name}.json").write_text(json.dumps(
        {"shape": name, "triangles": len(mesh.faces), "distances": DIST.tolist(),
         "note": "'f64' is the phase itself; the rest are errors against it (rad)", "phase": out},
        indent=1))


if __name__ == "__main__":
    if sys.argv[1] == "kernel":
        kernel()
    elif sys.argv[1] == "diagnose":
        diagnose(sys.argv[2])
    else:
        body(sys.argv[2], tuple(int(x) for x in sys.argv[3].split(",")) if len(sys.argv) > 3 else (32, 128))
