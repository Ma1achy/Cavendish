"""B10: the exact free asymmetric top of van Zon & Schofield 2007 (J. Comput. Phys. 225, 145-164,
doi:10.1016/j.jcp.2006.11.019), Sec. 3.3 and the pseudo-code of Sec. 4.2, checked against B2's case
and reference, against a 4th-order splitting at small steps, and towards the separatrix 2E = L^2/I_2.

Conventions as the paper: A is the attitude matrix, lab -> body (A = R^T for B2's body -> world R).
Pass criteria (written before running):
  (a) B2 case, T = 20 s: exact vs RK4 reference (h = 1/8192) <= 1e-8 rad (the reference floor is ~3e-9);
  (b) exact vs Yoshida-4 splitting at h = 1/1920: <= 1e-9 rad;
  (c) |L_world| drift and E drift of the exact solution <= 1e-13 relative over 1e4 s;
  (d) separatrix: report the attitude error vs Yoshida-4 at h = 1/1920, T = 20 s, as
      delta = (2E - L^2/I2)/(L^2/I2) -> 1e-12, and the series lengths it needs; no criterion, a measurement.
"""
import time

import numpy as np
from scipy.special import ellipj, ellipk, ellipkinc, ellipkm1

EPS = np.finfo(float).eps


def F(x, m):
    """Incomplete elliptic integral of the first kind in the paper's notation F(x|m), x = sin(phi)."""
    return ellipkinc(np.arcsin(np.clip(x, -1.0, 1.0)), m)


def K_of(m):
    return ellipkm1(1.0 - m) if m > 0.5 else ellipk(m)


class ExactRotor:
    """Initialisation and evolution routines of Sec. 4.2 (asymmetric top, Iy the middle moment)."""

    def __init__(self, Ix, Iy, Iz, w0, A0):
        w0 = np.asarray(w0, float)
        L2 = Ix * Ix * w0[0] ** 2 + Iy * Iy * w0[1] ** 2 + Iz * Iz * w0[2] ** 2
        E2 = Ix * w0[0] ** 2 + Iy * w0[1] ** 2 + Iz * w0[2] ** 2
        self.order = (E2 > L2 / Iy and Ix < Iz) or (E2 < L2 / Iy and Ix > Iz)
        U = np.array([[0.0, 0, 1], [0, -1, 0], [1, 0, 0]]) if self.order else np.eye(3)
        self.U = U
        if self.order:
            I1, I2, I3 = Iz, Iy, Ix
            w10, w20, w30 = w0[2], -w0[1], w0[0]
        else:
            I1, I2, I3 = Ix, Iy, Iz
            w10, w20, w30 = w0
        self.I = (I1, I2, I3)
        L = np.sqrt(L2)
        self.L = L
        s1 = np.sign(w10) if w10 != 0 else 1.0
        s3 = np.sign(w30) if w30 != 0 else 1.0
        self.w1m = s1 * np.sqrt((L2 - E2 * I3) / (I1 * (I1 - I3)))
        self.w2m = -s1 * np.sqrt((L2 - E2 * I3) / (I2 * (I2 - I3)))
        self.w3m = s3 * np.sqrt((L2 - E2 * I1) / (I3 * (I3 - I1)))
        self.wp = np.sign(I2 - I3) * s3 * np.sqrt((L2 - E2 * I1) * (I3 - I2) / (I1 * I2 * I3))
        m = (L2 - E2 * I3) * (I1 - I2) / ((L2 - E2 * I1) * (I3 - I2))
        self.m = m
        self.eps = F(w20 / self.w2m, m)
        K = K_of(m)
        Kp = K_of(1.0 - m)
        self.K = K
        q = np.exp(-np.pi * Kp / K)
        self.q = q
        eta = s3 * Kp - F(I3 * self.w3m / L, 1.0 - m)
        xi = np.exp(np.pi * eta / K)
        A2 = L / I1 + np.pi * self.wp * (xi + 1.0) / (2.0 * K * (xi - 1.0))
        n = 1
        while True:
            dA2 = -(np.pi * self.wp / K) * (q ** (2 * n) / (1.0 - q ** (2 * n))) * (xi**n - xi ** (-n))
            A2 += dA2
            n += 1
            if abs(dA2) < EPS * abs(A2) or n > 100000:
                break
        self.nA2 = n
        self.A2 = A2
        NT = int(np.ceil(np.log(EPS) / np.log(q))) if q > 0 else 1
        NT = max(NT, 1)
        self.NT = NT
        k = np.arange(NT + 1)
        pref = 2.0 * q ** (k * (k + 1) + 0.25)
        self.cr = (-1.0) ** k * pref * np.cosh((2 * k + 1) * np.pi * eta / (2 * K))
        self.ci = (-1.0) ** (k + 1) * pref * np.sinh((2 * k + 1) * np.pi * eta / (2 * K))
        self.k2 = 2 * k + 1
        r0, i0 = self._theta(self.eps)
        self.A1 = np.arctan2(i0, r0)
        Ab0 = self._T1(np.array([w10, w20, w30]))
        self.B = Ab0.T @ U @ A0

    def _theta(self, u):
        arg = self.k2 * np.pi * u / (2.0 * self.K)
        return np.dot(self.cr, np.sin(arg)), np.dot(self.ci, np.cos(arg))

    def _T1(self, w):
        I1, I2, I3 = self.I
        Lb = np.array([I1 * w[0], I2 * w[1], I3 * w[2]])
        L = self.L
        Lp = np.hypot(Lb[0], Lb[1])
        e1 = np.array([Lb[0] * Lb[2] / (L * Lp), Lb[1] * Lb[2] / (L * Lp), -Lp / L])
        e2 = np.array([-Lb[1] / Lp, Lb[0] / Lp, 0.0])
        return np.column_stack([e1, e2, Lb / L])

    def at(self, t):
        u = self.wp * t + self.eps
        sn, cn, dn, _ = ellipj(u, self.m)
        w = np.array([self.w1m * cn, self.w2m * sn, self.w3m * dn])
        re, im = self._theta(u)
        r = np.hypot(re, im)
        C, S = np.cos(self.A1 + self.A2 * t), np.sin(self.A1 + self.A2 * t)
        c, s = (C * re + S * im) / r, (S * re - C * im) / r
        T2 = np.array([[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]])
        A = self.U @ self._T1(w) @ T2 @ self.B
        wphys = np.array([w[2], -w[1], w[0]]) if self.order else w
        return wphys, A


# ---- B2's integrators, copied from b2_rotation.py (state R body->world, Lb body momentum) ----
I = np.array([1.0, 2.0, 3.0])


def rot(axis, a):
    c, s = np.cos(a), np.sin(a)
    if axis == 0:
        return np.array([[1, 0, 0], [0, c, -s], [0, s, c]])
    if axis == 1:
        return np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]])
    return np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]])


def skew(w):
    return np.array([[0, -w[2], w[1]], [w[2], 0, -w[0]], [-w[1], w[0], 0]])


def expm_skew(w):
    th = np.linalg.norm(w)
    if th < 1e-300:
        return np.eye(3)
    K = skew(w / th)
    return np.eye(3) + np.sin(th) * K + (1 - np.cos(th)) * K @ K


def splitting(R, Lb, h):
    for k, f in ((0, 0.5), (1, 0.5), (2, 1.0), (1, 0.5), (0, 0.5)):
        Rk = rot(k, Lb[k] / I[k] * h * f)
        R, Lb = R @ Rk, Rk.T @ Lb
    return R, Lb


w1 = 1 / (2 - 2 ** (1 / 3))
w0_ = -(2 ** (1 / 3)) * w1


def yoshida(R, Lb, h):
    for w in (w1, w0_, w1):
        R, Lb = splitting(R, Lb, w * h)
    return R, Lb


def rk4_ref(R, Lb, h, n):
    def f(Lb):
        return np.cross(Lb, Lb / I)

    for _ in range(n):
        k1 = f(Lb)
        k2 = f(Lb + 0.5 * h * k1)
        k3 = f(Lb + 0.5 * h * k2)
        k4 = f(Lb + h * k3)
        Lb2 = Lb + h / 6 * (k1 + 2 * k2 + 2 * k3 + k4)
        R = R @ expm_skew(((Lb / I) + (Lb2 / I)) * 0.5 * h)
        Lb = Lb2
    return R, Lb


def angle(Ra, Rb):
    M = Ra.T @ Rb
    sv = np.array([M[2, 1] - M[1, 2], M[0, 2] - M[2, 0], M[1, 0] - M[0, 1]]) / 2
    return np.arctan2(np.linalg.norm(sv), (np.trace(M) - 1) / 2)


def run_yoshida(Lb0, h, T):
    R, Lb = np.eye(3), Lb0.copy()
    for _ in range(int(round(T / h))):
        R, Lb = yoshida(R, Lb, h)
    return R


T = 20.0
Lb0 = np.array([0.05, 2.0, 0.04])
ex = ExactRotor(1.0, 2.0, 3.0, Lb0 / I, np.eye(3))
print(f"B2 case: m = {ex.m:.6f}, q = {ex.q:.4f}, theta terms NT = {ex.NT}, A2 series terms = {ex.nA2}, order flag = {ex.order}")
w, A = ex.at(T)
Rex = A.T
Rr, Lr = rk4_ref(np.eye(3), Lb0, 1 / 8192, int(T / (1 / 8192)))
print(f"(a) exact vs RK4 reference (h=1/8192) at 20 s: {angle(Rex, Rr):.2e} rad; body L diff {np.linalg.norm(I * w - Lr):.1e}")
for h in (1 / 240, 1 / 960, 1 / 1920):
    print(f"(b) exact vs Yoshida-4 h=1/{round(1 / h)} at 20 s: {angle(Rex, run_yoshida(Lb0, h, T)):.2e} rad")
# (c) conservation over long times
Lw0 = (np.eye(3).T @ (I * (Lb0 / I)))
E0 = 0.5 * np.sum(Lb0 * Lb0 / I)
dL = dE = orth = 0.0
for t in np.linspace(0, 1e4, 2001)[1:]:
    w, A = ex.at(t)
    Lw = A.T @ (I * w)
    dL = max(dL, np.linalg.norm(Lw - Lw0) / np.linalg.norm(Lw0))
    dE = max(dE, abs(0.5 * np.sum(I * w * w) - E0) / E0)
    orth = max(orth, np.linalg.norm(A @ A.T - np.eye(3)))
print(f"(c) over 1e4 s (2000 samples): max |L_w - L_w0|/|L| = {dL:.1e}, max |dE/E| = {dE:.1e}, max ||A A^T - 1|| = {orth:.1e}")
# finite-difference check of the attitude ODE dA/dt = -W(w) A (2.12) at a few times
res = 0.0
for t in (0.3, 7.7, 19.1):
    hh = 1e-5
    _, Ap = ex.at(t + hh)
    _, Am = ex.at(t - hh)
    w, A = ex.at(t)
    res = max(res, np.linalg.norm((Ap - Am) / (2 * hh) + skew(w) @ A))
print(f"    residual of dA/dt + W(w)A by central difference (h=1e-5): {res:.1e}")
# cost
t0 = time.perf_counter()
for t in np.linspace(0, 20, 20000):
    ex.at(t)
print(f"    cost: {1e6 * (time.perf_counter() - t0) / 20000:.1f} us per evaluation in Python/numpy (not representative of Rust)")
# (d) towards the separatrix: perturb the middle-axis spin so delta -> small
print("(d) towards the separatrix 2E = L^2/I2 (T = 20 s, reference Yoshida-4 h=1/1920):")
for delta_target in (1e-2, 1e-4, 1e-6, 1e-8, 1e-10, 1e-12):
    # choose Lb = (a, 2, a*s) with E2 - L2/I2 = delta*L2/I2 approx: a^2(1-1/2) - (a s)^2 (1/2 - 1/3) ... pick s=1, solve for a
    # 2E - L^2/I2 = a^2 (1/1 - 1/2) + (a s)^2 (1/3 - 1/2) = a^2 (1/2 - s^2/6); with s=1: a^2/3.
    a = np.sqrt(3 * delta_target * (4.0 / 2.0))
    Lb = np.array([a, 2.0, a])
    L2 = np.sum(Lb * Lb)
    delta = (np.sum(Lb * Lb / I) - L2 / 2) / (L2 / 2)
    try:
        exs = ExactRotor(1.0, 2.0, 3.0, Lb / I, np.eye(3))
        t0 = time.perf_counter()
        _, A = exs.at(T)
        dt_us = 1e6 * (time.perf_counter() - t0)
        err = angle(A.T, run_yoshida(Lb, 1 / 1920, T))
        print(f"    delta={delta:.1e}: m=1-{1 - exs.m:.1e}, K={exs.K:.2f}, q={exs.q:.6f}, NT={exs.NT}, A2 terms={exs.nA2}, "
              f"err vs Yoshida={err:.2e} rad, one eval {dt_us:.0f} us")
    except Exception as e:  # noqa: BLE001
        print(f"    delta={delta:.1e}: failed: {e!r}")
# (e) which side is wrong at the separatrix: Yoshida at two steps, and the exact solution's own residuals
print("(e) separatrix diagnosis (T = 20 s):")
for delta_target in (1e-8, 1e-12):
    a = np.sqrt(3 * delta_target * 2.0)
    Lb = np.array([a, 2.0, a])
    exs = ExactRotor(1.0, 2.0, 3.0, Lb / I, np.eye(3))
    Ry1, Ry2 = run_yoshida(Lb, 1 / 1920, T), run_yoshida(Lb, 1 / 3840, T)
    w, A = exs.at(T)
    res = 0.0
    for t in (0.3, 7.7, 19.1):
        hh = 1e-5
        _, Ap = exs.at(t + hh)
        _, Am = exs.at(t - hh)
        wt, At = exs.at(t)
        res = max(res, np.linalg.norm((Ap - Am) / (2 * hh) + skew(wt) @ At))
    _, A0c = exs.at(0.0)
    print(f"    delta~{delta_target:.0e}: Yoshida 1/1920 vs 1/3840 {angle(Ry1, Ry2):.1e} rad; exact vs Yoshida 1/3840 {angle(A.T, Ry2):.1e} rad; "
          f"exact A(0) vs 1 {np.linalg.norm(A0c - np.eye(3)):.1e}; dA/dt residual {res:.1e}; omega(0) error {np.linalg.norm(exs.at(0.0)[0] - Lb / I):.1e}")
