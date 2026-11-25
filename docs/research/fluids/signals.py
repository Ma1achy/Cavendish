"""Signal sizes that set the fluid's accuracy requirements (AION-10 gradiometer).

1. Phase from a point mass of 1 g near a tower, so a mass error can be read in radians.
2. The interferometer's response to an oscillating gradient, against frequency, so we know
   which pressure (acoustic) frequencies reach the phase at all.
3. The density change per pascal and per kelvin of air, to put pressure waves on the same
   scale as the heat note's per-kelvin figure.

Constants from docs/research/s13-heat.md and exact-skin-and-partial-fill.md:
S = k_eff T^2 = 4.80e9 rad per m s^-2, baseline 5 m, T = 0.73 s, one shot 5e-5 rad,
one-hour mean 1.2e-6 rad.
"""

import numpy as np

G = 6.674e-11
S = 4.80e9
BASE = 5.0
T = 0.73
SHOT = 5e-5
HOUR = 1.2e-6


def phase_point(m: float, src: np.ndarray) -> float:
    """Phase k_eff T^2 [g_z(upper) - g_z(lower)] for a point mass m (kg) at src (m), lower point at origin."""
    out = 0.0
    for z, sign in ((0.0, -1.0), (BASE, 1.0)):
        d = src - np.array([0.0, 0.0, z])
        r = np.linalg.norm(d)
        out += sign * G * m * d[2] / r**3
    return S * out


def main() -> None:
    print("1. phase of 1 g, by position relative to the lower interferometer point")
    for dist in (1.0, 2.0, 5.0, 10.0):
        beside = abs(phase_point(1e-3, np.array([dist, 0.0, 0.0])))
        below = abs(phase_point(1e-3, np.array([0.0, 0.0, -dist])))
        print(
            f"   D = {dist:5.1f} m: beside {beside:.2e} rad, below {below:.2e} rad;"
            f" mass for one shot: {SHOT / max(beside, below) * 1e3:.3g} g,"
            f" for a 1 h mean: {HOUR / max(beside, below) * 1e3:.3g} g"
        )

    print("2. response to an oscillating uniform gradient, relative to a static one")
    # phase = k int g(t) f(t) dt with the triangle f (t on [0,T], 2T - t on [T,2T]);
    # |F(w)| / F(0) = (sin(wT/2) / (wT/2))^2, checked here by quadrature.
    t = np.linspace(0.0, 2 * T, 200001)
    f = np.where(t < T, t, 2 * T - t)
    for freq in (0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 17.0, 100.0):
        w = 2 * np.pi * freq
        num = abs(np.trapezoid(f * np.exp(1j * w * t), t)) / np.trapezoid(f, t)
        x = w * T / 2
        closed = (np.sin(x) / x) ** 2
        env = 1.0 / x**2
        print(f"   f = {freq:6.2f} Hz: |H| = {num:.3e} (closed form {closed:.3e}, envelope {env:.1e})")

    print("3. air: density change per pascal and per kelvin")
    p0, t0, gamma, r_air = 101325.0, 293.15, 1.4, 287.05
    rho0 = p0 / (r_air * t0)
    c = np.sqrt(gamma * p0 / rho0)
    per_pa = 1.0 / c**2  # adiabatic acoustic
    per_k = rho0 / t0  # isobaric
    print(f"   rho0 = {rho0:.4f} kg/m^3, c = {c:.1f} m/s")
    print(f"   per Pa (adiabatic) {per_pa:.3e} kg/m^3; per K (isobaric) {per_k:.3e} kg/m^3")
    print(f"   1 Pa is equivalent to {per_pa / per_k * 1e3:.2f} mK of the same air")
    print(f"   with the heat note's 2e-2 rad per K of hall air: 1 Pa ~ {2e-2 * per_pa / per_k:.1e} rad")
    print(f"   acoustic crossing of a 20 m hall: {20 / c * 1e3:.0f} ms; lowest mode of a 20 m hall: {c / 40:.1f} Hz")

    print("4. water: hydrostatic pressure and spurious-current scale")
    print(f"   1 m^3 of water = 1000 kg; 1e-6 relative = 1 g; f32 epsilon 6e-8 of a 1 cm cell = {6e-8 * 1e-6 * 1000 * 1e3:.1e} g")


if __name__ == "__main__":
    main()
