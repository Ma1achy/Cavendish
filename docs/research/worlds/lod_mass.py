"""Does a coarse octree node generated straight from the recipe carry the mass of the fine terrain?

    python docs/research/worlds/lod_mass.py            # about a minute; writes lod_mass.out.txt beside it

A flat world's terrain is a height field h(x, y) above the reference plane z0 = 0 (the S5 `Plane`
base: only departures are read). h is fBm of value noise: octaves of lattice spacing 2048 m down to
4 m, amplitude 100 m halving per octave, lattice values from an integer hash (SplitMix64), the
quintic fade of Perlin 2002. Three kinds of octave are compared:

    standard        value noise as usually written;
    mass-preserving each octave's lattice is split into aligned blocks of K = 4 lattice cells; the
                    interior lattice values of a block are shifted by one constant so the octave's
                    integral over the block is exactly zero (boundary values are shared, untouched,
                    so the surface stays continuous). Every octree node that is a union of blocks
                    then gets exactly zero mass from that octave.
    mass+dipole     the same, with the least-norm interior correction that also zeroes the octave's
                    two horizontal first moments over the block.

A node of side S at horizontal range R is summarised two ways:

    truth    every octave, integrated by Gauss-Legendre quadrature on the finest lattice (6 x 6
             points a 4 m cell: exact for these piecewise polynomials, to round-off);
    summary  only the octaves whose block is larger than S ("coarse"), by the same exact
             quadrature on the coarse lattice: O(1) work a node, no fine generation.

Pass criteria, written before running:
    C1  mass-preserving kinds: |mass(summary) - mass(truth)| <= 1e-12 of the node's |mass|;
    C2  control: standard octaves miss mass by >= 1e-6 of the node's mass at every node;
    C3  report (not gated) the phase error of each summary at its range (monopole at the summary's
        centroid against the truth's), and the ring sum (2 pi R / S nodes), against 1e-6 rad.
"""

from __future__ import annotations

import time
from dataclasses import dataclass
from pathlib import Path

import numpy as np
from numpy.typing import NDArray

F64 = NDArray[np.float64]
U64 = NDArray[np.uint64]

G = 6.674_30e-11            # m^3 kg^-1 s^-2
RHO = 2_670.0               # kg m^-3, standard crustal density
K_PHASE = 4.8e9             # rad per (m s^-2) of g_z(upper) - g_z(lower), the S5 note's factor
BASELINE = 5.0              # m
K_BLOCK = 4                 # lattice cells a block side
SPACINGS = [2048.0 / 2**o for o in range(10)]       # 2048 m ... 4 m
AMPLITUDES = [100.0 / 2**o for o in range(10)]      # 100 m ... 0.2 m
SEED = 0x5EED_C0FF_EE00_0001

# ========================================= HASH ==========================================


def _splitmix(z: U64) -> U64:
    with np.errstate(over = "ignore"):
        z = z + np.uint64(0x9E37_79B9_7F4A_7C15)
        z = (z ^ (z >> np.uint64(30))) * np.uint64(0xBF58_476D_1CE4_E5B9)
        z = (z ^ (z >> np.uint64(27))) * np.uint64(0x94D0_49BB_1331_11EB)
        return z ^ (z >> np.uint64(31))


def lattice_hash(ix: NDArray[np.int64], iy: NDArray[np.int64], octave: int) -> F64:
    """Uniform in [-1, 1) from integer lattice coordinates: integer operations only."""
    with np.errstate(over = "ignore"):
        k = _splitmix(np.uint64(SEED) ^ np.uint64(octave * 0x1000_0000_0000_0001))
        z = _splitmix(k ^ ix.astype(np.uint64))
        z = _splitmix(z ^ iy.astype(np.uint64))
    return (z >> np.uint64(11)).astype(np.float64) * 2.0**-52 - 1.0


def _constraints() -> tuple[F64, F64]:
    """Rows: the octave's mass, x-moment and y-moment over a block, per lattice value (units s).

    1-D integrals of a node's fade hat over the block [0, K]: interior 1 and (i - K/2); the ends
    1/2 and 1/7 - K/4 (left), 5/14 + (K/2 - 1)/2 (right), from the quintic fade's moments
    (int_0^1 f = 1/2, int_0^1 u f = 5/14)."""
    k = K_BLOCK
    c = k / 2
    a = np.ones(k + 1)
    a[0] = a[k] = 0.5
    b = np.arange(k + 1) - c
    b[0] = 1.0 / 7.0 - c / 2.0
    b[k] = 5.0 / 14.0 + (k - 1 - c) / 2.0
    rows = np.stack([np.outer(a, a).ravel(), np.outer(b, a).ravel(), np.outer(a, b).ravel()])
    interior = np.zeros((k + 1, k + 1), dtype = bool)
    interior[1:k, 1:k] = True
    return rows, interior.ravel()


ROWS, INTERIOR = _constraints()


def block_correction(bx: NDArray[np.int64], by: NDArray[np.int64], octave: int, mode: int) -> F64:
    """[blocks, K-1, K-1]: what is taken off each block's interior values.

    mode 1 zeroes the octave's integral over the block (one constant off the interior);
    mode 2 zeroes its integral and both first moments (least-norm correction of the interior)."""
    k = K_BLOCK
    i, j = np.meshgrid(np.arange(k + 1), np.arange(k + 1), indexing = "ij")
    w = lattice_hash(bx[:, None] * k + i.ravel()[None, :], by[:, None] * k + j.ravel()[None, :], octave)
    rows = ROWS[:1] if mode == 1 else ROWS
    r = w @ rows.T                                   # [blocks, constraints]
    ci = rows[:, INTERIOR]
    corr = r @ np.linalg.solve(ci @ ci.T, ci)        # [blocks, interior]
    return corr.reshape(-1, k - 1, k - 1)


def lattice_value(ix: NDArray[np.int64], iy: NDArray[np.int64], octave: int, mode: int) -> F64:
    w = lattice_hash(ix, iy, octave)
    if mode == 0:
        return w
    bx, by = np.floor_divide(ix, K_BLOCK), np.floor_divide(iy, K_BLOCK)
    lx, ly = ix - bx * K_BLOCK, iy - by * K_BLOCK
    interior = (lx > 0) & (ly > 0)
    out = w.copy()
    if interior.any():
        keys = np.stack([bx[interior], by[interior]], axis = 1)
        uniq, inverse = np.unique(keys, axis = 0, return_inverse = True)
        corr = block_correction(uniq[:, 0], uniq[:, 1], octave, mode)
        out[interior] -= corr[inverse.ravel(), lx[interior] - 1, ly[interior] - 1]
    return out


@dataclass(frozen = True)
class Table:
    """An octave's lattice values over a node, indices from (ix0, iy0)."""
    ix0:    int
    iy0:    int
    values: F64


def table(x0: float, y0: float, side: float, o: int, mp: int) -> Table:
    s = SPACINGS[o]
    ix0, iy0 = int(np.floor(x0 / s)), int(np.floor(y0 / s))
    n = int(np.floor((x0 + side) / s)) - ix0 + 2
    ix, iy = np.meshgrid(np.arange(ix0, ix0 + n), np.arange(iy0, iy0 + n), indexing = "ij")
    return Table(ix0, iy0, lattice_value(ix.ravel(), iy.ravel(), o, mp).reshape(n, n))


# ========================================= NOISE =========================================


def fade(t: F64) -> F64:
    return t * t * t * (t * (6.0 * t - 15.0) + 10.0)


def octave_height(x: F64, y: F64, o: int, tab: Table) -> F64:
    s = SPACINGS[o]
    gx, gy = x / s, y / s
    ix, iy = np.floor(gx).astype(np.int64), np.floor(gy).astype(np.int64)
    u, v = fade(gx - ix), fade(gy - iy)
    i, j = ix - tab.ix0, iy - tab.iy0
    V = tab.values
    v00, v10, v01, v11 = V[i, j], V[i + 1, j], V[i, j + 1], V[i + 1, j + 1]
    a = v00 + u * (v10 - v00)
    b = v01 + u * (v11 - v01)
    return AMPLITUDES[o] * (a + v * (b - a))


# ======================================= QUADRATURE ======================================

GL_X, GL_W = np.polynomial.legendre.leggauss(6)


def moments(x0: float, y0: float, side: float, cell: float, octaves: list[int], mp: int) -> F64:
    """[M0, Mx, My, Mz] of rho (h - 0) over the node, h the sum of `octaves`, exact quadrature
    on a lattice of `cell` (every octave a polynomial on it). Mz = rho * int h^2 / 2."""
    n = round(side / cell)
    tabs = {o: table(x0, y0, side, o, mp) for o in octaves}
    out = np.zeros(4)
    t = 0.5 * (GL_X + 1.0)
    for row in range(n):                    # row by row to bound memory
        cy = y0 + (row + t) * cell
        cx = x0 + (np.arange(n)[:, None] + t[None, :]).ravel() * cell
        X, Y = np.meshgrid(cx, cy, indexing = "ij")
        W = np.outer(np.tile(GL_W, n), GL_W) * (0.5 * cell) ** 2
        h = np.zeros_like(X)
        for o in octaves:
            h += octave_height(X, Y, o, tabs[o])
        out += RHO * np.array([(W * h).sum(), (W * X * h).sum(), (W * Y * h).sum(), 0.5 * (W * h * h).sum()])
    return out


def phase_of(m: F64) -> float:
    """Gradiometer phase of a point mass at the moments' centroid, tower at the origin."""
    mass = m[0]
    c = m[1:] / mass
    c[2] = m[3] / mass
    def gz(z: float) -> float:
        d = c - np.array([0.0, 0.0, z])
        return float(-G * mass * d[2] / np.linalg.norm(d) ** 3 * -1.0)
    return K_PHASE * (gz(BASELINE) - gz(0.0))


# ========================================== RUN ==========================================


KINDS = {0: "standard", 1: "mass-preserving", 2: "mass+dipole"}


@dataclass(frozen = True)
class Row:
    rng:     float
    side:    float
    mode:    int
    mass:    float
    dmass:   float
    dfirst:  float
    dphase:  float
    dph_xy:  float
    dph_z:   float
    ring:    float


def main() -> None:
    lines: list[str] = []
    rows: list[Row] = []
    rnd = np.random.default_rng(7)
    started = time.perf_counter()
    for rng_m, side in [(250.0, 64.0), (1000.0, 256.0), (4000.0, 1024.0)]:
        coarse = [o for o in range(len(SPACINGS)) if SPACINGS[o] * K_BLOCK > side]
        for trial in range(3):
            ang = rnd.uniform(0, 2 * np.pi)
            cx, cy = rng_m * np.cos(ang), rng_m * np.sin(ang)
            x0 = np.floor((cx - side / 2) / side) * side   # aligned to the node grid
            y0 = np.floor((cy - side / 2) / side) * side
            for mode in (0, 1, 2):
                truth = moments(x0, y0, side, SPACINGS[-1], list(range(len(SPACINGS))), mode)
                summ = moments(x0, y0, side, SPACINGS[coarse[-1]], coarse, mode)
                dm = summ[0] - truth[0]
                dfirst = float(np.hypot(summ[1] - truth[1], summ[2] - truth[2]) / (abs(truth[0]) * side))
                dph = phase_of(summ) - phase_of(truth)
                only_xy = truth.copy(); only_xy[:3] = summ[:3]
                only_z = truth.copy(); only_z[3] = summ[3]
                nring = 2 * np.pi * rng_m / side
                rows.append(Row(rng_m, side, mode, truth[0], dm, dfirst, dph,
                                phase_of(only_xy) - phase_of(truth), phase_of(only_z) - phase_of(truth), nring))
                r = rows[-1]
                lines.append(f"R={rng_m:6.0f} S={side:5.0f} t{trial} {KINDS[mode]:15s} m {truth[0]:+.3e} kg "
                             f"dm/m {dm / abs(truth[0]):+.1e} dx/S {dfirst:.1e} | dphase {dph:+.1e} "
                             f"(mass+xy {r.dph_xy:+.1e}, z {r.dph_z:+.1e}) ring x{nring:.0f}")
    elapsed = time.perf_counter() - started

    # Cost: a 1 km square at 4 m: lattice tables per tile, then evaluation.
    rates: list[str] = []
    gx = (np.arange(250) + 0.5) * 4.0
    X, Y = np.meshgrid(gx, gx, indexing = "ij")
    for mode in (0, 1, 2):
        t0 = time.perf_counter()
        h = np.zeros_like(X)
        for o in range(len(SPACINGS)):
            h += octave_height(X, Y, o, table(0.0, 0.0, 1000.0, o, mode))
        dt = time.perf_counter() - t0
        rates.append(f"numpy, 10 octaves, {KINDS[mode]}: {dt / X.size * 1e9:.0f} ns a column (tables per 1 km tile)")

    def worst(mode: int, f: str) -> float:
        return max(abs(getattr(r, f)) / (abs(r.mass) if f == "dmass" else 1.0) for r in rows if r.mode == mode)
    c1 = max(worst(1, "dmass"), worst(2, "dmass"))
    c2 = min(abs(r.dmass / r.mass) for r in rows if r.mode == 0)
    c3 = worst(2, "dfirst")
    lines += ["",
              f"C1 worst |dm/m|, mass-preserving kinds = {c1:.2e} (pass if <= 1e-12): {'PASS' if c1 <= 1e-12 else 'FAIL'}",
              f"C2 least |dm/m|, standard              = {c2:.2e} (pass if >= 1e-6):  {'PASS' if c2 >= 1e-6 else 'FAIL'}",
              f"C2b worst |d first moment|/(m S), mass+dipole = {c3:.2e} (pass if <= 1e-12): {'PASS' if c3 <= 1e-12 else 'FAIL'}"]
    for mode in (0, 1, 2):
        for rng_m in (250.0, 1000.0, 4000.0):
            sel = [r for r in rows if r.mode == mode and r.rng == rng_m]
            lines.append(f"C3 {KINDS[mode]:15s} R={rng_m:5.0f}: worst node |dphase| {max(abs(r.dphase) for r in sel):.1e} rad, "
                         f"its mass+xy part {max(abs(r.dph_xy) for r in sel):.1e}, z part {max(abs(r.dph_z) for r in sel):.1e}")
    lines += [*rates, f"elapsed {elapsed:.1f} s"]
    text = "\n".join(lines)
    print(text)
    Path(__file__).with_name("lod_mass.out.txt").write_text(text + "\n", encoding = "utf-8")


if __name__ == "__main__":
    main()
