"""Highest point an arm reaches, from the engine's arm formulas (forward.rs, Arm), for the two presets."""
import numpy as np
hbar = 1.054571817e-34; m = 1.44315587e-25; vk = 1000 * hbar * 2 * np.pi / 698e-9 / m
g = 9.81
def top_of_upper_ifo(u0, T, dr):
    v1 = u0 + vk                      # the upper arm leaves faster by one kick
    rise_T = v1 * T - 0.5 * g * T * T
    v = v1 - g * T - vk               # after the pi pulse the kick is removed
    apex = rise_T + (v * v / (2 * g) if v > 0 else 0.0)
    return dr + max(rise_T, apex)
for name, u0, T, dr, tower in [("AION-10", 3.86, 0.73, 5.0, 10.0), ("AICE", g * 2.5 - vk / 2, 2.5, 70.0, 100.0)]:
    print(f"{name}: u0={u0:.3f} m/s, highest arm point {top_of_upper_ifo(u0, T, dr):.3f} m, tower {tower} m")
