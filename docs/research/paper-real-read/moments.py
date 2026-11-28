"""Exact raw moments of each clipped piece to order 5, and its l = 3, 4, 5 multipole coefficients.

    raw = piece_moments(vox)          # [C, 56]: ∫ s^β dV over each piece, s = x − centroid, |β| ≤ 5
    coef = multipole_coefficients(raw)  # [C, 46]: N_3, N_4, N_5 as polynomials in R (below)

A piece is a closed triangle soup (the voxeliser's `piece_tris`, `piece_of`). Each triangle and the
piece's centroid span a signed tetrahedron; the moments are summed over them with a collapsed
(Duffy) Gauss–Legendre rule of 4 × 4 × 3 points, exact for every polynomial of degree ≤ 5.

The potential of a piece about its centroid, R = point − centroid (U = ∫dV/|R − s|, positive), is
Σ_l ∫ s^l R^l P_l(ŝ·R̂) dV / R^(2l+1). For l = 3, 4, 5 the numerators are homogeneous polynomials in R:

    N_3 = ∫ 5(s·R)³ − 3(s·R) s² R²                           U_3 = N_3 / (2 R⁷)
    N_4 = ∫ 35(s·R)⁴ − 30(s·R)² s² R² + 3 s⁴ R⁴              U_4 = N_4 / (8 R⁹)
    N_5 = ∫ 63(s·R)⁵ − 70(s·R)³ s² R² + 15 (s·R) s⁴ R⁴       U_5 = N_5 / (8 R¹¹)

`multipole_coefficients` gives their coefficients on the monomials R^α in `MONOMIALS[l]` order
(the Rust kernel's order: a from l down to 0, then b from l − a down to 0).
"""

from __future__ import annotations

import math
from collections import defaultdict

import numpy as np
import numpy.typing as npt

F64 = npt.NDArray[np.float64]
Exp = tuple[int, int, int]


def monomials(n: int) -> list[Exp]:
    return [(a, b, n - a - b) for a in range(n, -1, -1) for b in range(n - a, -1, -1)]


MONOMIALS = {n: monomials(n) for n in range(6)}
ALL = [e for n in range(6) for e in MONOMIALS[n]]  # 56 raw-moment exponents, degree 0..5
INDEX = {e: k for k, e in enumerate(ALL)}


def _rule() -> tuple[F64, F64]:
    """Points (x1, x2, x3) and weights on the unit tetrahedron, exact to degree 5."""
    xu, wu = np.polynomial.legendre.leggauss(4)
    xv, wv = np.polynomial.legendre.leggauss(4)
    xw, ww = np.polynomial.legendre.leggauss(3)
    u, v, w = (0.5 * (x + 1) for x in (xu, xv, xw))
    U, V, W = np.meshgrid(u, v, w, indexing = "ij")
    WT = np.einsum("i,j,k->ijk", wu, wv, ww) / 8.0 * (1 - U) ** 2 * (1 - V)
    x1, x2, x3 = U, (1 - U) * V, (1 - U) * (1 - V) * W
    return np.stack([x1.ravel(), x2.ravel(), x3.ravel()], axis = 1), WT.ravel()


RULE_X, RULE_W = _rule()


def piece_moments(piece_tris: F64, piece_of: npt.NDArray[np.int64], centroid: F64, chunk: int = 40_000) -> F64:
    """Raw moments ∫ s^β dV (m^(3+|β|)) of every piece about its centroid, [C, 56] in `ALL` order."""
    n_cells = len(centroid)
    out = np.zeros((n_cells, len(ALL)))
    for s0 in range(0, len(piece_tris), chunk):
        t = piece_tris[s0 : s0 + chunk]
        own = piece_of[s0 : s0 + chunk]
        c = centroid[own]
        a, b, cc = t[:, 0] - c, t[:, 1] - c, t[:, 2] - c
        det = np.einsum("nk,nk->n", a, np.cross(b, cc))  # 6 × signed volume
        pts = (RULE_X[None, :, 0, None] * a[:, None] + RULE_X[None, :, 1, None] * b[:, None]
               + RULE_X[None, :, 2, None] * cc[:, None])  # [n, q, 3]
        wq = det[:, None] * RULE_W[None, :]
        powers = []
        for ax in range(3):
            pa = [np.ones_like(pts[..., 0])]
            for _ in range(5):
                pa.append(pa[-1] * pts[..., ax])
            powers.append(pa)
        cols = np.stack([(wq * powers[0][e[0]] * powers[1][e[1]] * powers[2][e[2]]).sum(axis = 1) for e in ALL],
                        axis = 1)  # [n, 56]
        np.add.at(out, own, cols)
    return out


# ---- the N_l numerators as linear maps of the raw moments of degree l ----


def _poly_mul(p: dict, q: dict) -> dict:
    r: dict = defaultdict(float)
    for (sa, ra), x in p.items():
        for (sb, rb), y in q.items():
            r[(tuple(i + j for i, j in zip(sa, sb)), tuple(i + j for i, j in zip(ra, rb)))] += x * y
    return dict(r)


def _pow(p: dict, n: int) -> dict:
    out = {((0, 0, 0), (0, 0, 0)): 1.0}
    for _ in range(n):
        out = _poly_mul(out, p)
    return out


def _numerator_map(l: int) -> F64:
    """T [monomials(l) in R, monomials(l) in s]: N_l coefficients = T @ (raw moments of degree l)."""
    e = [(1, 0, 0), (0, 1, 0), (0, 0, 1)]
    sr = {(e[i], e[i]): 1.0 for i in range(3)}  # s·R
    s2 = {(tuple(2 * x for x in e[i]), (0, 0, 0)): 1.0 for i in range(3)}
    r2 = {((0, 0, 0), tuple(2 * x for x in e[i])): 1.0 for i in range(3)}
    a, b, c = {3: (5, -3, 0), 4: (35, -30, 3), 5: (63, -70, 15)}[l]
    total: dict = defaultdict(float)
    for coef, k in ((a, 0), (b, 1), (c, 2)):
        if coef == 0 or l - 2 * k < 0:
            continue
        term = _poly_mul(_pow(sr, l - 2 * k), _poly_mul(_pow(s2, k), _pow(r2, k)))
        for key, x in term.items():
            total[key] += coef * x
    rows, cols = MONOMIALS[l], MONOMIALS[l]
    t = np.zeros((len(rows), len(cols)))
    for (se, re), x in total.items():
        t[rows.index(re), cols.index(se)] += x
    return t


MAPS = {l: _numerator_map(l) for l in (3, 4, 5)}
DENOM = {3: 2.0, 4: 8.0, 5: 8.0}


def multipole_coefficients(raw: F64) -> F64:
    """[C, 46]: N_3 (10), N_4 (15), N_5 (21) coefficients, each divided by its constant 2, 8, 8."""
    parts = []
    for l in (3, 4, 5):
        idx = [INDEX[e] for e in MONOMIALS[l]]
        parts.append(raw[:, idx] @ MAPS[l].T / DENOM[l])
    return np.concatenate(parts, axis = 1)


def u_direct(raw: F64, centroid: F64, pts: F64) -> F64:
    """Slow numpy evaluation of U_3 + U_4 + U_5 (for tests), [M, 3] per l."""
    coef = multipole_coefficients(raw)
    out = np.zeros((len(pts), 3))
    off = 0
    for k, l in enumerate((3, 4, 5)):
        mons = MONOMIALS[l]
        for p_i, p in enumerate(pts):
            r = p - centroid  # [C, 3]
            rl = np.linalg.norm(r, axis = 1)
            mono = np.stack([r[:, 0] ** e[0] * r[:, 1] ** e[1] * r[:, 2] ** e[2] for e in mons], axis = 1)
            out[p_i, k] = np.sum((coef[:, off : off + len(mons)] * mono).sum(axis = 1) / rl ** (2 * l + 1))
        off += len(mons)
    return out


def check_rule() -> float:
    """The rule against the exact barycentric formula on random tetrahedra, every monomial ≤ 5."""
    rng = np.random.default_rng(3)
    worst = 0.0
    for _ in range(5):
        a, b, c = rng.normal(size = (3, 3))
        det = float(np.dot(a, np.cross(b, c)))
        pts = RULE_X[:, 0, None] * a + RULE_X[:, 1, None] * b + RULE_X[:, 2, None] * c
        for e in ALL:
            got = det * np.sum(RULE_W * np.prod(pts**np.array(e), axis = 1))
            # exact: expand s^e with s = x1 a + x2 b + x3 c, ∫ x1^i x2^j x3^k = i! j! k! / (i+j+k+3)!
            ref = 0.0
            n = sum(e)
            vecs = [a, b, c]
            # multinomial expansion over which vertex each factor takes
            factors = [ax for ax in range(3) for _ in range(e[ax])]
            for choice in np.ndindex(*([3] * n)) if n else [()]:
                cnt = [0, 0, 0]
                val = 1.0
                for f, v in zip(factors, choice):
                    val *= vecs[v][f]
                    cnt[v] += 1
                ref += val * math.factorial(cnt[0]) * math.factorial(cnt[1]) * math.factorial(cnt[2]) / math.factorial(n + 3)
            ref *= det
            worst = max(worst, abs(got - ref) / max(abs(ref), 1e-300) if abs(ref) > 1e-12 else abs(got - ref))
    return worst
