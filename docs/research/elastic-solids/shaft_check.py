"""Settle the 'Harms Eq. 97 vs Badurina App. B' question for a point inside the ground.

Pass criteria (written before running):
  P1 an independent evaluation of the Rayleigh-wave potential at depth h (surface mass layer plus
     the bulk density -rho div(xi), each layer's potential -2 pi G sigma e^{-k|d|}/k, integrated
     numerically) equals Harms 2019 Eq. 97 to 1e-9 relative at several depths and nu;
  P2 Badurina et al. Eq. B.19 (with the square root read from the rendered page, not the extracted
     text) equals Eq. 97 to 1e-3 (their q, s are quoted to two figures; the test uses exact values);
  P4 Hughes & Thorne's reduced transfer function beta' (their Eq. 1.24, normalised to the
     direction-averaged displacement W) evaluated for the half-space Rayleigh mode gives 0.86 at
     nu = 0.33 (their Eq. 2.7) and 0.17 for the subsurface (compression) term alone (their text
     after Eq. 2.7), and equals sqrt(3/2) * gamma * |xi_V| / sqrt(xi_H^2 + xi_V^2) with gamma
     of Harms Eq. 99;
  P5 Junca et al. 2019 Eqs. 12-14 (gamma, b, horizontal acceleration at depth h, written
     differently) equal the horizontal derivative of Harms Eq. 97 to rounding;
  P6 Eq. 96 (above ground) and Eq. 97 (below) are consistent at the surface: the vertical
     acceleration jumps by 4 pi G sigma, sigma = rho xi_z (the mass sheet of the displaced surface);
  P7 Harms Eq. 39 as printed (no minus on the horizontal component, checked on the rendered page, p. 29
     of the Living Review) is NOT traction-free at the free surface, the version with the minus is
     (control arm: the printed sign must fail);
  P3 the acceleration-level ratio B/A of B.21-B.22 equals -(1+zeta)/(2q) = the z-derivative of Eq. 97,
     and the 1.5 'discrepancy' of the earlier note is reproduced by reading sqrt(q/s) as q/s and
     comparing potential-level with acceleration-level coefficients.
"""
import numpy as np
from scipy.integrate import quad
from rayleigh import params

G = 6.674e-11
rho = 1800.0


def check(nu, k=1.0, xi0=1.0):
    cp, cr, qP, qS, zeta = params(nu, k)
    A = xi0 / (qP - zeta * k)  # Harms Eq. 98
    kP2 = k * k * (cr / cp) ** 2 * 0 + (qP * qP - k * k) * -1  # kP^2 = k^2 - qP^2
    kP2 = k * k - qP * qP
    out = []
    for h in (0.5, 1.0, 3.0):
        # independent: layers at depth coordinate z' in (-inf,0); observation at z = -h
        surf = rho * xi0 * np.exp(-k * h)
        f = lambda zp: rho * A * kP2 * np.exp(qP * zp) * np.exp(-k * abs(zp + h))
        bulk = quad(f, -np.inf, -h)[0] + quad(f, -h, 0)[0]
        phi_num = -(2 * np.pi * G / k) * (surf + bulk)
        # Harms Eq. 97
        phi_h = 2 * np.pi * G * rho * A * (-2 * np.exp(-h * qP) + (1 + zeta) * np.exp(-h * k))
        # Badurina B.19 amplitude: -2 pi G rho xi cH/(q w) (1+s^2)/(1-s^2) [(1+sqrt(q/s)) e^{-w z/cH} - 2 e^{-q w z/cH}]
        q, s = qP / k, qS / k
        phi_b = (-2 * np.pi * G * rho * xi0 / (q * k) * (1 + s * s) / (1 - s * s)
                 * ((1 + np.sqrt(q / s)) * np.exp(-k * h) - 2 * np.exp(-q * k * h)))
        out.append((h, phi_num, phi_h, phi_b))
    return out


if __name__ == "__main__":
    for nu in (0.25, 0.33):
        cp, cr, qP, qS, zeta = params(nu, 1.0)
        q, s = qP, qS
        print(f"nu={nu}: q={q:.4f} s={s:.4f} zeta=sqrt(q/s)={zeta:.4f}")
        print(f"  Harms prefactor 1/(q-zeta) = {1/(q-zeta):.5f};  Badurina (1+s^2)/((1-s^2) q) = {(1+s*s)/((1-s*s)*q):.5f}"
              " (equal in magnitude, opposite sign: Harms's A is negative)")
        for h, pn, ph, pb in check(nu):
            print(f"  h*k={h}: numeric {pn:.6e}  Eq97 {ph:.6e}  B.19 {pb:.6e}  rel(num-97)={abs(pn-ph)/abs(ph):.1e} rel(B19-97)={abs(pb-ph)/abs(ph):.1e}")
        # acceleration-level (z-derivative) ratio of the e^{-kh} term to the e^{-qkh} term
        print(f"  potential-level coefficient ratio (Harms 97): -(1+zeta)/2 = {-(1+zeta)/2:.4f}")
        print(f"  acceleration-level ratio (B.22/B.21 and d/dh of Eq 97): -(1+zeta)/(2q) = {-(1+zeta)/(2*q):.4f}")
        mis = -(1 + q / s) / (2 * q)
        print(f"  earlier note's misreading (sqrt lost): -(1+q/s)/(2q) = {mis:.4f}; ratio to potential-level = {mis/(-(1+zeta)/2):.3f}")
    # the numbers the earlier note used (q=0.88, s=0.36)
    q, s = 0.88, 0.36
    print("note's rounded values q=0.88 s=0.36:", (-(1+q/s)/(2*q)) / (-(1+np.sqrt(q/s))/2))

    print("Hughes & Thorne Eq. 1.24 against Harms Eq. 99 (half-space fundamental Rayleigh mode):")
    for nu in (0.25, 0.33):
        k = 1.0
        cp, cr, qP, qS, zeta = params(nu, k)
        A = 1.0 / (qP - zeta * k)
        xiV = A * (qP - zeta * k)
        xiH = A * (k - zeta * qS)
        kP2 = k * k - qP * qP
        sub = A * kP2 / (qP + k)  # integral of R(z) e^{-kz}, R = -div xi = A kP^2 e^{-qP z}
        norm = np.sqrt(xiH ** 2 + xiV ** 2)
        beta = np.sqrt(1.5) * abs(xiV + sub) / norm
        beta_sub = np.sqrt(1.5) * abs(sub) / norm
        g = k * (1 - zeta) / (qP - k * zeta)
        print(f"  nu={nu}: anisotropy A=sqrt2*|xiV/xiH|={np.sqrt(2)*abs(xiV/xiH):.2f}  beta'={beta:.3f}  (subsurface only {beta_sub:.3f})"
              f"  sqrt(3/2)*gamma*xiV/norm={np.sqrt(1.5)*g*xiV/norm:.3f}  gamma(Eq99)={g:.4f}  gamma from 1+sub/xiV={(xiV+sub)/xiV:.4f}")

    print("Junca et al. 2019 Eqs. 12-14 against the x-derivative of Harms Eq. 97 (amplitude / (2 pi G rho xi_z)):")
    for nu in (0.25, 0.33):
        k = 1.0
        cp, cr, qP, qS, zeta = params(nu, k)
        x2, y2 = (cr / cp) ** 2, cr ** 2  # (c_R/alpha)^2, (c_R/beta)^2 with c_S = 1
        r = ((1 - x2) / (1 - y2)) ** 0.25
        gJ = (r - 1) / (r - np.sqrt(1 - x2))
        bJ = 2 / (1 - r)
        gH = k * (1 - zeta) / (qP - k * zeta)
        worst = 0.0
        for h in (0.2, 1.0, 4.0):
            junca = gJ * (np.exp(-h * k) + bJ * (np.exp(-h * qP) - np.exp(-h * k)))
            harms = (k / (qP - k * zeta)) * (2 * np.exp(-h * qP) - (1 + zeta) * np.exp(-h * k))
            worst = max(worst, abs(junca - harms) / abs(harms))
        print(f"  nu={nu}: gamma Junca Eq.13 = {gJ:.6f}, Harms Eq.99 = {gH:.6f}; b = {bJ:.4f}; worst relative difference over depths {worst:.1e}")

    print("Surface jump of a_z between Eq. 96 (above) and Eq. 97 (below), in units of 4 pi G rho xi_z (should be 1):")
    for nu in (0.25, 0.33):
        k = 1.0
        cp, cr, qP, qS, zeta = params(nu, k)
        A = 1.0 / (qP - zeta * k)
        a_above = -A * k * (1 - zeta)                       # 2 pi G rho = 1; a_z = -dU/dh_above, U = -A(1-zeta)e^{-kh}
        a_below = A * k * (2 * qP - 1 - zeta)               # a_z = +dU/dd, U = A(-2 e^{-qP d} + (1+zeta) e^{-k d})
        print(f"  nu={nu}: (a_below - a_above)/(2 * xi_z) = {abs(a_below - a_above) / (2 * 1.0):.12f}")

    print("Harms Eq. 39 sign check: free-surface tractions (sigma_xz, sigma_zz), rho = 1, c_S = 1, A = 1:")
    for nu in (0.25, 0.33):
        k = 1.0
        cp, cr, qP, qS, zeta = params(nu, k)
        lam, mu = cp ** 2 - 2, 1.0
        for sgn, label in ((-1, "with a minus on xi_k (derived from potentials)"), (+1, "as printed")):
            dxx_dz = sgn * (k * qP - zeta * qS * qS)
            dxz_dx = -k * (qP - zeta * k)
            dxx_dx = sgn * k * (k - zeta * qS)
            dxz_dz = qP * qP - zeta * k * qS
            sxz = mu * (dxx_dz + dxz_dx)
            szz = lam * (dxx_dx + dxz_dz) + 2 * mu * dxz_dz
            print(f"  nu={nu}: {label:48s} sigma_xz = {sxz:+.3e}, sigma_zz = {szz:+.3e}")
