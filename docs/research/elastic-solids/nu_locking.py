"""Does 'Q1 + incompatible modes' survive a nearly incompressible material?

Same clamped voxel beam as cantilever.py (L/h = 10, cubic cells), Poisson's ratio raised from 0.2.
Elements: Q1 (full), Q1-SRI (selective reduced integration of the volumetric part), Q1-IM
(9 incompatible modes, condensed), and Q1-IM-SRI (the deviatoric part of Q1 and of the modes fully
integrated, the volumetric part at one point: the combination MacNeal & Harder 1985 Table 6
describe for the MSC HEXA(8) brick, "selective reduced order integration and internal strain
functions equivalent to bubble functions"; this is my reading of that line, not their code).

Pass criteria written before running:
  N1 at nu = 0.3 all four are within 1 % of the converged value at 2 cells (control: same as before);
  N2 the reference is Q1-IM-SRI at 8 cells (frequency should be almost independent of nu
     for a slender beam: Euler-Bernoulli ratio reported);
  the question: does Q1-IM at 1-2 cells through the thickness lock (> 5 % high) at nu = 0.49 and
  0.4999, and does Q1-IM-SRI cure it?
"""
import itertools

import numpy as np

import cantilever as c


ORIG_K = c.element_K  # cantilever.py's elements; beam() looks element_K up in that module


def element_K(E, nu, s, kind):
    if kind != "Q1-IM-SRI":
        return ORIG_K(E, nu, s, kind)
    D, lam, mu = c.elasticity(E, nu)
    K = lam + 2 * mu / 3
    m = np.array([1, 1, 1, 0, 0, 0.0])
    Dvol = K * np.outer(m, m)
    Ddev = D - Dvol
    w = (s / 2) ** 3
    pts = [np.array(q) for q in itertools.product(c.GP, repeat=3)]
    B0 = c.B_matrix(c.dN_q1(np.zeros(3), s))
    Kuu = 8 * w * B0.T @ Dvol @ B0
    Kua = np.zeros((24, 9)); Kaa = np.zeros((9, 9))
    for p in pts:
        Bu = c.B_matrix(c.dN_q1(p, s)); Ba = c.B_matrix(c.dN_bubble(p, s))
        Kuu += Bu.T @ Ddev @ Bu * w
        Kua += Bu.T @ Ddev @ Ba * w
        Kaa += Ba.T @ Ddev @ Ba * w
    return Kuu - Kua @ np.linalg.solve(Kaa, Kua.T)


if __name__ == "__main__":
    c.element_K = element_K
    E, rho, h, L_over_h = 30e9, 2400.0, 1.0, 10
    I = h ** 4 / 12
    f_eb = 1.875104068711961 ** 2 / (2 * np.pi) * np.sqrt(E * I / (rho * h * h * (L_over_h * h) ** 4))
    kinds = ("Q1", "Q1-SRI", "Q1-IM", "Q1-IM-SRI")
    for nu in (0.2, 0.3, 0.49, 0.4999):
        print(f"nu = {nu}")
        # patch test for the new element
        Ke = element_K(E, nu, 1.0, "Q1-IM-SRI")
        ev, u = c.patch_test(Ke, 1.0)
        D, _, _ = c.elasticity(E, nu)
        print(f"  Q1-IM-SRI patch test rel. error {abs(u @ Ke @ u - ev @ D @ ev) / (ev @ D @ ev):.1e}")
        res = {}
        for kind in kinds:
            for n in (1, 2, 4):
                f, *_ = c.beam(n, L_over_h, E, nu, rho, kind, h)
                res[(kind, n)] = f[0]
        ref, *_ = c.beam(8, L_over_h, E, nu, rho, "Q1-IM-SRI", h)
        ref = ref[0]
        print(f"  reference Q1-IM-SRI n=8: f1 = {ref:.4f} Hz, f1/f_EB = {ref / f_eb:.4f}")
        for kind in kinds:
            print("  " + f"{kind:10s}" + "  ".join(f"n={n}: {100 * (res[(kind, n)] - ref) / ref:+7.2f} %" for n in (1, 2, 4)))
