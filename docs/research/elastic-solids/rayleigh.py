"""Rayleigh-wave Newtonian gravity: closed form (Harms 2019, Eqs. 39, 96-99) against a voxel
kinematic sum (Harms Eq. 45: each cell a parcel displaced by the analytic field).

Pass criteria written before running:
  P1 free-surface tractions of the Eq. 39 field vanish to < 1e-10 relative at c_R;
  P2 the voxel sum of Delta a_z between two heights converges to the closed form within 1 %
     once the box is large enough; report the box size and cell size that reach 1 % and 0.1 %.
"""
import numpy as np
from scipy.optimize import brentq

G = 6.674e-11


def speeds(nu):
    """c_P/c_S and c_R/c_S for Poisson's ratio nu."""
    a2 = 2 * (1 - nu) / (1 - 2 * nu)  # (c_P/c_S)^2
    def f(x):  # Rayleigh function in x = (c_R/c_S)^2
        return (2 - x) ** 2 - 4 * np.sqrt(1 - x) * np.sqrt(1 - x / a2)
    x = brentq(f, 1e-6, 1 - 1e-12)
    return np.sqrt(a2), np.sqrt(x)


def params(nu, k):
    cp, cr = speeds(nu)
    qS = k * np.sqrt(1 - cr ** 2)
    qP = k * np.sqrt(1 - (cr / cp) ** 2)
    zeta = np.sqrt(qP / qS)
    return cp, cr, qP, qS, zeta


def gamma(nu):
    k = 1.0
    _, _, qP, qS, zeta = params(nu, k)
    return k * (1 - zeta) / (qP - k * zeta)


def traction_check(nu):
    """sigma_xz and sigma_zz at z=0 for xi of Harms Eq. 39 (A=1, c_S=1, mu=1)."""
    k = 1.0
    cp, cr, qP, qS, zeta = params(nu, k)
    mu = 1.0
    lam = cp ** 2 - 2  # rho=1, c_S=1
    # xi_x = -(k e^{qP z} - zeta qS e^{qS z}) sin(kx), xi_z = (qP e^{qP z} - zeta k e^{qS z}) cos(kx)
    # xi_x carries a minus sign (derived from potentials; Eq. 39 as extracted lost it)
    dxx_dz = -(k * qP - zeta * qS * qS)         # coefficient of sin
    dxz_dx = -k * (qP - zeta * k)               # coefficient of sin
    dxx_dx = -k * (k - zeta * qS)               # coefficient of cos
    dxz_dz = qP * qP - zeta * k * qS            # coefficient of cos
    sxz = mu * (dxx_dz + dxz_dx)
    szz = lam * (dxx_dx + dxz_dz) + 2 * mu * dxz_dz
    scale = (lam + 2 * mu) * k * k
    return abs(sxz) / scale, abs(szz) / scale


def closed_form_az(nu, k, xi0, rho, h):
    """Vertical gravity perturbation amplitude at height h > 0 above the surface, at the phase
    where xi_z(0) = +xi0 (Harms Eq. 100: a_z = -2 pi G rho gamma xi_z e^{-kh})."""
    return -2 * np.pi * G * rho * gamma(nu) * xi0 * np.exp(-k * h)


def voxel_az(nu, k, xi0, rho, hs, s, R, D):
    """a_z at points (0,0,h) from parcels of size s in |x|,|y|<R, -D<z<0, each displaced by the
    analytic field at its centre (t=0, x=0 a crest of xi_z). Dipole kernel, Harms Eq. 45."""
    cp, cr, qP, qS, zeta = params(nu, k)
    A = xi0 / (qP - zeta * k)
    xs = (np.arange(-R / s, R / s) + 0.5) * s
    X, Y = np.meshgrid(xs, xs, indexing="ij")
    X = X.ravel(); Y = Y.ravel()
    out = np.zeros(len(hs))
    m = rho * s ** 3
    for iz in range(int(round(D / s))):
        z = -(iz + 0.5) * s
        ux = -A * (k * np.exp(qP * z) - zeta * qS * np.exp(qS * z)) * np.sin(k * X)
        uz = A * (qP * np.exp(qP * z) - zeta * k * np.exp(qS * z)) * np.cos(k * X)
        for j, h in enumerate(hs):
            dx, dy, dz = X, Y, z - h  # r - r0
            r2 = dx * dx + dy * dy + dz * dz
            r = np.sqrt(r2)
            edot = (ux * dx + uz * dz) / r
            # a(r0) = G m [xi - 3 (e.xi) e] / r^3 ; z-component
            out[j] += G * m * np.sum((uz - 3 * edot * dz / r) / (r2 * r))
    return out


if __name__ == "__main__":
    print("nu   cP/cS  cR/cS  gamma   |s_xz|  |s_zz| (relative)")
    for nu in (0.1, 0.2, 0.25, 0.3, 0.33, 0.4, 0.45, 0.49):
        cp, cr = speeds(nu)
        t = traction_check(nu)
        print(f"{nu:4.2f} {cp:6.3f} {cr:6.4f} {gamma(nu):6.4f}  {t[0]:.1e} {t[1]:.1e}")
    # Cavendish-scale case: 10 Hz, c_R = 250 m/s -> lambda = 25 m; nu = 0.33; rho = 1800
    nu, rho, xi0 = 0.33, 1800.0, 1e-9
    lam_R = 25.0
    k = 2 * np.pi / lam_R
    h1, h2 = 1.0, 11.0
    cf = closed_form_az(nu, k, xi0, rho, np.array([h1, h2]))
    dcf = cf[1] - cf[0]
    print(f"\nlambda_R = {lam_R} m, heights {h1}, {h2} m, xi0 = 1 nm, closed form a_z = {cf}, "
          f"Delta = {dcf:.4e} m/s^2")
    print("  s[m]  R[m]  D[m]   cells      Delta a_z(voxel)   rel. error")
    for s, R, D in [(1.0, 25, 40), (1.0, 50, 60), (1.0, 100, 80), (1.0, 200, 80),
                    (0.5, 100, 80), (0.5, 200, 80), (2.0, 200, 80), (2.0, 400, 80)]:
        v = voxel_az(nu, k, xi0, rho, [h1, h2], s, R, D)
        dv = v[1] - v[0]
        n = (2 * R / s) ** 2 * D / s
        print(f"  {s:4.1f} {R:5.0f} {D:5.0f} {n:10.2e}   {dv:.4e}   {abs(dv - dcf) / abs(dcf):.2e}")
