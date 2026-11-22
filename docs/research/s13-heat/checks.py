"""Measurements behind docs/research/s13-heat.md (GPU-free, f64, numpy/scipy/iapws only).

Run:  python3 -I checks.py <part>      parts: water, views, bimetal, sphere, signals, linearised, all

Each part prints the numbers quoted in the note. Pass criteria are written next to each check
before the number is computed, as the project's quality bar asks.
"""
from __future__ import annotations

import sys
from collections.abc import Callable

import numpy as np
from numpy.typing import NDArray

F64 = NDArray[np.float64]
G = 6.67430e-11  # m^3 kg^-1 s^-2 (CODATA 2018)

# ======================================= WATER =======================================


def iapws_density(t_c: F64, p_mpa: float = 0.101325) -> F64:
    """IAPWS-95 density [kg/m^3] at pressure p over temperatures t_c [deg C]."""
    from iapws import IAPWS95  # type: ignore[import-not-found,import-untyped]

    return np.array([IAPWS95(T = float(t) + 273.15, P = p_mpa).rho for t in t_c])


def tanaka_density(t_c: F64) -> F64:
    """Tanaka et al. (2001) / CIPM-2001 density of air-free water, 0-40 C, via chempy's transcription."""
    from chempy.properties.water_density_tanaka_2001 import water_density  # type: ignore[import-not-found,import-untyped]

    return np.array([water_density(float(t) + 273.15, units = None, warn = False) for t in t_c])


def gebhart_mollendorf(t_c: F64) -> F64:
    """Gebhart and Mollendorf (1977): rho = rho_m (1 - 9.297173e-6 |T - 3.9863|^1.894816), recalled, not verified."""
    return 999.972 * (1.0 - 9.297173e-6 * np.abs(t_c - 3.9863) ** 1.894816)


def water() -> None:
    t = np.linspace(0.0, 100.0, 201)
    rho = iapws_density(t)
    i = int(np.argmax(rho))
    # refine the maximum on a fine grid
    tf = np.linspace(3.5, 4.5, 401)
    rf = iapws_density(tf)
    j = int(np.argmax(rf))
    print(f"IAPWS-95 at 101.325 kPa: max density {rf[j]:.4f} kg/m3 at {tf[j]:.4f} C (coarse grid {t[i]:.2f})")
    t40 = np.linspace(0.0, 40.0, 161)
    r40 = iapws_density(t40)
    tk = tanaka_density(t40)
    print(f"Tanaka-IAPWS95, 0-40 C: max |diff| {np.max(np.abs(tk - r40)) * 1e3:.3f} g/m3 "
          f"({np.max(np.abs(tk - r40)) / 998 * 1e6:.2f} ppm)")
    gm = gebhart_mollendorf(t40)
    print(f"Gebhart-Mollendorf vs IAPWS-95, 0-20 C: max |diff| "
          f"{np.max(np.abs(gm - r40)[t40 <= 20]) * 1e3:.1f} g/m3; 0-40 C: {np.max(np.abs(gm - r40)) * 1e3:.1f} g/m3")
    print("Polynomial least-squares fits of rho(T) to IAPWS-95 (max abs error, g/m3):")
    print("  degree   0-10 C    0-40 C    0-95 C   (monotone beyond range? coefficients in T/100)")
    grids = {hi: (np.linspace(0.0, hi, 201), iapws_density(np.linspace(0.0, hi, 201))) for hi in (10.0, 40.0, 95.0)}
    for deg in (2, 3, 4, 5, 6, 8):
        out: list[str] = []
        for hi, (tt, rr) in grids.items():
            c = np.polynomial.polynomial.polyfit(tt / 100.0, rr, deg)
            err = np.max(np.abs(np.polynomial.polynomial.polyval(tt / 100.0, c) - rr))
            out.append(f"{err * 1e3:9.2f}")
        print(f"  {deg:5d}  " + " ".join(out))
    # a Boussinesq linear law about a reference temperature
    for t0 in (4.0, 10.0, 20.0, 60.0):
        r0 = float(iapws_density(np.array([t0]))[0])
        dr = (iapws_density(np.array([t0 + 1.0]))[0] - iapws_density(np.array([t0 - 1.0]))[0]) / 2.0
        beta = -dr / r0
        tt = np.linspace(t0 - 10.0, t0 + 10.0, 201)
        lin = r0 * (1.0 - beta * (tt - t0))
        err = np.max(np.abs(lin - iapws_density(tt)))
        print(f"  linear beta at {t0:4.0f} C: beta = {beta * 1e6:8.1f} 1e-6/K; max error over +-10 K = {err:.4f} kg/m3")
    # air: ideal gas at constant pressure vs Boussinesq linear, and the size of the compressibility term
    for dt in (1.0, 10.0, 30.0, 50.0):
        r0, r1 = 1.0 / 293.15, 1.0 / (293.15 + dt)
        lin = 1.0 - dt / 293.15
        print(f"  air dT = {dt:4.0f} K: rho ratio ideal {r1 / r0:.5f}, Boussinesq {lin:.5f}, "
              f"relative error of the anomaly {(lin - r1 / r0) / (r1 / r0 - 1.0):+.3%}")
    # pressure effect on water density near the maximum: 1 m of water head
    r_a = iapws_density(np.array([4.0]), 0.101325)[0]
    r_b = iapws_density(np.array([4.0]), 0.101325 + 0.00981)[0]
    print(f"  hydrostatic compression, +1 m head at 4 C: {(r_b - r_a) / r_a:.2e} (relative)")
    # the maximum moves with pressure
    for p in (0.101325, 10.0):
        tf = np.linspace(0.0, 6.0, 121)
        rf = iapws_density(tf, p)
        print(f"  density maximum at {p:7.3f} MPa: {tf[int(np.argmax(rf))]:.3f} C")


# ======================================= VIEW FACTORS =======================================


def f_parallel_rect(a: float, b: float, c: float) -> float:
    """Aligned parallel rectangles a x b at distance c (Howell C-11)."""
    x, y = a / c, b / c
    sx, sy = np.sqrt(1 + x * x), np.sqrt(1 + y * y)
    v = (0.5 * np.log((1 + x * x) * (1 + y * y) / (1 + x * x + y * y))
         + x * sy * np.arctan(x / sy) + y * sx * np.arctan(y / sx) - x * np.arctan(x) - y * np.arctan(y))
    return float(2.0 / (np.pi * x * y) * v)


def f_perp_rect(h: float, w: float, l: float) -> float:
    """Perpendicular rectangles sharing an edge of length l; A1 is l x w, A2 is l x h (Howell C-14)."""
    hh, ww = h / l, w / l
    s = hh * hh + ww * ww
    term = (np.log((1 + ww * ww) * (1 + hh * hh) / (1 + s))
            + ww * ww * np.log(ww * ww * (1 + s) / ((1 + ww * ww) * s))
            + hh * hh * np.log(hh * hh * (1 + s) / ((1 + hh * hh) * s)))
    v = ww * np.arctan(1 / ww) + hh * np.arctan(1 / hh) - np.sqrt(s) * np.arctan(1 / np.sqrt(s)) + 0.25 * term
    return float(v / (np.pi * ww))


def f_coaxial_discs(r1: float, r2: float, h: float) -> float:
    rr1, rr2 = r1 / h, r2 / h
    s = 1 + (1 + rr2 * rr2) / (rr1 * rr1)
    return float(0.5 * (s - np.sqrt(s * s - 4 * (rr2 / rr1) ** 2)))


def mc_view_factor(sample1: Callable[[np.random.Generator, int], F64], n1: F64,
                   sample2: Callable[[np.random.Generator, int], F64], n2: F64,
                   area2: float, n: int, seed: int = 0) -> tuple[float, float]:
    """Monte-Carlo F_12 = (1/A1) int int cos1 cos2 / (pi r^2) dA1 dA2, uniform samples, unoccluded."""
    rng = np.random.default_rng(seed)
    p1, p2 = sample1(rng, n), sample2(rng, n)
    d = p2 - p1
    r2 = np.sum(d * d, axis = 1)
    dh = d / np.sqrt(r2)[:, None]
    c1 = np.clip(dh @ n1, 0, None)
    c2 = np.clip(-dh @ n2, 0, None)
    v = c1 * c2 / (np.pi * r2) * area2
    return float(v.mean()), float(v.std(ddof = 1) / np.sqrt(n))


def views() -> None:
    f_par = f_parallel_rect(1, 1, 1)
    f_per = f_perp_rect(1, 1, 1)
    print(f"closed forms: unit squares parallel at 1: {f_par:.6f}; perpendicular sharing an edge: {f_per:.6f}")
    print(f"              coaxial discs r=1,1,h=1: {f_coaxial_discs(1, 1, 1):.6f}; concentric spheres r1/r2=0.5: {1 - 0.25:.4f}")

    def sq(z: float, axes: tuple[int, int, int]) -> Callable[[np.random.Generator, int], F64]:
        def f(rng: np.random.Generator, n: int) -> F64:
            u = rng.random((n, 2))
            p = np.zeros((n, 3))
            p[:, axes[0]], p[:, axes[1]], p[:, axes[2]] = u[:, 0], u[:, 1], z
            return p
        return f

    nz = np.array([0.0, 0.0, 1.0])
    print("Monte-Carlo (uniform area sampling), pass: |MC - closed form| < 4 sigma, sigma ~ N^-1/2")
    for n in (10**3, 10**4, 10**5, 10**6):
        m, s = mc_view_factor(sq(0.0, (0, 1, 2)), nz, sq(1.0, (0, 1, 2)), -nz, 1.0, n)
        print(f"  parallel   N = {n:8d}: {m:.5f} +- {s:.5f}   err {abs(m - f_par):.5f}  rel sigma {s / m:.2%}")
    # perpendicular: A1 in z=0 plane, A2 in x=0 plane facing +x
    def a2(rng: np.random.Generator, n: int) -> F64:
        u = rng.random((n, 2))
        return np.stack([np.zeros(n), u[:, 0], u[:, 1]], axis = 1)

    for n in (10**4, 10**6):
        m, s = mc_view_factor(sq(0.0, (0, 1, 2)), nz, a2, np.array([1.0, 0, 0]), 1.0, n)
        print(f"  perpend.   N = {n:8d}: {m:.5f} +- {s:.5f}   err {abs(m - f_per):.5f}")
    # reciprocity and summation for a unit cube (6 faces): closed forms give sum = 1
    s = 2 * f_perp_rect(1, 1, 1) * 2 + f_parallel_rect(1, 1, 1)
    print(f"  unit cube, one face: 4 x perpendicular + 1 x parallel = {4 * f_perp_rect(1, 1, 1) + f_parallel_rect(1, 1, 1):.6f} (must be 1; summation rule)")
    del s
    # staircase area: mean (|nx|+|ny|+|nz|) over the sphere = 3/2
    rng = np.random.default_rng(1)
    v = rng.normal(size = (10**6, 3))
    v /= np.linalg.norm(v, axis = 1)[:, None]
    print(f"  voxel-face (staircase) area / true area for a sphere = mean(|nx|+|ny|+|nz|) = {np.abs(v).sum(axis = 1).mean():.4f}  (3/2)")
    # radiative coefficient linearisation
    print("  linearised radiation: sigma(T1^4 - T2^4) vs 4 sigma Tm^3 dT, error (dT/2Tm)^2:")
    for tm, dt in ((293.15, 10.0), (305.0, 50.0), (320.0, 100.0), (400.0, 200.0), (500.0, 400.0)):
        t1, t2 = tm + dt / 2, tm - dt / 2
        ex, li = t1**4 - t2**4, 4 * tm**3 * dt
        print(f"    Tm = {tm:6.1f} K, dT = {dt:5.0f} K: linear / exact = {li / ex:.5f}  (1/(1+(dT/2Tm)^2) = {1 / (1 + (dt / 2 / tm) ** 2):.5f})")
    print("  Monte-Carlo rays for 1 % relative sigma on F: N ~ (1-F)/(F x 1e-4); for F = 0.2: "
          f"{(1 - 0.2) / (0.2 * 1e-4):.0f}; for F = 0.02: {(1 - 0.02) / (0.02 * 1e-4):.0f}")


# ======================================= BIMETAL AND SPHERE =======================================


def bimetal() -> None:
    """Timoshenko's strip against plane-section beam theory, solved here from force and moment balance."""
    def timo(a1: float, a2: float, h1: float, h2: float, e1: float, e2: float, dt: float) -> float:
        m, n, h = h1 / h2, e1 / e2, h1 + h2
        return 6 * (a2 - a1) * dt * (1 + m) ** 2 / (h * (3 * (1 + m) ** 2 + (1 + m * n) * (m * m + 1 / (m * n))))

    def beam(a1: float, a2: float, h1: float, h2: float, e1: float, e2: float, dt: float) -> float:
        # strain eps(y) = eps0 + kappa y - alpha dT; layer 1 is y in [0, h1], layer 2 is [h1, h1+h2]; N = M = 0
        def moments(e: float, y0: float, y1: float) -> tuple[float, float, float]:
            return e * (y1 - y0), e * (y1**2 - y0**2) / 2, e * (y1**3 - y0**3) / 3
        a_1, b_1, d_1 = moments(e1, 0, h1)
        a_2, b_2, d_2 = moments(e2, h1, h1 + h2)
        mat = np.array([[a_1 + a_2, b_1 + b_2], [b_1 + b_2, d_1 + d_2]])
        rhs = np.array([a_1 * a1 * dt + a_2 * a2 * dt, b_1 * a1 * dt + b_2 * a2 * dt])
        return float(np.linalg.solve(mat, rhs)[1])

    print("Timoshenko 1925 bimetal curvature vs plane-section beam theory (pass: agree to 1e-12 relative)")
    for case in ((12e-6, 23e-6, 1e-3, 1e-3, 200e9, 70e9, 50.0), (12e-6, 23e-6, 2e-3, 1e-3, 200e9, 70e9, 50.0),
                 (17e-6, 10e-6, 0.5e-3, 1.5e-3, 120e9, 200e9, -30.0)):
        k1, k2 = timo(*case), beam(*case)
        print(f"  kappa Timoshenko {k1:.6e} 1/m, beam theory {k2:.6e} 1/m, rel diff {abs(k1 - k2) / abs(k2):.1e}")
    k = timo(12e-6, 23e-6, 1e-3, 1e-3, 200e9, 70e9, 50.0)
    print(f"  steel/aluminium 1+1 mm, dT = 50 K: radius {1 / k:.3f} m; tip deflection of a 0.1 m cantilever {k * 0.1**2 / 2 * 1e3:.3f} mm")


def sphere() -> None:
    """Thermoelastic sphere (Timoshenko and Goodier, art. 147): check the radial equation and the free surface."""
    import sympy as sp

    r, s, big_r, al, nu = sp.symbols("r s R alpha nu", positive = True)

    def disp(temp: sp.Expr) -> sp.Expr:
        i_r = sp.integrate(temp.subs(r, s) * s**2, (s, 0, r))
        i_big = sp.integrate(temp.subs(r, s) * s**2, (s, 0, big_r))
        return (1 + nu) / (1 - nu) * al / r**2 * i_r + 2 * (1 - 2 * nu) / (1 - nu) * al * r / big_r**3 * i_big

    print("u = (1+nu)/(1-nu) a/r^2 int_0^r T s^2 ds + 2(1-2nu)/(1-nu) a r/R^3 int_0^R T s^2 ds")
    for name, temp in (("uniform 1", sp.Integer(1)), ("r^2", r**2), ("r^4 + 3", r**4 + 3), ("exp(-r^2)", sp.exp(-r**2))):
        u = disp(temp)
        navier = sp.simplify(sp.diff(sp.diff(r**2 * u, r) / r**2, r) - (1 + nu) / (1 - nu) * al * sp.diff(temp, r))
        srr = ((1 - nu) * sp.diff(u, r) + 2 * nu * u / r - (1 + nu) * al * temp).subs(r, big_r)
        print(f"  T = {name:10s}: Navier residual {navier}; sigma_rr(R) x (1-2nu)/2G = {sp.simplify(srr)}")
    print(f"  uniform T: u = {sp.simplify(disp(sp.Integer(1)))}  (must be alpha r)")
    print("  mass enclosed inside radius r changes by -4 pi rho0 r^2 u(r) (mass conservation, first order); outside the sphere "
          "the field changes only through the displaced surface, which is O(alpha dT R) relative.")


# ======================================= SIGNAL SIZES =======================================

K_EFF = 1000 * 2 * np.pi / 698e-9  # rad/m, n = 1000 kicks of one 698 nm photon
T_HALF = 0.73  # s
S = K_EFF * T_HALF**2  # rad per (m/s^2) of difference in vertical acceleration between the two interferometers
Z_LO, Z_HI = 2.5, 7.5  # m above the floor: interferometer centres, 5 m apart (instrument.md: ifo_sep 5 m)


def d_acc(pos: F64, mass: F64, z_lo: float = Z_LO, z_hi: float = Z_HI, tower_xy: tuple[float, float] = (0.0, 0.0)) -> float:
    """a_z(z_hi) - a_z(z_lo) [m/s^2] at the tower axis from point masses (kg) at pos (N,3); G m dz / r^3 with sign: attraction."""
    out = 0.0
    for z in (z_hi, z_lo):
        d = pos - np.array([tower_xy[0], tower_xy[1], z])
        r3 = np.sum(d * d, axis = 1) ** 1.5
        a = float(np.sum(G * mass * d[:, 2] / r3))
        out += a if z == z_hi else -a
    return out


def phase(pos: F64, mass: F64) -> float:
    return S * d_acc(pos, mass)


def plume_deficit(power_w: float, z0: float, z1: float, alpha: float = 0.083, t0: float = 293.15) -> tuple[F64, F64]:
    """Morton-Taylor-Turner top-hat plume: per-unit-height mass deficit lambda(z) = rho0 B / (g w) [kg/m] on a z grid."""
    rho0, cp, g = 1.2, 1005.0, 9.81
    b = g * power_w / (rho0 * cp * t0)
    z = np.linspace(z0, z1, 400)
    w = 5.0 / (6.0 * alpha) * (0.9 * alpha * b) ** (1 / 3) * z ** (-1 / 3)
    return z, rho0 * b / (g * w)


def plume_phase(power_w: float, dist: float, z_floor: float = 1.0, z_ceiling: float = 2.6, alpha: float = 0.083) -> tuple[float, float]:
    """Phase [rad] and total deficit mass [kg] from a plume rising from a source at z_floor to the ceiling, then a layer."""
    z, lam = plume_deficit(power_w, 0.1, z_ceiling - z_floor, alpha)
    dz = z[1] - z[0]
    m = -lam * dz  # a deficit is a negative mass
    pos = np.stack([np.full_like(z, dist), np.zeros_like(z), z + z_floor], axis = 1)
    return phase(pos, m), float(m.sum())


def box_cells(lo: tuple[float, float, float], hi: tuple[float, float, float], h: float) -> F64:
    axes = [np.arange(l + h / 2, u, h) for l, u in zip(lo, hi)]
    gx, gy, gz = np.meshgrid(*axes, indexing = "ij")
    return np.stack([gx.ravel(), gy.ravel(), gz.ravel()], axis = 1)


def wall_phase(dist: float, mode: str, strain: float = 1e-4, bow_m: float = 4e-3, h: float = 0.1) -> float:
    """Wall 10 m wide x 10 m tall x 0.3 m thick (concrete 2400 kg/m3) with its plane x = dist, centred in y, base at z = 0.

    'rigid' toward the tower: whole wall moves 1 mm; 'strain': in-plane uniform strain about the base centre and
    thickness strain; 'bow': out-of-plane bow with mid-height deflection bow_m, sin profile, away from the tower."""
    cells = box_cells((dist, -5.0, 0.0), (dist + 0.3, 5.0, 10.0), h)
    m = np.full(len(cells), 2400.0 * h**3)
    if mode == "rigid":
        u = np.tile(np.array([-1e-3, 0.0, 0.0]), (len(cells), 1))
    elif mode == "strain":
        rel = cells - np.array([dist + 0.15, 0.0, 0.0])
        u = strain * rel  # isotropic free expansion about the base-centre of the wall
    elif mode == "bow":
        u = np.zeros_like(cells)
        u[:, 0] = bow_m * np.sin(np.pi * cells[:, 2] / 10.0)
    else:
        raise ValueError(mode)
    # first-order: dPhi = sum m_i grad(K)(x_i) . u_i, gradient of the difference kernel by central differences
    eps = 1e-3
    tot = 0.0
    for ax in range(3):
        e = np.zeros(3)
        e[ax] = eps
        kp = np.array([d_acc(c[None, :] + e, np.array([1.0])) for c in cells])
        km = np.array([d_acc(c[None, :] - e, np.array([1.0])) for c in cells])
        tot += float(np.sum(m * (kp - km) / (2 * eps) * u[:, ax]))
    return S * tot


def column_phase(delta_rho: Callable[[F64], F64], dist: float, radius: float, z_lo: float, z_hi: float, h: float = 0.25) -> float:
    """A cylinder-shaped column of air with density anomaly delta_rho(z) at horizontal distance dist (axis), radius."""
    cells = box_cells((dist - radius, -radius, z_lo), (dist + radius, radius, z_hi), h)
    inside = (cells[:, 0] - dist) ** 2 + cells[:, 1] ** 2 <= radius**2
    cells = cells[inside]
    return phase(cells, delta_rho(cells[:, 2]) * h**3)


def tank_phase(dist: float, height: float, area: float, t_profile: Callable[[F64], F64], z_floor: float = 0.0) -> tuple[float, float]:
    """Open tank (square footprint) filled with a fixed mass of water that is uniform at 20 C when cold; with the
    temperature profile T(z) the same mass occupies more height. Returns (phase relative to cold, surface rise [m])."""
    side = np.sqrt(area)
    nz = 200
    zc = (np.arange(nz) + 0.5) * height / nz
    rho_cold = float(iapws_density(np.array([20.0]))[0])
    mass_layer = rho_cold * area * height / nz  # each layer keeps its mass
    # hot: each layer thickness = mass / (rho(T) area), stacked from the floor
    rho_hot = iapws_density(t_profile(zc))
    th = mass_layer / (rho_hot * area)
    z_hot = np.cumsum(th) - th / 2
    rise = float(th.sum() - height)
    nx = 8
    xs = (np.arange(nx) + 0.5) * side / nx - side / 2 + dist
    ys = (np.arange(nx) + 0.5) * side / nx - side / 2
    gx, gy = np.meshgrid(xs, ys, indexing = "ij")
    gx, gy = gx.ravel(), gy.ravel()

    def build(z: F64) -> tuple[F64, F64]:
        pos = np.stack([np.repeat(gx[None, :], len(z), 0).ravel(), np.repeat(gy[None, :], len(z), 0).ravel(),
                        np.repeat(z[:, None] + z_floor, len(gx), 1).ravel()], axis = 1)
        return pos, np.full(len(pos), mass_layer / len(gx))

    p0, m0 = build(zc)
    p1, m1 = build(z_hot)
    return phase(p1, m1) - phase(p0, m0), rise


def signals() -> None:
    print(f"S = k_eff T^2 = {S:.4e} rad per m/s^2; k_eff = {K_EFF:.4e} rad/m; interferometers at z = {Z_LO}, {Z_HI} m")
    # calibration against the plan's numbers
    for d, mm in ((2.0, 70.0),):
        p = phase(np.array([[d, 0.0, 5.0]]), np.array([mm]))
        print(f"calibration: {mm:.0f} kg point mass {d} m from the axis at mid-height: {p:.2f} rad (plan: ~3 rad at 2 m)")
    p = phase(np.array([[10.0, 0.0, 5.0]]), np.array([10.0]))
    print(f"calibration: 10 kg at 10 m, mid-height, static: {p * 1e3:.1f} mrad (instrument.md anchor: ~7 mrad moving at 2.5 m/s)")
    print()
    print("(1) a person as a 100 W source, plume from z = 1 m to a 2.6 m ceiling, MTT top-hat, alpha = 0.083")
    for power in (60.0, 100.0):
        for dist in (1.0, 2.0, 5.0, 10.0):
            ph, mass = plume_phase(power, dist)
            print(f"    P = {power:5.0f} W, D = {dist:4.1f} m: plume deficit {mass * 1e3:7.2f} g, phase {ph:+.2e} rad")
    for a in (0.083, 0.12, 0.16):
        ph, mass = plume_phase(100.0, 2.0, alpha = a)
        print(f"    alpha sensitivity at 100 W, 2 m: alpha = {a}: deficit {mass * 1e3:.2f} g, phase {ph:+.2e} rad")
    ph70 = phase(np.array([[2.0, 0.0, 1.0]]), np.array([70.0]))
    print(f"    for scale: the person's own 70 kg at 2 m (z = 1 m) reads {ph70:.2f} rad; plume / body = {plume_phase(100.0, 2.0)[0] / ph70:+.1e}")
    print()
    print("(2) a radiator: 500 W convective plume, 0.3 to 2.6 m, at D = 1, 2, 3 m; v2 recorded 2.2e-2 rad at 1 m for a 70 C panel")
    for dist in (1.0, 2.0, 3.0):
        ph, mass = plume_phase(500.0, dist, z_floor = 0.3)
        print(f"    D = {dist:3.1f} m: deficit {mass * 1e3:7.1f} g, phase {ph:+.2e} rad")
    print()
    print("(2b) the whole air of a room warmed uniformly: 1 K, leaky room (constant pressure)")
    rho_air = 1.2
    drho = -rho_air / 293.15 * 1.0
    for name, radius, zl, zh in (("shaft 1 m radius, 0-10 m", 1.0, 0.0, 10.0), ("hall 5 m radius, 0-6 m", 5.0, 0.0, 6.0),
                                 ("hall 5 m radius, 0-12 m", 5.0, 0.0, 12.0), ("hall 10 m radius, 0-12 m", 10.0, 0.0, 12.0)):
        ph = column_phase(lambda z: np.full_like(z, drho), 0.0, radius, zl, zh, h = 0.5 if radius > 2 else 0.25)
        print(f"    {name}: phase per kelvin of whole-room temperature change {ph:+.2e} rad")
    print(f"    infinite horizontal slab (closed form): dPhi = 4 pi G S |drho| dz = {4 * np.pi * G * S * abs(drho) * 5.0:.2e} rad per K "
          "for the 5 m between the interferometers")
    print()
    print("(3) sunlit concrete wall 10 x 10 x 0.3 m, 2400 kg/m3 (72 t), plane at D, free expansion, strain 1e-4 (alpha 1e-5 x dT 10 K)")
    for dist in (5.0, 10.0):
        for mode in ("strain", "bow", "rigid"):
            if mode == "bow":
                ph = wall_phase(dist, mode, bow_m = 4e-3)
            else:
                ph = wall_phase(dist, mode)
            print(f"    D = {dist:4.1f} m, {mode:6s}: {ph:+.2e} rad")
    print()
    print("(4) warm water: open tank, 1 m tall, 1 m^2, fixed mass of water from 20 C; D to axis")
    for label, prof in (("uniform 60 C", lambda z: np.full_like(z, 60.0)),
                        ("uniform 30 C", lambda z: np.full_like(z, 30.0)),
                        ("stratified 20 C bottom to 60 C top", lambda z: 20.0 + 40.0 * z / 1.0)):
        for dist in (2.0, 5.0):
            ph, rise = tank_phase(dist, 1.0, 1.0, prof)
            print(f"    {label:36s} D = {dist:3.1f} m: surface rise {rise * 1e3:6.2f} mm, phase {ph:+.2e} rad")
    for label, area, prof in (("200 L cylinder (0.2 m^2 x 1 m) uniform 60 C", 0.2, lambda z: np.full_like(z, 60.0)),
                              ("1 m^2 x 1 m tank, 20 -> 21 C", 1.0, lambda z: np.full_like(z, 21.0))):
        ph, rise = tank_phase(2.0, 1.0, area, prof)
        print(f"    {label:44s} D = 2.0 m: surface rise {rise * 1e3:6.3f} mm, phase {ph:+.2e} rad")
    print()
    print("(5) the plan's closed forms for the read itself")
    for dz in (1.0,):
        print(f"    infinite stratified column: Delta a = -4 pi G int drho dz; with drho = -4e-3 kg/m3 over 5 m: S x that = "
              f"{S * 4 * np.pi * G * 4e-3 * 5.0:.3e} rad")
    print()
    print("Noise: one-sided PSD 1e-4 rad/sqrt(Hz); one shot at 2 s cadence (f_N = 0.25 Hz): "
          f"{1e-4 * np.sqrt(0.25):.1e} rad; a mean over 1 h: {1e-4 / np.sqrt(2 * 3600):.1e} rad; over 1 day: {1e-4 / np.sqrt(2 * 86400):.1e} rad")


# ======================================= LINEARISED SCALARS =======================================


def linearised() -> None:
    """Quasi-static gradient and the non-linearity of the density read: second order in dT."""
    for dt in (1.0, 10.0, 50.0):
        r = 1.2 * (293.15 / (293.15 + dt) - 1.0)
        lin = -1.2 * dt / 293.15
        print(f"  air dT = {dt:4.0f} K: exact anomaly {r:+.5f} kg/m3, linear {lin:+.5f}; second-order share {abs(r - lin) / abs(lin):.2%}")


# ======================================= LINEAR THEORY, CORRELATIONS, ELEMENTS =======================================


def rb_critical(bc: str) -> tuple[float, float]:
    """Critical Rayleigh number and wavenumber of a layer, bc in {rigid-rigid, rigid-free, free-free}, by Chebyshev collocation.

    (D^2 - k^2)^2 W = k^2 Ra Theta ; (D^2 - k^2) Theta = -W on z in [0, 1]; Theta = 0 at both plates."""
    from scipy.linalg import eig
    from scipy.optimize import minimize_scalar

    n = 28
    x = np.cos(np.pi * np.arange(n + 1) / n)
    c = np.hstack([2.0, np.ones(n - 1), 2.0]) * (-1.0) ** np.arange(n + 1)
    dx = x[:, None] - x[None, :]
    d = np.outer(c, 1.0 / c) / (dx + np.eye(n + 1))
    d -= np.diag(d.sum(axis = 1))
    d2 = 2.0 * d  # z = (x + 1)/2 so d/dz = 2 d/dx; x = +1 is z = 1 (index 0), x = -1 is z = 0 (index n)
    i = np.eye(n + 1)

    def ra(k: float) -> float:
        lap = d2 @ d2 - k * k * i
        a = np.zeros((2 * n + 2, 2 * n + 2))
        b = np.zeros_like(a)
        a[: n + 1, : n + 1] = lap @ lap
        b[: n + 1, n + 1:] = k * k * i
        a[n + 1:, : n + 1] = i
        a[n + 1:, n + 1:] = lap
        rows = {"W0": 0, "W1": 1, "Wn1": n - 1, "Wn": n}
        for r in rows.values():
            a[r, :] = 0.0
            b[r, :] = 0.0
        a[0, 0], a[n, n] = 1.0, 1.0  # W = 0 at both plates
        lower, upper = bc.split("-")
        a[1, : n + 1] = d2[0] if upper == "rigid" else (d2 @ d2)[0]  # z = 1 plate: W' = 0 or W'' = 0
        a[n - 1, : n + 1] = d2[n] if lower == "rigid" else (d2 @ d2)[n]
        for r in (n + 1, 2 * n + 1):
            a[r, :] = 0.0
            b[r, :] = 0.0
            a[r, r] = 1.0  # Theta = 0
        w = eig(a, b, right = False)
        w = w[np.isfinite(w) & (np.abs(w.imag) < 1e-6 * np.abs(w.real) + 1e-9) & (w.real > 0)].real
        return float(w.min())

    res = minimize_scalar(ra, bounds = (1.0, 5.0), method = "bounded", options = {"xatol": 1e-9})
    return float(res.fun), float(res.x)


def churchill_chu(ra: float, pr: float, laminar: bool = False) -> float:
    if laminar:
        return 0.68 + 0.670 * ra**0.25 / (1 + (0.492 / pr) ** (9 / 16)) ** (4 / 9)
    return (0.825 + 0.387 * ra ** (1 / 6) / (1 + (0.492 / pr) ** (9 / 16)) ** (8 / 27)) ** 2


def references() -> None:
    from scipy.special import erf

    print("Rayleigh-Benard linear stability, Chebyshev collocation (N = 28), Pr-independent for the marginal state:")
    for bc in ("rigid-rigid", "rigid-free", "free-free"):
        r, k = rb_critical(bc)
        print(f"  {bc:12s}: Ra_c = {r:10.3f}, k_c = {k:.4f}   (free-free closed form 27 pi^4/4 = {27 * np.pi**4 / 4:.3f}, k = pi/sqrt2 = {np.pi / np.sqrt(2):.4f})")
    print(f"  box width pi/k_c for one half-wavelength with free-slip adiabatic side walls: Gamma = {np.pi / 3.1163:.4f} x depth")
    print("Schluter-Lortz-Busse weakly nonlinear slope (recalled): Nu - 1 = eps / (0.69942 - 0.00472/Pr + 0.00832/Pr^2):")
    for pr in (0.71, 7.0, 1e9):
        print(f"    Pr = {pr:8.2f}: slope {1 / (0.69942 - 0.00472 / pr + 0.00832 / pr**2):.4f}")
    print("Churchill-Chu isothermal vertical plate, Pr = 0.71 (air); Nu_L = hL/k (all-range form, laminar form for Ra < 1e9):")
    for ra in (1e3, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10):
        lam = churchill_chu(ra, 0.71, laminar = True) if ra <= 1e9 else float("nan")
        print(f"    Ra_L = {ra:8.0e}: Nu = {churchill_chu(ra, 0.71):8.2f}  laminar form {lam:8.2f}   boundary layer ~ L/Nu = {1 / churchill_chu(ra, 0.71):.4f} L")
    print(f"  worked example in the textbook source (Pr 0.69, Gr 2.63e9 -> Ra {0.69 * 2.63e9:.3e}): Nu = {churchill_chu(0.69 * 2.63e9, 0.69):.2f} (ht docs: 147.16)")
    # closed forms used in the proposed cases
    print("Fourier-series plate, one hot edge of a square: centre temperature by series vs the symmetry value 1/4:")
    for nmax in (1, 3, 21, 201):
        s = sum(4 / np.pi / m * np.sin(m * np.pi / 2) * np.sinh(m * np.pi / 2) / np.sinh(m * np.pi) for m in range(1, nmax + 1, 2))
        print(f"    terms up to n = {nmax:3d}: T(centre)/T0 = {s:.6f}")
    # heat kernel: enclosed fraction of a spreading Gaussian
    print("Heat kernel enclosed fraction F(r, t) = erf(r / 2 sqrt(kappa t)) - r/sqrt(pi kappa t) exp(-r^2 / 4 kappa t), checked by quadrature:")
    kappa, t = 1.0, 0.7
    for r in (0.5, 1.0, 2.0, 4.0):
        rr = np.linspace(0.0, r, 200001)
        dens = (4 * np.pi * kappa * t) ** -1.5 * np.exp(-rr**2 / (4 * kappa * t)) * 4 * np.pi * rr**2
        q = float(np.sum((dens[1:] + dens[:-1]) / 2 * np.diff(rr)))
        f = float(erf(r / (2 * np.sqrt(kappa * t))) - r / np.sqrt(np.pi * kappa * t) * np.exp(-(r**2) / (4 * kappa * t)))
        print(f"    r = {r:3.1f}: formula {f:.8f}, quadrature {q:.8f}")
    print("  mass deficit of 100 J released as a point in air at 293.15 K (constant pressure): "
          f"beta Q / c_p = {100.0 / (1005.0 * 293.15) * 1e3:.3f} g")
    print("Dissipation number g beta L / c_p (neglected by Boussinesq): "
          f"air 3 m {9.81 / 293.15 * 3 / 1005:.1e}; water 1 m at 20 C {9.81 * 2.07e-4 * 1 / 4182:.1e}")
    print("Buoyant plume (top-hat MTT), B = g P/(rho cp T0): volume flux at 1 m for 100 W, alpha = 0.083 "
          f"{(6 * np.pi * 0.083 / 5) * (0.9 * 0.083 * 9.81 * 100 / (1.2 * 1005 * 293.15)) ** (1 / 3) * 1e3:.1f} L/s")
    print("Solid conduction explicit limits dt = h^2/(6 alpha), alpha: concrete 7e-7, steel 1.3e-5, copper 1.1e-4, air 2.2e-5, water 1.4e-7 m^2/s")
    for h in (0.05, 0.01, 0.005):
        print(f"    h = {h * 100:4.1f} cm: " + ", ".join(f"{nm} {h * h / (6 * al):9.2f} s" for nm, al in
                                                      (("concrete", 7e-7), ("steel", 1.3e-5), ("copper", 1.1e-4), ("air", 2.2e-5), ("water", 1.4e-7))))
    print("Radiative coefficient 4 sigma T^3 at 293 K: "
          f"{4 * 5.670374419e-8 * 293.15**3:.2f} W/m2/K (times emissivity); sealed-room pressure rate (gamma-1) P / V for 100 W in 50 m3: "
          f"{0.4 * 100 / 50:.2f} Pa/s")


def q4_beam(ny: int, wilson: bool) -> float:
    """Free beam, L = 10 h, thermal gradient across the thickness; returns kappa_fe / kappa_exact (plane stress, E = 1, nu = 0.3)."""
    e, nu, h = 1.0, 0.3, 1.0 / ny
    nx = 10 * ny
    dmat = e / (1 - nu**2) * np.array([[1, nu, 0], [nu, 1, 0], [0, 0, (1 - nu) / 2]])
    nn = (nx + 1) * (ny + 1)
    k = np.zeros((2 * nn, 2 * nn))
    f = np.zeros(2 * nn)
    gp = np.array([-1.0, 1.0]) / np.sqrt(3.0)
    xi_n = np.array([-1, 1, 1, -1.0])
    eta_n = np.array([-1, -1, 1, 1.0])
    for j in range(ny):
        for i in range(nx):
            ids = [j * (nx + 1) + i, j * (nx + 1) + i + 1, (j + 1) * (nx + 1) + i + 1, (j + 1) * (nx + 1) + i]
            dofs = np.array([[2 * a, 2 * a + 1] for a in ids]).ravel()
            ke = np.zeros((8, 8)); kua = np.zeros((8, 4)); kaa = np.zeros((4, 4))
            fe = np.zeros(8); fa = np.zeros(4)
            for xi in gp:
                for eta in gp:
                    dn = np.stack([xi_n * (1 + eta_n * eta) / 4, eta_n * (1 + xi_n * xi) / 4])  # dN/dxi, dN/deta
                    dnx = dn * (2.0 / h)
                    b = np.zeros((3, 8))
                    b[0, 0::2], b[1, 1::2], b[2, 0::2], b[2, 1::2] = dnx[0], dnx[1], dnx[1], dnx[0]
                    ycoord = (j + (eta + 1) / 2) * h
                    eth = np.array([1.0, 1.0, 0.0]) * 1.0 * (ycoord - 0.5)  # alpha = 1, T = y - 1/2 (dT across thickness = 1)
                    w = (h / 2) ** 2
                    ke += b.T @ dmat @ b * w
                    fe += b.T @ dmat @ eth * w
                    if wilson:
                        gx = np.array([-2 * xi, 0.0]) * (2.0 / h)  # d(1 - xi^2)/dx, dy
                        gy = np.array([0.0, -2 * eta]) * (2.0 / h)
                        ba = np.zeros((3, 4))
                        ba[0, 0], ba[0, 1] = gx[0], gy[0]
                        ba[1, 2], ba[1, 3] = gx[1], gy[1]
                        ba[2, 0], ba[2, 1], ba[2, 2], ba[2, 3] = gx[1], gy[1], gx[0], gy[0]
                        kua += b.T @ dmat @ ba * w
                        kaa += ba.T @ dmat @ ba * w
                        fa += ba.T @ dmat @ eth * w
            if wilson:
                sol = np.linalg.solve(kaa, kua.T)
                ke = ke - kua @ sol
                fe = fe - kua @ np.linalg.solve(kaa, fa)
            k[np.ix_(dofs, dofs)] += ke
            f[dofs] += fe
    fixed = [0, 1, 2 * nx + 1]  # node 0 both dofs, bottom-right node y
    free = np.setdiff1d(np.arange(2 * nn), fixed)
    u = np.zeros(2 * nn)
    u[free] = np.linalg.solve(k[np.ix_(free, free)], f[free])
    mid = nx // 2
    return float(-8.0 * u[2 * mid + 1] / (nx * h) ** 2)


def beam() -> None:
    print("Pure thermal bending of a free beam, L = 10 h, plane stress, square elements: kappa_FE / (alpha dT / h)")
    print("  elements through thickness:    1       2       4       8")
    for wil in (False, True):
        r = [q4_beam(n, wil) for n in (1, 2, 4, 8)]
        print(f"  {'Q4 + incompatible modes' if wil else 'Q4 (Q1 hex analogue)  '}: " + "  ".join(f"{x:6.4f}" for x in r))


def spline() -> None:
    from scipy.interpolate import CubicSpline

    for step, lo, hi in ((1.0, 0.0, 95.0), (2.0, 0.0, 95.0), (0.5, 0.0, 12.0)):
        nodes = np.arange(lo, hi + 1e-9, step)
        sp = CubicSpline(nodes, iapws_density(nodes))
        tt = np.linspace(lo, hi, 400)
        err = np.abs(sp(tt) - iapws_density(tt))
        derr = np.abs(sp(tt, 1) - np.gradient(iapws_density(tt), tt))
        print(f"  cubic spline, {step} K nodes over {lo}-{hi} C: max |rho error| {err.max() * 1e3:.4f} g/m3; max drho/dT error (finite difference) {derr[2:-2].max():.5f} kg/m3/K")
    from scipy.optimize import minimize_scalar
    res = minimize_scalar(lambda t: -float(iapws_density(np.array([t]))[0]), bracket = (3.0, 4.0, 5.0), tol = 1e-10)
    print(f"  IAPWS-95 density maximum, refined: {res.x:.4f} C, {-res.fun:.5f} kg/m3")


def main() -> None:
    parts = {"references": references, "beam": beam, "spline": spline, "water": water, "views": views, "bimetal": bimetal, "sphere": sphere, "signals": signals, "linearised": linearised}
    which = sys.argv[1] if len(sys.argv) > 1 else "all"
    for name, fn in parts.items():
        if which in (name, "all"):
            print(f"==================== {name} ====================")
            fn()
            print()


if __name__ == "__main__":
    main()
