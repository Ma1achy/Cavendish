"""Spectral windows of cadences: closed forms against numbers. Pure numpy; no physics.
docs/research/s2-replay-spec-paths-cadence.md section 4.  Run: python3 -I window_lab.py   (about a minute)

W(nu) = (1/N) sum_n exp(-2 pi i nu t_n); a unit tone sampled at t_n has the classical periodogram |W(nu - f)|^2.
"""
import numpy as np

rng = np.random.default_rng(1)


def window(t, nu):
    return np.exp(-2j * np.pi * np.outer(nu, t)).mean(axis=1)


def phi(nu, t0, d, kind):
    """E exp(-2 pi i nu T) for one cycle time T: positive U(t0, t0+d) or symmetric U(t0-d, t0+d)."""
    if kind == "pos":
        return np.exp(-2j * np.pi * nu * (t0 + d / 2)) * np.sinc(nu * d)
    return np.exp(-2j * np.pi * nu * t0) * np.sinc(2 * nu * d)


def mean_power(n, p):
    """E|W|^2 when t_n is a sum of n iid cycle times with characteristic function p (a random walk)."""
    s = p * (n * (1 - p) - (1 - p**n)) / (1 - p) ** 2
    return (n + 2 * s.real) / n**2


print("== 1 uniform sampling: Dirichlet kernel")
n, t0 = 400, 5.0
t = np.arange(n) * t0
nu = np.array([0.0013, 0.0371, 0.1999, 0.2 + 1 / (n * t0) / 2, 0.07])
dirichlet = np.abs(np.sin(n * np.pi * nu * t0) / (n * np.sin(np.pi * nu * t0)))
print("  max |numeric - Dirichlet| =", np.abs(np.abs(window(t, nu)) - dirichlet).max())
print("  |W(1/T0)| =", abs(window(t, np.array([1 / t0]))[0]), "(the alias is as tall as the peak)")

print("== 2 cycle times drawn independently, so shot times are a random walk")
for kind, d in [("pos", 0.1), ("pos", 0.3), ("sym", 0.1), ("sym", 0.3)]:
    t0, n = 5.0, 500
    tbar = t0 + (d / 2 if kind == "pos" else 0)
    nus = np.array([1 / tbar, 1 / tbar + 2e-4, 2 / tbar, 0.37])
    seeds = 4000
    power = np.zeros((seeds, len(nus)))
    for s in range(seeds):
        cycle = rng.uniform(t0, t0 + d, n) if kind == "pos" else rng.uniform(t0 - d, t0 + d, n)
        times = np.concatenate([[0], np.cumsum(cycle[:-1])])
        power[s] = np.abs(window(times, nus)) ** 2
    predicted = mean_power(n, phi(nus, t0, d, kind))
    print(f"  {kind} d={d}, N={n}, {seeds} seeds: measured / closed form =", np.round(power.mean(0) / predicted, 3),
          "+-", np.round(power.std(0) / np.sqrt(seeds) / predicted, 3))
print("  alias-to-peak amplitude sqrt(E|W|^2) at nu = 1/Tbar, positive jitter, T0 = 5 s:")
for d in [0.1, 0.2, 0.3]:
    t0 = 5.0
    tbar = t0 + d / 2
    p = phi(np.array([1 / tbar]), t0, d, "pos")
    row = [float(np.sqrt(mean_power(n, p)[0])) for n in (5000, 20000, 100000)]
    print(f"    d={d}: N=5e3 {row[0]:.4f}  N=2e4 {row[1]:.4f}  N=1e5 {row[2]:.4f}   1-|phi| = {1 - abs(p[0]):.2e}")

print("== 2b the other reading of jitter: t_n = n T0 + e_n, e_n independent (a Debye-Waller factor)")
n, t0, a = 2000, 5.0, 0.3
nus = np.array([1 / t0, 2 / t0, 0.0371, 0.123])
power = np.zeros(len(nus))
for _ in range(2000):
    power += np.abs(window(np.arange(n) * t0 + rng.uniform(-a, a, n), nus)) ** 2
power /= 2000
psi = np.sinc(2 * nus * a)
at_line = np.abs(psi) ** 2 + (1 - np.abs(psi) ** 2) / n
off_line = np.abs(psi) ** 2 * np.abs(np.sin(n * np.pi * nus * t0) / (n * np.sin(np.pi * nus * t0))) ** 2 + (1 - np.abs(psi) ** 2) / n
print("  measured / predicted:", np.round(power / np.array([at_line[0], at_line[1], off_line[2], off_line[3]]), 3),
      "; the coherent alias amplitude |psi(1/T0)| =", round(float(abs(psi[0])), 4), "does not fall with N")

print("== 3 alternating cycle times T_A, T_B: sideband heights |cos(pi k T_A / T_s)|")
for ta, tb in [(5.5, 6.0), (5.0, 5.5), (5.0, 8.0)]:
    ts = ta + tb
    heights = []
    for n in (2000, 20000):
        times = np.empty(n)
        times[0::2] = np.arange(n // 2) * ts
        times[1::2] = np.arange(n // 2) * ts + ta
        k = np.arange(0, 7)
        heights.append(np.abs(window(times, k / ts)))
    predicted = np.abs(np.cos(np.pi * np.arange(0, 7) * ta / ts))
    print(f"  ({ta},{tb}) Ts={ts}: predicted {np.round(predicted, 4)}; N=2e3 {np.round(heights[0], 4)}; N=2e4 {np.round(heights[1], 4)}")

print("== 4 planned gaps: L shots on, P-L off, period P shots")
p_, l_, n = 100, 40, 20000
times = np.arange(n) * 5.0
mask = (np.arange(n) % p_) < l_
k = np.arange(0, 6)
w = np.abs(window(times[mask], k / (p_ * 5.0)))
exact = np.abs(np.sin(np.pi * k * l_ / p_) / (l_ * np.sin(np.pi * np.maximum(k, 1e-12) / p_)))
exact[0] = 1
print("  relative to k=0: predicted", np.round(exact, 4), "measured", np.round(w / w[0], 4))

print("== 5 shots lost independently with probability p: power off the lines, normalised by the kept count")
n, t0 = 4000, 5.0
times = np.arange(n) * t0
for p in (0.01, 0.05):
    nus = rng.uniform(0.01, 0.09, 40)
    acc = 0.0
    for _ in range(300):
        keep = rng.random(n) > p
        acc += np.abs(window(times[keep], nus)) ** 2
    print(f"  p={p}: measured {(acc / 300).mean():.3e}; p/((1-p)N) = {p / ((1 - p) * n):.3e}")

print("== 6 Lomb-Scargle of a pure tone against the window")


def lomb_scargle(t, y, f):
    y = y - y.mean()
    wt = 2 * np.pi * f[:, None] * t[None, :]
    a = 0.5 * np.arctan2((2 * np.cos(wt) * np.sin(wt)).sum(1), (np.cos(wt) ** 2 - np.sin(wt) ** 2).sum(1))
    c, s = np.cos(wt - a[:, None]), np.sin(wt - a[:, None])
    return 0.5 * ((c @ y) ** 2 / (c * c).sum(1) + (s @ y) ** 2 / (s * s).sum(1))


n, t0, d, fs = 3000, 5.0, 0.2, 0.05
for name, cycle in [("uniform", np.full(n, t0)), ("positive jitter 0.2", rng.uniform(t0, t0 + d, n)), ("alternating 5.5/6", np.tile([5.5, 6.0], n // 2))]:
    times = np.concatenate([[0], np.cumsum(cycle[:-1])])
    y = np.cos(2 * np.pi * fs * times)
    tbar = cycle.mean()
    for k in (1, 2):
        fa = abs(k / 11.5 - fs) if name.startswith("alt") else (abs(1 / tbar - fs) if k == 1 else 2 / tbar - fs)
        fq = np.linspace(fa - 3e-4, fa + 3e-4, 601)
        fm = np.linspace(fs - 3e-4, fs + 3e-4, 601)
        ratio_ls = np.sqrt(lomb_scargle(times, y, fq).max() / lomb_scargle(times, y, fm).max())
        ratio_w = max(np.abs(window(times, fq - fs)).max(), np.abs(window(times, fq + fs)).max())
        print(f"  {name:<20} k={k} at {fa:.5f} Hz: LS sqrt-peak ratio {ratio_ls:.4f}; window max|W| in the same band {ratio_w:.4f}")
