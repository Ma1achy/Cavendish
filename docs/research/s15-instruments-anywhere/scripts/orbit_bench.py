"""Q4 benchmark: orbital gradient tensor, J2, laser-phase of a gradiometer in free fall, ISS bound.

Convention: V = +GM/r (geodesy/GOCE sign), acceleration a = +grad V, V_ij = d2V/dxi dxj.
LVLH frame: x along track (velocity), z radial (outward), y = z cross x (cross-track).
"""

import numpy as np
import sympy as sp
from scipy.integrate import solve_ivp

GM = 3.986004418e14  # m3/s2 (EGM2008/WGS84 value; CODATA-independent)
RE = 6378137.0  # m
J2 = 1.08262668e-3
G = 6.67430e-11
HBAR = 1.054571817e-34
E = 1e-9  # 1 eotvos in s^-2

print("=== 1. Point-mass tensor on a circular orbit (computed here) ===")
for h in (224e3, 255e3, 400e3):
    r = RE + h
    n2 = GM / r**3
    print(f"h={h/1e3:5.0f} km  n={np.sqrt(n2):.4e} rad/s  GM/r^3={n2:.4e} s^-2 = {n2/E:7.1f} E   "
          f"Vxx=Vyy={-n2/E:7.1f} E  Vzz={2*n2/E:7.1f} E  period={2*np.pi/np.sqrt(n2)/60:.2f} min")

print("\n=== 2. J2 Hessian in LVLH, sympy vs finite difference ===")
X, Y, Z = sp.symbols("X Y Z", real=True)
r_ = sp.sqrt(X**2 + Y**2 + Z**2)
s = Z / r_
V = GM / r_ * (1 - J2 * (RE / r_) ** 2 * (3 * s**2 - 1) / 2)
H = sp.hessian(V, (X, Y, Z))
Hf = sp.lambdify((X, Y, Z), H, "numpy")
Vf = sp.lambdify((X, Y, Z), V, "numpy")


def frame(u, inc):
    rh = np.array([np.cos(u), np.sin(u) * np.cos(inc), np.sin(u) * np.sin(inc)])
    vh = np.array([-np.sin(u), np.cos(u) * np.cos(inc), np.cos(u) * np.sin(inc)])
    yh = np.cross(rh, vh)
    return vh, yh, rh  # x, y, z


def fd_hessian(p, h=50.0):
    out = np.zeros((3, 3))
    for i in range(3):
        for j in range(3):
            ei, ej = np.eye(3)[i] * h, np.eye(3)[j] * h
            out[i, j] = (Vf(*(p + ei + ej)) - Vf(*(p + ei - ej)) - Vf(*(p - ei + ej)) + Vf(*(p - ei - ej))) / (4 * h * h)
    return out


inc = np.deg2rad(96.7)
r0 = RE + 255e3
for u_deg in (0, 45, 90, 135):
    u = np.deg2rad(u_deg)
    xh, yh, zh = frame(u, inc)
    B = np.array([xh, yh, zh])
    p = r0 * zh
    Hs = np.array(Hf(*p), dtype=float)
    L = B @ Hs @ B.T
    Lfd = B @ fd_hessian(p) @ B.T
    P = GM / r0**3 * np.diag([-1, -1, 2.0])
    P2 = (3 * (zh[2]) ** 2 - 1) / 2
    vrr_closed = 2 * GM / r0**3 * (1 - 6 * J2 * (RE / r0) ** 2 * P2)
    print(f"u={u_deg:3d} deg lat_sin={zh[2]:+.3f}  Vzz(J2)={L[2,2]/E:8.2f} E (closed rr form {vrr_closed/E:8.2f} E, "
          f"fd {Lfd[2,2]/E:8.2f})  dVzz/Vzz={(L[2,2]-P[2,2])/P[2,2]:+.2e}  "
          f"dVxx={(L[0,0]-P[0,0])/E:+7.2f} E dVyy={(L[1,1]-P[1,1])/E:+7.2f} E Vxz={L[0,2]/E:+6.2f} E  "
          f"|sympy-fd|max={np.abs(L-Lfd).max()/E:.2e} E")
print("control: closed-form check vs a wrong coefficient (5 instead of 6):",
      f"{abs(2*GM/r0**3*(1-5*J2*(RE/r0)**2*1.0)-L[2,2])/E:.2f} E off at pole-ish (should be >> fd error)")
print(f"J2 first-order relative size: 6 J2 (R/r)^2 = {6*J2*(RE/r0)**2:.3e} (max |P2|=1)")

print("\n=== 3. Gradiometer laser phase in free fall: exact Kepler flight vs closed forms ===")
k_rb = 4 * np.pi / 780.241e-9  # two-photon, Rb-87 D2 (computed here)


def sat_state(t, r0):
    n = np.sqrt(GM / r0**3)
    # circular orbit in the plane, node on x (inclination irrelevant for a point mass: use equatorial)
    rs = r0 * np.array([np.cos(n * t), np.sin(n * t), 0.0])
    return rs, n


def flight_phase(axis, d0, case, T, r0):
    """Phase of one atom cloud released at offset d0 (m, LVLH vector) at t=0.
    case 'A': at rest in the rotating (satellite) frame; 'B': inertial rest relative to the satellite."""
    n = np.sqrt(GM / r0**3)
    # LVLH at t=0 for equatorial orbit u=0: x=(0,1,0) along track, y=(0,0,1) cross (z x x = x_hat? check), z=(1,0,0)
    zh, xh = np.array([1.0, 0, 0]), np.array([0, 1.0, 0])
    yh = np.cross(zh, xh)
    d = d0[0] * xh + d0[1] * yh + d0[2] * zh
    Om = n * np.cross(zh, xh) * (-1.0) * (-1.0)  # orbital angular velocity vector = n * (z-hat of inertial) = n*(x cross y)
    Om = np.array([0, 0, n])
    v = np.cross(Om, d) if case == "A" else np.zeros(3)

    def rhs(t, y):
        rs, _ = sat_state(t, r0)
        dd = y[:3]
        gs = -GM * rs / np.linalg.norm(rs) ** 3
        ra = rs + dd
        ga = -GM * ra / np.linalg.norm(ra) ** 3
        return np.concatenate([y[3:], ga - gs])

    sol = solve_ivp(rhs, (0, 2 * T), np.concatenate([d, v]), method="DOP853", rtol=1e-13, atol=1e-14,
                    t_eval=[0, T, 2 * T], dense_output=False)
    ph = 0.0
    for w, t, dd in zip((1, -2, 1), sol.t, sol.y.T):
        rs, _ = sat_state(t, r0)
        zhat = rs / np.linalg.norm(rs)
        that = np.array([-zhat[1], zhat[0], 0.0])
        ax = {"z": zhat, "x": that, "y": np.array([0, 0, 1.0])}[axis]
        ph += w * ax @ dd[:3]
    return k_rb * ph


def grad_phase(axis, L, case, T, r0):
    idx = {"x": 0, "y": 1, "z": 2}[axis]
    top, bot = np.zeros(3), np.zeros(3)
    top[idx], bot[idx] = L / 2, -L / 2
    return flight_phase(axis, top, case, T, r0) - flight_phase(axis, bot, case, T, r0)


r0 = RE + 255e3
n = np.sqrt(GM / r0**3)
n2 = n * n
Lb = 0.5
print(f"n^2={n2:.4e}; kL T^2 n^2 unit for T=5: {k_rb*Lb*25*n2:.3f} rad")
rows = []
for T in (5.0, 10.0):
    for axis, tag in (("z", "radial"), ("x", "along"), ("y", "cross")):
        res = {}
        for case in ("A", "B"):
            ph = grad_phase(axis, Lb, case, T, r0)
            res[case] = ph
        unit = k_rb * Lb * T**2 * n2
        rows.append((T, tag, res["A"], res["B"], unit))
        print(f"T={T:4.1f}s axis={tag:7s}  A(rest in rotating frame): {res['A']:+10.4f} rad = {res['A']/unit:+.4f} n^2 kLT^2   "
              f"B(inertial rest): {res['B']:+10.4f} rad = {res['B']/unit:+.4f} n^2 kLT^2")
# closed form for A radial: Hill/CW solution, phase = k L * 6 cos(nT)(1-cos nT)
for T in (5.0, 10.0):
    y = n * T
    cf = k_rb * Lb * 6 * np.cos(y) * (1 - np.cos(y))
    lead = k_rb * Lb * 3 * n2 * T**2
    sim = grad_phase("z", Lb, "A", T, r0)
    print(f"T={T}: Hill closed form {cf:.6f} rad; sim {sim:.6f}; leading 3 n^2 kLT^2 {lead:.6f}; "
          f"sim/closed-1={sim/cf-1:+.2e}; closed/lead-1={cf/lead-1:+.3e} (series -7/12 y^2={-7/12*y*y:+.3e}); "
          f"leading-term error in rad = {lead-cf:.4f}")
print("(2nd-order tidal-only radial, naive k*2n^2*L*T^2, T=5:",
      f"{k_rb*Lb*2*n2*25:.3f} rad vs rotating-frame 3n^2: {k_rb*Lb*3*n2*25:.3f} rad)")

print("\n=== 4. Signal sizes (radians), leading order k*Gamma*L*T^2, Gamma_eff,radial = 3 n^2 (rotating attitude) ===")
k_sr = 2 * np.pi * 1000 / 698e-9  # engine's convention: 1000 photon kicks, k_eff = 2 pi n / lambda
for label, k, T, Lm in (("Rb k=1.61e7  T=5  L=0.5 (Trimeche-like)", k_rb, 5.0, 0.5),
                        ("Rb k=1.61e7  T=10 L=0.5", k_rb, 10.0, 0.5),
                        ("Rb k=1.61e7  T=1  L=0.5", k_rb, 1.0, 0.5),
                        ("Sr 1000 kicks k=9.0e9 T=0.73 L=5 (AION-10 geometry)", k_sr, 0.73, 5.0),
                        ("Sr 1000 kicks k=9.0e9 T=0.73 L=0.5", k_sr, 0.73, 0.5)):
    for h in (255e3, 400e3):
        nn2 = GM / (RE + h) ** 3
        print(f"{label:55s} h={h/1e3:.0f} km: {k*3*nn2*Lm*T*T:12.3e} rad  (tidal-only 2n^2: {k*2*nn2*Lm*T*T:.3e})")
print(f"engine k_eff (1000 kicks, 698 nm) = {k_sr:.4e} m^-1; Rb two-photon 780 nm k = {k_rb:.4e}")
print("phase noise -> gradient: 1 mrad, k=Rb, T=5, L=0.5:",
      f"{1e-3/(k_rb*0.5*25)/1e-12:.2f} mE per shot (single diffraction k; halve for double diffraction)")
print("common-mode non-grav accel 1e-7 m/s^2: k a T^2 =", f"{k_rb*1e-7*25:.1f} rad (T=5), {k_rb*1e-7*100:.1f} rad (T=10)")
print("dynamic range: phase 1e3 rad at 1 urad needs", f"{1e-6/1e3:.0e} relative; f64 eps 1.1e-16 -> fine")

print("\n=== 5. ISS (4.2e5 kg) at 400 km as a source for a ground tower: AION-10 geometry (computed here) ===")
M, d = 4.2e5, 400e3
Gam = 2 * G * M / d**3
print(f"tidal tensor eigenvalue bound 2GM/d^3 = {Gam:.3e} s^-2 = {Gam/E:.2e} E; GM/d^2 = {G*M/d**2:.3e} m/s2")
T, Lm, kk = 0.73, 5.0, k_sr
print(f"quasi-static bound k*Gam*dr*T^2 = {kk*Gam*Lm*T*T:.3e} rad")

# engine integral, point mass overhead, floor off, u0=3.86, same formula as forward.rs
mS, u0 = 1.4432e-25, 3.86
g = 9.81
kick = 1000 * HBAR * (2 * np.pi / 698e-9) / mS


def rise(first, kick_, tau):
    out = np.where(tau <= T, first * tau - 0.5 * g * tau**2, 0)
    v2 = first - g * T + kick_
    s = tau - T
    h = first * T - 0.5 * g * T * T
    return np.where(tau <= T, first * tau - 0.5 * g * tau**2, h + v2 * s - 0.5 * g * s * s)


xg, wg = np.polynomial.legendre.leggauss(8)
phase = [0.0, 0.0]
panels = 40
edges = np.linspace(0, 2 * T, panels + 1)
for a, b in zip(edges[:-1], edges[1:]):
    tau = 0.5 * (b - a) * xg + 0.5 * (b + a)
    w = 0.5 * (b - a) * wg
    low, up = rise(u0, kick, tau), rise(u0 + kick, -kick, tau)
    for k_, hgt in enumerate((0.0, Lm)):
        below, above = hgt + low - d, hgt + up - d  # source overhead at height d (z offset negative for arms)
        A, Bq = np.abs(above), np.abs(below)
        dif = -M * (low - up) * (below + above) / (A * Bq * (A + Bq))
        phase[k_] += np.sum(w * dif)
scale = G * mS / HBAR
print(f"engine-formula gradiometer phase (lower-upper) = {scale*(phase[0]-phase[1]):+.3e} rad; "
      f"single-IFO phase {scale*phase[0]:+.3e} rad")
print("control (10 kg at 10 m scale, for reference): GM/d^2 for 10 kg at 10 m =", f"{G*10/100:.2e} m/s2")

print("\n=== 6. Cross-checks of published numbers (computed here, added after reading the papers) ===")
import sympy as sp
# 6a. Carraz 2014 Eq. (2): Phi = k [ (z0-aM/gam)(1 + ch(s(T+T')) - 2 ch(sT)) ...], T'=T, s = sqrt(gam). Series coefficient.
x = sp.symbols("x", positive=True)
ser = sp.series(1 + sp.cosh(2 * x) - 2 * sp.cosh(x), x, 0, 7).removeO()
print("1 + ch(2x) - 2 ch(x) =", sp.expand(ser), "  (x = sqrt(gamma) T)  -> x^2 + (7/12) x^4 : matches Carraz Eq.(2) and Hogan Table II term 4")
# 6b. Trimeche 2019 Sec. IV numbers: double diffraction => Phi = 2 k z Tzz T^2
k = 4 * np.pi / 780e-9
Tzz, D, Tt, Nn = 2.7e-6, 0.5, 5.0, 1e6
print(f"Trimeche: k = {k:.3e} 1/m; Delta Phi = 2 k D Tzz T^2 = {2*k*D*Tzz*Tt**2:.1f} rad (paper: 1087 rad)")
print(f"Trimeche: sigma_Tzz = sqrt(2)/(2 sqrt(N) k D T^2) = {np.sqrt(2)/(2*np.sqrt(Nn)*k*D*Tt**2)/1e-12:.2f} mE/shot (paper: 3.5)")
kc = 8 * np.pi / 780e-9
print(f"Carraz Eq.(6): sqrt(2)/(sqrt(N) k_eff d T^2) sqrt(Tcycle) with k_eff=8pi/lam, Tcycle=1 s = {np.sqrt(2)/(np.sqrt(Nn)*kc*0.5*25)/1e-12:.2f} mE/sqrt(Hz) (paper: 3.5)")
print(f"        1 mrad per interferometer => single-diffraction k L T^2: {1e-3/(k*0.5*25)/1e-12:.2f} mE; double diffraction 2kLT^2: {1e-3/(2*k*0.5*25)/1e-12:.2f} mE")
# 6c. STE-QUEST: sigma_a = sqrt(2/N)/(C k T^2), N=1e6, C=0.6, k=8pi/780nm, T=5 s
print(f"STE-QUEST: sqrt(2/N)/(C k T^2) = {np.sqrt(2/1e6)/(0.6*kc*25):.3e} m/s2 per shot (paper: 2.93e-12 for Tc = 20 s; times sqrt(Tc) as an ASD: {np.sqrt(2/1e6)/(0.6*kc*25)*np.sqrt(20):.3e} m/s2/sqrt(Hz))")
# 6d. MAGIS-space geometry: a = 2.4e7 m circular, true anomalies 0 and 138 deg
a = 2.4e7
chord = 2 * a * np.sin(np.radians(138) / 2)
print(f"MAGIS-space: chord at 138 deg = {chord:.3e} m (paper L = 4.4e7); altitude a-R = {(a-RE)/1e3:.0f} km; period = {2*np.pi*np.sqrt(a**3/GM)/60:.0f} min (paper: 600 min)")
# 6e. J2 from Trimeche Eq. (12) normalisation: C20bar = -J2/sqrt5, Pbar20 = sqrt5 P2
J2v = 1.08263e-3
print("Eq.(12) degree-2 order-0 term: (GM/R)(R/r)^3 Cbar20 Pbar20 = -(GM/r) J2 (R/r)^2 P2  with Cbar20=-J2/sqrt(5):", np.isclose(-J2v/np.sqrt(5)*np.sqrt(5), -J2v))
# 6f. GOCE: 0.5 m baseline, 2e-12 m/s2/sqrt(Hz) per accelerometer (L1b handbook Table 2): per-axis gradient noise
print(f"GOCE handbook Table 2: sqrt(2)*2e-12/0.5 = {np.sqrt(2)*2e-12/0.5/1e-12:.1f} mE/sqrt(Hz) (US-US pair, specification level; published results are 10-20 mE per Trimeche)")
