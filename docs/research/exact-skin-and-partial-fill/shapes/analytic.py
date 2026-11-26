"""The analytic sphere and torus: exact references, exact cell classification and exact fill moments.

    Ball(radius)              centred on the origin
    Torus(big_r, small_r)     axis z, centred on the origin

Each shape gives, with no mesh anywhere:
    g_ref(pts)        the attraction per unit G·ρ (m): the ball's closed form (shell theorem: a point
                      mass outside); the torus as a sum of thin coaxial rings, each in closed form
                      with complete elliptic integrals, over its circular cross-section by product
                      Gauss–Legendre × trapezoid quadrature (spectrally convergent for points outside)
    classify(lo, h)   for cells [lo, lo+h]³: wholly inside, wholly outside, or crossed, exactly
                      (the extremes of the defining function over a box are found in closed form)
    moments(lo, h)    for crossed cells: volume, centroid and central second moment of cell ∩ shape,
                      to ~1e-14 relative, by integrating the z-chord exactly and the (x, y) square
                      with Gauss–Legendre on pieces split at every kink and with a cosine map at each
                      piece's ends (which absorbs the square-root behaviour at a silhouette)

Both shapes are "inside iff z² < c(x, y)", with c(x, y) a function of ρ = √(x² + y²) alone:
ball c = R² − ρ², torus c = r² − (ρ − R)². For fixed (x, y) the inside part of the cell's z range is
[−√c, √c] ∩ [z0, z1], so its length and z-moments are exact. The integrand over (x, y) has kinks
only where c ∈ {0, z0², z1²}, which are circles ρ = ρ_j about the axis: split there.
"""

from __future__ import annotations

import math
from dataclasses import dataclass

import numpy as np
import numpy.typing as npt
from scipy.special import ellipe, ellipk

F64 = npt.NDArray[np.float64]

_GL_N = 24
_GL_X, _GL_W = np.polynomial.legendre.leggauss(_GL_N)
_T = 0.5 * (_GL_X + 1.0)  # nodes on [0, 1]
# Cosine map u = (1 − cos πt)/2 on [0, 1]: du = (π/2) sin πt dt; kills √ end behaviour.
_U = 0.5 * (1.0 - np.cos(np.pi * _T))
_WU = 0.5 * _GL_W * (0.5 * np.pi * np.sin(np.pi * _T))


@dataclass(frozen=True)
class Ball:
    radius: float = 1.0
    name:   str = "sphere"

    @property
    def volume(self) -> float:
        return 4.0 / 3.0 * math.pi * self.radius**3

    def c_of_rho(self, rho: F64) -> F64:
        return self.radius**2 - rho**2

    def radii(self, kappas: list[float]) -> list[float]:
        """Radii ρ where c = κ."""
        return [math.sqrt(self.radius**2 - k) for k in kappas if k <= self.radius**2]

    def classify(self, lo: F64, h: float) -> tuple[npt.NDArray[np.bool_], npt.NDArray[np.bool_]]:
        """(inside, outside) for cells with lower corners `lo` (N, 3)."""
        hi = lo + h
        near = np.clip(0.0, lo, hi)
        far = np.maximum(np.abs(lo), np.abs(hi))
        dmin = np.linalg.norm(near, axis=1)
        dmax = np.linalg.norm(far, axis=1)
        return dmax <= self.radius, dmin >= self.radius

    def inside(self, p: F64) -> npt.NDArray[np.bool_]:
        return np.linalg.norm(p, axis=-1) < self.radius

    def g_ref(self, pts: F64) -> F64:
        """Closed form: outside, the whole mass at the centre (shell theorem). Points are outside."""
        r = np.linalg.norm(pts, axis=1)
        assert np.all(r > self.radius), "g_ref is for points outside the ball"
        return -self.volume * pts / r[:, None] ** 3


@dataclass(frozen=True)
class Torus:
    big_r:   float = 1.0
    small_r: float = 0.4
    name:    str = "torus"

    @property
    def volume(self) -> float:
        return 2.0 * math.pi**2 * self.big_r * self.small_r**2

    def c_of_rho(self, rho: F64) -> F64:
        return self.small_r**2 - (rho - self.big_r) ** 2

    def radii(self, kappas: list[float]) -> list[float]:
        out = []
        for k in kappas:
            if k <= self.small_r**2:
                d = math.sqrt(self.small_r**2 - k)
                out += [self.big_r - d, self.big_r + d]
        return [x for x in out if x >= 0]

    def classify(self, lo: F64, h: float) -> tuple[npt.NDArray[np.bool_], npt.NDArray[np.bool_]]:
        hi = lo + h
        nx = np.clip(0.0, lo[:, 0], hi[:, 0])
        ny = np.clip(0.0, lo[:, 1], hi[:, 1])
        rmin = np.hypot(nx, ny)
        rmax = np.hypot(np.maximum(np.abs(lo[:, 0]), np.abs(hi[:, 0])),
                        np.maximum(np.abs(lo[:, 1]), np.abs(hi[:, 1])))
        zmin2 = np.clip(0.0, lo[:, 2], hi[:, 2]) ** 2
        zmax2 = np.maximum(lo[:, 2] ** 2, hi[:, 2] ** 2)
        fmin = (np.clip(self.big_r, rmin, rmax) - self.big_r) ** 2 + zmin2
        fmax = np.maximum((rmin - self.big_r) ** 2, (rmax - self.big_r) ** 2) + zmax2
        return fmax <= self.small_r**2, fmin >= self.small_r**2

    def inside(self, p: F64) -> npt.NDArray[np.bool_]:
        rho = np.hypot(p[..., 0], p[..., 1])
        return (rho - self.big_r) ** 2 + p[..., 2] ** 2 < self.small_r**2

    def g_ref(self, pts: F64, ns: int = 40, nt: int = 96) -> F64:
        """Sum of thin rings over the cross-section (s, t): ring radius a = R + s cos t, height
        z0 = s sin t, mass 2πa·s ds dt (per unit ρ). Each ring's field in closed form."""
        x, w = np.polynomial.legendre.leggauss(ns)
        s = 0.5 * self.small_r * (x + 1)
        ws = 0.5 * self.small_r * w
        t = 2 * np.pi * np.arange(nt) / nt
        ss, tt = np.meshgrid(s, t, indexing="ij")
        a = (self.big_r + ss * np.cos(tt)).ravel()
        z0 = (ss * np.sin(tt)).ravel()
        mass = (2 * np.pi * a * ss.ravel() * np.repeat(ws, nt) * (2 * np.pi / nt))
        return ring_g(a, z0, mass, pts)


def ring_g(a: F64, z0: F64, mass: F64, pts: F64) -> F64:
    """g per unit G (m s⁻² / G) of thin rings (radius a, height z0, axis z, mass `mass`) at `pts`.

    U = (2M/π) K(m)/√Q with Q = (ρ + a)² + (z − z0)², m = 4aρ/Q; g = ∇U, from dK/dm =
    (E − (1 − m)K)/(2m(1 − m)).
    """
    out = np.zeros((len(pts), 3))
    for i, p in enumerate(pts):
        rho = math.hypot(p[0], p[1])
        dz = p[2] - z0
        q = (rho + a) ** 2 + dz**2
        m = 4 * a * rho / q
        k, e = ellipk(m), ellipe(m)
        sq = np.sqrt(q)
        with np.errstate(invalid="ignore", divide="ignore"):
            dk = np.where(m > 1e-12, (e - (1 - m) * k) / (2 * m * (1 - m)), np.pi / 8)
        dm_dz = -m * 2 * dz / q
        dm_dr = 4 * a / q - m * 2 * (rho + a) / q
        du_dz = (2 * mass / np.pi) * (dk * dm_dz / sq - k * dz / q**1.5)
        du_dr = (2 * mass / np.pi) * (dk * dm_dr / sq - k * (rho + a) / q**1.5)
        gr = du_dr.sum()
        out[i, 2] = du_dz.sum()
        if rho > 0:
            out[i, 0] = gr * p[0] / rho
            out[i, 1] = gr * p[1] / rho
    # U here is ∫ρ/|x − p|, so ∇_p U points at the mass: the sign convention of shapes_lib.
    return out


Shape = Ball | Torus


def _pieces(a: float, b: float, cuts: F64) -> F64:
    """Sorted break points in [a, b] including the ends."""
    c = cuts[(cuts > a) & (cuts < b)]
    return np.concatenate([[a], np.sort(c), [b]])


def cell_moments(shape: Shape, lo: F64, h: float) -> F64:
    """Raw moments of cell ∩ shape: [V, ∫x, ∫y, ∫z, ∫x², ∫y², ∫z², ∫xy, ∫xz, ∫yz] (m³ … m⁵)."""
    x0, y0, z0 = lo
    x1, y1, z1 = lo + h
    kappas = [0.0, z0 * z0, z1 * z1]
    rad = np.array(shape.radii(kappas))
    # x break points: ±ρ_j and ±√(ρ_j² − y_k²) for y_k in {y0, y1}.
    xc = [rad, -rad]
    for yk in (y0, y1):
        d = rad**2 - yk * yk
        r = np.sqrt(d[d > 0])
        xc += [r, -r]
    xb = _pieces(x0, x1, np.concatenate(xc))
    xa, xbb = xb[:-1], xb[1:]
    xs = (xa[:, None] + (xbb - xa)[:, None] * _U[None]).ravel()
    wx = ((xbb - xa)[:, None] * _WU[None]).ravel()
    # y break points for each x node: ±√(ρ_j² − x²); padded to a fixed count, then sorted.
    d = rad[None, :] ** 2 - xs[:, None] ** 2
    yr = np.sqrt(np.maximum(d, 0.0))
    cand = np.concatenate([yr, -yr], axis=1)
    cand = np.where(np.concatenate([d, d], axis=1) > 0, cand, y0)
    cand = np.clip(cand, y0, y1)
    yb = np.sort(np.concatenate([np.full((len(xs), 1), y0), cand, np.full((len(xs), 1), y1)], axis=1), axis=1)
    ya, ybb = yb[:, :-1], yb[:, 1:]
    ys = ya[..., None] + (ybb - ya)[..., None] * _U  # (nx, np, nq)
    wy = (ybb - ya)[..., None] * _WU
    xx = np.broadcast_to(xs[:, None, None], ys.shape)
    c = shape.c_of_rho(np.hypot(xx, ys))
    sq = np.sqrt(np.maximum(c, 0.0))
    a = np.maximum(z0, -sq)
    b = np.minimum(z1, sq)
    ok = (c > 0) & (b > a)
    length = np.where(ok, b - a, 0.0)
    m1 = np.where(ok, 0.5 * (b * b - a * a), 0.0)
    m2 = np.where(ok, (b**3 - a**3) / 3.0, 0.0)
    w = wx[:, None, None] * wy
    f = [length, xx * length, ys * length, m1, xx * xx * length, ys * ys * length, m2, xx * ys * length,
         xx * m1, ys * m1]
    return np.array([(w * q).sum() for q in f])


def analytic_voxels(shape: Shape, h: float, origin: F64) -> dict[str, F64]:
    """Interior cells and crossed cells with exact fill, centroid and central second moment."""
    r = shape.radius if isinstance(shape, Ball) else shape.big_r + shape.small_r
    zr = shape.radius if isinstance(shape, Ball) else shape.small_r
    lo_i = np.floor((np.array([-r, -r, -zr]) - origin) / h).astype(int) - 1
    hi_i = np.floor((np.array([r, r, zr]) - origin) / h).astype(int) + 1
    axes = [np.arange(lo_i[k], hi_i[k] + 1) for k in range(3)]
    ijk = np.stack(np.meshgrid(*axes, indexing="ij"), -1).reshape(-1, 3)
    lo = origin + ijk * h
    inside, outside = shape.classify(lo, h)
    crossed = ~inside & ~outside
    mom = np.array([cell_moments(shape, p, h) for p in lo[crossed]])
    vol = mom[:, 0]
    keep = vol > 1e-15 * h**3
    mom, clo = mom[keep], lo[crossed][keep]
    vol = mom[:, 0]
    cen = mom[:, 1:4] / vol[:, None]
    sec = np.zeros((len(vol), 3, 3))
    idx = {(0, 0): 4, (1, 1): 5, (2, 2): 6, (0, 1): 7, (0, 2): 8, (1, 2): 9}
    for (i, j), k in idx.items():
        sec[:, i, j] = mom[:, k] - vol * cen[:, i] * cen[:, j]
        sec[:, j, i] = sec[:, i, j]
    centres = clo + 0.5 * h
    return {"interior": lo[inside] + 0.5 * h, "centres": centres, "frac": vol / h**3, "centroid": cen,
            "second": sec, "ctr_in": shape.inside(centres).astype(float)}
