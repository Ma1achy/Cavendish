"""Differentially heated square cavity (de Vahl Davis 1983) on a uniform MAC grid.

The discretisation S8 proposes, in two dimensions: staggered velocities, second-order
conservative central advection (Harlow & Welch), explicit viscous and thermal diffusion,
an exact projection (Neumann Poisson solved by a cosine transform, so divergence is zero
to round-off), Boussinesq buoyancy on vertical faces. SSP-RK3 in time, projected per stage.

Non-dimensional: length L, velocity alpha/L, time L^2/alpha.
    du/dt + div(u u) = -grad p + Pr lap u + Ra Pr T y_hat
    dT/dt + div(u T) = lap T
Hot wall x = 0 at T = +1/2, cold wall x = 1 at T = -1/2, adiabatic top and bottom, no slip.

Usage: python -I cavity.py RA N [T_END]   -> prints one JSON line.
Purpose: measure the Nusselt error against de Vahl Davis at each pitch, so the S8 exit
("Nu within 1 %") can be given a grid before the slice starts.
"""

import ctypes
import json
import sys
import time

import numpy as np

PR = 0.71


def lower_priority() -> None:
    """Run below normal priority so the desktop stays responsive (Windows only)."""
    try:
        k = ctypes.windll.kernel32  # type: ignore[attr-defined]
        k.SetPriorityClass(k.GetCurrentProcess(), 0x4000)
    except AttributeError:
        pass


def neumann_basis(n: int, h: float) -> tuple[np.ndarray, np.ndarray]:
    """Orthonormal eigenvectors and eigenvalues of the 1D cell-centred Neumann Laplacian."""
    i = np.arange(n) + 0.5
    k = np.arange(n)
    c = np.cos(np.pi * np.outer(i, k) / n)
    c /= np.linalg.norm(c, axis=0)
    lam = -(2.0 - 2.0 * np.cos(np.pi * k / n)) / h**2
    return c, lam


def run(ra: float, n: int, t_end: float) -> dict[str, float]:
    h = 1.0 / n
    c, lam = neumann_basis(n, h)
    denom = lam[:, None] + lam[None, :]
    denom[0, 0] = 1.0

    u = np.zeros((n + 1, n))  # x-faces, u[i, j] at x = i h, y = (j + 1/2) h
    v = np.zeros((n, n + 1))  # y-faces
    x = (np.arange(n) + 0.5) * h
    temp = np.tile((0.5 - x)[:, None], (1, n))  # conduction profile to start

    def project(uu: np.ndarray, vv: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
        div = (uu[1:, :] - uu[:-1, :] + vv[:, 1:] - vv[:, :-1]) / h
        dh = c.T @ div @ c
        ph = dh / denom
        ph[0, 0] = 0.0
        p = c @ ph @ c.T
        uu = uu.copy()
        vv = vv.copy()
        uu[1:-1, :] -= (p[1:, :] - p[:-1, :]) / h
        vv[:, 1:-1] -= (p[:, 1:] - p[:, :-1]) / h
        return uu, vv

    def rhs(uu: np.ndarray, vv: np.ndarray, tt: np.ndarray):
        # temperature with ghosts: Dirichlet in x, Neumann in y
        tg = np.empty((n + 2, n + 2))
        tg[1:-1, 1:-1] = tt
        tg[0, 1:-1] = 2 * 0.5 - tt[0, :]
        tg[-1, 1:-1] = 2 * -0.5 - tt[-1, :]
        tg[1:-1, 0] = tt[:, 0]
        tg[1:-1, -1] = tt[:, -1]
        # face temperatures (central) and fluxes
        tfx = 0.5 * (tg[:-1, 1:-1] + tg[1:, 1:-1])  # (n+1, n) at x-faces
        tfy = 0.5 * (tg[1:-1, :-1] + tg[1:-1, 1:])  # (n, n+1)
        adv_t = (uu * tfx)[1:, :] - (uu * tfx)[:-1, :] + (vv * tfy)[:, 1:] - (vv * tfy)[:, :-1]
        lap_t = (tg[2:, 1:-1] + tg[:-2, 1:-1] + tg[1:-1, 2:] + tg[1:-1, :-2] - 4 * tt) / h**2
        dt_t = -adv_t / h + lap_t

        # u momentum at interior x-faces i = 1..n-1
        ug = np.zeros((n + 1, n + 2))
        ug[:, 1:-1] = uu
        ug[:, 0] = -uu[:, 0]
        ug[:, -1] = -uu[:, -1]
        uc = 0.5 * (uu[:-1, :] + uu[1:, :])  # u at cell centres (n, n)
        fxx = uc * uc
        vcorner = 0.5 * (vv[:-1, :] + vv[1:, :])  # v at corners (n-1, n+1), interior x
        ucorner = 0.5 * (ug[1:-1, :-1] + ug[1:-1, 1:])  # u at corners (n-1, n+1)
        fxy = ucorner * vcorner
        adv_u = (fxx[1:, :] - fxx[:-1, :]) / h + (fxy[:, 1:] - fxy[:, :-1]) / h
        lap_u = (ug[2:, 1:-1] + ug[:-2, 1:-1] + ug[1:-1, 2:] + ug[1:-1, :-2] - 4 * uu[1:-1, :]) / h**2
        du = np.zeros_like(uu)
        du[1:-1, :] = -adv_u + PR * lap_u

        # v momentum at interior y-faces j = 1..n-1
        vg = np.zeros((n + 2, n + 1))
        vg[1:-1, :] = vv
        vg[0, :] = -vv[0, :]
        vg[-1, :] = -vv[-1, :]
        vc = 0.5 * (vv[:, :-1] + vv[:, 1:])
        fyy = vc * vc
        ucorner2 = 0.5 * (uu[:, :-1] + uu[:, 1:])  # u at corners (n+1, n-1)
        vcorner2 = 0.5 * (vg[:-1, 1:-1] + vg[1:, 1:-1])  # v at corners (n+1, n-1)
        fyx = ucorner2 * vcorner2
        adv_v = (fyy[:, 1:] - fyy[:, :-1]) / h + (fyx[1:, :] - fyx[:-1, :]) / h
        lap_v = (vg[2:, 1:-1] + vg[:-2, 1:-1] + vg[1:-1, 2:] + vg[1:-1, :-2] - 4 * vv[:, 1:-1]) / h**2
        buoy = ra * PR * 0.5 * (tt[:, :-1] + tt[:, 1:])
        dv = np.zeros_like(vv)
        dv[:, 1:-1] = -adv_v + PR * lap_v + buoy
        return du, dv, dt_t

    def nusselt(tt: np.ndarray) -> tuple[float, float]:
        hot = -(-8 * 0.5 + 9 * tt[0, :] - tt[1, :]) / (3 * h)
        cold = -(-8 * -0.5 + 9 * tt[-1, :] - tt[-2, :]) / (3 * h)
        return float(hot.mean()), float(-cold.mean())

    t = 0.0
    steps = 0
    t0 = time.time()
    resid = 1.0
    umax_guess = max(1.0, 0.25 * np.sqrt(ra))
    while t < t_end:
        umax = max(np.abs(u).max(), np.abs(v).max(), 1e-12)
        dt = min(0.2 * h * h / max(PR, 1.0), 0.5 * h / max(umax, 1e-3 * umax_guess))
        # SSP-RK3, projecting each stage
        a = rhs(u, v, temp)
        u1, v1 = project(u + dt * a[0], v + dt * a[1])
        t1 = temp + dt * a[2]
        b = rhs(u1, v1, t1)
        u2, v2 = project(0.75 * u + 0.25 * (u1 + dt * b[0]), 0.75 * v + 0.25 * (v1 + dt * b[1]))
        t2 = 0.75 * temp + 0.25 * (t1 + dt * b[2])
        cc = rhs(u2, v2, t2)
        un, vn = project(u / 3 + 2 / 3 * (u2 + dt * cc[0]), v / 3 + 2 / 3 * (v2 + dt * cc[1]))
        tn = temp / 3 + 2 / 3 * (t2 + dt * cc[2])
        steps += 1
        if steps % 200 == 0:
            resid = float(np.abs(tn - temp).max() / dt)
            if resid < 1e-5 and t > 0.05:
                u, v, temp = un, vn, tn
                t += dt
                break
        u, v, temp = un, vn, tn
        t += dt

    hot, cold = nusselt(temp)
    div = (u[1:, :] - u[:-1, :] + v[:, 1:] - v[:, :-1]) / h
    # vertical velocity max on the horizontal midline, horizontal on the vertical midline
    vmid = 0.5 * (v[:, n // 2] + v[:, n // 2]) if n % 2 == 0 else v[:, n // 2]
    umid = u[n // 2, :]
    return {
        "Ra": ra,
        "N": n,
        "t": t,
        "steps": steps,
        "dT_dt_max": resid,
        "Nu_hot": hot,
        "Nu_cold": cold,
        "umax_midline": float(np.abs(umid).max()),
        "vmax_midline": float(np.abs(vmid).max()),
        "div_max": float(np.abs(div).max()),
        "wall_s": time.time() - t0,
    }


if __name__ == "__main__":
    lower_priority()
    ra_arg = float(sys.argv[1])
    n_arg = int(sys.argv[2])
    t_arg = float(sys.argv[3]) if len(sys.argv) > 3 else 3.0
    print(json.dumps(run(ra_arg, n_arg, t_arg)), flush=True)
