"""Order-of-magnitude estimates behind docs/research/s14-wider-world.md.

Every number in the note's tables is printed here from the formula the note shows, so that a reader
(or the slice's builder) can change an assumption and see the table move.

    python3 -I docs/research/s14-wider-world/estimates.py > docs/research/s14-wider-world/estimates.out.txt

Standard library only. Nothing here is an oracle: these are estimates, with the assumptions at the top.
Instrument constants are the repo's (crates/cav-core/src/defaults.rs, constants.rs, plan/VALIDATION-AND-RESEARCH.md §1);
the AICE kick number and gradiometer separation are ASSUMED equal to AION's and 70 m (to confirm with George).
"""

from __future__ import annotations

import math
from dataclasses import dataclass

# ======================================= CONSTANTS =======================================

G         = 6.67430e-11        # m^3 kg^-1 s^-2 (CODATA 2018)
GM_EARTH  = 3.986004418e14     # m^3 s^-2
GM_MOON   = 4.9028001e12       # m^3 s^-2
GM_SUN    = 1.32712440018e20   # m^3 s^-2
R_EARTH   = 6.371e6            # m, mean radius
G_SURF    = 9.81               # m s^-2
OMEGA     = 7.292115e-5        # rad s^-1, Earth's rotation rate
D_MOON    = 3.844e8            # m, mean distance
D_SUN     = 1.495978707e11     # m
LATITUDE  = math.radians(51.76)  # Oxford (Beecroft); AICE site unknown, same assumed
K2, H2    = 0.30, 0.61         # nominal degree-2 Love numbers (IERS 2010 ch. 7), rounded
RHO_AIR   = 1.2               # kg m^-3 near the ground
P_SURF    = 101325.0           # Pa
H_ATM     = 8400.0             # m, isothermal scale height
C_SOUND   = 340.0              # m/s
RHO_WATER = 1025.0             # kg m^-3, sea water
RHO_CRUST = 2670.0             # kg m^-3
FREE_AIR3 = 6 * GM_EARTH / R_EARTH ** 4    # s^-2 per m: d(gradient)/dz, the cost of moving the tower vertically
WAVELENGTH = 698e-9
KICKS      = 1000


@dataclass(frozen = True)
class Instrument:
    """One gradiometer: T the half sequence, dr the IFO separation, cycle the shot spacing."""
    name:  str
    T:     float
    dr:    float
    cycle: float
    u0:    float   # launch speed, m/s

    @property
    def k_eff(self) -> float:
        return 2 * math.pi * KICKS / WAVELENGTH

    @property
    def per_gradient(self) -> float:
        """rad per s^-2 of vertical gradient: k_eff T^2 dr."""
        return self.k_eff * self.T ** 2 * self.dr

    @property
    def shot_sigma(self) -> float:
        """rad per shot, from 1e-4 rad/sqrt(Hz) one-sided: sigma = S^(1/2) / sqrt(2 cycle)."""
        return 1e-4 / math.sqrt(2 * self.cycle)

    def H(self, f: float) -> float:
        """Response to a differential acceleration at f, relative to its f -> 0 value T^2: |4 sin^2(wT/2) / (w^2 T^2)|."""
        w = 2 * math.pi * f
        return 1.0 if w == 0 else abs(4 * math.sin(w * self.T / 2) ** 2 / (w * self.T) ** 2)

    def phase(self, gradient: float, f: float = 0.0) -> float:
        return self.per_gradient * abs(gradient) * self.H(f)

    def snr_tone(self, amp: float, duration: float) -> float:
        """Matched filter on a sinusoid of known frequency in white shot noise: A sqrt(N/2) / sigma."""
        n = max(duration / self.cycle, 1.0)
        return amp * math.sqrt(n / 2) / self.shot_sigma


AION  = Instrument("AION-10", T = 0.73, dr = 5.0,  cycle = 2.0, u0 = 3.86)
AICE  = Instrument("AICE",    T = 2.5,  dr = 70.0, cycle = 5.0, u0 = 21.27)
BOTH  = (AION, AICE)


def row(label: str, gradient: float, f: float, duration: float) -> str:
    cells = []
    for ins in BOTH:
        ph = ins.phase(gradient, f)
        cells.append(f"{ph:9.2e} rad  x{ph / ins.shot_sigma:9.2e} shot  SNR(avg){ins.snr_tone(ph, duration):9.2e}")
    return f"{label:<58s} G={gradient:9.2e} s^-2 | " + " | ".join(cells)


def h(title: str) -> None:
    print(f"\n=== {title} ===")


# ======================================= INSTRUMENT =======================================

h("instrument")
for ins in BOTH:
    print(f"{ins.name}: k_eff = {ins.k_eff:.4e} rad/m; k_eff T^2 = {ins.k_eff * ins.T ** 2:.4e} rad per m/s^2; "
          f"per unit vertical gradient = {ins.per_gradient:.4e} rad per s^-2 (= {ins.per_gradient * 1e-9:.3f} rad per E); "
          f"shot sigma = {ins.shot_sigma:.2e} rad/shot; Nyquist {0.5 / ins.cycle:.2f} Hz; first null of H at {1 / ins.T:.2f} Hz")
print(f"uniform differential acceleration giving one shot sigma: AION {AION.shot_sigma / (AION.k_eff * AION.T**2):.2e} m/s^2, "
      f"AICE {AICE.shot_sigma / (AICE.k_eff * AICE.T**2):.2e} m/s^2")
print(f"gradient giving one shot sigma: AION {AION.shot_sigma / AION.per_gradient:.2e} s^-2, AICE {AICE.shot_sigma / AICE.per_gradient:.2e} s^-2")
print(f"static Earth gradient 2GM/R^3 = {2 * GM_EARTH / R_EARTH ** 3:.3e} s^-2 -> phase {AION.phase(2 * GM_EARTH / R_EARTH ** 3):.3e} rad (AION), "
      f"{AICE.phase(2 * GM_EARTH / R_EARTH ** 3):.3e} rad (AICE): constant, compensated by the chirp")
print(f"free-air cost of moving the tower up 1 m: 6GM/R^4 = {FREE_AIR3:.3e} s^-2")
print(f"air's own Bouguer-like term 4 pi G rho_air = {4 * math.pi * G * RHO_AIR:.3e} s^-2 (~1 E)")

# ======================================= TIDES =======================================

h("body tides (direct, induced, displacement)")
gm_d3_moon = GM_MOON / D_MOON ** 3
gm_d3_sun  = GM_SUN / D_SUN ** 3
print(f"GM/d^3: Moon {gm_d3_moon:.3e}, Sun {gm_d3_sun:.3e} s^-2 (zenith value 2x, horizon -1x)")
delta_gamma = 1 + 6 * K2 - 3 * H2
delta_g     = 1 + H2 - 1.5 * K2
print(f"gradient factor 1 + 6 k2 - 3 h2 = {delta_gamma:.3f}; gravimetric factor 1 + h2 - 1.5 k2 = {delta_g:.3f} (check: textbook ~1.16)")
print(f"equilibrium vertical displacement h2*W/g: Moon zenith {H2 * GM_MOON * R_EARTH ** 2 / D_MOON ** 3 / G_SURF:.3f} m (the 3cos^2-1 = 2 peak)")


def tide_gradient(decl_m: float, ha_m: float, decl_s: float, ha_s: float, lat: float) -> float:
    """Direct tidal vertical gradient from circular-orbit Moon and Sun: sum GM/d^3 (3 cos^2 psi - 1)."""
    total = 0.0
    for gm_d3, decl, ha in ((gm_d3_moon, decl_m, ha_m), (gm_d3_sun, decl_s, ha_s)):
        cos_psi = math.sin(lat) * math.sin(decl) + math.cos(lat) * math.cos(decl) * math.cos(ha)
        total += gm_d3 * (3 * cos_psi ** 2 - 1)
    return total


# sample a year: the Moon's declination swings +-(23.44+5.15) deg over 27.3 d, hour angle once per 24.84 h; Sun +-23.44 over 365.25 d.
values = []
n_steps = 365 * 24 * 4
for i in range(n_steps):
    t = i / (4 * 24)                         # days
    decl_s = math.radians(23.44) * math.sin(2 * math.pi * t / 365.25)
    decl_m = math.radians(28.58) * math.sin(2 * math.pi * t / 27.32)
    ha_m   = 2 * math.pi * t / 1.0350
    ha_s   = 2 * math.pi * t / 1.0
    values.append(tide_gradient(decl_m, ha_m, decl_s, ha_s, LATITUDE))
lo, hi = min(values), max(values)
print(f"direct tidal vertical gradient over a year at lat {math.degrees(LATITUDE):.1f} deg (circular orbits): min {lo:.3e}, max {hi:.3e}, "
      f"peak-to-peak {hi - lo:.3e} s^-2; amplitude about {(hi - lo) / 2:.3e}")
amp = (hi - lo) / 2 * delta_gamma
print(row("body tide, amplitude x gradient factor, f = 2.2e-5 Hz", amp, 2.24e-5, 86400))
print(row("body tide, same, one year of data", amp, 2.24e-5, 3.15e7))
print(f"largest per-shot step: slope ~ 2 pi f A: AION {AION.per_gradient * amp * 2 * math.pi * 2.24e-5 * AION.cycle:.2e} rad/shot, "
      f"AICE {AICE.per_gradient * amp * 2 * math.pi * 2.24e-5 * AICE.cycle:.2e} rad/shot")
print(f"Moon distance modulation: d^-3 gives x{(D_MOON / 3.633e8) ** 3:.2f} at perigee (3.633e8 m) and x{(D_MOON / 4.055e8) ** 3:.2f} at apogee (4.055e8 m)")

h("ocean tide: Newtonian attraction (half plane, coast distance d) and loading displacement")
for d_km, a_m in ((100, 1.0), (100, 2.0), (1000, 1.0), (10, 1.0)):
    gamma = math.pi * G * RHO_WATER * a_m / (d_km * 1e3)
    print(row(f"sea of amplitude {a_m} m beyond a straight coast {d_km} km away, f=2.2e-5 Hz", gamma, 2.24e-5, 86400))
print("(upper bound: real tides change sign across amphidromic systems; the proper sum is the load Green's function, Farrell 1972.)")
for u_mm in (1, 10, 50):
    gamma = FREE_AIR3 * u_mm * 1e-3
    print(row(f"loading displacement {u_mm} mm x 6GM/R^4", gamma, 2.24e-5, 86400))

# ======================================= ATMOSPHERE =======================================

h("atmospheric pressure systems: far field")
print("transfer of a barotropic pressure anomaly dp with horizontal wavenumber k to the vertical gradient at the ground:")
print("   G_zz(k) = 2 pi G k  Int d(rho)(z) exp(-k z) dz  =  2 pi G (rho0/p0) dp  k H / (1 + k H)   [H = scale height; ground-level instrument, flat ground]")
for lam_km, dp_hpa in ((1000, 10), (300, 5), (3000, 20)):
    k = 2 * math.pi / (lam_km * 1e3)
    d_rho = RHO_AIR * dp_hpa * 100 / P_SURF
    gamma = 2 * math.pi * G * d_rho * k * H_ATM / (1 + k * H_ATM)
    # moving at 10 m/s
    f = 10.0 / (lam_km * 1e3)
    print(row(f"pressure wave lambda {lam_km} km, {dp_hpa} hPa, f = {f:.1e} Hz (10 m/s)", gamma, f, 86400))
print(f"near-field limit (kH >> 1, e.g. air within a few km): 2 pi G d_rho: 1% density change -> {2 * math.pi * G * RHO_AIR * 0.01:.2e} s^-2; 10 hPa -> {2 * math.pi * G * RHO_AIR * 1000 / P_SURF:.2e} s^-2")
print("  below ground level the horizontally layered part gives exactly 0 (the gradient jumps from -4 pi G d_rho above to 0 below the surface)")
print(row("air temperature +-3% over the whole boundary layer (H=1 km), lambda 100 km, f=1e-5", 2 * math.pi * G * RHO_AIR * 0.03 * (2 * math.pi / 1e5) * 1000 / (1 + (2 * math.pi / 1e5) * 1000), 1e-5, 43200))
print(f"pressure loading displacement ~0.4 mm/hPa (order, unverified): 10 hPa -> {FREE_AIR3 * 4e-3:.2e} s^-2")

h("clouds and rain overhead: finite cylinder on the axis")
print("   G_zz = 2 pi G d_rho [ z2/sqrt(a^2+z2^2) - z1/sqrt(a^2+z1^2) ]  (cylinder radius a, heights z1..z2, density anomaly d_rho)")


def cylinder(d_rho: float, a: float, z1: float, z2: float) -> float:
    return 2 * math.pi * G * d_rho * (z2 / math.hypot(a, z2) - z1 / math.hypot(a, z1) if z1 > 0 else z2 / math.hypot(a, z2))


print(row("cumulus: a=1 km, 1.5-2.5 km up, d_rho = +2e-3 (thermal, ~0.5 K); f=1e-3", cylinder(2e-3, 1e3, 1.5e3, 2.5e3), 1e-3, 600))
print(row("cumulus: same with water only (LWC 0.5 g/m^3 = 5e-4 kg/m^3)", cylinder(5e-4, 1e3, 1.5e3, 2.5e3), 1e-3, 600))
print(row("rain cell: a=5 km, 0-2 km, 5e-4 kg/m^3 of falling water (upper bound); f=5e-4", cylinder(4.6e-4, 5e3, 0, 2e3), 5e-4, 1800))

h("surface water contrast: disc of surface density sigma at depth d below the tower, radius a (soil moisture, rain on a catchment)")
print("   G_zz = 2 pi G sigma a^2 / (a^2 + d^2)^(3/2)  -> 2 pi G sigma / a for d << a;  an infinite sheet gives 0, so only CONTRASTS count")
for sigma, a in ((10, 1e4), (10, 1e3), (20, 100), (1, 1e5)):
    gamma = 2 * math.pi * G * sigma / a
    print(row(f"sigma = {sigma} kg/m^2 (= {sigma} mm water) over radius {a:.0e} m; f=1e-5", gamma, 1e-5, 86400))

h("infrasound (plane acoustic wave filling the half space): G_zz = 2 pi G dp / c^2")
for dp, f in ((0.1, 0.2), (1.0, 0.2), (10.0, 0.05)):
    gamma = 2 * math.pi * G * dp / C_SOUND ** 2
    print(row(f"infrasound {dp} Pa at {f} Hz", gamma, f, 3600))

h("Hunga Tonga Lamb wave: lambda ~ 1100 km, dp ~ 100 Pa assumed, adiabatic d_rho = dp / c^2")
c_lamb = 310.0
for dp in (100.0, 200.0):
    k = 2 * math.pi / 1.1e6
    d_rho = dp / c_lamb ** 2
    gamma = 2 * math.pi * G * d_rho * k * H_ATM / (1 + k * H_ATM)
    print(row(f"Lamb wave dp = {dp} Pa, period ~1 h (f = 2.8e-4 Hz)", gamma, 2.8e-4, 7200))
    print(f"   loading displacement ~0.4 mm/hPa: {FREE_AIR3 * 0.4e-3 * dp / 100:.2e} s^-2")

# ======================================= SEISMIC =======================================

h("seismic: Rayleigh-wave Newtonian gradient at the surface, G_zz ~ gamma 2 pi G rho k xi_z (gamma ~ 0.5-1; formula from Harms 2015, unverified here)")


def rayleigh_gradient(xi: float, f: float, c: float, rho: float = RHO_CRUST, gamma: float = 0.7) -> float:
    return gamma * 2 * math.pi * G * rho * (2 * math.pi * f / c) * xi


for s_xi, f, c, label in ((1e-7, 0.2, 1500.0, "microseism quiet, 1e-7 m/sqrt(Hz)"), (1e-6, 0.2, 1500.0, "microseism storm, 1e-6 m/sqrt(Hz)")):
    gamma_psd = rayleigh_gradient(s_xi, f, c)          # per sqrt(Hz)
    for ins in BOTH:
        amp = ins.per_gradient * ins.H(f) * gamma_psd
        print(f"{label:<45s} {ins.name}: phase ASD {amp:.2e} rad/sqrt(Hz) = {amp / 1e-4:.3f} of the shot-noise ASD")
    print(f"   vs free-air displacement term: {FREE_AIR3 * s_xi:.2e} s^-2/sqrt(Hz) against Newtonian {gamma_psd:.2e}")

print("\nIASPEI surface-wave magnitude Ms = log10(A/T) + 1.66 log10(Delta_deg) + 3.3 (A in micrometres, T in s) -> amplitude at 20 s:")
for ms, delta in ((6.0, 80), (7.0, 80), (7.5, 80), (8.5, 80), (7.5, 150)):
    a_um = 20 * 10 ** (ms - 3.3 - 1.66 * math.log10(delta))
    xi = a_um * 1e-6
    gamma = rayleigh_gradient(xi, 0.05, 3500.0)
    print(row(f"Ms {ms} at {delta} deg: A = {a_um:8.1f} um (20 s Rayleigh), 10 min of wave train", gamma, 0.05, 600))
print("  smallest Ms visible at SNR 5 for a 10 min train at 80 deg:")
for ins in BOTH:
    lo_ms, hi_ms = 3.0, 9.0
    for _ in range(40):
        mid = (lo_ms + hi_ms) / 2
        xi = 20 * 10 ** (mid - 3.3 - 1.66 * math.log10(80)) * 1e-6
        ph = ins.per_gradient * ins.H(0.05) * rayleigh_gradient(xi, 0.05, 3500.0)
        if ins.snr_tone(ph, 600) > 5:
            hi_ms = mid
        else:
            lo_ms = mid
    print(f"   {ins.name}: Ms ~ {hi_ms:.1f}")

h("prompt elasto-gravity signal, scaling only: dg ~ C G M0 / (alpha^2 r^2), C ~ 0.1 (calibrated to ~1 microGal for Tohoku, unverified); gradient ~ dg / r")
alpha = 8000.0
for mw, r_km in ((9.0, 1000), (7.0, 300), (7.0, 100), (8.0, 1000)):
    m0 = 10 ** (1.5 * mw + 9.1)
    dg = 0.1 * G * m0 / (alpha ** 2 * (r_km * 1e3) ** 2)
    gamma = dg / (r_km * 1e3)
    window = r_km * 1e3 / alpha
    print(row(f"Mw {mw} at {r_km} km: dg = {dg:.1e} m/s^2, window {window:.0f} s", gamma, 0.02, window))

h("tsunami: sheet of surface density rho_w a over area A at distance d: 2 G m / d^3")
for a_m, area_km2, d_km in ((0.5, 100 * 100, 500), (0.5, 100 * 100, 150), (2.0, 50 * 50, 100)):
    m = RHO_WATER * a_m * area_km2 * 1e6
    gamma = 2 * G * m / (d_km * 1e3) ** 3
    print(row(f"tsunami {a_m} m over {area_km2} km^2 at {d_km} km (period ~20 min, f=8e-4)", gamma, 8e-4, 3600))

h("free oscillations: only the free-air term of the surface displacement, 6GM/R^4 xi (the mode's own potential adds order-one factors)")
for xi_mm in (0.1, 1.0):
    gamma = FREE_AIR3 * xi_mm * 1e-3
    print(row(f"0S2 (54 min, f=3.1e-4) with {xi_mm} mm vertical displacement, a day of ringing", gamma, 3.1e-4, 86400))

h("pole tide: vertical displacement ~ 10 mm at the Chandler period (433 d)")
print(row("pole tide 10 mm (Chandler 433 d, f = 2.7e-8)", FREE_AIR3 * 1e-2, 2.7e-8, 3.15e7))

# ======================================= TRAFFIC, TRAINS, WIND =======================================

h("moving masses beyond the scene: point mass, G_zz = 2 G m / r^3 (on axis, worst case)")
for m, r, label in ((3e4, 500, "30 t lorry at 500 m"), (3e4, 1000, "30 t lorry at 1 km"), (3e4, 5000, "30 t lorry at 5 km"),
                    (4e5, 500, "400 t train at 500 m"), (4e5, 2000, "400 t train at 2 km"), (1.5e3, 100, "car at 100 m")):
    print(row(label + " (passage ~ 10 s, f ~ 0.05)", 2 * G * m / r ** 3, 0.05, 20))
print("distance at which a mass m gives a phase tolerance eps: r = (2 G m C / eps)^(1/3), C = k_eff T^2 dr:")
for m in (1.5e3, 3e4, 4e5):
    cells = []
    for ins in BOTH:
        for eps in (1e-6, 5e-5):
            cells.append(f"{ins.name} eps {eps:.0e}: {(2 * G * m * ins.per_gradient / eps) ** (1 / 3):8.0f} m")
    print(f"   m = {m:8.1e} kg:  " + ";  ".join(cells))
print("rms of a Poisson road (Campbell): var G_zz = nu m^2 4 G^2 * 3 pi / (8 d^5)  per unit line density nu (vehicles/m) at distance d")
for nu, m, d in ((0.01, 1500.0, 200.0), (0.01, 1500.0, 1000.0), (0.002, 3e4, 500.0)):
    sd = math.sqrt(nu * m ** 2 * 4 * G ** 2 * 3 * math.pi / (8 * d ** 5))
    print(row(f"road nu={nu}/m, m={m:.0e} kg, d={d:.0f} m: rms", sd, 0.05, 3600))
print("a building of mass M swaying by x at distance d: dG ~ 6 G M x / d^4")
for m, x, d in ((1e7, 1e-3, 100), (1e7, 1e-3, 1000), (1e8, 1e-3, 1000)):
    print(row(f"M={m:.0e} kg sways {x * 1e3:.0f} mm at {d} m (f 0.5 Hz)", 6 * G * m * x / d ** 4, 0.5, 600))

# ======================================= ROTATION =======================================

h("rotating frame")
omega_n = OMEGA * math.cos(LATITUDE)
omega_u = OMEGA * math.sin(LATITUDE)
print(f"Omega = {OMEGA:.6e} rad/s; at latitude {math.degrees(LATITUDE):.2f}: north {omega_n:.3e}, up {omega_u:.3e}; f = 2 Omega sin(lat) = {2 * omega_u:.3e} 1/s (inertial period {2 * math.pi / (2 * omega_u) / 3600:.1f} h)")
print(f"centrifugal at the surface: Omega^2 R cos(lat) = {OMEGA ** 2 * R_EARTH * math.cos(LATITUDE):.3e} m/s^2; its gradient scale Omega^2 = {OMEGA ** 2:.2e} s^-2 (~{OMEGA ** 2 / 1e-9:.1f} E)")
for ins in BOTH:
    t = 2 * ins.T
    z_int = ins.u0 * t ** 2 / 2 - G_SURF * t ** 3 / 6              # integral of z(t)-z0 over the flight
    vx = -2 * omega_n * (ins.u0 * t - G_SURF * t ** 2 / 2)        # eastward speed at the end (Coriolis from vertical velocity)
    x = -2 * omega_n * z_int                                        # eastward displacement at the end
    print(f"{ins.name}: centre-of-mass free fall for 2T = {t:.2f} s from u0 = {ins.u0} m/s: Coriolis displacement {x * 1e3:+.3f} mm, final east speed {vx * 1e3:+.3f} mm/s")
    print(f"   phase per unit differential east launch velocity: 2 k_eff Omega cos(lat) T^2 = {2 * ins.k_eff * omega_n * ins.T ** 2:.3e} rad per m/s; "
          f"one shot sigma = {ins.shot_sigma:.1e} rad needs it below {ins.shot_sigma / (2 * ins.k_eff * omega_n * ins.T ** 2):.2e} m/s")
    print(f"   Coriolis moves the atoms {abs(x) * 1e3:.2f} mm sideways: a source at r changes the phase by ~3 dx/r = {3 * abs(x) / 1.0:.1e} relative at 1 m, {3 * abs(x) / 10:.1e} at 10 m")

lod = 1e-3  # s per day, peak to peak order
d_omega = OMEGA * lod / 86400
print(f"length of day: 1 ms/day -> d Omega = {d_omega:.2e} rad/s ({100 * d_omega / OMEGA:.1e}%); phase through the Coriolis term with 1 mm/s east velocity mismatch: AION {2 * AION.k_eff * d_omega * math.cos(LATITUDE) * AION.T ** 2 * 1e-3:.1e} rad")
print(f"polar motion 0.3 arcsec = {0.3 / 206265:.1e} rad -> centrifugal gravity change ~ 2 Omega^2 R m = {2 * OMEGA ** 2 * R_EARTH * 0.3 / 206265:.1e} m/s^2 (common-mode); displacement route: see pole tide")
print("Rossby number Ro = U / (f L), f = 2 Omega sin(lat):")
f_cor = 2 * omega_u
for label, u, l in (("room, 0.1 m/s, 5 m", 0.1, 5.0), ("building, 1 m/s, 50 m", 1.0, 50.0), ("town wind, 5 m/s, 5 km", 5.0, 5e3),
                    ("sea breeze, 5 m/s, 50 km", 5.0, 5e4), ("synoptic, 10 m/s, 1000 km", 10.0, 1e6), ("river/harbour water, 1 m/s, 1 km", 1.0, 1e3)):
    print(f"   {label:<36s} Ro = {u / (f_cor * l):9.3g}; Coriolis/inertial = {f_cor * l / u:9.3g}")
print(f"   time for Coriolis to turn a 1 m/s flow by 1 rad: {1 / f_cor / 3600:.1f} h")
