"""Sizes, in radians, of environmental and instrumental effects on the AION-10 gradiometer.

Standard library only. Every number in the note's ranked table that is not quoted from a paper
comes from here. Run: python -I sizes.py > sizes.out.txt

Instrument (Cavendish defaults, crates/cav-core/src/defaults.rs and forward.rs):
  87Sr, 698 nm, n = 1000 kicks, T = 0.73 s, u0 = 3.86 m/s, dr = 5 m, g = 9.81, floor at launch.
Arms as forward.rs builds them: one n-photon kick at t = 0 (upper arm), the pi pulse at T swaps.
"""

import math

# --- constants -------------------------------------------------------------------------------
G = 6.674_30e-11
HBAR = 1.054_571_817e-34
U = 1.660_539_066_60e-27
M_SR87 = 86.908_877_5 * U
LAMBDA = 698.0e-9
N_KICK = 1000
T = 0.73
U0 = 3.86
DR = 5.0
GRAV = 9.81
GAMMA_EARTH = 3.086e-6  # s^-2, free-air vertical gradient
OMEGA = 7.292_115e-5
LAT = math.radians(51.76)
CYCLE = 2.0
ASD = 1e-4  # rad/sqrt(Hz), AION-10 goal (Badurina et al. 2020, Table 1)

K = 2 * math.pi / LAMBDA
KEFF = N_KICK * K
V_KICK = N_KICK * HBAR * K / M_SR87
C_GRAD = KEFF * T**2 * DR  # rad per s^-2 of uniform Gamma_zz
SHOT = ASD / math.sqrt(2 * CYCLE)


def line(name: str, value: float, unit: str = "rad", note: str = "") -> None:
    print(f"{name:<62s} {value: .3e} {unit:<12s} {note}")


# --- arms ------------------------------------------------------------------------------------
def rise(v0: float, t: float) -> float:
    """Free fall from the launch height with a floor at it (as forward.rs's Arm)."""
    land = 2 * v0 / GRAV if v0 > 0 else 0.0
    if t >= land:
        return 0.0
    return v0 * t - 0.5 * GRAV * t * t


def upper(t: float) -> float:
    if t <= T:
        return rise(U0 + V_KICK, t)
    z_t = rise(U0 + V_KICK, T)
    v_t = (U0 + V_KICK) - GRAV * T if z_t > 0 else 0.0
    return free_from(z_t, v_t - V_KICK, t - T)


def lower(t: float) -> float:
    if t <= T:
        return rise(U0, t)
    z_t = rise(U0, T)
    v_t = U0 - GRAV * T if z_t > 0 else 0.0
    return free_from(z_t, v_t + V_KICK, t - T)


def free_from(z0: float, v0: float, s: float) -> float:
    """Height after s seconds from (z0, v0), stopping at 0 once it falls back to it."""
    # time to reach 0 while falling
    disc = v0 * v0 + 2 * GRAV * z0
    t_land = (v0 + math.sqrt(disc)) / GRAV
    if s >= t_land:
        return 0.0
    return z0 + v0 * s - 0.5 * GRAV * s * s


def integrate(f, a: float, b: float, n: int = 20000) -> float:
    h = (b - a) / n
    s = 0.5 * (f(a) + f(b))
    for i in range(1, n):
        s += f(a + i * h)
    return s * h


print("== Instrument")
line("k_eff", KEFF, "rad/m")
line("n-photon recoil velocity", V_KICK, "m/s")
line("C = k_eff T^2 dr", C_GRAD, "rad/s^-2")
line("shot noise per shot (1e-4 rad/rtHz, 2 s cycle)", SHOT)
line("arm separation at T", upper(T) - lower(T), "m")
line("top of upper arm (lower IFO)", max(upper(i * 1e-3) for i in range(1461)), "m")

# --- state-dependent (clock) shifts ----------------------------------------------------------
# Single-photon MZ: upper arm excited on [0, T], lower arm excited on [T, 2T].
A = integrate(upper, 0, T) - integrate(lower, T, 2 * T)  # m s
print("\n== Clock-shift transfer (excited arm only; simple single-kick MZ, an upper bound)")
line("A = int_0^T z_u - int_T^2T z_l", A, "m s")
line("per-IFO phase per (Hz/m) static gradient of shift: 2 pi A", 2 * math.pi * A, "rad/(Hz/m)")
line("gradiometer phase per (Hz/m^2) curvature: 2 pi A dr", 2 * math.pi * A * DR, "rad/(Hz/m^2)")
line("per-IFO phase per (Hz/s) uniform drift: 2 pi T^2", 2 * math.pi * T * T, "rad/(Hz/s)")


def uniform_noise_transfer(f: float) -> float:
    w = 2 * math.pi * f
    return 2 * math.pi * 4 / w * math.sin(w * T / 2) ** 2  # rad per Hz of shift amplitude


for f in (0.01, 0.1, 0.25):
    line(f"uniform shift noise transfer at {f} Hz", uniform_noise_transfer(f), "rad/Hz")

# magnetic, linear Zeeman of the stretched 9/2->9/2 line (TDR sec 2.3)
C1 = 489.0  # Hz/G
B_CURV = 5e-3 / (DR**2)  # G/m^2: 5 mG inhomogeneity over 5 m, read as curvature
line("static: 5 mG/(5 m)^2 curvature, 489 Hz/G, gradiometer", 2 * math.pi * A * DR * C1 * B_CURV)
line("noise: 1 uG/rtHz differential at 0.1 Hz, 489 Hz/G", C1 * 1e-6 * uniform_noise_transfer(0.1), "rad/rtHz")
line("  same, per shot", C1 * 1e-6 * uniform_noise_transfer(0.1) / math.sqrt(2 * CYCLE))
# quadratic Zeeman after co-magnetometry (MAGIS-100 sec 5.8, -0.23 Hz/G^2)
B0 = 10.0
line("quadratic: 2*0.23*B0*dB, B0=10 G, dB=1 uG/rtHz, 0.1 Hz", 2 * 0.23 * B0 * 1e-6 * uniform_noise_transfer(0.1), "rad/rtHz")

# blackbody: -2.354(32) Hz (T/300 K)^4 (Falke et al. 2011, Eq. 1; MAGIS-100 sec 5.9 rounds to -2.3)
BBR_300 = 2.354  # Hz, magnitude of the clock (differential) shift at 300 K
DNU_DT = 4 * BBR_300 / 300.0
line("BBR dnu/dT at 300 K", DNU_DT, "Hz/K")
line("static: 1 K parabolic over 5 m (T''=0.08 K/m^2), gradiometer", 2 * math.pi * A * DR * DNU_DT * 0.08)
line("static: 1 K/m linear gradient, per IFO (cancels in gradiometer)", 2 * math.pi * A * DNU_DT * 1.0)
line("drift: 0.1 K/h differential between IFOs", 2 * math.pi * T * T * DNU_DT * 0.1 / 3600)
line("s13 note's estimate (0.1 rad/K), for comparison", 0.1)

# --- kinematics x gradient, rotation ----------------------------------------------------------
print("\n== Launch jitter through Earth's gradient and rotation (per IFO; independent in the two)")
line("k Gamma T^2 per nm of launch height", KEFF * GAMMA_EARTH * T * T * 1e-9)
line("k Gamma T^3 per nm/s of launch velocity", KEFF * GAMMA_EARTH * T**3 * 1e-9)
line("Coriolis 2 k Omega cos(lat) T^2 per nm/s transverse", 2 * KEFF * OMEGA * math.cos(LAT) * T * T * 1e-9)
line("static gradiometer phase of Earth's gradient C*Gamma", C_GRAD * GAMMA_EARTH)
line("Roura dk/k = Gamma T^2 / 2", GAMMA_EARTH * T * T / 2, "")
line("  as a laser frequency change at 698 nm", GAMMA_EARTH * T * T / 2 * 299_792_458 / LAMBDA, "Hz")
line("  kick change at T", V_KICK * GAMMA_EARTH * T * T / 2, "m/s")
line("tip-tilt rate (horizontal Earth rotation)", OMEGA * math.cos(LAT), "rad/s")
line("  mirror rotation over 2T", OMEGA * math.cos(LAT) * 2 * T, "rad")
line("rot x grad x v (MAGIS 3-loop form) per nm/s, (17/3+4rt2)kOGT^4",
     (17 / 3 + 4 * math.sqrt(2)) * KEFF * OMEGA * math.cos(LAT) * GAMMA_EARTH * T**4 * 1e-9)
DV_GRAD = GAMMA_EARTH * DR * T
line("velocity difference of the two clouds at T from Gamma", DV_GRAD, "m/s")

# --- MAGIS-100 scalings moved to n = 1000 (their reference values otherwise) -------------------
print("\n== MAGIS-100 noise scalings at n = 1000 (rad/rtHz), their reference parameters")
for name, base in (
    ("wavefront x kinematics, first order (eps 5 mrad, dx 10 um/rtHz)", 5e-4),
    ("pointing jitter x wavefront (30 nrad/rtHz)", 6e-4),
    ("AC Stark (dI/I 1e-5/rtHz)", 6e-4),
    ("mean field (dN/N 0.01)", 5e-5),
):
    line(name, base * N_KICK / 100, "rad/rtHz")
line("wavefront second order (n^2)", 3e-8 * (N_KICK / 100) ** 2, "rad/rtHz")
line("timing jitter, 1 ns/rtHz, dv from Gamma", 1e-4 * N_KICK / 100 * DV_GRAD / 100e-6, "rad/rtHz")
line("mirror vibration 1e-4 m/s^2/rtHz, dv from Gamma", 1e-8 * N_KICK / 100 * DV_GRAD / 100e-6 * T, "rad/rtHz")
line("laser frequency noise 10 Hz/rtHz", 1e-13 * N_KICK / 100 * DV_GRAD / 100e-6, "rad/rtHz")

# --- wavefront curvature x cloud temperature (Dickerson et al. 2013, Table I, term 6) -----------
# Term 6 is (1/2) k_eff alpha (v_x^2 + v_y^2) T^2 with alpha the wavefront sag curvature (a sag of
# alpha r^2). A spherical front of radius R has alpha = 1/(2R); a thermal cloud has
# <v_x^2 + v_y^2> = 2 sigma_v^2, so the mean shift is k_eff sigma_v^2 T^2 / (2R).
print("\n== Wavefront curvature x cloud temperature: Dickerson term 6, k_eff sigma_v^2 T^2 / (2R)")
KB = 1.380_649e-23
for temp in (1e-9, 10e-9):
    sv2 = KB * temp / M_SR87
    for r in (1e3, 1e4):
        line(f"  {temp*1e9:.0f} nK, R = {r/1e3:.0f} km", KEFF * T * T * sv2 / (2 * r))
# Dickerson's own Table I value, as a check that the form is read right: 87Rb, k_eff = 2 x 2 pi/780 nm,
# T = 1.15 s, v_i = 2 mm/s on each transverse axis, alpha = (lambda/10)/cm^2. The table prints 0.04.
ALPHA_D = (780e-9 / 10) / 1e-4
line("  Dickerson's Table I term 6 recomputed (table: 0.04)",
     0.5 * (2 * 2 * math.pi / 780e-9) * ALPHA_D * 2 * (2e-3) ** 2 * 1.15**2)

# --- finite kick duration --------------------------------------------------------------------
print("\n== Finite kick duration: relative change of the arms' separation integral")
for tau in (1e-3, 1e-2, 5e-2, 0.2):
    inst = V_KICK * T * T  # area of the separation-time triangle, one half
    # linear ramp of duration tau at the start of each half (a crude model of an LMT train)
    ramp = V_KICK * (T * T / 2 - tau * T / 2 + tau * tau / 6) * 2
    line(f"  tau = {tau*1e3:.0f} ms", 1 - ramp / inst, "relative")

# --- environment, checks on S14's numbers ----------------------------------------------------
print("\n== Environment")
line("solid tide gradient 1.5e-13 s^-2 (s14)", C_GRAD * 1.5e-13)
H = 8400.0
P0 = 101_325.0
RHO0 = 1.225
for lam, dp in ((1e6, 1000.0), (3e5, 500.0)):
    k = 2 * math.pi / lam
    gam = 2 * math.pi * G * RHO0 / P0 * dp * k * H / (1 + k * H)
    line(f"pressure system lambda {lam/1e3:.0f} km, {dp/100:.0f} hPa", C_GRAD * gam)
line("Bouguer gravity of 1 hPa of air, 2 pi G dp/g (uGal)", 2 * math.pi * G * 100 / GRAV / 1e-8, "uGal")
CS = 343.0
line("infrasound per Pa, 2 pi G dp/c^2", C_GRAD * 2 * math.pi * G / CS**2, "rad/Pa")
line("air in the hall, per K (s13 table, 5 m x 6 m hall)", 2.2e-2)
# rain on a roof above the stairwell: disc of water sigma at height d above the upper IFO
for sigma, a, d in ((10.0, 20.0, 10.0), (10.0, 50.0, 10.0)):
    def gzz(dd: float) -> float:
        return 2 * math.pi * G * sigma * a * a / (a * a + dd * dd) ** 1.5
    # gradiometer reads a_z difference; integrate gzz over the 5 m baseline would be exact for a
    # thin disc: a_z(d) = 2 pi G sigma (1 - d/sqrt(a^2+d^2))
    def az(dd: float) -> float:
        return 2 * math.pi * G * sigma * (1 - dd / math.sqrt(a * a + dd * dd))
    line(f"rain {sigma:.0f} mm on a roof r={a:.0f} m, {d:.0f} m above upper IFO",
         KEFF * T * T * (az(d) - az(d + DR)))
# people: closed form Carlton & McCabe eq. 13, far field
for m, dd, h in ((80.0, 3.0, 2.5), (80.0, 5.0, 12.0), (1000.0, 20.0, 12.0)):
    ph = -KEFF * T * T * G * m * DR * (dd * dd - 2 * h * h) / (dd * dd + h * h) ** 2.5
    line(f"Carlton eq13: M={m:.0f} kg D={dd:.0f} m h={h:.0f} m", ph)

# --- added 25 Nov 2026, after reading the primary sources --------------------------------------
print("\n== Cross-checks against the primary sources")
line("Bouguer plate of 1 m of water, 2 pi G rho_w h (Creutzfeldt 2008: 42 uGal flat)",
     2 * math.pi * G * 1000.0 * 1.0 / 1e-8, "uGal")
line("Merriam 1992 local-zone admittance (quoted)", -0.356, "uGal/hPa")
line("Van Camp 2017 A1.3: -4 attraction +1 loading (quoted)", -3.0, "nm/s^2/hPa")
line("Van Camp 2017 Fig. A7: point mass 45 kg at 1 m, G M / r^2 (measured 2.8)",
     G * 45.0 / 1.0**2 / 1e-9, "nm/s^2")
line("Dubetsky & Kasevich Table I col. 3: (7/192) k g T3^4 Tzz, T3 = 2T, per IFO",
     7 / 192 * KEFF * GRAV * (2 * T) ** 4 * GAMMA_EARTH)
line("  the same as (7/12) k g Gamma T^4", 7 / 12 * KEFF * GRAV * GAMMA_EARTH * T**4)

# Blackbody force (Haslinger et al. 2018): the ground-state a.c. Stark shift in the thermal field is a
# potential h nu_g(T(z)) felt by both arms, whatever their internal state. Its curvature along the
# tower is a gravity-gradient-like term C * (h/m) * nu_g''. The ground-state static polarizability
# of Sr, 197.2 a.u., and <E^2> = (831.9 V/m)^2 (T/300 K)^4 are from memory (not from the papers read).
H_PLANCK = 2 * math.pi * HBAR
AU_POL = 1.648_777_274e-41  # C m^2 / V
E2_300 = 831.9**2  # (V/m)^2
NU_G_300 = 0.5 * 197.2 * AU_POL * E2_300 / H_PLANCK  # Hz, magnitude of the ground-state shift
DNU_G_DT = 4 * NU_G_300 / 300.0
print("\n== Blackbody force: ground-state Stark potential on both arms (state-independent)")
line("ground-state BBR shift magnitude at 300 K (alpha_g = 197.2 a.u.)", NU_G_300, "Hz")
line("d nu_g / dT at 300 K", DNU_G_DT, "Hz/K")
line("gradiometer phase per (Hz/m^2) of potential curvature, C h/m", C_GRAD * H_PLANCK / M_SR87,
     "rad/(Hz/m^2)")
line("static: 1 K parabolic over 5 m (T''=0.08 K/m^2), gradiometer",
     C_GRAD * H_PLANCK / M_SR87 * DNU_G_DT * 0.08)
line("acceleration per K/m of temperature gradient, (h/m) d nu_g/dT",
     H_PLANCK / M_SR87 * DNU_G_DT, "m s^-2/(K/m)")


# Wind-advected temperature in the outdoor air (Creighton 2008, sec. 3, order-of-magnitude Eq. (8)),
# moved from a LIGO test mass to the gradiometer's vertical baseline:
#   S_dg(f) = 2 v l^(3+p) (G rho c_T / T0)^2 * integral over streamlines dA |K~(f)|^2,
#   l = v / (2 pi f), K the a_z(z2) - a_z(z1) kernel of a unit mass, K~ its transform along x = v t.
# Uniform wind along x; streamlines may not pass through the building (|y| < b and z < Hb).
def advected_temperature_asd(f: float, v: float, ct2: float, b: float, hb: float,
                             z1: float = 2.0, p: float = 2 / 3) -> float:
    rho, t0 = 1.2, 290.0
    z2 = z1 + DR
    w = 2 * math.pi * f
    lsc = v / w
    kx = w / v

    def geom(a: float, b_: float, n: int) -> list[float]:
        return [a * (b_ / a) ** (i / (n - 1)) for i in range(n)]

    ys = [0.0] + geom(0.25, 400.0, 40)
    zs = geom(1.0, 400.0, 40)

    def widths(xs: list[float]) -> list[float]:
        out = []
        for i in range(len(xs)):
            lo = xs[i] - xs[i - 1] if i > 0 else 0.0
            hi = xs[i + 1] - xs[i] if i + 1 < len(xs) else 0.0
            out.append(0.5 * (lo + hi))
        return out

    wy, wz = widths(ys), widths(zs)
    us = [-9 + 18 * i / 600 for i in range(601)]
    du = 18 / 600
    total = 0.0
    for iy, y in enumerate(ys):
        for iz, z in enumerate(zs):
            if abs(y) < b and z < hb:
                continue
            s = max(1.0, math.hypot(y, z - 0.5 * (z1 + z2)))
            re = im = 0.0
            for u in us:
                x = s * math.sinh(u)
                jac = s * math.cosh(u) * du
                r1 = (x * x + y * y + (z - z1) ** 2) ** 1.5
                r2 = (x * x + y * y + (z - z2) ** 2) ** 1.5
                k = (z - z2) / r2 - (z - z1) / r1
                re += k * math.cos(kx * x) * jac
                im -= k * math.sin(kx * x) * jac
            kt2 = (re * re + im * im) / (v * v)
            total += kt2 * wy[iy] * wz[iz] * (2 if y > 0 else 1)
    s_dg = 2 * v * lsc ** (3 + p) * (G * rho / t0) ** 2 * ct2 * total
    resp = KEFF * 4 / (w * w) * math.sin(w * T / 2) ** 2
    return resp * math.sqrt(s_dg)


print("\n== Wind-advected temperature (Creighton 2008 sec. 3), gradiometer phase ASD (rad/rtHz)")
print("   c_T^2 = 0.2 K^2 m^-2/3 (his 'typical daytime peak'), lower IFO 2 m above ground")
for f in (0.05, 0.1, 0.25):
    for v, b, hb in ((10.0, 10.0, 25.0), (10.0, 5.0, 12.0), (10.0, 20.0, 30.0), (5.0, 10.0, 25.0)):
        line(f"  f = {f} Hz, v = {v:.0f} m/s, building half-width {b:.0f} m, height {hb:.0f} m",
             advected_temperature_asd(f, v, 0.2, b, hb), "rad/rtHz")
