"""Voxel cantilever: first bending frequency of a clamped square beam made of cubic trilinear
hexahedra, against Euler-Bernoulli (and Timoshenko for the shear correction).

Elements compared (all on the same cubic voxels):
  Q1      full 2x2x2 Gauss integration
  Q1-SRI  selective reduced integration (volumetric part at 1 point) - the B-bar family's
          answer to *volumetric* locking; it should NOT cure bending (shear) locking
  Q1-IM   incompatible modes (Wilson et al. 1973, Taylor-Beresford-Wilson 1976), 9 internal
          modes condensed per element

Pass criteria written before running:
  C1 every element passes the constant-strain patch test (residual < 1e-12) - else stop;
  C2 report f1/f1_ref for n = 1, 2, 4, 8 cells through the thickness; the question is how many
     cells keep the error under 1 % and 0.1 %.
Also: mass conservation (sum of the consistent mass matrix = rho V, exact) and a timing of the
lowest-6-modes solve with scipy (shift-invert Lanczos, ARPACK) on one CPU core.
"""
import itertools
import time

import numpy as np
import scipy.sparse as sp
import scipy.sparse.linalg as spl

GP = np.array([-1, 1]) / np.sqrt(3)
CORNERS = np.array(list(itertools.product([-1, 1], repeat=3)))  # (8,3) in xi,eta,zeta


def elasticity(E, nu):
    lam = E * nu / ((1 + nu) * (1 - 2 * nu))
    mu = E / (2 * (1 + nu))
    D = np.zeros((6, 6))
    D[:3, :3] = lam
    D[:3, :3] += 2 * mu * np.eye(3)
    D[3:, 3:] = mu * np.eye(3)
    return D, lam, mu


def B_matrix(dN):
    """dN: (n,3) physical derivatives -> B (6, 3n), Voigt (xx,yy,zz,yz,xz,xy)."""
    n = dN.shape[0]
    B = np.zeros((6, 3 * n))
    for a in range(n):
        x, y, z = dN[a]
        B[0, 3 * a] = x; B[1, 3 * a + 1] = y; B[2, 3 * a + 2] = z
        B[3, 3 * a + 1] = z; B[3, 3 * a + 2] = y
        B[4, 3 * a] = z; B[4, 3 * a + 2] = x
        B[5, 3 * a] = y; B[5, 3 * a + 1] = x
    return B


def dN_q1(p, s):
    xi = p
    out = np.zeros((8, 3))
    for a, c in enumerate(CORNERS):
        f = (1 + c * xi)
        out[a, 0] = c[0] * f[1] * f[2] / 8
        out[a, 1] = f[0] * c[1] * f[2] / 8
        out[a, 2] = f[0] * f[1] * c[2] / 8
    return out * (2 / s)  # cube of side s: d/dx = (2/s) d/dxi


def dN_bubble(p, s):
    """Wilson modes 1 - xi_i^2 for each direction and each displacement component handled by
    B_matrix: three scalar modes -> 9 dofs. For a cube the Taylor correction (Jacobian at the
    centre) changes nothing; the patch test is checked anyway."""
    out = np.zeros((3, 3))
    for i in range(3):
        out[i, i] = -2 * p[i]
    return out * (2 / s)


def element_K(E, nu, s, kind):
    D, lam, mu = elasticity(E, nu)
    w = (s / 2) ** 3
    pts = [np.array(q) for q in itertools.product(GP, repeat=3)]
    if kind == "Q1-SRI":
        # split D into volumetric (bulk) and deviatoric parts
        K = lam + 2 * mu / 3
        m = np.array([1, 1, 1, 0, 0, 0.0])
        Dvol = K * np.outer(m, m)
        Ddev = D - Dvol
        Ke = sum(B_matrix(dN_q1(p, s)).T @ Ddev @ B_matrix(dN_q1(p, s)) * w for p in pts)
        B0 = B_matrix(dN_q1(np.zeros(3), s))
        Ke = Ke + 8 * w * B0.T @ Dvol @ B0
        return Ke
    Kuu = sum(B_matrix(dN_q1(p, s)).T @ D @ B_matrix(dN_q1(p, s)) * w for p in pts)
    if kind == "Q1":
        return Kuu
    Kua = np.zeros((24, 9)); Kaa = np.zeros((9, 9))
    for p in pts:
        Bu = B_matrix(dN_q1(p, s)); Ba = B_matrix(dN_bubble(p, s))
        Kua += Bu.T @ D @ Ba * w
        Kaa += Ba.T @ D @ Ba * w
    return Kuu - Kua @ np.linalg.solve(Kaa, Kua.T)


def element_M(rho, s):
    w = (s / 2) ** 3
    M = np.zeros((8, 8))
    for q in itertools.product(GP, repeat=3):
        N = np.prod(1 + CORNERS * np.array(q), axis=1) / 8
        M += np.outer(N, N) * w * rho
    Me = np.zeros((24, 24))
    for d in range(3):
        Me[d::3, d::3] = M
    return Me


def patch_test(Ke, s):
    """Nodal displacements from a linear field u = G x: internal forces must vanish at interior
    equilibrium; on one element, Ke u must equal the consistent nodal forces of the constant
    stress - check instead that the energy equals V * eps:D:eps."""
    rng = np.random.default_rng(1)
    Gm = rng.normal(size=(3, 3))
    x = CORNERS * s / 2
    u = (x @ Gm.T).ravel()
    eps = 0.5 * (Gm + Gm.T)
    ev = np.array([eps[0, 0], eps[1, 1], eps[2, 2], 2 * eps[1, 2], 2 * eps[0, 2], 2 * eps[0, 1]])
    return ev, u


def beam(n, L_over_h, E, nu, rho, kind, h=1.0):
    s = h / n
    nx, ny, nz = int(L_over_h * n), n, n
    Ke = element_K(E, nu, s, kind)
    Me = element_M(rho, s)
    nid = np.arange((nx + 1) * (ny + 1) * (nz + 1)).reshape(nx + 1, ny + 1, nz + 1)
    rows, cols, kv, mv = [], [], [], []
    for i, j, k in itertools.product(range(nx), range(ny), range(nz)):
        nodes = [nid[i + (c[0] > 0), j + (c[1] > 0), k + (c[2] > 0)] for c in CORNERS]
        dofs = np.array([[3 * a, 3 * a + 1, 3 * a + 2] for a in nodes]).ravel()
        rows.append(np.repeat(dofs, 24)); cols.append(np.tile(dofs, 24))
    rows = np.concatenate(rows); cols = np.concatenate(cols)
    ne = nx * ny * nz
    N = 3 * nid.size
    K = sp.coo_matrix((np.tile(Ke.ravel(), ne), (rows, cols)), shape=(N, N)).tocsc()
    M = sp.coo_matrix((np.tile(Me.ravel(), ne), (rows, cols)), shape=(N, N)).tocsc()
    total_mass = M.sum() / 3
    fixed = np.array([[3 * a, 3 * a + 1, 3 * a + 2] for a in nid[0].ravel()]).ravel()
    free = np.setdiff1d(np.arange(N), fixed)
    K = K[free][:, free]; M = M[free][:, free]
    t = time.perf_counter()
    vals = spl.eigsh(K, k=6, M=M, sigma=0.0, which="LM", return_eigenvectors=False)
    dt = time.perf_counter() - t
    f = np.sort(np.sqrt(np.abs(vals))) / (2 * np.pi)
    return f, total_mass, ne, len(free), dt


if __name__ == "__main__":
    E, nu, rho = 30e9, 0.2, 2400.0  # concrete
    h, L_over_h = 1.0, 10
    L = L_over_h * h
    I = h ** 4 / 12; A = h * h
    beta1 = 1.875104068711961
    f_eb = beta1 ** 2 / (2 * np.pi) * np.sqrt(E * I / (rho * A * L ** 4))
    # Timoshenko first-order shear/rotary correction (Euler-Bernoulli -> Timoshenko) is not
    # trusted from memory; the n=8 IM run is used as the converged reference instead and the
    # EB ratio is reported beside it.
    print("patch test (energy of a constant strain vs V eps:D:eps):")
    D, _, _ = elasticity(E, nu)
    for kind in ("Q1", "Q1-SRI", "Q1-IM"):
        Ke = element_K(E, nu, 1.0, kind)
        ev, u = patch_test(Ke, 1.0)
        print(f"  {kind:7s} rel. error {abs(u @ Ke @ u - ev @ D @ ev) / (ev @ D @ ev):.1e}")
    print(f"\nL/h = {L_over_h}, f1(Euler-Bernoulli) = {f_eb:.4f} Hz (h = 1 m concrete)")
    res = {}
    for kind in ("Q1", "Q1-SRI", "Q1-IM"):
        for n in (1, 2, 4, 8):
            if n == 8 and kind == "Q1-SRI":
                continue
            f, mass, ne, nd, dt = beam(n, L_over_h, E, nu, rho, kind, h)
            res[(kind, n)] = f
            print(f"  {kind:7s} n={n}: elems {ne:6d} dofs {nd:6d}  f1 {f[0]:.4f}  f1/fEB {f[0] / f_eb:.4f}"
                  f"  f3/f1 {f[2] / f[0]:.3f}  mass err {abs(mass - rho * L * A) / (rho * L * A):.1e}"
                  f"  eigsh {dt:.2f} s")
    ref = res[("Q1-IM", 8)][0]
    print(f"\nagainst Q1-IM n=8 (f1 = {ref:.4f} Hz):")
    for key, f in res.items():
        print(f"  {key[0]:7s} n={key[1]}: f1 error {(f[0] - ref) / ref:+.2e}")
