"""Benchmark for docs/research/s5-world-octree.md, "Changed after reading the primary sources".

How loose is the Salmon & Warren (1994) rigorous multipole error bound, and the gradient analogue that S4 and
this note use, for a uniform cube (a voxel node)?  Unit cube side s = 1, unit density, G = 1, centre at origin
(= centre of mass, so the dipole vanishes).  Target at distance d along three directions: the cube axis, a face
diagonal and the body diagonal.

  actual  = the exact remainder after truncating the multipole expansion of the Newtonian potential at order p,
            by Gauss-Legendre quadrature over the cube (exact to rounding for d >= 1.5 s with 10 points/axis).
  SW      = Salmon & Warren eq. (62), acceleration, p = order kept, B_(n) = integral rho |x|^n (absolute moment,
            exact by quadrature), b_max = sqrt(3)/2; once with the minus term (using the exact B_(p+2), the tightest
            lower bound could ever be) and once without it (the simple form).
  GRAD    = the gradient analogue used in this note, sum over l > p of (l+1)(l+2) B_(l) / d^(l+3), compared with the
            remainder of dg_z/dz (the quantity a vertical gradiometer reads), target on the z axis.
Pure numpy. Run: python mac_bound.py
"""
import numpy as np
from numpy.polynomial.legendre import leggauss
from numpy.polynomial import legendre as L

SQ3 = np.sqrt(3.0) / 2.0  # b_max of a unit cube about its centre
x1, w1 = leggauss(10)
X, Y, Z = np.meshgrid(x1 / 2, x1 / 2, x1 / 2, indexing="ij")
W = (w1[:, None, None] * w1[None, :, None] * w1[None, None, :]) / 8.0
P = np.stack([X.ravel(), Y.ravel(), Z.ravel()], 1)
W = W.ravel()
R = np.linalg.norm(P, axis=1)


def B(n):  # absolute moment about the centre
    return float(np.sum(W * R**n))


def phi_terms(r, nmax):
    """potential contributions phi_n(r) = -sum w |x|^n P_n(cos g) / d^(n+1) for n = 0..nmax"""
    d = np.linalg.norm(r)
    cosg = (P @ r) / (R * d + 1e-300)
    cosg = np.where(R < 1e-14, 0.0, cosg)
    out = []
    for n in range(nmax + 1):
        c = np.zeros(n + 1); c[n] = 1.0
        out.append(-np.sum(W * R**n * L.legval(cosg, c)) / d ** (n + 1))
    return np.array(out)


def phi_exact(r):
    dr = r[None, :] - P
    return -np.sum(W / np.linalg.norm(dr, axis=1))


def grad(f, r, h):
    g = np.zeros(3)
    for i in range(3):
        e = np.zeros(3); e[i] = h
        g[i] = (f(r + e) - f(r - e)) / (2 * h)
    return g


def acc_exact(r): return -grad(phi_exact, r, 1e-5)
def acc_trunc(r, p): return -grad(lambda q: phi_terms(q, p).sum(), r, 1e-5)


def dgzdz_exact(r, h=2e-3):
    e = np.array([0, 0, h])
    return (acc_exact(r + e)[2] - acc_exact(r - e)[2]) / (2 * h)


def dgzdz_trunc(r, p, h=2e-3):
    e = np.array([0, 0, h])
    return (acc_trunc(r + e, p)[2] - acc_trunc(r - e, p)[2]) / (2 * h)


print("uniform unit cube; ratio = actual remainder / bound (1 = tight; the bound is never below the actual)")
print(f"{'p':>2} {'d/s':>5} {'dir':>5} | {'|dA| actual':>12} {'SW eq62':>10} {'ratio':>8} | {'SW no minus':>11} {'ratio':>8}"
      f" | {'dgz/dz actual':>14} {'GRAD bound':>11} {'ratio':>9}")
for p in (1, 2, 3):
    for d in (2.0, 3.0, 5.0, 10.0):
        for name, u in (("axis", np.array([0, 0, 1.0])), ("face", np.array([1, 0, 1.0]) / np.sqrt(2)),
                        ("body", np.array([1, 1, 1.0]) / np.sqrt(3))):
            r = d * u
            dA = np.linalg.norm(acc_exact(r) - acc_trunc(r, p))
            pref = 1.0 / (d**2 * (1 - SQ3 / d) ** 2)
            sw = pref * ((p + 2) * B(p + 1) / d ** (p + 1) - (p + 1) * B(p + 2) / d ** (p + 2))
            swn = pref * ((p + 2) * B(p + 1) / d ** (p + 1))
            if name == "axis":
                ra = np.array([0, 0, d])
                gd = abs(dgzdz_exact(ra) - dgzdz_trunc(ra, p))
                gb = sum((l + 1) * (l + 2) * B(l) / d ** (l + 3) for l in range(p + 1, 80))
                gs = f"{gd:14.3e} {gb:11.3e} {gb / gd:9.1f}"
            else:
                gs = ""
            print(f"{p:>2} {d:>5.0f} {name:>5} | {dA:12.3e} {sw:10.3e} {sw / dA:8.1f} | {swn:11.3e} {swn / dA:8.1f} | {gs}")


# ---- Part 2: spend the signed moments on the leading remainder term, bound only the tail --------------------------
# The remainder after order p starts at n = p+1.  For a cube about its centre n = 1, 2, 3 vanish exactly (cubic
# symmetry), so the absolute-moment bound above is loose by the angular factor, not by sloppy algebra.  S&W eq. (46)-(47)
# give the n-th term exactly from the traceless signed moment tensor.  Keep that term (sup over direction, so the
# bound does not depend on which way the target lies) and bound only n >= p+2 with eq. (62) / the gradient sum.
rng = np.random.default_rng(7)
dirs = rng.normal(size=(300, 3)); dirs /= np.linalg.norm(dirs, axis=1)[:, None]


def term_acc(r, n): return -grad(lambda q: phi_terms(q, n)[n], r, 1e-5)


def term_dgzdz(r, n, h=2e-3):
    e = np.array([0, 0, h])
    return (term_acc(r + e, n)[2] - term_acc(r - e, n)[2]) / (2 * h)


print("\nPart 2: exact leading term (sup over 300 directions) + absolute-moment tail, uniform unit cube")
print(f"{'p':>2} {'d/s':>5} | {'acc: actual(axis)':>17} {'part-1 bound':>13} {'ratio':>7} {'part-2 bound':>13} {'ratio':>7}"
      f" | {'grad: actual(axis)':>18} {'part-1':>10} {'ratio':>7} {'part-2':>10} {'ratio':>7}")
for p in (1, 2, 3):
    for d in (3.0, 5.0, 10.0):
        r = d * np.array([0, 0, 1.0])
        dA = np.linalg.norm(acc_exact(r) - acc_trunc(r, p))
        pref = 1.0 / (d**2 * (1 - SQ3 / d) ** 2)
        sw1 = pref * ((p + 2) * B(p + 1) / d ** (p + 1) - (p + 1) * B(p + 2) / d ** (p + 2))
        n = p + 1
        lead = max(np.linalg.norm(term_acc(d * u, n)) for u in dirs[:120])
        sw2 = lead + pref * ((n + 2) * B(n + 1) / d ** (n + 1) - (n + 1) * B(n + 2) / d ** (n + 2))
        gd = abs(dgzdz_exact(r) - dgzdz_trunc(r, p))
        g1 = sum((l + 1) * (l + 2) * B(l) / d ** (l + 3) for l in range(p + 1, 80))
        leadg = max(abs(term_dgzdz(d * u, n)) for u in dirs[:120])
        g2 = leadg + sum((l + 1) * (l + 2) * B(l) / d ** (l + 3) for l in range(n + 1, 80))
        print(f"{p:>2} {d:>5.0f} | {dA:17.3e} {sw1:13.3e} {sw1 / dA:7.1f} {sw2:13.3e} {sw2 / dA:7.1f}"
              f" | {gd:18.3e} {g1:10.3e} {g1 / gd:7.1f} {g2:10.3e} {g2 / gd:7.1f}")
