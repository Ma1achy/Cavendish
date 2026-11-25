"""Sizing estimates for the elastic-solids note. Each printed line states its assumptions."""
import numpy as np

G = 6.674e-11
KT2 = 9.00e9 * 0.5329  # AION-10 k_eff T^2 [rad per m/s^2], from docs/research/s14-wider-world.md (E1)
T = np.sqrt(0.5329)     # s
DT = 2.0                # cycle [s] (Nyquist 0.25 Hz in the S14 note)


def H(f):
    """|H(f)|/T^2 of the three-pulse sequence for a differential acceleration (triangle weight)."""
    x = np.pi * f * T
    return np.where(x == 0, 1.0, (np.sin(x) / x) ** 2)


print("(a) AION-10 response to a differential acceleration at f (T = %.3f s)" % T)
for f in (0.01, 0.1, 0.25, 0.5, 1.0, 1.37, 2, 5, 10, 20, 50):
    env = 1 / (np.pi * f * T) ** 2
    print(f"   f = {f:6.2f} Hz   |H|/T^2 = {float(H(f)):.2e}   envelope 1/(pi f T)^2 = {min(env, 1):.2e}")

print("\n(b) a slab sagging under a person, against the person (point estimates)")
E, nu, rho, h, a = 30e9, 0.2, 2400.0, 0.20, 6.0
Dp = E * h ** 3 / (12 * (1 - nu ** 2))
P = 70 * 9.81
xs = (np.arange(120) + 0.5) * a / 120
X, Y = np.meshgrid(xs, xs, indexing="ij")
w = np.zeros_like(X)
for m in range(1, 60, 2):
    for n in range(1, 60, 2):
        w += (np.sin(m * np.pi / 2) * np.sin(n * np.pi / 2) * np.sin(m * np.pi * X / a)
              * np.sin(n * np.pi * Y / a) / ((m / a) ** 2 + (n / a) ** 2) ** 2)
w *= 4 * P / (np.pi ** 4 * Dp * a * a)
dA = (a / 120) ** 2
p = rho * h * np.sum(w) * dA  # downward mass dipole [kg m]
print(f"   slab {a}x{a}x{h} m concrete, simply supported, {P:.0f} N at the centre: w_max = {w.max()*1e6:.1f} um,"
      f" mass dipole {p:.3f} kg m")


def az_point(m, src, z):
    d = np.array([0, 0, z]) - src
    r = np.linalg.norm(d)
    return -G * m * d[2] / r ** 3


def az_dipole_field(z, Xc, Yc, dm_dz):
    """a_z at (0,0,z) from parcels at (Xc, Yc, 0) displaced vertically by dz with mass dm:
    delta a = -(dz) d/dz_src of the point-mass field, done exactly by finite difference."""
    eps = 1e-4
    def field(zs):
        dx, dy, ddz = -Xc, -Yc, z - zs
        r = np.sqrt(dx * dx + dy * dy + ddz * ddz)
        return -G * ddz / r ** 3
    return np.sum(dm_dz * (field(eps) - field(-eps)) / (2 * eps))


z1, z2 = 1.0, 6.0
for d in (0.5, 2.0):  # slab edge distance from the tower axis
    Xc = X + d; Yc = Y - a / 2
    dmdz = -rho * h * w * dA  # each parcel's mass times its (downward) displacement
    da_slab = az_dipole_field(z2, Xc, Yc, dmdz) - az_dipole_field(z1, Xc, Yc, dmdz)
    person = np.array([d + a / 2, 0, 1.0])
    da_person = az_point(70, person, z2) - az_point(70, person, z1)
    print(f"   slab edge {d} m from the tower (person {d + a/2} m away, CoM 1 m up), arms at {z1}, {z2} m:"
          f" person {KT2 * da_person:+.2e} rad, slab sag {KT2 * da_slab:+.2e} rad"
          f" (ratio {da_slab / da_person:+.1e})")

print("\n(c) thermoelastic coupling number delta = E^2 alpha^2 T0 / ((1-2nu)^2 rho c (lambda+2mu))")
for name, E, nu, a_, rho, c in [("steel", 200e9, 0.30, 12e-6, 7850, 460),
                                 ("aluminium", 70e9, 0.33, 23e-6, 2700, 900),
                                 ("concrete", 30e9, 0.20, 10e-6, 2400, 880),
                                 ("granite", 50e9, 0.25, 8e-6, 2650, 790)]:
    lam = E * nu / ((1 + nu) * (1 - 2 * nu)); mu = E / (2 * (1 + nu))
    threeK = 3 * lam + 2 * mu
    delta = threeK ** 2 * a_ ** 2 * 293 / ((lam + 2 * mu) * rho * c)
    print(f"   {name:9s} delta = {delta:.1e}")

print("\n(d) modal read cost: one weight per mode and tower, then a dot product per shot")
for leaves, modes, towers in [(1e6, 64, 5), (1e8, 64, 5), (1e9, 256, 5)]:
    print(f"   {leaves:.0e} leaves x {modes} modes x {towers} towers: weights = {leaves*modes*towers:.1e} kernel"
          f" evaluations once; per shot {modes*towers} multiply-adds")
