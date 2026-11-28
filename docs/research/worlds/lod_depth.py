"""How many octaves must a directly generated node's summary include? (follow-up to lod_mass.py)

    python docs/research/worlds/lod_depth.py           # about two minutes; writes lod_depth.out.txt

With mass+dipole-preserving octaves (lod_mass.py, mode 2) a node's mass and horizontal first
moments are exact from the coarse octaves alone; what is left is the vertical moment
rho/2 * int h^2, which is not linear in the octaves. Here the summary includes every octave whose
lattice spacing is above S / (K * 2^j), j = 0..4, and the phase error of the vertical part is
measured against the full sum, at the same nodes as lod_mass.py.

Expectation, written before running: each extra octave cuts the residual by about 4 (the dropped
octaves' squared amplitude, gain 1/2), so j = 4 is about 250x below j = 0.
"""

from __future__ import annotations

from pathlib import Path

import numpy as np

import lod_mass as lm


def main() -> None:
    lines: list[str] = []
    rnd = np.random.default_rng(7)
    for rng_m, side in [(250.0, 64.0), (1000.0, 256.0), (4000.0, 1024.0)]:
        for trial in range(3):
            ang = rnd.uniform(0, 2 * np.pi)
            cx, cy = rng_m * np.cos(ang), rng_m * np.sin(ang)
            x0 = np.floor((cx - side / 2) / side) * side
            y0 = np.floor((cy - side / 2) / side) * side
            truth = lm.moments(x0, y0, side, lm.SPACINGS[-1], list(range(len(lm.SPACINGS))), 2)
            errs: list[str] = []
            for j in range(5):
                octs = [o for o in range(len(lm.SPACINGS)) if lm.SPACINGS[o] * lm.K_BLOCK * 2**j > side]
                summ = lm.moments(x0, y0, side, min(lm.SPACINGS[octs[-1]], side), octs, 2)
                only_z = truth.copy()
                only_z[3] = summ[3]
                errs.append(f"{abs(lm.phase_of(only_z) - lm.phase_of(truth)):.1e}")
            lines.append(f"R={rng_m:6.0f} S={side:5.0f} t{trial}: |z part| for j=0..4: " + " ".join(errs))
    text = "\n".join(lines)
    print(text)
    Path(__file__).with_name("lod_depth.out.txt").write_text(text + "\n", encoding = "utf-8")


if __name__ == "__main__":
    main()
