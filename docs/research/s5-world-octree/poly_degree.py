"""Section 2.2 of docs/research/s5-world-octree.md: how many terms a local expansion of the far field needs.
Fits g_z of the far field of the shared cut (theta 0.25, 942 nodes) over the ball of radius rho about the
tower centre by a total-degree-q polynomial, and reports the error at test points on the arm axis, as
phase (rad) and as the spread between points (what a difference sees). Run: python poly_degree.py"""
import numpy as np
import lod_walk as w

occ = w.build_world()
lv = w.pyramid(occ)
cx = w.N // 2
p1, p2 = np.array([cx + 0.3, cx + 0.2, 20.5]), np.array([cx + 0.3, cx + 0.2, 25.5])
cT = 0.5 * (p1 + p2)
rho = 2.6
far, near = w.shared_cut(lv, cT, rho, lambda L, M, a, Rm: a <= 0.25 * Rm)
print(f"far nodes {len(far[0])}, near {len(near[0])}; nearest far node centre {np.sqrt(((far[1]-cT)**2).sum(1)).min():.1f} m")
rng = np.random.default_rng(5)
tz = np.linspace(20.5, 25.5, 40)
tpts = np.stack([cx + 0.3 + rng.uniform(-0.01, 0.01, 40), cx + 0.2 + rng.uniform(-0.01, 0.01, 40), tz], 1)
exact = np.array([w.gz_at(x, far) for x in tpts])
print("degree  terms   max |dg| rad   spread rad")
for q in (2, 4, 6, 8, 10):
    ns = 3 * (q + 1) * (q + 2) * (q + 3) // 6
    u = rng.normal(size=(ns, 3))
    u *= (rho * rng.random(ns) ** (1 / 3) / np.linalg.norm(u, axis=1))[:, None]
    gs = np.array([w.gz_at(cT + d, far) for d in u])
    coef, *_ = np.linalg.lstsq(w.monomials(u / rho, q), gs, rcond=None)
    fit = w.monomials((tpts - cT) / rho, q) @ coef
    d = w.KT2 * (exact - fit)
    print(f"{q:5d} {len(coef):6d} {np.abs(d).max():12.2e} {d.max() - d.min():12.2e}")
