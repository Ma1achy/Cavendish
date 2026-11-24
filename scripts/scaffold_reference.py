#!/usr/bin/env python3
# pyright: strict
"""The scaffold's potential by George's own integrand, integrated tightly: the reference the
engine's scaffold is held to where the scripts' tables are not accurate enough.

The scripts (``scaffold``, ``scaffold_multimode``) integrate each tube's potential with
``scipy.integrate.nquad`` at its default tolerances, an *absolute* 1.49e-8 on values of about
1e-9 J/kg: the first estimate is kept, good to about 2e-7 of the potential. The change of the
potential as the scaffold moves (the signal) is about 1e-15 J/kg, so the scripts' tables of it are
mostly that error (measured on 24 Nov 2026: −1.80e-14 J/kg in the table against +9.7e-16 tightly
at α = −0.636, z = 7.57 m). This harness evaluates the scripts' own ``cylinder_potential``
integrand (copied in its arithmetic, not in its text) with ``epsabs = 0`` and ``epsrel = 1e-12``
at a sample of the tables' points, and writes them beside the fixtures:

    <out>/<case>/tight_alpha.npy   (A,)    the α of each row, from the case's alpha_grid
    <out>/<case>/tight_z.npy       (Z,)    the heights, metres, from the case's z_grid
    <out>/<case>/tight_V.npy       (A, Z)  the potential on the tower's axis, J/kg

The members and amplitudes are the fixtures' own (``member_*.npy``, ``amplitudes.npy``).

Usage:
    python -I scripts/scaffold_reference.py <fixtures-dir>

Needs numpy and scipy.
"""

from __future__ import annotations

import sys
import time
from pathlib import Path

import numpy as np
from numpy.typing import NDArray
from scipy import integrate  # pyright: ignore[reportMissingTypeStubs]

Array = NDArray[np.float64]

G       = 6.67430e-11   # the scripts' G
EPS_REL = 1e-12
CASES   = ("scaffold", "scaffold_multimode")


def tube(point: Array, centre: Array, mass: float, radius: float, length: float, axis: int) -> float:
    """The potential at ``point`` of one solid tube, J/kg: the scripts' integrand, tightly integrated.

    Args:
        point  (Array): Where, metres.
        centre (Array): The tube's centre, metres.
        mass   (float): Kilograms.
        radius (float): Metres.
        length (float): Metres.
        axis   (int):   0, 1, 2 for x, y, z.

    Returns:
        float: The potential, J/kg.
    """
    density = mass / (np.pi * radius ** 2 * length)
    along   = axis
    across  = [a for a in range(3) if a != axis]

    def integrand(zp: float, rp: float, theta: float) -> float:
        local          = np.zeros(3)
        local[along]   = zp
        local[across[0]] = rp * np.cos(theta)
        local[across[1]] = rp * np.sin(theta)
        r = float(np.linalg.norm(point - centre - local))
        return -G * density * rp / r

    bounds = [[-length / 2, length / 2], [0.0, radius], [0.0, 2 * np.pi]]
    value: float = integrate.nquad(integrand, bounds, opts = {"limit": 200, "epsabs": 0.0, "epsrel": EPS_REL})[0]  # pyright: ignore[reportUnknownMemberType, reportUnknownVariableType]
    return value


def main(arguments: list[str]) -> int:
    """Writes the tight potentials for both cases. ``arguments`` is the fixtures' directory."""
    if len(arguments) != 1:
        print(__doc__)
        return 2
    root = Path(arguments[0]).resolve()
    for case in CASES:
        directory = root / case
        load      = lambda name: np.load(directory / f"{name}.npy").astype(np.float64)  # noqa: E731
        alphas, zs = load("alpha_grid"), load("z_grid")
        centres    = load("member_centre").reshape(-1, 3)
        masses, radii, lengths, axes, amplitudes = (load(n) for n in ("member_mass", "member_radius", "member_length", "member_axis", "amplitudes"))
        reference  = int(np.argmin(np.abs(alphas)))
        rows       = np.array([alphas[reference], alphas[0], alphas[3]])
        heights    = zs[::5]
        started    = time.monotonic()
        table      = np.zeros((len(rows), len(heights)))
        for i, alpha in enumerate(rows):
            for j, z in enumerate(heights):
                point = np.array([0.0, 0.0, z])
                table[i, j] = sum(
                    tube(point, centres[m] + np.array([amplitudes[m] * alpha, 0.0, 0.0]), masses[m], radii[m], lengths[m], int(axes[m]))
                    for m in range(len(masses))
                )
        np.save(directory / "tight_alpha.npy", rows)
        np.save(directory / "tight_z.npy", heights)
        np.save(directory / "tight_V.npy", table)
        print(f"{case}: {table.size} points in {time.monotonic() - started:.0f} s")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
