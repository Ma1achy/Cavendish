"""B8: the 1000:1 stack (b3's case), with the two things the primary sources suggest.
Added after reading Tonge 2012 and Macklin 2019 (see docs/research/rigid-bodies.md, "Changed after reading").
1D heights only, frame 1/60 s, g = -9.81, 6 s, restitution 0, same gap/rest conventions as b3_stack.py.
 A  PGS + Baumgarte, 1 substep, K sweeps       (copied from b3: the reference that fails)
 E  Tonge 2012 mass splitting (Jacobi, effective mass with m_i/n_i, full mass to apply), 1 substep, K sweeps,
    same Baumgarte as A.  (Tonge Eq. 15 scalar form; no blocks, because 1D contacts are one per body pair.)
 D  hard contacts, velocity-level LCP solved EXACTLY each substep (active set + dense solve; stands in for a
    direct / converged Krylov solve, Macklin 2019 s10.1.5), Baumgarte bias beta/h, K substeps.
Output: max penetration (mm), max |v| in the last second (m/s), top-box pose error at the end (mm).
"""
import numpy as np
G = -9.81; DT = 1/60
GAP0 = 0.0   # initial gap between boxes (m); a drop test sets it to 2 mm
def make(m): return list(m), [i+0.5+i*GAP0 for i in range(len(m))], [0.0]*len(m)
def gap(y, i): return (y[i]-(y[i-1] if i else 0.0))-(1.0 if i else 0.5)
def warm(lam, v, m):
    for i in range(len(m)):
        v[i] += lam[i]/m[i]
        if i: v[i-1] -= lam[i]/m[i-1]
def solver_A(m, y, v, K, frames, sink, beta=0.2, slop=0.005):
    n = len(m); lam = [0.0]*n; h = DT
    for f in range(frames):
        for i in range(n): v[i] += G*h
        warm(lam, v, m)
        for _ in range(K):
            for i in range(n):
                s = gap(y, i); vb = v[i]-(v[i-1] if i else 0.0)
                bias = beta/h*max(-s-slop, 0.0)
                im = 1/m[i]+(1/m[i-1] if i else 0.0)
                d = -(vb-bias)/im if s <= 0.0 else -(vb+s/h)/im
                new = max(lam[i]+d, 0.0); d = new-lam[i]; lam[i] = new
                v[i] += d/m[i]
                if i: v[i-1] -= d/m[i-1]
        for i in range(n): y[i] += v[i]*h
        sink(f, y, v)
def solver_E(m, y, v, K, frames, sink, beta=0.2, slop=0.005):
    n = len(m); lam = [0.0]*n; h = DT
    nc = [0]*n                       # contacts per body (ground counts for box 0's contact only)
    for i in range(n):
        nc[i] += 1                   # contact i (below box i)
        if i: nc[i-1] += 1           # same contact seen from box i-1
        # box i also has contact i+1 above it: counted when that i is visited
    for f in range(frames):
        for i in range(n): v[i] += G*h
        warm(lam, v, m)
        for _ in range(K):
            dl = [0.0]*n
            for i in range(n):
                s = gap(y, i); vb = v[i]-(v[i-1] if i else 0.0)
                bias = beta/h*max(-s-slop, 0.0)
                im = nc[i]/m[i]+(nc[i-1]/m[i-1] if i else 0.0)       # split masses: m/n in the effective mass
                d = -(vb-bias)/im if s <= 0.0 else -(vb+s/h)/im
                new = max(lam[i]+d, 0.0); dl[i] = new-lam[i]; lam[i] = new
            for i in range(n):                                      # Jacobi: apply all, FULL mass
                v[i] += dl[i]/m[i]
                if i: v[i-1] -= dl[i]/m[i-1]
        for i in range(n): y[i] += v[i]*h
        sink(f, y, v)
def solver_D(m, y, v, K, frames, sink, beta=0.2):
    n = len(m); h = DT/K
    N = np.zeros((n, n))
    for i in range(n):
        N[i, i] = 1/m[i]+(1/m[i-1] if i else 0.0)
        if i: N[i, i-1] = N[i-1, i] = -1/m[i-1]
    for f in range(frames):
        for _ in range(K):
            for i in range(n): v[i] += G*h
            vb0 = np.array([v[i]-(v[i-1] if i else 0.0) for i in range(n)])
            s = np.array([gap(y, i) for i in range(n)])
            c = np.where(s > 0, -s/h, beta*(-s)/h)                  # required relative velocity (>=)
            act = set(i for i in range(n) if vb0[i] < c[i])
            lam = np.zeros(n)
            for _it in range(60):
                A = sorted(act); lam[:] = 0.0
                if A:
                    lam[A] = np.linalg.solve(N[np.ix_(A, A)], (c-vb0)[A])
                neg = [i for i in A if lam[i] < -1e-15]
                if neg: act.discard(min(neg, key=lambda i: lam[i])); continue
                vb = vb0+N@lam
                bad = [i for i in range(n) if i not in act and vb[i] < c[i]-1e-15]
                if bad: act.add(bad[0]); continue
                break
            for i in range(n):
                v[i] += lam[i]/m[i]
                if i: v[i-1] -= lam[i]/m[i-1]
            for i in range(n): y[i] += v[i]*h
        sink(f, y, v)
def metrics(solver, masses, K, secs=6):
    m, y, v = make(masses); frames = int(secs*60); pen = [0.0]; mv = [0.0]
    def sink(f, y, v):
        pen[0] = max(pen[0], max(-gap(y, i) for i in range(len(y))))
        if f >= frames-60: mv[0] = max(mv[0], max(abs(x) for x in v))
    solver(m, y, v, K, frames, sink)
    return pen[0], mv[0], y[-1]-(len(y)-0.5)
cases = {"10 equal boxes": [1.0]*10, "1000 kg on five 1 kg boxes": [1.0]*5+[1000.0],
         "5 x 1000 kg under 1 kg": [1000.0]*5+[1.0],
         "geometric 8..32768 kg (Macklin Fig. 14, 4096:1), 5 boxes": [8.0, 64.0, 512.0, 4096.0, 32768.0]}
runs = (("A PGS+Baum", solver_A), ("E mass-split Jacobi", solver_E))
for name, ms in cases.items():
    print(f"\n{name}")
    print(f"{'K':>4} {'solver':<22} {'max pen mm':>11} {'max|v| last 1s':>15} {'top pose err mm':>16}")
    for K in (4, 20, 100, 1000):
        for nm, s in runs:
            p, mvv, e = metrics(s, ms, K)
            print(f"{K:>4} {nm:<22} {p*1e3:>11.3f} {mvv:>15.2e} {e*1e3:>16.3f}")
    for K in (4, 8, 20):
        p, mvv, e = metrics(solver_D, ms, K)
        print(f"{K:>4} {'D exact solve, K sub':<22} {p*1e3:>11.3f} {mvv:>15.2e} {e*1e3:>16.3f}")

# drop test: every box starts 2 mm above its resting place (so the whole stack lands), 1000 kg on five 1 kg boxes
GAP0 = 0.002
print("\n1000 kg on five 1 kg boxes, dropped from 2 mm gaps (rest = zero penetration, |v| -> 0)")
print(f"{'K':>4} {'solver':<22} {'max pen mm':>11} {'max|v| last 1s':>15} {'top pose err mm':>16}")
for K in (20, 100, 1000):
    p, mvv, e = metrics(solver_A, cases["1000 kg on five 1 kg boxes"], K)
    print(f"{K:>4} {'A PGS+Baum':<22} {p*1e3:>11.3f} {mvv:>15.2e} {e*1e3:>16.3f}")
for K in (4, 8, 20):
    p, mvv, e = metrics(solver_D, cases["1000 kg on five 1 kg boxes"], K)
    print(f"{K:>4} {'D exact solve, K sub':<22} {p*1e3:>11.3f} {mvv:>15.2e} {e*1e3:>16.3f}")
