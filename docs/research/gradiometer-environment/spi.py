"""The surface-pressure-independent (SPI) air-mass signal, carried to the AION-10 gradiometer.

Standard library only. Run: python -I spi.py > spi.out.txt

Neumeyer et al. 2004 (J. Geodyn. 38, 437, Section 6, Fig. 7-8) and Abe et al. (BIM 146, Section 3-1,
Fig. 6) find that air moving aloft at constant surface pressure changes surface gravity by ~2 uGal
(Neumeyer, max of the difference, Vienna) and 4.5 uGal peak to peak from the zone within 0.5 deg
(Abe, Moxa). A barometer-driven model (the note's row 13) cannot see this part.

A gradiometer sees only its lateral structure. For any mass distribution above the sensor whose
horizontal structure has wavenumber k, the field below it obeys Laplace's equation and decays as
exp(-k (h - z)), so the vertical gradient at the ground is exactly k times the gravity amplitude,
whatever the heights involved. A laterally uniform part gives zero gradient (an infinite slab).
The papers do not give the SPI field's horizontal scale, so it is swept: 100 km (about Abe's
innermost zone), 300 km and 1000 km (synoptic).

Pass criterion, written before running: if every case is at most the note's row 13 range
(6e-3 to 9e-3 rad, rounded up to 1e-2), no row moves and the SPI part is a remark under row 13.
"""

import math

# The note's instrument constant (sizes.py): C = k_eff T^2 dr, rad per s^-2 of uniform Gamma_zz.
LAMBDA = 698.0e-9
N_KICK = 1000
T = 0.73
DR = 5.0
KEFF = N_KICK * 2 * math.pi / LAMBDA
C_GRAD = KEFF * T**2 * DR

ROW13_MAX = 1e-2  # rad, row 13 range rounded up

print(f"C = k_eff T^2 dr = {C_GRAD:.3e} rad/s^-2")
print(f"{'source':<44}{'amp nm/s^2':>11}{'lambda km':>11}{'Gamma s^-2':>12}{'phase rad':>11}  pass")
cases = (
    ("Neumeyer 2004 Fig. 7, max difference 2 uGal", 20.0),
    ("Abe Fig. 6a, 4.5 uGal p-p, amplitude", 22.5),
)
worst = 0.0
for name, amp_nms2 in cases:
    for lam_km in (100.0, 300.0, 1000.0):
        k = 2 * math.pi / (lam_km * 1e3)
        gamma = k * amp_nms2 * 1e-9
        phase = C_GRAD * gamma
        worst = max(worst, phase)
        ok = "yes" if phase <= ROW13_MAX else "no"
        print(f"{name:<44}{amp_nms2:>11.1f}{lam_km:>11.0f}{gamma:>12.2e}{phase:>11.2e}  {ok}")

# Control arm: the formula must give row 13 back when fed row 13's own gravity amplitude.
# Row 13 at lambda = 1000 km, 10 hPa: Bouguer 2 pi G rho0 H dp / p0 times kH/(1+kH), divided by H
# (the gradient is k g for kH << 1). Recompute g from row 13's formula and compare.
G = 6.674_30e-11
H = 8400.0
P0 = 101_325.0
RHO0 = 1.225
k = 2 * math.pi / 1e6
gam13 = 2 * math.pi * G * RHO0 / P0 * 1000.0 * k * H / (1 + k * H)
g13 = 2 * math.pi * G * RHO0 / P0 * 1000.0 * H
print(f"control: row 13 gradient {gam13:.3e} vs k*g {k * g13:.3e} (ratio {gam13 / (k * g13):.4f}, "
      f"1/(1+kH) = {1 / (1 + k * H):.4f})")
print(f"worst SPI case {worst:.2e} rad; criterion {'met' if worst <= ROW13_MAX else 'NOT met'}")
