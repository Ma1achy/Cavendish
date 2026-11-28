"""Size the effects the paper's first-order, instantaneous-kick model leaves out (referee S3).

Point-mass stand-ins for the paper's bodies, Cavendish's AION-10 arms (87Sr, n = 1000, T = 0.73 s,
u0 = 3.86 m/s, dr = 5 m, no floor). Phase convention of the paper's eq. 5: dphi_k = (m/hbar) int
[V(upper) - V(lower)] dt, DeltaPhi = dphi_lower - dphi_upper.

1. first order (eq. 5), as a check against the paper's signals;
2. second order in the source: (m/2hbar)[int (a_u.r1u - a_l.r1l) dt + (r1u'.r1l - r1l'.r1u)(2T)],
   r1'' = a(r0(t)), derived in the response to the referee (sign: the paper's convention flips it);
3. Earth's rotation: the arms deflected sideways by Coriolis, dr'' = -2 Omega x v0, the read on the
   deflected arms minus the read on the model's (the 'arms are elsewhere' term);
4. Earth's gradient: dz'' = Gamma_E z0 (Gamma_E = 2g/R), the same difference.
"""
import numpy as np

G = 6.674e-11
U = 1.66053906660e-27
M_SR = 86.9088775 * U
HBAR = 1.054571817e-34
MH = M_SR / HBAR
LAM = 698e-9
N = 1000
VK = N * HBAR * 2 * np.pi / LAM / M_SR
T = 0.73
U0 = 3.86
DR = 5.0
GE = 9.81
GAMMA_E = 2 * GE / 6.371e6
OMEGA = 7.2921e-5
LAT = np.radians(51.76)  # Oxford
OM = OMEGA * np.array([0.0, np.cos(LAT), np.sin(LAT)])  # ENU: x east, y north, z up

NT = 400001
t = np.linspace(0, 2 * T, NT)
dt = t[1] - t[0]


def cumint(y):
    out = np.zeros_like(y)
    out[1:] = np.cumsum(0.5 * (y[1:] + y[:-1]) * dt, axis=0)
    return out


def arms(z0):
    """Unperturbed (upper, lower) arm positions (NT, 3) and velocities for a launch at height z0."""
    vu = np.where(t < T, U0 + VK - GE * t, U0 - GE * t)
    vl = np.where(t < T, U0 - GE * t, U0 + VK - GE * t)
    out = []
    for v in (vu, vl):
        z = z0 + cumint(v)
        r = np.zeros((NT, 3)); r[:, 2] = z
        vel = np.zeros((NT, 3)); vel[:, 2] = v
        out.append((r, vel))
    return out


def field(r, src, m):
    d = r - src
    dist = np.linalg.norm(d, axis=1)
    return -G * m / dist, -G * m * d / dist[:, None] ** 3  # V, a = -grad V


def trap(y):
    return float(np.sum(0.5 * (y[1:] + y[:-1])) * dt)


def ifo(z0, src, m):
    (ru, vu), (rl, vl) = arms(z0)
    Vu, au = field(ru, src, m)
    Vl, al = field(rl, src, m)
    first = MH * trap(Vu - Vl)
    # trajectories perturbed by the source
    r1u, r1l = cumint(cumint(au)), cumint(cumint(al))
    v1u, v1l = cumint(au), cumint(al)
    second = -0.5 * MH * (trap(np.sum(au * r1u, 1) - np.sum(al * r1l, 1))
                          + v1u[-1] @ r1l[-1] - v1l[-1] @ r1u[-1])
    # Coriolis deflection of each arm
    dru = cumint(cumint(-2 * np.cross(OM, vu)))
    drl = cumint(cumint(-2 * np.cross(OM, vl)))
    rot = MH * trap(-np.sum(au * dru, 1) + np.sum(al * drl, 1))  # grad V = -a
    rot_sep = float(np.linalg.norm(dru[-1] - drl[-1]))
    rot_max = float(max(np.abs(dru).max(), np.abs(drl).max()))
    # Earth's gradient
    dzu = np.zeros((NT, 3)); dzl = np.zeros((NT, 3))
    dzu[:, 2] = cumint(cumint(GAMMA_E * (ru[:, 2] - 0)))
    dzl[:, 2] = cumint(cumint(GAMMA_E * (rl[:, 2] - 0)))
    grad = MH * trap(-np.sum(au * dzu, 1) + np.sum(al * dzl, 1))
    return first, second, rot, grad, rot_sep, rot_max, float(np.abs(dzu[-1, 2] - dzl[-1, 2]))


def gradiometer(src, m):
    lo = ifo(0.0, src, m)
    hi = ifo(DR, src, m)
    return [a - b for a, b in zip(lo[:4], hi[:4])] + list(lo[4:])


if __name__ == "__main__":
    print(f"v_kick {VK:.4f} m/s, m/hbar {MH:.4e} s/m^2, k_eff T^2 {N*2*np.pi/LAM*T*T:.3e}")
    scenes = {
        "budget cube 2.7 t, centre 2.0 m off, 1 m below, east": (np.array([2.0, 0, -1.0]), 2700.0),
        "budget cube 2.7 t, centre 2.0 m off, 1 m below, north": (np.array([0, 2.0, -1.0]), 2700.0),
        "benchmark cube 1.38 t, 1.5 m off, level, east": (np.array([1.5, 0, 0.0]), 0.512 * 2700),
        "benchmark cube 1.38 t, 1.5 m off, level, north": (np.array([0, 1.5, 0.0]), 0.512 * 2700),
        "bunny 540 kg, centre ~1.9 m off, 1 m below, east": (np.array([1.9, 0, -1.0]), 540.0),
    }
    for name, (src, m) in scenes.items():
        f, s, r, g, sep, mx, gsep = gradiometer(src, m)
        print(f"{name}: first {f:+.4g} rad; second order {s:+.2e}; rotation {r:+.2e}; "
              f"Earth gradient {g:+.2e}; Coriolis deflection max {mx*1e3:.3f} mm, arms' end gap "
              f"{sep*1e3:.3f} mm; gradient end gap {gsep*1e6:.2f} um")
