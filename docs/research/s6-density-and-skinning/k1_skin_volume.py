"""K1: what linear blend skinning (LBS) and dual-quaternion skinning (DQS) do to a tetrahedral cage's volume,
and whether either reproduces a single bone's rigid transform bit for bit.

The body is a square bar 0.1 x 0.1 x 0.6 m along x, cut into n x n x 6n cubes and each cube into the six Kuhn
(Freudenthal) tetrahedra (conforming, all positive).  Two bones: bone 0 is the identity, bone 1 rotates about the
joint at x = 0.3 m (a bend about z, or a twist about x).  Weight of bone 1 at a vertex: smoothstep over
x in [0.25, 0.35] m, so the blend region is 0.1 m long.  LBS: Kavan et al. 2008 Eq. (1).  DQS: their Eq. (11) with
Algorithm 1 (normalise the blended dual quaternion, rotate by Eq. (15)).  Each tet keeps its rest mass, so its
density after skinning is m / V' and V'/V = det A of the tet's affine map.

Part A: total volume ratio, per-tet det A range and inverted tets, for bends and twists, LBS and DQS.
Part B: convergence of the deformed centroid and second moment (uniform density) as the cage refines, DQS bend 90.
Part C: bit-exactness of the rigid limit: weights (1, 0) against applying bone 1's transform directly.
Part D: inverted tets against the width of the blend region.
Part E: the distance between the LBS and DQS mass distributions (centroid, second moment).
"""
import numpy as np

# ---------- quaternion and dual-quaternion helpers (w, x, y, z) ----------
def qmul(a, b):
    aw, ax, ay, az = a[..., 0], a[..., 1], a[..., 2], a[..., 3]
    bw, bx, by, bz = b[..., 0], b[..., 1], b[..., 2], b[..., 3]
    return np.stack([aw*bw - ax*bx - ay*by - az*bz, aw*bx + ax*bw + ay*bz - az*by,
                     aw*by - ax*bz + ay*bw + az*bx, aw*bz + ax*by - ay*bx + az*bw], -1)

def qaxis(axis, ang):
    q = np.zeros(4); q[0] = np.cos(ang/2); q[1:] = np.sin(ang/2)*np.asarray(axis, float)/np.linalg.norm(axis)
    return q

def qmat(q):
    w, x, y, z = q
    return np.array([[1-2*(y*y+z*z), 2*(x*y-w*z), 2*(x*z+w*y)],
                     [2*(x*y+w*z), 1-2*(x*x+z*z), 2*(y*z-w*x)],
                     [2*(x*z-w*y), 2*(y*z+w*x), 1-2*(x*x+y*y)]])

def dq_from(q, t):           # unit dual quaternion of x -> R(q) x + t
    return q, 0.5*qmul(np.array([0.0, *t]), q)

def dqs_apply(w, dqs, p):    # Kavan Algorithm 1 for points p (N,3), weights w (N,k)
    b0 = sum(w[:, [i]]*dqs[i][0] for i in range(len(dqs)))
    be = sum(w[:, [i]]*dqs[i][1] for i in range(len(dqs)))
    nrm = np.linalg.norm(b0, axis=1, keepdims=True)
    c0, ce = b0/nrm, be/nrm
    a0, d0 = c0[:, :1], c0[:, 1:]; a, d = ce[:, :1], ce[:, 1:]
    return (p + 2*np.cross(d0, np.cross(d0, p) + a0*p) + 2*(a0*d - a*d0 + np.cross(d0, d)))

def lbs_apply(w, mats, p):   # sum_i w_i (R_i p + t_i)
    return sum(w[:, [i]]*(p @ R.T + t) for i, (R, t) in enumerate(mats))

# ---------- the cage ----------
KUHN = [(0, 1, 3, 7), (0, 1, 5, 7), (0, 2, 3, 7), (0, 2, 6, 7), (0, 4, 5, 7), (0, 4, 6, 7)]  # corners bit-coded zyx

def bar(n):
    nx, ny, nz = 6*n, n, n; L, H = 0.6, 0.1
    xs, ys, zs = np.linspace(0, L, nx+1), np.linspace(-H/2, H/2, ny+1), np.linspace(-H/2, H/2, nz+1)
    X, Y, Z = np.meshgrid(xs, ys, zs, indexing="ij"); V = np.stack([X.ravel(), Y.ravel(), Z.ravel()], 1)
    idx = lambda i, j, k: (i*(ny+1) + j)*(nz+1) + k
    T = []
    for i in range(nx):
        for j in range(ny):
            for k in range(nz):
                c = [idx(i+(b & 1), j+((b >> 1) & 1), k+((b >> 2) & 1)) for b in range(8)]
                T += [[c[a] for a in t] for t in KUHN]
    T = np.array(T)
    D = V[T[:, 1:]] - V[T[:, :1]]; vol = np.linalg.det(D)/6
    T[vol < 0] = T[vol < 0][:, [0, 2, 1, 3]]           # orient positively
    return V, T

def tet_vol(V, T):
    return np.linalg.det(V[T[:, 1:]] - V[T[:, :1]])/6

def weights(V, width=0.1):
    s = np.clip((V[:, 0] - (0.3 - width/2))/width, 0, 1); s = s*s*(3 - 2*s)
    return np.stack([1 - s, s], 1)

def bones(kind, deg):
    j = np.array([0.3, 0, 0]); axis = [0, 0, 1] if kind == "bend" else [1, 0, 0]
    q = qaxis(axis, np.radians(deg)); R = qmat(q); t = j - R @ j
    return [(np.eye(3), np.zeros(3)), (R, t)], [dq_from(np.array([1.0, 0, 0, 0]), np.zeros(3)), dq_from(q, t)]

def tet_mass_moments(P, T, mass):
    """Uniform density in each deformed tet, mass fixed: centroid and second-moment tensor sum."""
    X = P[T]                                  # (M,4,3)
    c = X.mean(1)
    # second moment of a uniform tet about origin: m/20 * (sum_i x_i x_i^T + (sum x)(sum x)^T)
    S = np.einsum("mi,mj->mij", X.sum(1), X.sum(1)) + np.einsum("mki,mkj->mij", X, X)
    M2 = (mass[:, None, None]/20.0*S).sum(0)
    return (mass[:, None]*c).sum(0)/mass.sum(), M2

print("Part A: volume after skinning (n = 8: 2,304 cubes, 13,824 tets)")
V, T = bar(8); v_rest = tet_vol(V, T); W = weights(V)
print(f"{'case':<12}{'method':<6}{'V_total/V0 - 1':>16}{'min detA':>11}{'max detA':>11}{'inverted':>10}")
for kind, degs in (("bend", (45, 90, 135, 170)), ("twist", (90, 135, 180))):
    for deg in degs:
        mats, dqs = bones(kind, deg)
        for name, P in (("LBS", lbs_apply(W, mats, V)), ("DQS", dqs_apply(W, dqs, V))):
            r = tet_vol(P, T)/v_rest
            print(f"{kind+' '+str(deg):<12}{name:<6}{r.dot(v_rest)/v_rest.sum()-1:>16.4e}{r.min():>11.4f}{r.max():>11.4f}{int((r <= 0).sum()):>10}")

print("\nPart B: convergence as the cage refines, DQS bend 90 deg, rest mass 1 kg/m^3 x volume")
rows = []
for n in (1, 2, 4, 8, 16):
    V, T = bar(n); W = weights(V); m = tet_vol(V, T)          # density 1 at rest
    _, dqs = bones("bend", 90); P = dqs_apply(W, dqs, V)
    com, M2 = tet_mass_moments(P, T, m); rows.append((n, com, M2))
ref = rows[-1]
for n, com, M2 in rows[:-1]:
    print(f"n={n:<3} |com - com_16| = {np.linalg.norm(com-ref[1]):.3e} m   |M2 - M2_16|/|M2_16| = {np.linalg.norm(M2-ref[2])/np.linalg.norm(ref[2]):.3e}")

print("\nPart C: rigid limit, weights exactly (1, 0) on bone 1, f64, 100,000 random points and 1,000 random poses")
rng = np.random.default_rng(1)
mism_dqs = mism_lbs = mism_dqs_rot = 0; maxdiff = 0.0; total = 0
for _ in range(1000):
    q = rng.normal(size=4); q /= np.linalg.norm(q); t = rng.normal(size=3)
    R = qmat(q); p = rng.uniform(-1, 1, size=(100, 3))
    direct = p @ R.T + t                                        # the per-bone path: R p + t
    w = np.tile([0.0, 1.0], (100, 1))
    dq_lbs = lbs_apply(w, [(np.eye(3), np.zeros(3)), (R, t)], p)
    dq_dqs = dqs_apply(w, [dq_from(np.array([1.0, 0, 0, 0]), np.zeros(3)), dq_from(q, t)], p)
    mism_lbs += int((dq_lbs != direct).any(1).sum()); mism_dqs += int((dq_dqs != direct).any(1).sum())
    maxdiff = max(maxdiff, float(np.abs(dq_dqs - direct).max())); total += 100
print(f"LBS (sum of w_i (R_i p + t_i)) differs from R p + t in {mism_lbs} of {total} points")
print(f"DQS (Algorithm 1)              differs from R p + t in {mism_dqs} of {total} points; max |diff| = {maxdiff:.2e} m (|p| <= 1.7 m)")

print("\nPart D: inverted tets against blend-region width (n = 8), bar 0.1 m thick")
for width in (0.05, 0.1, 0.2, 0.3):
    V, T = bar(8); v_rest = tet_vol(V, T); W = weights(V, width)
    out = []
    for deg in (90, 135):
        mats, dqs = bones("bend", deg)
        for name, P in (("LBS", lbs_apply(W, mats, V)), ("DQS", dqs_apply(W, dqs, V))):
            r = tet_vol(P, T)/v_rest; out.append(f"{name} {deg}: {int((r <= 0).sum()):4d} inv, min detA {r.min():+.3f}")
    print(f"width {width:.2f} m: " + " | ".join(out))

print("\nPart E: how far apart LBS and DQS put the mass (n = 8, blend width 0.1 m, rest density 1)")
V, T = bar(8); W = weights(V); m = tet_vol(V, T)
for kind, deg in (("bend", 90), ("bend", 135), ("twist", 180)):
    mats, dqs = bones(kind, deg)
    cl, Ml = tet_mass_moments(lbs_apply(W, mats, V), T, m); cd, Md = tet_mass_moments(dqs_apply(W, dqs, V), T, m)
    print(f"{kind} {deg}: |com_LBS - com_DQS| = {np.linalg.norm(cl-cd):.3e} m, |M2_LBS - M2_DQS|/|M2| = {np.linalg.norm(Ml-Md)/np.linalg.norm(Md):.3e}")
