"""The real interferometer read for the paper's static and moving bodies, and fast field kernels.

    import realread as R
    read = R.AxisRead()                      # static: g_z at fixed heights on each launch axis
    pts = read.points(lowers)                # lowers [D, 3]: the lower launch points
    real, leading = read.phases(g)           # g [len(pts), 3] per unit G·rho -> rad, each [D]
    pr = R.PassRead(times, lowers, u)        # moving: U at the flight nodes of every shot
    phase = pr.phase(U)                      # U [len(pr.points)] per unit G·rho -> rad [S]

The real read is Cavendish's flight integral (`../paper-flight-phase/flight.py`, which is `cav run`
to 1.8e-15 rad): per interferometer (m_A/ħ)∫[V(upper arm) − V(lower arm)]dt, lower minus upper.
With V = −Gρ U (U = ∫dV/r, the shapes study's sign) and the arms' rises z_l(τ), z_u(τ) above
each launch point p_k:

    ΔΦ = (m_A/ħ) G ρ Σ_i w_i [−U(p₁+z_u) + U(p₁+z_l) + U(p₂+z_u) − U(p₂+z_l)]          (moving)

For a static body V(p+z_u) − V(p+z_l) = −Gρ ∫_{z_l}^{z_u} g_z dz along the vertical. Interpolating
g_z in height on n Legendre nodes z_j of [0, Z] (Z the arms' highest rise) and integrating the
interpolant exactly between the arms at every flight node gives fixed weights A_j (m s):

    ΔΦ = (m_A/ħ) G ρ Σ_j A_j [g_z(p₂ + z_j) − g_z(p₁ + z_j)]                              (static)

with Σ_j A_j = ∫Δz dt = v_kick T², so a uniform gradient gives k_eff T² Δg_z, the paper's read,
exactly. `AxisRead` also returns that leading read, from g at the two launch points.

The kernels (`kern/`, Rust, four threads) are the shapes study's formulas compiled: `tri` is
`skin_bench.polyhedron_field(stable=True)`, `points` is `g_points` + `g_quadrupole`, `series` is
`g_series4`. `patch_shapes_lib()` swaps them into `shapes_lib`, so its techniques run unchanged.
"""

from __future__ import annotations

import ctypes
import math
import subprocess
import sys
import tempfile
from dataclasses import replace
from pathlib import Path

import numpy as np
import numpy.typing as npt

HERE = Path(__file__).resolve().parent
RESEARCH = HERE.parent
sys.path.insert(0, str(RESEARCH / "paper-flight-phase"))
sys.path.insert(0, str(RESEARCH / "exact-skin-and-partial-fill" / "shapes"))
sys.path.insert(0, str(RESEARCH / "exact-skin-and-partial-fill"))
import flight as FL  # noqa: E402
import shapes_lib as L  # noqa: E402

F64 = npt.NDArray[np.float64]
KERN = HERE / "kern" / "target" / "release" / ("kern.exe" if sys.platform == "win32" else "kern")
INS = FL.AION10
RHO = L.RHO
#: rad per (U or g per unit G·rho) per (m s) of node weight: (m_A/ħ) G ρ.
COUPLING = FL.SR87_MASS / FL.HBAR * FL.BIG_G * RHO
#: rad per (g per unit G·rho) for the leading read: k_eff T² G ρ (= shapes_lib.PHASE_PER_DG·G·ρ).
LEADING = INS.k_eff * INS.half**2 * FL.BIG_G * RHO


def below_normal() -> None:
    """Lower this process (and the kernels it starts) to below-normal priority on Windows."""
    if sys.platform == "win32":
        k32 = ctypes.windll.kernel32  # type: ignore[attr-defined]
        k32.SetPriorityClass(k32.GetCurrentProcess(), 0x4000)


# ======================================= KERNELS =======================================

_TMP = Path(tempfile.mkdtemp(prefix = "realread-"))


def _run(mode: str, arrays: list[F64 | str], out_len: int) -> F64:
    names = []
    for k, a in enumerate(arrays):
        if isinstance(a, str):
            names.append(a)
            continue
        p = _TMP / f"{mode}-{k}.bin"
        np.ascontiguousarray(a, dtype = np.float64).tofile(p)
        names.append(str(p))
    out = _TMP / f"{mode}-out.bin"
    subprocess.run([str(KERN), mode, *names, str(out)], check = True)
    res = np.fromfile(out, dtype = np.float64)
    assert len(res) == out_len, (mode, len(res), out_len)
    return res


def tri_field(tris: F64, weights: F64, pts: F64) -> tuple[F64, F64]:
    """(U [W, M], g [W, M, 3]) per unit G·rho of a weighted closed triangle soup (`weights` [N, W])."""
    n, w = len(tris), weights.shape[1]
    m = len(pts)
    if n == 0 or m == 0:
        return np.zeros((w, m)), np.zeros((w, m, 3))
    res = _run("tri", [tris.reshape(n, 9), weights, str(w), pts], w * m * 4).reshape(w, m, 4)
    return res[..., 0], res[..., 1:]


def point_field(pos: F64, mass: F64, second: F64 | None, pts: F64) -> tuple[F64, F64, F64, F64]:
    """(U, g) of point masses, then (U, g) of their second moments' extra term (zero if None)."""
    m = len(pts)
    if len(mass) == 0:
        z = np.zeros(m)
        return z, np.zeros((m, 3)), z, np.zeros((m, 3))
    sec: F64 | str = "-" if second is None else second.reshape(len(mass), 9)
    res = _run("points", [pos, mass, sec, pts], 2 * m * 4).reshape(2, m, 4)
    return res[0, :, 0], res[0, :, 1:], res[1, :, 0], res[1, :, 1:]


def series_field(cen: F64, mass: F64, h: float, pts: F64) -> tuple[F64, F64]:
    """(U, g) of uniform cubes of side h as monopole + l = 4 (shapes_lib.g_series4)."""
    m = len(pts)
    if len(mass) == 0:
        return np.zeros(m), np.zeros((m, 3))
    res = _run("series", [cen, mass, repr(float(h)), pts], m * 4).reshape(m, 4)
    return res[:, 0], res[:, 1:]


def mpole_field(cen: F64, coef: F64, pts: F64) -> F64:
    """[M, 3]: Σ_cells U_3, U_4, U_5 per unit G·rho from `moments.multipole_coefficients`."""
    m = len(pts)
    if len(cen) == 0:
        return np.zeros((m, 3))
    return _run("mpole", [cen, coef, pts], m * 3).reshape(m, 3)


def patch_shapes_lib() -> None:
    """Swap the compiled kernels into shapes_lib (same signatures, same formulas)."""

    def g_mesh(tris: F64, pts: F64, weights: F64 | None = None) -> F64:
        w = np.ones((len(tris), 1)) if weights is None else weights.reshape(len(tris), -1)
        g = tri_field(tris, w, pts)[1]
        return g[0] if weights is None or weights.ndim == 1 else g

    def g_points(mass: F64, pos: F64, pts: F64) -> F64:
        return point_field(pos, mass, None, pts)[1]

    def g_quadrupole(second: F64, pos: F64, pts: F64) -> F64:
        return point_field(pos, np.zeros(len(pos)), second, pts)[3]

    def g_series4(mass: F64, c: F64, h: float, pts: F64) -> F64:
        return series_field(c, mass, h, pts)[1]

    L.g_mesh, L.g_points, L.g_quadrupole, L.g_series4 = g_mesh, g_points, g_quadrupole, g_series4


def u_mesh(tris: F64, pts: F64) -> F64:
    return tri_field(tris, np.ones((len(tris), 1)), pts)[0][0]


def u_cubes(centres: F64, h: float, fill: F64, pts: F64, ratio: float = 0.01) -> F64:
    """U of uniform cubes: the series where (h/2)/distance < `ratio`, the polyhedral cube elsewhere
    (shapes_lib.g_cubes_fast's rule)."""
    if len(centres) == 0:
        return np.zeros(len(pts))
    dmin = np.full(len(centres), np.inf)
    for s in range(0, len(pts), 256):  # nearest point to every cell, in blocks
        blk = pts[s : s + 256]
        d = np.sqrt(((centres[:, None, :] - blk[None, :, :]) ** 2).sum(axis = 2)).min(axis = 1)
        dmin = np.minimum(dmin, d)
    far = 0.5 * h / dmin < ratio
    u = series_field(centres[far], fill[far] * h**3, h, pts)[0]
    near = ~far
    if near.any():
        t = L.sb.cube_tris(centres[near], h)
        u = u + tri_field(t, np.repeat(fill[near], 12)[:, None], pts)[0][0]
    return u


# ======================================= STATIC READ =======================================


class AxisRead:
    """The static flight read as fixed weights on g_z at n heights above each launch point."""

    def __init__(self, n: int = 48, ins: FL.Instrument = INS) -> None:
        self.ins, self.n = ins, n
        # The arms on their natural pieces (cut at T and the landings), Gauss order 64 on each:
        # the interpolant's antiderivative along a quadratic arm is a polynomial of degree ≤ 2(n+1).
        exact = replace(ins, order = max(64, n + 2), panel = 10.0)
        tau, w, z_lo, z_up = FL.nodes(exact)
        self.top = float(max(z_up.max(), z_lo.max())) * (1 + 1e-12)
        x, wx = np.polynomial.legendre.leggauss(n)
        self.z = 0.5 * self.top * (x + 1.0)
        # Legendre coefficients of the interpolant from its node values: c_k = (2k+1)/2 Σ_j wx_j P_k(x_j) f_j.
        pk = np.polynomial.legendre.legvander(x, n - 1)  # [n nodes, n degrees]
        to_coef = ((2 * np.arange(n) + 1) / 2)[:, None] * (pk * wx[:, None]).T  # [k, j]
        xu, xl = 2 * z_up / self.top - 1, 2 * z_lo / self.top - 1
        d = np.empty(n)
        for k in range(n):
            q = np.polynomial.legendre.legint(np.eye(n)[k])
            d[k] = np.sum(w * (np.polynomial.legendre.legval(xu, q) - np.polynomial.legendre.legval(xl, q)))
        self.weights = 0.5 * self.top * (d @ to_coef)  # A_j, m s
        self.area = float(np.sum(w * (z_up - z_lo)))  # ∫Δz dt, m s

    def points(self, lowers: F64) -> F64:
        """[lower ifo: per launch point n nodes then the point itself; upper ifo: the same, Δr up]."""
        lowers = np.atleast_2d(lowers)
        off = np.append(self.z, 0.0)
        lo = (lowers[:, None, :] + off[None, :, None] * np.array([0.0, 0.0, 1.0])).reshape(-1, 3)
        return np.concatenate([lo, lo + np.array([0.0, 0.0, self.ins.separation])])

    def phases(self, g: F64) -> tuple[F64, F64]:
        """(real [D], leading [D]) in rad from g [len(points), 3] per unit G·rho."""
        m = len(g) // 2
        dg = (g[m:, 2] - g[:m, 2]).reshape(-1, self.n + 1)
        return COUPLING * dg[:, :-1] @ self.weights, LEADING * dg[:, -1]


def both(read: AxisRead):  # noqa: ANN201
    """A `shapes_lib.phase` replacement returning [real (D), leading (D)] as one vector."""
    def ph(g: F64) -> F64:
        r, lead = read.phases(g)
        return np.concatenate([r, lead])
    return ph


# ======================================= MOVING READ =======================================


class PassRead:
    """The flight read of shots whose lower launch points move in the body's frame.

    `pulses` [S, 3]: the lower launch point in the body's frame at each shot's π pulse (launch + T);
    `u` [3]: the points' velocity in the body's frame (m/s; −v for a body moving at +v); `times`
    [S]: the π-pulse scene times. Points are [a; b; c; d] per node (lower ifo upper arm, lower ifo
    lower arm, upper ifo upper arm, upper ifo lower arm), shots outermost.
    """

    def __init__(self, times: F64, pulses: F64, u: F64, order: int = 8, panel: float = 0.2,
                 ins: FL.Instrument = INS) -> None:
        self.ins = replace(ins, order = order, panel = panel)
        tau, w, z_lo, z_up = FL.nodes(self.ins)
        self.w, self.nn = w, len(tau)
        rel = tau - ins.half  # node time relative to the π pulse
        self.node_times = (np.asarray(times)[:, None] + rel[None, :])  # [S, N]
        base = pulses[:, None, :] + rel[None, :, None] * np.asarray(u)[None, None, :]  # [S, N, 3]
        up = np.array([0.0, 0.0, 1.0])
        dr = ins.separation
        arms = [z_up, z_lo, dr + z_up, dr + z_lo]
        self.points = np.stack([base + h[None, :, None] * up for h in arms], axis = 2).reshape(-1, 3)
        self.shots = len(times)
        self.lead_points = np.concatenate([pulses, pulses + dr * up])

    def _combine(self, f: F64) -> F64:
        """Σ_i w_i [−f_a + f_b + f_c − f_d] per shot, f [S·N·4, ...]."""
        f = f.reshape(self.shots, self.nn, 4, *f.shape[1:])
        comb = -f[:, :, 0] + f[:, :, 1] + f[:, :, 2] - f[:, :, 3]
        return np.einsum("n,sn...->s...", self.w, comb)

    def phase(self, u: F64) -> F64:
        """Real phase per shot (rad) from U [len(points)] per unit G·rho."""
        return COUPLING * self._combine(u)

    def derivative(self, g: F64, axis: int, with_time: bool = False) -> F64:
        """d phase / d (body shift along `axis`) per shot, rad/m, from g = ∇U at the points; with
        `with_time`, d phase / d (speed along `axis`) (the shift is v·t at each node), rad/(m/s)."""
        f = g.reshape(self.shots, self.nn, 4, 3)[..., axis]
        if with_time:
            f = f * self.node_times[:, :, None]
        comb = f[:, :, 0] - f[:, :, 1] - f[:, :, 2] + f[:, :, 3]  # d/ds U(p − s e) = −g·e
        return COUPLING * np.einsum("n,sn->s", self.w, comb)

    def leading(self, g_lead: F64) -> F64:
        """The paper's read per shot from g at `lead_points`."""
        m = len(g_lead) // 2
        return LEADING * (g_lead[m:, 2] - g_lead[:m, 2])


def json_safe(x: object) -> object:
    if isinstance(x, np.ndarray):
        return x.tolist()
    if isinstance(x, (np.floating, np.integer)):
        return x.item()
    if isinstance(x, float) and not math.isfinite(x):
        return None
    return x
