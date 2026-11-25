"""Toy CRB benchmark for the Gradar research note (not repository code).

A point mass walking in a straight line past the default five-tower AION-10 array. The phase per
shot is the arm integral (m_atom/hbar) * int_0^2T [V(upper arm) - V(lower arm)] dt for each
interferometer, upper interferometer minus lower, with the source moving during the flight.
Arms: lower arm z0 + u0 t - g t^2/2; upper arm adds v_kick * t (t < T) and v_kick * (2T - t) after.
Constants are the reference scripts' (docs/research/s1-periodograms-and-script-fixtures.md).

Answers four questions with numbers:
  1. Is a finite-difference Jacobian good enough for the Fisher matrix (f64 vs f32 phases)?
  2. How large are the CRBs on mass, position and velocity at realistic distances?
  3. Does a matched-filter bank + Gauss-Newton (the M17 baseline) attain the CRB, and where not?
  4. How far is a grid inversion (Tikhonov + positivity, centroid) from the CRB?
"""

import sys
import time

import numpy as np

G, HBAR, M_ATOM, GRAV = 6.67e-11, 1.055e-34, 1.46e-25, 9.81
V_KICK, T_HALF, U0, DR, CYCLE = 6.504651743802706, 0.73, 3.86, 5.0, 2.0
TOWERS = np.array([[0.0, 0.0], [4.0, 0.0], [-4.0, 0.0], [0.0, 4.0], [0.0, -4.0]])
Z_SOURCE = 1.0  # walker's centre of mass height, known
NODES, WEIGHTS = np.polynomial.legendre.leggauss(64)


def arm_tables(nodes: np.ndarray) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    t = T_HALF * (nodes + 1.0)  # [0, 2T]
    low = U0 * t - 0.5 * GRAV * t**2
    kick = np.where(t < T_HALF, V_KICK * t, V_KICK * (2 * T_HALF - t))
    return t, low, low + kick


def phases(theta: np.ndarray, shot_times: np.ndarray, nodes=NODES, weights=WEIGHTS) -> np.ndarray:
    """Phase [shots, towers] for theta = (mass kg, x0 m, y0 m, vx m/s, vy m/s), position at t = 0."""
    m, x0, y0, vx, vy = theta
    t, low, up = arm_tables(nodes)
    w = weights * T_HALF
    tt = shot_times[:, None] + t[None, :]  # [S, Q] source time during flight
    xs, ys = x0 + vx * tt, y0 + vy * tt
    dx = TOWERS[None, :, 0, None] - xs[:, None, :]  # [S, D, Q]
    dy = TOWERS[None, :, 1, None] - ys[:, None, :]
    h2 = dx**2 + dy**2
    total = 0.0
    for base, sign in ((DR, 1.0), (0.0, -1.0)):  # upper IFO minus lower IFO
        for z, s2 in ((base + up, 1.0), (base + low, -1.0)):  # upper arm minus lower arm
            dz = z[None, None, :] - Z_SOURCE
            total = total + sign * s2 * (-G * m / np.sqrt(h2 + dz**2))
    return (M_ATOM / HBAR) * np.einsum("sdq,q->sd", total, w)


def jacobian(theta, shot_times, steps, cast=np.float64):
    cols = []
    for i, h in enumerate(steps):
        e = np.zeros(5)
        e[i] = h
        fp = phases(theta + e, shot_times).astype(cast).astype(np.float64)
        fm = phases(theta - e, shot_times).astype(cast).astype(np.float64)
        cols.append(((fp - fm) / (2 * h)).ravel())
    return np.stack(cols, axis=1)


def fisher(theta, shot_times, sigma, steps=None, cast=np.float64):
    steps = steps if steps is not None else np.array([1e-4 * theta[0], 1e-4, 1e-4, 1e-5, 1e-5])
    j = jacobian(theta, shot_times, steps, cast)
    return j.T @ j / sigma**2


# ---------------------------------------------------------------------------------------------
def q1_fd_accuracy(theta, shot_times):
    print("\n## Q1 finite-difference Jacobian accuracy")
    s = phases(theta, shot_times).ravel()
    exact_dm = s / theta[0]  # linear in mass: exact derivative, the control
    for h_rel in (1e-2, 1e-4, 1e-6, 1e-8):
        j = jacobian(theta, shot_times, np.array([h_rel * theta[0], 1e-4, 1e-4, 1e-5, 1e-5]))
        err = np.max(np.abs(j[:, 0] - exact_dm)) / np.max(np.abs(exact_dm))
        print(f"  mass step {h_rel:.0e} rel: max rel error of dphi/dm = {err:.1e}")
    # Position derivative: Richardson reference from tiny-step f64 vs steps, f64 and f32 phases.
    ref = jacobian(theta, shot_times, np.array([1.0, 1e-3, 1e-3, 1e-4, 1e-4]))
    ref2 = jacobian(theta, shot_times, np.array([1.0, 5e-4, 5e-4, 5e-5, 5e-5]))
    rich = (4 * ref2 - ref) / 3
    for cast, name in ((np.float64, "f64"), (np.float32, "f32")):
        for h in (1e-1, 1e-2, 1e-3, 1e-4):
            j = jacobian(theta, shot_times, np.array([1.0, h, h, h, h]), cast)
            err = np.max(np.abs(j[:, 1] - rich[:, 1])) / np.max(np.abs(rich[:, 1]))
            print(f"  {name} phases, step {h:.0e} m: max rel error of dphi/dx0 = {err:.1e}")
    # Quadrature convergence: 32 vs 128 nodes.
    n128, w128 = np.polynomial.legendre.leggauss(128)
    d = np.max(np.abs(phases(theta, shot_times) - phases(theta, shot_times, n128, w128)))
    print(f"  64- vs 128-node quadrature: max |dphi| = {d:.1e} rad (peak {np.max(np.abs(s)):.2e} rad)")


# ---------------------------------------------------------------------------------------------
def build_bank(shot_times, xs, ys, vxs, vys):
    grid = np.array(np.meshgrid(xs, ys, vxs, vys, indexing="ij")).reshape(4, -1).T
    bank = np.empty((len(grid), len(shot_times) * len(TOWERS)))
    for k, (x, y, vx, vy) in enumerate(grid):
        bank[k] = phases(np.array([1.0, x, y, vx, vy]), shot_times).ravel()
    norms = np.sqrt(np.einsum("ij,ij->i", bank, bank))
    return grid, bank / norms[:, None], norms


LO = np.array([0.0, -150.0, -150.0, -5.0, -5.0])
HI = np.array([5000.0, 150.0, 150.0, 5.0, 5.0])


def gauss_newton(theta, data, shot_times, iters=30):
    """Damped Gauss-Newton inside the prior box LO..HI (a step that raises chi2 is halved)."""
    th = np.clip(theta.copy(), LO, HI)
    c = chi2(th, data, shot_times)
    for _ in range(iters):
        r = data - phases(th, shot_times).ravel()
        j = jacobian(th, shot_times, np.array([1e-4 * abs(th[0]) + 1e-6, 1e-4, 1e-4, 1e-5, 1e-5]))
        step = np.linalg.lstsq(j, r, rcond=None)[0]
        for _ in range(12):
            cand = np.clip(th + step, LO, HI)
            cc = chi2(cand, data, shot_times)
            if cc <= c:
                break
            step = step / 2
        else:
            break
        moved = np.max(np.abs(cand[1:] - th[1:]))
        th, c = cand, cc
        if moved < 1e-7:
            break
    return th


def log_r_max(theta, shot_times, sigma):
    """Vallisneri (2008) eq. 47 on the Fisher 1-sigma surface along each eigenvector, max |log r|."""
    f = fisher(theta, shot_times, sigma)
    j = jacobian(theta, shot_times, np.array([1e-4 * theta[0], 1e-4, 1e-4, 1e-5, 1e-5]))
    h0 = phases(theta, shot_times).ravel()
    lam, vec = np.linalg.eigh(f)
    worst = 0.0
    for k in range(5):
        for sgn in (1.0, -1.0):
            dth = sgn * vec[:, k] / np.sqrt(lam[k])
            dh = phases(theta + dth, shot_times).ravel() - h0
            res = dh - j @ dth
            worst = max(worst, 0.5 * float(res @ res) / sigma**2)
    return worst


def chi2(theta, data, shot_times):
    return float(np.sum((data - phases(theta, shot_times).ravel()) ** 2))


_INV_CACHE: dict = {}


def grid_inversion(data, shot_times, sigma, window=10.0, alpha=1e-3):
    """M17 baseline (b): static unit masses on a 1 m grid at z = Z_SOURCE, data in |t| <= window/2.

    Tikhonov-regularised, positivity by projected gradient; returns (mass, x, y) as total and
    centroid. The regularisation is chosen by the discrepancy principle in a crude scan.
    """
    sel = np.abs(shot_times) <= window / 2
    d = data.reshape(len(shot_times), len(TOWERS))[sel].ravel()
    gx, gy = np.meshgrid(np.arange(-20, 20.5, 1.0), np.arange(-2, 30.5, 1.0), indexing="ij")
    cells = np.stack([gx.ravel(), gy.ravel()], 1)
    key = (window, len(TOWERS))
    if key not in _INV_CACHE:
        _INV_CACHE[key] = np.stack(
            [phases(np.array([1.0, c[0], c[1], 0.0, 0.0]), shot_times[sel]).ravel() for c in cells], 1
        )
    a = _INV_CACHE[key]
    best = None
    for lam in alpha * np.logspace(-4, 2, 7) * np.linalg.norm(a, 2) ** 2:
        m = np.zeros(len(cells))
        lr = 1.0 / (np.linalg.norm(a, 2) ** 2 + lam)
        for _ in range(400):
            m = np.maximum(m - lr * (a.T @ (a @ m - d) + lam * m), 0.0)
        misfit = np.sum((a @ m - d) ** 2) / sigma**2
        if best is None or abs(misfit - d.size) < abs(best[0] - d.size):
            best = (misfit, m)
    m = best[1]
    tot = m.sum()
    if tot <= 0:
        return np.array([0.0, np.nan, np.nan])
    return np.array([tot, *(m @ cells / tot)])


# ---------------------------------------------------------------------------------------------
def main():
    global TOWERS, Z_SOURCE
    rng = np.random.default_rng(20261125)
    shot_times = np.arange(-30.0, 30.0 + 1e-9, CYCLE)  # 31 shots per tower, all towers in step
    print(f"shots per tower {len(shot_times)}, towers {len(TOWERS)}, data {len(shot_times) * len(TOWERS)}")

    # Sanity check against the S15 note's figure: 10 kg at 10 m reads about -8.5 mrad on AION-10.
    for zh in (0.0, 1.0, 2.5, 5.0):
        old, Z_SOURCE = Z_SOURCE, zh
        p = phases(np.array([10.0, 10.0, 0.0, 0.0, 0.0]), np.array([0.0]))[0, 0]
        Z_SOURCE = old
        print(f"  10 kg at 10 m horizontal, height {zh} m: phase on the centre tower {p * 1e3:+.2f} mrad")

    theta_ref = np.array([75.0, 0.0, 8.0, 1.0, 0.0])
    q1_fd_accuracy(theta_ref, shot_times)

    print("\n## Q2/Q3 CRB and the matched-filter + Gauss-Newton baseline")
    t0 = time.perf_counter()
    xs = np.arange(-6.0, 6.01, 1.0)
    vxs = np.arange(0.5, 1.51, 0.25)
    vys = np.arange(-0.3, 0.31, 0.15)
    sigma = 1e-3
    n_trials = int(sys.argv[1]) if len(sys.argv) > 1 else 200
    rows = []
    for y0, mass in ((8.0, 75.0), (15.0, 75.0), (30.0, 75.0), (45.0, 75.0), (60.0, 75.0), (60.0, 750.0)):
        theta = np.array([mass, 0.0, y0, 1.0, 0.0])
        s = phases(theta, shot_times).ravel()
        snr = np.linalg.norm(s) / sigma
        f = fisher(theta, shot_times, sigma)
        crb = np.sqrt(np.diag(np.linalg.inv(f)))
        cond = np.linalg.cond(f / np.outer(np.sqrt(np.diag(f)), np.sqrt(np.diag(f))))
        ys = y0 + np.arange(-0.4, 0.41, 0.1) * y0
        tb = time.perf_counter()
        grid, bank, norms = build_bank(shot_times, xs, ys, vxs, vys)
        t_bank = time.perf_counter() - tb
        est, est_inv, miss = [], [], 0
        for _ in range(n_trials):
            d = s + sigma * rng.standard_normal(s.size)
            score = bank @ d
            k = int(np.argmax(score))  # positive mass: maximise the signed correlation
            m0 = score[k] / norms[k]
            th0 = np.array([m0, *grid[k]])
            th = gauss_newton(th0, d, shot_times)
            th_true_start = gauss_newton(theta, d, shot_times)
            if chi2(th_true_start, d, shot_times) < chi2(th, d, shot_times) - 1e-6 * sigma**2:
                miss += 1
                th = th_true_start
            est.append(th)
            if len(est_inv) < min(n_trials, 60):
                est_inv.append(grid_inversion(d, shot_times, sigma))
        est = np.array(est)
        err = est - theta
        rmse = np.sqrt(np.mean(err**2, 0))
        bias = np.mean(err, 0)
        inv = np.array(est_inv)
        inv_rmse = np.sqrt(np.nanmean((inv - theta[:3]) ** 2, 0))
        # Expected coverage of the Fisher 1-sigma ellipsoid (chi2 with 5 dof: 0.5841 inside at d2<=5.0?)
        d2 = np.einsum("ni,ij,nj->n", err, f, err)
        cover68 = np.mean(d2 <= 5.886)  # chi2(5) 68.27 % quantile
        rows.append((y0, mass, snr, cond, crb, rmse, bias, miss, inv_rmse, cover68, t_bank))
        print(f"\n  [Vallisneri max |log r| on the Fisher 1-sigma surface: {log_r_max(theta, shot_times, sigma):.3g}]")
        print(f"  y0 = {y0:.0f} m, M = {mass:.0f} kg: optimal SNR {snr:.1f}, Fisher condition (normalised) {cond:.1e}")
        print(f"    CRB sigma  : M {crb[0]:.3g} kg, x0 {crb[1]:.3g} m, y0 {crb[2]:.3g} m, vx {crb[3]:.3g} m/s, vy {crb[4]:.3g} m/s")
        print(f"    MF+GN RMSE : M {rmse[0]:.3g}, x0 {rmse[1]:.3g}, y0 {rmse[2]:.3g}, vx {rmse[3]:.3g}, vy {rmse[4]:.3g}")
        print(f"    RMSE/CRB   : {np.array2string(rmse / crb, precision=2)}  bias/CRB {np.array2string(bias / crb, precision=2)}")
        print(f"    bank misses (GN from bank worse than GN from truth): {miss}/{n_trials}; bank {len(grid)} templates in {t_bank:.1f} s")
        print(f"    Fisher-ellipsoid 68% coverage of the ML estimates: {cover68:.2f}")
        print(f"    grid inversion (10 s window, {len(est_inv)} trials) RMSE: M {inv_rmse[0]:.3g}, x {inv_rmse[1]:.3g}, y {inv_rmse[2]:.3g}")
    print(f"\n  total {time.perf_counter() - t0:.0f} s")

    print("\n## Control: a Fisher matrix with a wrong noise level must fail the efficiency check")
    y0, mass = 8.0, 75.0
    r = rows[0]
    wrong = r[4] / 2.0  # CRB computed as if sigma were half the truth
    print(f"  RMSE / wrong-CRB = {np.array2string(r[5] / wrong, precision=2)}  (should be ~2, outside [0.8, 1.25])")

    print("\n## Single tower: degeneracy")
    keep = TOWERS
    TOWERS = keep[:1]
    for y0 in (8.0, 15.0):
        theta = np.array([75.0, 0.0, y0, 1.0, 0.0])
        f = fisher(theta, shot_times, sigma)
        fn = f / np.outer(np.sqrt(np.diag(f)), np.sqrt(np.diag(f)))
        w, v = np.linalg.eigh(fn)
        print(f"  one tower, y0 {y0}: normalised condition {w[-1] / w[0]:.1e}; weakest direction (M,x0,y0,vx,vy) {np.array2string(v[:, 0], precision=2)}")
    TOWERS = keep


if __name__ == "__main__":
    main()
