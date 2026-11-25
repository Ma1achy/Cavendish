"""Benchmark for docs/research/s5-world-octree.md, "Changed after reading the primary sources".

Where does the looseness of a summed error bound come from?  Salmon & Warren (1994) eq. (14) adds the per-node bounds
(triangle inequality) and eq. (16)-(17) add them in quadrature if the node errors are uncorrelated.  Here the node
errors of a real cut are measured, with the same town world and point-mass reference as lod_walk.py:

  for each accepted node i of the cut:   e_i = (node's quadrupole field) - (exact point-mass sum over its cells)
  phase error of the cut                 = | sum_i e_i |
  triangle-inequality ledger             =   sum_i |e_i|          (what a rigorous per-node ledger can at best report)
  root-sum-square ledger                 = sqrt(sum_i e_i^2)      (S&W eq. 16-17: assumes uncorrelated errors)
The per-node |e_i| here is the node's *actual* error, so these ratios are a floor on the looseness of any ledger built
from per-node bounds: a bound can only be looser than the |e_i| it bounds.  Run: python cut_cancellation.py (numpy, ~20 s).
"""
import numpy as np
import lod_walk as W

occ = W.build_world()
lv = W.pyramid(occ)
c = W.N // 2
pts = [np.array([c + 0.3, c + 0.2, 20.5]), np.array([c + 0.3, c + 0.2, 25.5])]
ax = np.arange(W.N) + 0.5
X, Y, Z = np.meshgrid(ax, ax, ax, indexing="ij")


def cell_gz(x):  # exact point-mass g_z of every occupied cell at x
    r = np.stack([x[0] - X, x[1] - Y, x[2] - Z], -1)
    d = np.linalg.norm(r, axis=-1)
    return np.where(occ, -W.G * W.RHO * r[..., 2] / d**3, 0.0)


gz = [cell_gz(p) for p in pts]
red = lambda a: a.reshape(a.shape[0] // 2, 2, a.shape[1] // 2, 2, a.shape[2] // 2, 2).sum((1, 3, 5))
exact = []  # exact[point][level] = block sums of g_z
for g in gz:
    e = [g]
    for _ in range(W.LEVELS):
        e.append(red(e[-1]))
    exact.append(e)


def cut(x, accept):
    idx = np.zeros((1, 3), int)
    acc = []
    for level in range(W.LEVELS, -1, -1):
        M, com, qc, a = W.node_data(lv, level, idx)
        keep = M > 0
        idx, M, com, qc, a = idx[keep], M[keep], com[keep], qc[keep], a[keep]
        R = np.sqrt(((x - com) ** 2).sum(1))
        ok = accept(level, M, a, R) if level > 0 else np.ones(len(M), bool)
        acc.append((level, idx[ok], M[ok], com[ok], qc[ok]))
        idx = idx[~ok]
        if len(idx) == 0:
            break
        off = np.array([[a_, b, c_] for a_ in (0, 1) for b in (0, 1) for c_ in (0, 1)])
        idx = (idx[:, None, :] * 2 + off[None]).reshape(-1, 3)
    return acc


def node_errors(pi, accept):
    """per-node signed g_z error at point pi, for the cut that this point's own walk makes"""
    out = []
    for level, idx, M, com, qc in cut(pts[pi], accept):
        if level == 0 or len(M) == 0:
            continue  # level-0 nodes are single cells: exact (point mass), zero error here
        ex = exact[pi][level][idx[:, 0], idx[:, 1], idx[:, 2]]
        ap = np.array([W.accel(pts[pi], M[k:k + 1], com[k:k + 1], qc[k:k + 1], 2)[2] for k in range(len(M))])
        out.append(ap - ex)
    return np.concatenate(out) if out else np.zeros(0)


print(f"{'ledger':<28} {'nodes':>8} {'|sum e|':>10} {'sum|e|':>10} {'sqrt(sum e^2)':>14} {'sum|e|/|sum e|':>15} {'rss/|sum e|':>12}")
for name, acc in (
    ("theta 0.35 (a/R)", lambda L, M, a, R: a <= 0.35 * R),
    ("theta 0.20", lambda L, M, a, R: a <= 0.20 * R),
    ("eps 1e-11 (S4-like l>=3)", lambda L, M, a, R: (W.G * M / R**2 * sum((l + 1) * np.minimum(a / R, .999) ** l for l in range(3, 40))) <= 1e-11),
    ("eps 1e-12", lambda L, M, a, R: (W.G * M / R**2 * sum((l + 1) * np.minimum(a / R, .999) ** l for l in range(3, 40))) <= 1e-12),
):
    e1 = node_errors(0, acc); e2 = node_errors(1, acc)
    # phase = KT2 * (gz(p2) - gz(p1)); per-point node sets differ, so the phase error is the difference of the two sums
    tot = abs(e2.sum() - e1.sum()) * W.KT2
    tri = (np.abs(e1).sum() + np.abs(e2).sum()) * W.KT2
    rss = np.sqrt((e1**2).sum() + (e2**2).sum()) * W.KT2
    print(f"{name:<28} {len(e1) + len(e2):>8} {tot:10.3e} {tri:10.3e} {rss:14.3e} {tri / tot:15.1f} {rss / tot:12.2f}")
