"""Benchmark for docs/research/s5-world-octree.md section 6.

A 128^3-cell world (1 m cells, 2000 kg/m^3): a ground slab, random box buildings and a ditch. One
octree (levels 0 = cell .. 7 = root) carries mass, centre of mass and second central moments per node.
A gradiometer reads g_z at two points 5 m apart (phase = 4.8e9 rad per m s^-2 of difference).

Measured against the leaf-exact sum (every cell a point mass; the walk error is what is compared):
  A. the walk error and node count against the opening angle theta, monopole vs quadrupole;
  B. the per-level LOD radius: the nearest distance at which a node of each level is accepted;
  C. error-driven opening (a per-node bound) against theta-driven opening, at equal node count;
  D. the slab: the on-axis gradient of a finite disc against its closed form, and the size of the
     hole left by a finite patch (why the plane must be analytic, not tiled).
Pure numpy, one thread, a few seconds. Run: python lod_walk.py
"""

import numpy as np

G = 6.674e-11
RHO = 2000.0
KT2 = 4.8e9  # rad per (m s^-2): k_eff T^2 of AION-10 (docs/research/exact-skin-and-partial-fill.md)
N = 128
LEVELS = 7  # 2^7 = 128


def build_world(seed=1):
    rng = np.random.default_rng(seed)
    occ = np.zeros((N, N, N), bool)
    occ[:, :, :8] = True  # ground slab, 8 m thick
    c = N // 2
    for _ in range(60):  # buildings
        x, y = rng.integers(0, N - 12, 2)
        w, d = rng.integers(4, 12, 2)
        h = rng.integers(4, 20)
        if abs(x + w / 2 - c) < 14 and abs(y + d / 2 - c) < 14:
            continue  # keep the tower's yard clear
        occ[x : x + w, y : y + d, 8 : 8 + h] = True
    occ[20:100, 30:36, 4:8] = False  # a ditch
    return occ


def pyramid(occ):
    """Per level: mass, first moment sum, second raw-moment sum (6), from cell centres."""
    m0 = occ.astype(float) * RHO
    ax = np.arange(N) + 0.5
    X, Y, Z = np.meshgrid(ax, ax, ax, indexing="ij")
    pos = [X, Y, Z]
    lv = [dict(m=m0, s=[m0 * p for p in pos], q=[m0 * pos[i] * pos[j] for i, j in IJ])]
    for _ in range(LEVELS):
        prev = lv[-1]
        red = lambda a: a.reshape(a.shape[0] // 2, 2, a.shape[1] // 2, 2, a.shape[2] // 2, 2).sum((1, 3, 5))
        lv.append(dict(m=red(prev["m"]), s=[red(a) for a in prev["s"]], q=[red(a) for a in prev["q"]]))
    return lv


IJ = [(0, 0), (1, 1), (2, 2), (0, 1), (0, 2), (1, 2)]


def accel(x, M, com, Q, order):
    """g at x from nodes: monopole (+ quadrupole when order 2). Arrays are per node."""
    r = x - com
    r2 = (r * r).sum(1)
    rr = np.sqrt(r2)
    g = -G * (M / rr**3)[:, None] * r
    if order >= 2:
        Qm = np.zeros((len(M), 3, 3))
        for k, (i, j) in enumerate(IJ):
            Qm[:, i, j] = Q[:, k]
            Qm[:, j, i] = Q[:, k]
        T = np.einsum("ni,nij,nj->n", r, Qm, r)
        trQ = np.einsum("nii->n", Qm)
        Qr = np.einsum("nij,nj->ni", Qm, r)
        g += 0.5 * G * (6 * Qr / (rr**5)[:, None] - 15 * (T / rr**7)[:, None] * r + 3 * (trQ / rr**5)[:, None] * r)
    return g.sum(0)


def node_data(lv, level, idx):
    """mass, COM, central second moments, bounding radius a about COM, for nodes idx (n,3)."""
    d = lv[level]
    i, j, k = idx.T
    M = d["m"][i, j, k]
    s = np.stack([d["s"][a][i, j, k] for a in range(3)], 1)
    q = np.stack([d["q"][a][i, j, k] for a in range(6)], 1)
    with np.errstate(invalid="ignore", divide="ignore"):
        com = s / M[:, None]  # empty nodes give NaN; the walk drops them (M == 0)
    qc = np.empty_like(q)
    for a, (p, r) in enumerate(IJ):
        qc[:, a] = q[:, a] - M * com[:, p] * com[:, r]
    side = 2.0**level
    lo = idx * side
    # bounding radius: farthest corner of the cube from the COM (conservative)
    far = np.maximum(np.abs(lo - com), np.abs(lo + side - com))
    a = np.sqrt((far**2).sum(1))
    return M, com, qc, a


def walk(lv, x, accept, order):
    """Open nodes from the root; accept(level, M, a, R, com) decides. Returns g, nodes, radii per level."""
    idx = np.zeros((1, 3), int)
    g = np.zeros(3)
    nodes = 0
    rad = {}
    for level in range(LEVELS, -1, -1):
        M, com, qc, a = node_data(lv, level, idx)
        keep = M > 0
        idx, M, com, qc, a = idx[keep], M[keep], com[keep], qc[keep], a[keep]
        R = np.sqrt(((x - com) ** 2).sum(1))
        ok = accept(level, M, a, R) if level > 0 else np.ones(len(M), bool)
        nodes += len(M)
        if ok.any():
            g += accel(x, M[ok], com[ok], qc[ok], order if level > 0 else 1)
            rad[level] = (R[ok] / 2.0**level).min()
        idx = idx[~ok]
        if len(idx) == 0:
            break
        off = np.array([[a_, b, c] for a_ in (0, 1) for b in (0, 1) for c in (0, 1)])
        idx = (idx[:, None, :] * 2 + off[None]).reshape(-1, 3)
    return g, nodes, rad


def read(lv, pts, accept, order):
    gs, n, rad = [], 0, {}
    for p in pts:
        g, k, r = walk(lv, p, accept, order)
        gs.append(g)
        n += k
        for L, v in r.items():
            rad[L] = min(rad.get(L, 1e9), v)
    phase = KT2 * (gs[1][2] - gs[0][2])
    return phase, n, rad


def main():
    occ = build_world()
    lv = pyramid(occ)
    c = N // 2
    pts = [np.array([c + 0.3, c + 0.2, 20.5]), np.array([c + 0.3, c + 0.2, 25.5])]
    ref, nref, _ = read(lv, pts, lambda L, M, a, R: np.zeros(len(M), bool), 1)
    print(f"cells occupied {occ.sum():,}; leaf-exact phase {ref:.6f} rad; leaf nodes visited {nref:,}")

    print("\nA. theta-driven opening (accept when a/R <= theta)")
    print("theta  order  nodes   |phase error| rad   crude bound rad")
    for th in (0.9, 0.7, 0.5, 0.35, 0.25, 0.15, 0.1):
        for order in (1, 2):
            ph, n, _ = read(lv, pts, lambda L, M, a, R, th=th: a <= th * R, order)
            # crude a-posteriori bound for the truncation: the first dropped order, gradient of the
            # potential remainder <= sum_{l>p} (l+1) M a^l / R^(l+2), summed over accepted nodes (here
            # taken at the accepted nodes of the walk, recomputed)
            bound = bound_of(lv, pts, th, order)
            print(f"{th:5.2f}  {order}  {n:8d}  {abs(ph - ref):14.3e}   {bound:14.3e}")

    print("\nB. LOD radius per level at theta = 0.35, quadrupole: nearest accepted R in node sides")
    _, _, rad = read(lv, pts, lambda L, M, a, R: a <= 0.35 * R, 2)
    for L in sorted(rad):
        print(f"  level {L} (node side {2**L:3d} m): R_min = {rad[L]:.2f} sides = {rad[L] * 2**L:.1f} m")

    print("\nC. error-driven opening: accept when the node's bound on g <= eps (m s^-2)")
    print("eps        nodes   |phase error| rad   sum of bounds (phase) rad")
    for eps in (1e-9, 1e-10, 1e-11, 1e-12, 1e-13):
        tot = [0.0]

        def acc(L, M, a, R, eps=eps, tot=tot):
            x = np.minimum(a / R, 0.999)
            b = G * M / R**2 * sum((l + 1) * x**l for l in range(3, 40))
            ok = b <= eps
            tot[0] += b[ok].sum()
            return ok

        ph, n, _ = read(lv, pts, acc, 2)
        print(f"{eps:8.0e} {n:8d}  {abs(ph - ref):14.3e}   {2 * KT2 * tot[0]:14.3e}")

    print("\nD. slab")
    rho, t = RHO, 8.0
    for Lr in (64.0, 640.0, 6400.0):
        h = 12.5
        gz = lambda h_: 2 * np.pi * G * rho * t * (1 - h_ / np.sqrt(h_**2 + Lr**2))
        dphi = KT2 * (gz(h + 5) - gz(h))
        print(f"  disc radius {Lr:7.0f} m: phase over 5 m = {dphi:.3e} rad (closed form; infinite slab: 0)")
    # numerical disc from the pyramid: slab only
    slab = np.zeros((N, N, N), bool)
    slab[:, :, :8] = True
    lvs = pyramid(slab)
    ph, _, _ = read(lvs, pts, lambda L, M, a, R: np.zeros(len(M), bool), 1)
    ph2, n2, _ = read(lvs, pts, lambda L, M, a, R: a <= 0.35 * R, 2)
    print(f"  128 m x 128 m square patch, leaf-exact {ph:.3e} rad; walk (theta 0.35) {ph2:.3e} rad ({n2} nodes)")


def bound_of(lv, pts, th, order):
    """Sum over accepted nodes of the crude truncation bound, as a phase."""
    tot = 0.0
    for p in pts:
        acc_list = []

        def acc(L, M, a, R, th=th):
            ok = a <= th * R
            x = np.minimum(a / R, 0.999)
            lo = order + 1
            b = G * M / R**2 * sum((l + 1) * x**l for l in range(lo, 60))
            acc_list.append(b[ok].sum())
            return ok

        walk(lv, p, acc, order)
        tot += sum(acc_list)
    return 2 * KT2 * tot  # both points, the worst-case sign


if __name__ == "__main__":
    main()
