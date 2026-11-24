#!/usr/bin/env python3
# pyright: strict
"""The phase pipeline of George's ``cuboid``, ``scaffold`` and ``scaffold_multimode`` scripts,
written again, so that its parts can be measured one at a time.

Each of the three scripts turns a table of the potential into a phase series the same way:

    V table (nquad)  ->  RectBivariateSpline  ->  integrand V(arm) - V(arm) on interp1d arm tables
                     ->  quad over [t_k - 1, t_k - 1 + 2T]  ->  (m/hbar)(phi_1 - phi_2)

The arm tables hold, under each step's start ``j*dt``, where the arm is a step later (the one-step
offset S1 found). This harness keeps all of that, and offers ``quad`` or *exact* integration of
the same integrand (Gauss-Legendre at 8 points on each step, between whose ends the arms are
straight lines). It is tied to the scripts: run with ``quad`` on a script's own table it gives
that script's published series (``phase.npy``) to rounding, which it checks before writing.

It writes beside the fixtures (``<fixtures>/scripts/<case>/``):

    cuboid/phase_exact.npy           (20,)   the script's own table and rule, quad replaced by
                                             exact integration: radians, with the mean
    <scaffold>/phase_ac_pipeline.npy        the script's pipeline (exact integration) on its
                                            integrand integrated tightly, on a grid of 24 alphas
                                            and 480 heights (the script's: 12 and 30): the AC
                                            series, radians
    <scaffold>/phase_ac_pipeline_half.npy   the same at 240 heights: the last doubling
    <scaffold>/phase_ac_pipeline_alpha_doubled.npy  the same at 48 alphas and 480 heights
    <scaffold>/phase_ac_pipeline_script_grid.npy    the same on the script's own grid (12 x 30)
    <scaffold>/phase_ac_pipeline_script_grid_quad.npy  ... with the script's own quad

On the script's own grid the spline does not resolve the rods: doubling it moves the series by
more than its size (single mode) and by a fifth (multimode), so the grid is refined in height
until a doubling moves it by little, and the series at the finest grid is the reference.

The tight potential of one tube is the scripts' integrand integrated in its long axis in closed
form (``asinh``) and over its cross-section by Gauss-Legendre in radius and the trapezoid rule in
angle (periodic, so spectrally accurate); it is checked against ``tight_V.npy`` (``nquad`` at
``epsrel = 1e-12``, ``scripts/scaffold_reference.py``) before anything is written.

Usage:
    python -I scripts/script_pipeline.py <fixtures-dir>/scripts

Needs numpy and scipy (the fixtures were made with numpy 2.5.5, scipy 1.18.1).
"""

from __future__ import annotations

import sys
import time
import warnings
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

import numpy as np
from numpy.typing import NDArray
from scipy.integrate import IntegrationWarning, quad  # pyright: ignore[reportMissingTypeStubs]
from scipy.interpolate import RectBivariateSpline, interp1d  # pyright: ignore[reportMissingTypeStubs]

Array = NDArray[np.float64]

# The scripts' constants.
G          = 6.67430e-11
U_INITIAL  = 3.86
M_ATOM     = 1.46e-25
HBAR       = 1.055e-34
WAVENUMBER = 2 * np.pi / 698e-9
T          = 0.73
KICK       = 1000 * HBAR * WAVENUMBER / M_ATOM
GRAVITY    = 9.81
DT         = 0.01
CYCLE      = 2.0

GAUSS = np.polynomial.legendre.leggauss(8)


# ======================================== THE ARMS ========================================

def _atom_path(z_0: float, u: float, t_start: float, t_end: float, floor: float) -> tuple[Array, float]:
    """The scripts' ``atom_path``: positions after each step (stored under its start), and the
    velocity at the end."""
    steps     = np.arange(t_start, t_end, DT)
    z, v      = z_0, u
    positions: list[float] = []
    for _ in steps:
        z += v * DT + 0.5 * -GRAVITY * DT ** 2
        v += -GRAVITY * DT
        if z <= floor and v < 0:
            v = 0.0
            break
        positions.append(z)
    return np.array(positions), v


def _arm(z_0: float, first: float, kick: float) -> Array:
    """One cycle of one arm's table, as ``lower_arm`` / ``upper_arm`` build it (200 entries)."""
    one, v = _atom_path(z_0, first, 0.0, T, z_0)
    two, _ = _atom_path(float(one[-1]), v + kick, T, 2 * T, z_0)
    path   = np.concatenate([one, two])
    return np.concatenate([path, np.full(int(round(CYCLE / DT)) - len(path), z_0)])


@dataclass(frozen = True)
class Arms:
    """The four arm tables, joined linearly as ``interp1d`` joins them.

    Attributes:
        lower1 (Callable): Lower arm of the lower interferometer, metres at seconds.
        upper1 (Callable): Its upper arm.
        lower2 (Callable): Lower arm of the upper interferometer (launched 5 m higher).
        upper2 (Callable): Its upper arm.
    """
    lower1: Callable[[Array], Array]
    upper1: Callable[[Array], Array]
    lower2: Callable[[Array], Array]
    upper2: Callable[[Array], Array]


def arms(cycles: int) -> Arms:
    """The scripts' arm tables over ``cycles`` cycles of 2 s."""
    def table(z_0: float, first: float, kick: float) -> Callable[[Array], Array]:
        values = np.concatenate([_arm(z_0, first, kick)] * cycles)
        times  = np.arange(0, cycles * CYCLE, DT)[: len(values)]
        f      = interp1d(times, values, bounds_error = False, fill_value = (values[0], values[-1]))  # pyright: ignore[reportUnknownVariableType]
        return lambda t: np.asarray(f(t), dtype = np.float64)  # pyright: ignore[reportUnknownLambdaType]
    return Arms(table(0, U_INITIAL, KICK), table(0, U_INITIAL + KICK, -KICK),
                table(5, U_INITIAL, KICK), table(5, U_INITIAL + KICK, -KICK))


# ======================================== THE SERIES ========================================

def series(
    spline:  RectBivariateSpline,
    source:  Callable[[Array], Array],
    arms:    Arms,
    starts:  Array,
    exact:   bool,
) -> Array:
    """The phase series, lower minus upper, radians, with its mean.

    Args:
        spline  (RectBivariateSpline): The table in (source coordinate, height).
        source  (Callable):            The source coordinate (x0 or alpha) at seconds.
        arms    (Arms):                The arm tables.
        starts  (Array):               Each shot's window start, seconds; the window is 2T.
        exact   (bool):                Exact integration on each step, or the scripts' ``quad``.

    Returns:
        Array: One phase a shot.
    """
    def v(t: Array, z: Array) -> Array:
        return np.asarray(spline(source(t), z, grid = False), dtype = np.float64)  # pyright: ignore[reportUnknownMemberType]

    def integrand(t: Array, lower: Callable[[Array], Array], upper: Callable[[Array], Array]) -> Array:
        return v(t, upper(t)) - v(t, lower(t))

    out = np.empty(len(starts))
    for k, start in enumerate(starts):
        if exact:
            nodes, weights = GAUSS
            left = start + DT * np.arange(int(round(2 * T / DT)))
            t    = (left[:, None] + 0.5 * DT * (nodes[None, :] + 1)).ravel()
            w    = np.tile(0.5 * DT * weights, len(left))
            one  = float(np.sum(w * integrand(t, arms.lower1, arms.upper1)))
            two  = float(np.sum(w * integrand(t, arms.lower2, arms.upper2)))
        else:
            def f(lower: Callable[[Array], Array], upper: Callable[[Array], Array]) -> Callable[[float], float]:
                return lambda t: float(integrand(np.array([t]), lower, upper)[0])
            with warnings.catch_warnings():
                warnings.simplefilter("ignore", IntegrationWarning)
                one = float(quad(f(arms.lower1, arms.upper1), start, start + 2 * T, limit = 100)[0])  # pyright: ignore[reportUnknownArgumentType]
                two = float(quad(f(arms.lower2, arms.upper2), start, start + 2 * T, limit = 100)[0])  # pyright: ignore[reportUnknownArgumentType]
        out[k] = M_ATOM / HBAR * (one - two)
    return out


def _spline(rows: Array, heights: Array, table: Array) -> RectBivariateSpline:
    return RectBivariateSpline(rows, heights, table)


def _tie(published: Array, ours: Array, what: str) -> None:
    """Refuses to go on unless the rewrite gives the published series to rounding."""
    gap = float(np.max(np.abs(published - ours)))
    off = gap / float(np.max(np.abs(published)))
    print(f"{what}: the rewrite with quad against the published series: {off:.2e} of its peak "
          f"({gap:.1e} rad, {gap / float(np.max(np.abs(published - published.mean()))):.1e} of its AC peak)")
    if off > 1e-9:
        raise SystemExit(f"{what}: the rewrite is not the script ({off:e})")


# ======================================== THE CUBOID ========================================

def cuboid(directory: Path) -> None:
    """The shaky wall: the tie, and the script's table and rule with quad replaced."""
    load   = lambda name: np.load(directory / f"{name}.npy").astype(np.float64)  # noqa: E731
    spline = _spline(load("x0_grid"), load("z_grid"), load("V_grid"))
    times  = load("subsample_times")
    starts = times - 1.0
    a      = arms(int(times[-1] // CYCLE) + 2)
    x0     = lambda t: 5 + 1e-6 * np.cos(2 * np.pi * 0.1 * t)  # noqa: E731
    _tie(load("phase"), series(spline, x0, a, starts, exact = False), "cuboid")
    exact = series(spline, x0, a, starts, exact = True)
    np.save(directory / "phase_exact.npy", exact)
    quad_error = float(np.max(np.abs(load("phase") - exact)) / np.max(np.abs(exact)))
    print(f"cuboid: the script's quad against exact integration: {quad_error:.3e} of the phase")


# ======================================== THE SCAFFOLD ========================================

@dataclass(frozen = True)
class Members:
    """The scaffold's 48 tubes, from the fixtures.

    Attributes:
        centre    (Array): ``[48, 3]``, metres, at rest.
        mass      (Array): ``[48]``, kg.
        radius    (Array): ``[48]``, metres.
        length    (Array): ``[48]``, metres.
        axis      (Array): ``[48]``, 0, 1, 2 for x, y, z.
        amplitude (Array): ``[48]``, metres of x for alpha = 1.
    """
    centre:    Array
    mass:      Array
    radius:    Array
    length:    Array
    axis:      Array
    amplitude: Array


def _asinh_difference(x: Array, y: Array) -> Array:
    """``asinh(x) - asinh(y)`` without cancellation when both are large and of one sign."""
    return np.arcsinh(x * np.sqrt(1 + y * y) - y * np.sqrt(1 + x * x))


def tight_potential(members: Members, alpha: float, z: float, radial: int = 24, angular: int = 64) -> float:
    """The scaffold's potential at height ``z`` on the axis with every member moved by its
    amplitude times ``alpha`` in x, J/kg: each tube integrated along its axis in closed form, and
    over its cross-section by Gauss-Legendre (``radial``) and the trapezoid rule (``angular``)."""
    r_nodes, r_weights = np.polynomial.legendre.leggauss(radial)
    theta              = 2 * np.pi * np.arange(angular) / angular
    total              = 0.0
    for m in range(len(members.mass)):
        axis          = int(members.axis[m])
        across        = [a for a in range(3) if a != axis]
        centre        = members.centre[m] + np.array([members.amplitude[m] * alpha, 0.0, 0.0])
        d             = np.array([0.0, 0.0, z]) - centre
        radius, half  = float(members.radius[m]), 0.5 * float(members.length[m])
        r             = 0.5 * radius * (r_nodes + 1)
        w             = 0.5 * radius * r_weights
        du            = d[across[0]] - r[:, None] * np.cos(theta)[None, :]
        dv            = d[across[1]] - r[:, None] * np.sin(theta)[None, :]
        rho           = np.sqrt(du * du + dv * dv)
        along         = _asinh_difference((d[axis] + half) / rho, (d[axis] - half) / rho)
        density       = members.mass[m] / (np.pi * radius ** 2 * members.length[m])
        inner         = np.sum(along * r[:, None], axis = 1) * (2 * np.pi / angular)
        total        += float(-G * density * np.sum(w * inner))
    return total


def _table(members: Members, alphas: Array, heights: Array) -> Array:
    return np.array([[tight_potential(members, float(a), float(z)) for z in heights] for a in alphas])


def scaffold(directory: Path, modes: list[float]) -> None:
    """One scaffold case: the tight table checked, the tie, and the pipeline on the tight tables."""
    load    = lambda name: np.load(directory / f"{name}.npy").astype(np.float64)  # noqa: E731
    members = Members(load("member_centre").reshape(-1, 3), load("member_mass"), load("member_radius"),
                      load("member_length"), load("member_axis"), load("amplitudes"))
    # The tight potential against nquad at epsrel 1e-12, and its changes with alpha.
    reference                   = load("tight_V")
    ref_alpha, ref_z            = load("tight_alpha"), load("tight_z")
    ours                        = _table(members, ref_alpha, ref_z)
    value_off                   = float(np.max(np.abs(ours / reference - 1)))
    change_ref, change_ours     = reference[1:] - reference[0], ours[1:] - ours[0]
    change_off                  = float(np.max(np.abs(change_ours - change_ref)) / np.max(np.abs(change_ref)))
    print(f"{directory.name}: tight potential against nquad (epsrel 1e-12): {value_off:.2e} of V, "
          f"{change_off:.2e} of the largest change")
    if value_off > 1e-11 or change_off > 1e-3:
        raise SystemExit(f"{directory.name}: the tight potential is not the scripts' integrand")
    alpha_grid, z_grid = load("alpha_grid"), load("z_grid")
    times              = load("subsample_times")
    starts             = times - 2 * T
    a                  = arms(int(times[-1] // CYCLE) + 2)
    source             = lambda t: sum(np.cos(2 * np.pi * f * t) for f in modes) + 0 * t  # noqa: E731
    _tie(load("phase"), series(_spline(alpha_grid, z_grid, load("V_grid")), source, a, starts, exact = False),
         directory.name)
    ac = lambda s: s - s.mean()  # noqa: E731

    def pipeline(n_alpha: int, n_z: int, exact: bool = True) -> Array:
        alphas  = np.linspace(alpha_grid[0], alpha_grid[-1], n_alpha)
        heights = np.linspace(z_grid[0], z_grid[-1], n_z)
        return ac(series(_spline(alphas, heights, _table(members, alphas, heights)), source, a, starts, exact))

    started = time.monotonic()
    # On the script's own grid, by its quad and exactly; then the grid refined in height (where
    # the spline does not resolve the rods) at twice the script's alphas, and alpha doubled at the
    # finest height.
    by_quad = pipeline(len(alpha_grid), len(z_grid), exact = False)
    own     = pipeline(len(alpha_grid), len(z_grid))
    heights = [len(z_grid) * 2 ** k for k in range(5)]
    refined = [pipeline(2 * len(alpha_grid), n) for n in heights]
    finest  = refined[-1]
    alpha2  = pipeline(4 * len(alpha_grid), heights[-1])
    peak    = float(np.max(np.abs(finest)))
    change  = lambda x, y: float(np.max(np.abs(x - y))) / peak  # noqa: E731
    print(f"{directory.name}: the pipeline on tight tables ({time.monotonic() - started:.0f} s); AC peak {peak:.4e} rad")
    print(f"  on the script's grid: {change(own, finest):.3e} of the peak from the finest; its quad against exact "
          f"{change(by_quad, own):.3e}")
    for n, (x, y) in zip(heights[1:], zip(refined[1:], refined[:-1])):
        print(f"  heights {n // 2} -> {n}: changes by {change(x, y):.3e}")
    print(f"  alphas {2 * len(alpha_grid)} -> {4 * len(alpha_grid)} at {heights[-1]} heights: {change(alpha2, finest):.3e}")
    np.save(directory / "phase_ac_pipeline.npy", finest)
    np.save(directory / "phase_ac_pipeline_half.npy", refined[-2])
    np.save(directory / "phase_ac_pipeline_alpha_doubled.npy", alpha2)
    np.save(directory / "phase_ac_pipeline_script_grid.npy", own)
    np.save(directory / "phase_ac_pipeline_script_grid_quad.npy", by_quad)

def main(arguments: list[str]) -> int:
    """Runs the three cases. ``arguments`` is the fixtures' ``scripts`` directory."""
    if len(arguments) != 1:
        print(__doc__)
        return 2
    root = Path(arguments[0]).resolve()
    cuboid(root / "cuboid")
    scaffold(root / "scaffold", [22.4])
    scaffold(root / "scaffold_multimode", [2.30, 2.34, 12.7, 35.1, 37.0, 38.7, 39.4, 40.5])
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
