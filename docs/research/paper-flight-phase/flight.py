"""The AION-10 gradiometer phase as Cavendish computes it: the flight integral along the arms.

    from flight import AION10, flight_phase, frozen_phase, leading_phase

Cavendish's forward model (`crates/cav-core/src/forward.rs`) reads one shot as

    δφ_k = (m_A/ħ) ∫₀^2T [V(upper arm of k, t) − V(lower arm of k, t)] dt,   ΔΦ = δφ₁ − δφ₂

(1 = the lower interferometer, launched from the tower's base; 2 launched Δr above it), with the
source evaluated at the same instant as the atoms. This module is that integral, rule for rule:

    arms (u₀, n ħk/m kick at 0, kick reversed at T, floor at the launch height)
      -> Gauss–Legendre panels cut at T, at each landing and at the source's breaks
      -> nodes (τ, w, z_lower_arm, z_upper_arm)
      -> Σ w · [ΔV(ifo 1) − ΔV(ifo 2)] · G m_A/ħ

It is first order in the source (the arms are the unperturbed paths), which is the perturbative
result of Storey & Cohen-Tannoudji (1994) used by Hogan, Johnson & Kasevich (2008, §5.2.1) and by
Carlton & McCabe (2023, eq. 11) for moving masses at this very site.

Three reads of one shot are compared throughout this study:
    flight_phase   the source moves during the shot (the real phase, first order)
    frozen_phase   the source frozen at the π pulse (launch + T), then the same flight integral
    leading_phase  k_eff T² [g_z(Δr) − g_z(0)] at one instant (what the paper uses): the field at
                   the two launch points, no arms
A potential source is a callable ``potential(points [N, 3] m, times [N] s) -> V/G [N] kg/m`` (each
point at its own scene time; V/G is −Σ m/r, so negative); a field source ``field(points, times) ->
g_z/G [N] kg/m²`` (g = −∇V, its upward component).
"""

from __future__ import annotations

import math
from collections.abc import Callable
from dataclasses import dataclass, replace

import numpy as np
import numpy.typing as npt

F64 = npt.NDArray[np.float64]
Potential = Callable[[F64, F64], F64]
Field = Callable[[F64, F64], F64]

# CODATA, as `crates/cav-core/src/constants.rs`.
BIG_G = 6.674_30e-11
HBAR = 6.626_070_15e-34 / (2.0 * math.pi)
SR87_MASS = 86.908_877_5 * 1.660_539_068_92e-27


@dataclass(frozen = True)
class Instrument:
    """Everything the phase depends on (the fields of `forward::Instrument`).

    Attributes:
        separation (float): Δr, m: the upper interferometer's launch above the lower's.
        half       (float): T, s: the π pulse at T, readout at 2T.
        launch     (float): u₀, m/s, the lower arm's launch velocity.
        gravity    (float): m/s², down.
        kicks      (float): photon momenta per kick, n.
        wavelength (float): m.
        floor      (bool):  an arm that falls back to its launch height stops there.
        order      (int):   Gauss nodes per panel.
        panel      (float): longest panel, s.
    """

    separation: float = 5.0
    half:       float = 0.73
    launch:     float = 3.86
    gravity:    float = 9.81
    kicks:      float = 1000.0
    wavelength: float = 698e-9
    floor:      bool  = True
    order:      int   = 10
    panel:      float = 0.05

    @property
    def kick(self) -> float:
        """n ħ k / m_A, m/s."""
        return self.kicks * HBAR * (2.0 * math.pi / self.wavelength) / SR87_MASS

    @property
    def k_eff(self) -> float:
        """n · 2π/λ, rad/m."""
        return self.kicks * 2.0 * math.pi / self.wavelength

    @property
    def scale(self) -> float:
        """G m_A / ħ, m s⁻¹ kg⁻¹: turns Σ w·ΔV/G (kg s/m) into radians."""
        return BIG_G * SR87_MASS / HBAR

    def symmetric(self) -> Instrument:
        """The launch gT − v_kick/2 that closes both arms at the launch height at 2T."""
        return replace(self, launch = self.gravity * self.half - 0.5 * self.kick)


AION10 = Instrument()


# ======================================= ARMS =======================================


class Arm:
    """One arm's rise above its launch point (a port of `forward::Arm`)."""

    def __init__(self, first: float, kick: float, half: float, gravity: float, floor: bool) -> None:
        self.first, self.half, self.g = first, half, gravity
        back = 2.0 * first / gravity if first > 0.0 else 0.0
        self.landed_first = back if (floor and back < half) else None
        if self.landed_first is not None:
            self.rise_half, before = 0.0, 0.0
        else:
            self.rise_half, before = first * half - 0.5 * gravity * half * half, first - gravity * half
        self.second = before + kick
        self.landed_second: float | None = None
        if floor:
            if self.rise_half <= 0.0 and self.second <= 0.0:
                back2 = 0.0
            else:
                back2 = (self.second + math.sqrt(self.second**2 + 2.0 * gravity * self.rise_half)) / gravity
            self.landed_second = back2 if back2 < half else None

    def rise(self, tau: F64) -> F64:
        """Height above the launch point at flight times `tau` [N] s, m."""
        tau = np.asarray(tau, dtype = float)
        g = self.g
        first = self.first * tau - 0.5 * g * tau * tau
        if self.landed_first is not None:
            first = np.where(tau >= self.landed_first, 0.0, first)
        since = tau - self.half
        second = self.rise_half + self.second * since - 0.5 * g * since * since
        if self.landed_second is not None:
            second = np.where(since >= self.landed_second, 0.0, second)
        elif self.landed_first is not None:
            second = np.where(since >= self.half, 0.0, second)
        return np.where(tau <= self.half, first, second)

    def landings(self) -> list[float]:
        out = [] if self.landed_first is None else [self.landed_first]
        if self.landed_second is not None:
            out.append(self.half + self.landed_second)
        return out

    def grounded(self, tau: float) -> bool:
        if tau <= self.half:
            return self.landed_first is not None and tau >= self.landed_first
        return self.landed_second is not None and tau - self.half >= self.landed_second


def arms(ins: Instrument) -> tuple[Arm, Arm]:
    """(lower arm, upper arm): launched at u₀ and kicked up at T; launched a kick faster, kicked down."""
    v = ins.kick
    return (Arm(ins.launch, v, ins.half, ins.gravity, ins.floor),
            Arm(ins.launch + v, -v, ins.half, ins.gravity, ins.floor))


def nodes(ins: Instrument, breaks: tuple[float, ...] = ()) -> tuple[F64, F64, F64, F64]:
    """The flight's nodes: (τ s, weight s, lower-arm rise m, upper-arm rise m), each [N].

    Pieces are cut at T, at every landing and at `breaks` (flight times, s); pieces with both arms
    on the floor are dropped (nothing to integrate there), as `Instrument::pieces` does.
    """
    lo, up = arms(ins)
    two_t = 2.0 * ins.half
    cuts = sorted({c for c in [0.0, ins.half, two_t, *lo.landings(), *up.landings(), *breaks] if 0.0 <= c <= two_t})
    merged = [cuts[0]]
    for c in cuts[1:]:
        if c - merged[-1] >= 1e-9:
            merged.append(c)
    x, w = np.polynomial.legendre.leggauss(ins.order)
    taus, weights = [], []
    for a, b in zip(merged[:-1], merged[1:]):
        mid = 0.5 * (a + b)
        if ins.floor and lo.grounded(mid) and up.grounded(mid):
            continue
        n = max(1, math.ceil((b - a) / ins.panel))
        width = (b - a) / n
        left = a + width * np.arange(n)
        taus.append((left[:, None] + 0.5 * width * (x[None, :] + 1.0)).ravel())
        weights.append(np.broadcast_to(0.5 * width * w, (n, ins.order)).ravel())
    tau = np.concatenate(taus)
    return tau, np.concatenate(weights), lo.rise(tau), up.rise(tau)


# ======================================= READS =======================================


def flight_phase(potential: Potential, ins: Instrument, tower: F64, launch: float,
                 breaks: tuple[float, ...] = (), frozen_at: float | None = None) -> float:
    """ΔΦ (rad) of the shot launched at scene time `launch` from the tower whose base is `tower` [3] m.

    `breaks` are scene times at which the source is not smooth. With `frozen_at` (scene time) the
    source is evaluated at that one instant for every node: the static flight integral. The source
    is asked once, for all four arm points of every node.
    """
    tau, w, z_lo, z_up = nodes(ins, tuple(b - launch for b in breaks))
    n = len(tau)
    times = np.full(n, frozen_at) if frozen_at is not None else launch + tau
    heights = np.concatenate([z_up, z_lo, ins.separation + z_up, ins.separation + z_lo])
    pts = np.repeat(tower[None, :], 4 * n, axis = 0)
    pts[:, 2] += heights
    v = potential(pts, np.tile(times, 4)).reshape(4, n)
    # lower interferometer minus upper; each is V(upper arm) − V(lower arm)
    total = float(np.sum(w * ((v[0] - v[1]) - (v[2] - v[3]))))
    return ins.scale * total


def frozen_phase(potential: Potential, ins: Instrument, tower: F64, launch: float) -> float:
    """The same shot with the source frozen at the π pulse (launch + T)."""
    return flight_phase(potential, ins, tower, launch, frozen_at = launch + ins.half)


def leading_phase(field: Field, ins: Instrument, tower: F64, time: float, height: float = 0.0) -> float:
    """The paper's read: k_eff T² [g_z(base + h + Δr) − g_z(base + h)] at one instant, rad, with
    h = `height` above each launch point (the paper: 0; `effective_height` makes it exact for any
    field quadratic in z)."""
    pts = np.stack([tower + [0.0, 0.0, height], tower + [0.0, 0.0, height + ins.separation]])
    gz = field(pts, np.full(2, time))
    return ins.k_eff * ins.half**2 * BIG_G * float(gz[1] - gz[0])


def effective_height(ins: Instrument) -> float:
    """The Δz-weighted mean height of the arms' midpoint above the launch point, m.

    For V = −a z − ½Γz² the arms' potential difference is −Δz [a + Γ (z_u + z_l)/2], so a uniform
    gradient read at this height is the flight integral exactly (2.17 m for AION-10's arms)."""
    tau, w, z_lo, z_up = nodes(replace(ins, order = 16, panel = 0.005))
    dz = z_up - z_lo
    return float(np.sum(w * dz * 0.5 * (z_up + z_lo)) / np.sum(w * dz))


# ======================================= SOURCES =======================================


def point_potential(mass: float, path: Callable[[F64], F64]) -> Potential:
    """A point mass `mass` kg at `path(times [N])` -> [N, 3] m."""
    def potential(pts: F64, times: F64) -> F64:
        return -mass / np.linalg.norm(pts - path(times), axis = 1)
    return potential


def point_field(mass: float, path: Callable[[F64], F64]) -> Field:
    def field(pts: F64, times: F64) -> F64:
        d = path(times) - pts
        return mass * d[:, 2] / np.linalg.norm(d, axis = 1) ** 3
    return field


def straight(start: F64, velocity: F64) -> Callable[[F64], F64]:
    """A path at constant velocity: start + velocity·t, vectorised over t [N] -> [N, 3] m."""
    a, v = np.asarray(start, dtype = float), np.asarray(velocity, dtype = float)
    return lambda t: a[None, :] + np.asarray(t, dtype = float)[:, None] * v[None, :]


def separation_transfer(ins: Instrument, freqs: F64) -> F64:
    """|∫ Δz(τ) e^{−iωτ} dτ| / ∫ Δz dτ for the arms' separation Δz = z_up − z_lo (dimensionless).

    For a uniform, time-varying acceleration a(t) (or gradient Γ(t) across the baseline) the phase
    is k_eff ∫ Δz(τ)/v_kick · a(τ) dτ, so this is the gradient-to-phase transfer function
    normalised to its value at f = 0, |H(f)|/H(0). For instant kicks and no floor it is
    sinc²(πfT) exactly (Cheinet et al. 2008, eq. 9, with H_a = k_eff H/ω²).
    """
    fine = replace(ins, order = 16, panel = 0.005)
    tau, w, z_lo, z_up = nodes(fine)
    dz = z_up - z_lo
    phase = np.exp(-2j * np.pi * np.outer(freqs, tau))
    return np.abs(phase @ (w * dz)) / float(np.sum(w * dz))
