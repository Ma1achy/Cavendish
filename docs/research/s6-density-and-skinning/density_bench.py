"""Measurements behind docs/research/s6-density-and-skinning.md (TASK-S6-07, S6-08).

Standard library only. Run: python -I density_bench.py > density_bench.out.txt

Pass criteria, written before running:

B1  Improved noise (Perlin 2002: fade 6t^5-15t^4+10t^3, the 12 cube-edge gradients) is a polynomial
    of degree <= 6 per axis inside one lattice cell, so an n-point Gauss-Legendre rule per axis
    (exact to degree 2n-1, DLMF 3.5.19) integrates its mass exactly for n >= 4 and its second
    moments exactly for n >= 5 on a lattice-aligned box. Pass: aligned n>=4 mass and n>=5 second
    moment agree with n=10 to <= 1e-15 absolute (values are O(0.1)); n=3 differs by > 1e-6
    (control arm: the rule is not exact by accident). A box straddling lattice planes, integrated
    without splitting, does not converge to rounding by n=10 (> 1e-9 at n=6); split at the planes
    it is exact at n=4.
B2  Determinism: a SHA-256 of the bit patterns of 4096 noise values, a golden a port must reproduce.
    Only +, -, *, floor and integer operations are used. (No pass criterion: it is a fixture.)
B3  The about-P split of a linear density, rho(s) = rho(P) + g.(s-P) (D'Urso 2014 Eq. 28,
    Pohanka 1998 Eq. 3/9), costs a cancellation of |g| R / rho_mean. Pass: the relative difference
    between the split and the direct sum grows no faster than ~ eps * |g| R / rho_mean * 10.
B4  Moments of a tetrahedron under an affine map, exact in rationals: for rest density
    rho0(X) = a + b.X and x = A X + t, the deformed body's mass, first and second moments equal
    the rest moments transported (m, A c + t m, A S A^T + ...), whatever det A. Pass: exact equality
    (Fraction arithmetic). Control arm: dropping the density's 1/det A in the direct integral
    breaks the equality.
B5  A sphere with density rho0 + k z has, outside it, exactly the field of a point mass plus a
    point dipole p_z = 4 pi k a^5 / 15. Pass: quadrature agrees to <= 1e-12 relative in g and G_zz.
B6  Gauss-Legendre remainder bound (DLMF 3.5.19 with 3.5.21's gamma_n, mapped to [x0, x0+h]) for
    rho = 1 + 0.5 sin(2 pi x / lambda): observed error <= bound in every case.
"""

from __future__ import annotations

import hashlib
import math
import struct
from fractions import Fraction
from itertools import product

EPS = 2.0**-52


# ---------------------------------------------------------------- Gauss-Legendre nodes (Newton)
def gauss_legendre(n: int) -> list[tuple[float, float]]:
    """Nodes and weights on [-1, 1]."""
    out: list[tuple[float, float]] = []
    for i in range(1, n + 1):
        x = math.cos(math.pi * (i - 0.25) / (n + 0.5))
        for _ in range(100):
            p0, p1 = 1.0, x
            for k in range(2, n + 1):
                p0, p1 = p1, ((2 * k - 1) * x * p1 - (k - 1) * p0) / k
            dp = n * (x * p1 - p0) / (x * x - 1.0) if n > 1 else 1.0
            dx = p1 / dp
            x -= dx
            if abs(dx) < 1e-17:
                break
        p0, p1 = 1.0, x
        for k in range(2, n + 1):
            p0, p1 = p1, ((2 * k - 1) * x * p1 - (k - 1) * p0) / k
        dp = n * (x * p1 - p0) / (x * x - 1.0) if n > 1 else 1.0
        out.append((x, 2.0 / ((1.0 - x * x) * dp * dp)))
    return out


def box_rule(lo: tuple[float, ...], hi: tuple[float, ...], n: int):
    gl = gauss_legendre(n)
    axes = []
    for a, b in zip(lo, hi):
        h = 0.5 * (b - a)
        m = 0.5 * (a + b)
        axes.append([(m + h * x, h * w) for x, w in gl])
    for pts in product(*axes):
        yield tuple(p[0] for p in pts), math.prod(p[1] for p in pts)


# ---------------------------------------------------------------- B1/B2 improved noise
GRADS = [(1, 1, 0), (-1, 1, 0), (1, -1, 0), (-1, -1, 0), (1, 0, 1), (-1, 0, 1), (1, 0, -1),
         (-1, 0, -1), (0, 1, 1), (0, -1, 1), (0, 1, -1), (0, -1, -1)]
MASK64 = (1 << 64) - 1


def hash3(i: int, j: int, k: int, seed: int = 0) -> int:
    """A 64-bit integer mix (splitmix64 finaliser) of the lattice point: integers only."""
    z = (i * 0x9E3779B97F4A7C15 + j * 0xC2B2AE3D27D4EB4F + k * 0x165667B19E3779F9 + seed) & MASK64
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK64
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK64
    return z ^ (z >> 31)


def fade(t: float) -> float:
    return t * t * t * (t * (t * 6.0 - 15.0) + 10.0)


def noise(x: float, y: float, z: float) -> float:
    """Perlin 2002 improved noise with the permutation table replaced by hash3 (mod 12)."""
    i, j, k = math.floor(x), math.floor(y), math.floor(z)
    u, v, w = x - i, y - j, z - k
    fu, fv, fw = fade(u), fade(v), fade(w)
    acc = 0.0
    for di, dj, dk in product((0, 1), repeat=3):
        g = GRADS[hash3(i + di, j + dj, k + dk) % 12]
        dot = g[0] * (u - di) + g[1] * (v - dj) + g[2] * (w - dk)
        wt = (fu if di else 1.0 - fu) * (fv if dj else 1.0 - fv) * (fw if dk else 1.0 - fw)
        acc += wt * dot
    return acc


def integrate(f, lo, hi, n):
    return math.fsum(w * f(*p) for p, w in box_rule(lo, hi, n))


def b1() -> None:
    print("B1 improved noise: Gauss-Legendre per axis on one lattice cell")
    lo, hi = (-3.0, 9.0, 2.0), (-2.0, 10.0, 3.0)  # a cell whose mass is not zero
    ref_m = integrate(noise, lo, hi, 10)
    ref_q = integrate(lambda x, y, z: noise(x, y, z) * (x + 2.5) ** 2, lo, hi, 10)
    print(f"  reference (n=10): mass {ref_m:+.17e}  x^2-moment {ref_q:+.17e}")
    for n in range(2, 8):
        m = integrate(noise, lo, hi, n)
        q = integrate(lambda x, y, z: noise(x, y, z) * (x + 2.5) ** 2, lo, hi, n)
        print(f"  aligned n={n}: |dmass| {abs(m - ref_m):.2e}  |dx2| {abs(q - ref_q):.2e}")
    print("  box straddling lattice planes, [3.37,4.37]x[-1.63,-0.63]x[5.37,6.37]:")
    lo2, hi2 = (3.37, -1.63, 5.37), (4.37, -0.63, 6.37)
    cuts = [(3.37, 4.0, 4.37), (-1.63, -1.0, -0.63), (5.37, 6.0, 6.37)]
    split = math.fsum(
        integrate(noise, (cuts[0][a], cuts[1][b], cuts[2][c]),
                  (cuts[0][a + 1], cuts[1][b + 1], cuts[2][c + 1]), 4)
        for a, b, c in product((0, 1), repeat=3))
    for n in (2, 4, 6, 8, 10):
        m = integrate(noise, lo2, hi2, n)
        print(f"  unsplit n={n:2d}: |m - split(n=4)| {abs(m - split):.2e}")
    split6 = math.fsum(
        integrate(noise, (cuts[0][a], cuts[1][b], cuts[2][c]),
                  (cuts[0][a + 1], cuts[1][b + 1], cuts[2][c + 1]), 6)
        for a, b, c in product((0, 1), repeat=3))
    print(f"  split n=4 vs split n=6: {abs(split - split6):.2e}")


def b2() -> None:
    h = hashlib.sha256()
    for a in range(4096):
        x = -37.25 + a * 0.0371
        y = 11.5 - a * 0.0213
        z = 1e3 + a * 0.0157
        h.update(struct.pack("<d", noise(x, y, z)))
    print("B2 determinism: sha256 of 4096 noise values (little-endian f64):")
    print("  " + h.hexdigest())


# ---------------------------------------------------------------- B3 about-P split cancellation
def b3() -> None:
    print("B3 linear density on a unit cube: direct vs about-P split (D'Urso 2014 Eq. 28 form)")
    rho0, grad = 2000.0, (200.0, 0.0, 0.0)  # kg/m^3 and kg/m^3/m: |g| s / rho = 0.1
    d = (1.0, 0.3, 0.2)
    nd = math.sqrt(sum(c * c for c in d))
    d = tuple(c / nd for c in d)
    pts = list(box_rule((-0.5,) * 3, (0.5,) * 3, 6))
    for R in (1.5, 10.0, 1e2, 1e3, 1e4, 1e5):
        p = tuple(R * c for c in d)
        rp = rho0 + sum(g * c for g, c in zip(grad, p))
        direct, split = [], []
        for comp in range(3):
            dv, s1, s2 = [], [], []
            for s, w in pts:
                r = tuple(si - pi for si, pi in zip(s, p))
                r2 = sum(c * c for c in r)
                k = w * r[comp] / (r2 * math.sqrt(r2))
                rho = rho0 + sum(g * c for g, c in zip(grad, s))
                dv.append(rho * k)
                s1.append(rp * k)
                s2.append(sum(g * c for g, c in zip(grad, r)) * k)
            direct.append(math.fsum(dv))
            split.append(math.fsum(s1) + math.fsum(s2))
        nrm = math.sqrt(sum(c * c for c in direct))
        err = math.sqrt(sum((a - b) ** 2 for a, b in zip(direct, split))) / nrm
        pred = EPS * 200.0 * R / rho0
        print(f"  R/s={R:8.1e}: rel diff {err:.2e}   eps*|g|R/rho {pred:.2e}")


# ---------------------------------------------------------------- B4 exact tetrahedron moments
def simplex_monomial(vs, alpha):
    """Exact integral of x^a y^b z^c over the tetrahedron vs (Fractions), by the barycentric
    formula: int prod(lambda_i^k_i) dV = 6 V prod(k_i!) / (sum k + 3)!, expanding x = sum lambda_i v_i."""
    v0, v1, v2, v3 = vs
    m = [[v1[i] - v0[i], v2[i] - v0[i], v3[i] - v0[i]] for i in range(3)]
    det = (m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
           - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
           + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]))
    vol = Fraction(abs(det), 6)
    # polynomial in barycentric lambdas: dict exponent-tuple -> coeff
    poly = {(0, 0, 0, 0): Fraction(1)}
    for axis, power in enumerate(alpha):
        for _ in range(power):
            new: dict[tuple[int, ...], Fraction] = {}
            for e, c in poly.items():
                for vi in range(4):
                    e2 = list(e)
                    e2[vi] += 1
                    key = tuple(e2)
                    new[key] = new.get(key, Fraction(0)) + c * vs[vi][axis]
            poly = new
    tot = Fraction(0)
    for e, c in poly.items():
        tot += c * 6 * vol * Fraction(math.prod(math.factorial(k) for k in e),
                                      math.factorial(sum(e) + 3))
    return tot, det


def b4() -> None:
    print("B4 tetrahedron under an affine map, exact rationals")
    F = Fraction
    rest = [(F(0), F(0), F(0)), (F(3, 2), F(0), F(1, 5)), (F(1, 3), F(2), F(0)),
            (F(1, 4), F(1, 7), F(5, 3))]
    a, b = F(1800), (F(40), F(-15), F(7))  # rho0(X) = a + b.X
    A = [[F(11, 10), F(1, 5), F(0)], [F(-1, 4), F(9, 10), F(1, 3)], [F(1, 6), F(0), F(3, 4)]]
    t = (F(5), F(-2), F(100))

    def rest_moment(alpha):  # int rho0(X) X^alpha dV
        tot = a * simplex_monomial(rest, alpha)[0]
        for ax in range(3):
            e = list(alpha)
            e[ax] += 1
            tot += b[ax] * simplex_monomial(rest, tuple(e))[0]
        return tot

    e = [(1, 0, 0), (0, 1, 0), (0, 0, 1)]
    m = rest_moment((0, 0, 0))
    c = [rest_moment(e[i]) for i in range(3)]
    S = [[rest_moment(tuple(e[i][k] + e[j][k] for k in range(3))) for j in range(3)]
         for i in range(3)]
    # transported: m' = m, c' = A c + t m, S' = A S A^T + (A c) t^T + t (A c)^T + m t t^T
    Ac = [sum(A[i][k] * c[k] for k in range(3)) for i in range(3)]
    ct = [Ac[i] + t[i] * m for i in range(3)]
    ASAt = [[sum(A[i][k] * S[k][l] * A[j][l] for k in range(3) for l in range(3)) for j in range(3)]
            for i in range(3)]
    St = [[ASAt[i][j] + Ac[i] * t[j] + t[i] * Ac[j] + m * t[i] * t[j] for j in range(3)]
          for i in range(3)]
    deformed = [tuple(sum(A[i][k] * v[k] for k in range(3)) + t[i] for i in range(3)) for v in rest]
    detA = (A[0][0] * (A[1][1] * A[2][2] - A[1][2] * A[2][1])
            - A[0][1] * (A[1][0] * A[2][2] - A[1][2] * A[2][0])
            + A[0][2] * (A[1][0] * A[2][1] - A[1][1] * A[2][0]))
    # inverse map X = A^-1 (x - t): density in the deformed frame rho0(X)/detA is linear in x
    inv = [[(A[(j + 1) % 3][(i + 1) % 3] * A[(j + 2) % 3][(i + 2) % 3]
             - A[(j + 1) % 3][(i + 2) % 3] * A[(j + 2) % 3][(i + 1) % 3]) / detA
            for j in range(3)] for i in range(3)]
    bx = [sum(b[k] * inv[k][i] for k in range(3)) for i in range(3)]
    ax0 = a - sum(bx[i] * t[i] for i in range(3))

    def deformed_moment(alpha, scale):
        tot = ax0 * simplex_monomial(deformed, alpha)[0]
        for ax in range(3):
            e2 = list(alpha)
            e2[ax] += 1
            tot += bx[ax] * simplex_monomial(deformed, tuple(e2))[0]
        return tot * scale

    for label, scale in (("with 1/det A", 1 / detA), ("control: without 1/det A", F(1))):
        md = deformed_moment((0, 0, 0), scale)
        cd = [deformed_moment(e[i], scale) for i in range(3)]
        Sd = [[deformed_moment(tuple(e[i][k] + e[j][k] for k in range(3)), scale)
               for j in range(3)] for i in range(3)]
        ok = md == m and cd == ct and Sd == St
        print(f"  {label}: mass equal {md == m}, first equal {cd == ct}, second equal {Sd == St}"
              f" -> {'exact' if ok else 'MISMATCH'}")
    # uniform tet: int x x^T = V/20 (sum v v^T + s s^T)
    vol = abs(simplex_monomial(rest, (0, 0, 0))[1]) / F(6)
    s = [sum(v[i] for v in rest) for i in range(3)]
    ok = all(simplex_monomial(rest, tuple(e[i][k] + e[j][k] for k in range(3)))[0]
             == vol / 20 * (sum(v[i] * v[j] for v in rest) + s[i] * s[j])
             for i in range(3) for j in range(3))
    print(f"  uniform tet second moment = V/20 (sum v v^T + s s^T): {ok}")
    print(f"  det A = {float(detA):.4f}; deformed volume / rest volume = {float(detA):.4f}")


# ---------------------------------------------------------------- B5 graded sphere oracle
def b5() -> None:
    print("B5 sphere, rho = rho0 + k z, radius a: quadrature vs point mass + point dipole")
    G, a, rho0, k = 6.674e-11, 2.0, 3000.0, 150.0
    M = 4.0 / 3.0 * math.pi * a**3 * rho0
    pz = 4.0 * math.pi * k * a**5 / 15.0
    nr, nt, nphi = 24, 48, 96
    glr, glt = gauss_legendre(nr), gauss_legendre(nt)
    for P in ((0.0, 0.0, 5.0), (3.0, 1.0, 2.5), (0.0, 4.0, -0.5)):
        gq = [[], [], []]
        gzz = []
        for xr, wr in glr:
            r = 0.5 * a * (xr + 1.0)
            for xt, wt in glt:
                ct, st = xt, math.sqrt(1.0 - xt * xt)
                for ip in range(nphi):
                    ph = 2.0 * math.pi * ip / nphi
                    s = (r * st * math.cos(ph), r * st * math.sin(ph), r * ct)
                    rho = rho0 + k * s[2]
                    dw = wr * 0.5 * a * wt * (2.0 * math.pi / nphi) * r * r * rho
                    d = (s[0] - P[0], s[1] - P[1], s[2] - P[2])
                    d2 = d[0] ** 2 + d[1] ** 2 + d[2] ** 2
                    dn = math.sqrt(d2)
                    for c in range(3):
                        gq[c].append(G * dw * d[c] / (d2 * dn))
                    gzz.append(G * dw * (3.0 * d[2] ** 2 - d2) / (d2 * d2 * dn))
        g_quad = [math.fsum(v) for v in gq]
        gzz_quad = math.fsum(gzz)
        # closed form: phi = -G M / r - G pz z / r^3 ; g = -grad phi
        x, y, z = P
        r2 = x * x + y * y + z * z
        r = math.sqrt(r2)
        g_cf = [-G * M * c / (r2 * r) for c in P]
        # dipole: phi_d = -G pz z / r^3 ; g = grad(G pz z / r^3)
        g_cf[0] += G * pz * (-3.0 * z * x / r**5)
        g_cf[1] += G * pz * (-3.0 * z * y / r**5)
        g_cf[2] += G * pz * (1.0 / r**3 - 3.0 * z * z / r**5)
        # G_zz = d g_z / dz
        gzz_cf = -G * M * (1.0 / r**3 - 3.0 * z * z / r**5)
        gzz_cf += G * pz * (-9.0 * z / r**5 + 15.0 * z**3 / r**7)
        eg = math.dist(g_quad, g_cf) / math.hypot(*g_cf)
        ez = abs(gzz_quad - gzz_cf) / abs(gzz_cf)
        print(f"  P={P}: rel err g {eg:.2e}  G_zz {ez:.2e}")


# ---------------------------------------------------------------- B6 GL remainder bound
def b6() -> None:
    print("B6 Gauss-Legendre error vs the DLMF 3.5.19 bound, rho = 1 + 0.5 sin(2 pi x / lam)")
    lam, x0 = 1.0, 0.123
    worst = 0.0
    for frac in (1.0, 0.5, 0.25, 0.125):
        h = frac * lam
        kw = 2.0 * math.pi / lam
        exact = h + 0.5 / kw * (math.cos(kw * x0) - math.cos(kw * (x0 + h)))
        row = []
        for n in (1, 2, 3, 4):
            est = math.fsum(0.5 * h * w * (1.0 + 0.5 * math.sin(kw * (x0 + 0.5 * h * (x + 1.0))))
                            for x, w in gauss_legendre(n))
            err = abs(est - exact)
            bound = (h ** (2 * n + 1) * math.factorial(n) ** 4
                     / ((2 * n + 1) * math.factorial(2 * n) ** 3) * 0.5 * kw ** (2 * n))
            worst = max(worst, err / bound)
            row.append(f"n={n}: {err:.1e}/{bound:.1e}")
        print(f"  h/lam={frac:5.3f}  " + "  ".join(row))
    print(f"  max observed/bound = {worst:.3f} (pass if <= 1)")


if __name__ == "__main__":
    b1()
    b2()
    b3()
    b4()
    b5()
    b6()
