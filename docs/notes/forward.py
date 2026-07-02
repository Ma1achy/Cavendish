"""
Forward model for a differential atom gradiometer observing a moving mass.

Pipeline:
    source mass distribution (voxel point masses)
        -> Newtonian gravity gradient tensor T_ij at each atom cloud
        -> per-cloud vertical acceleration a_z
        -> Mach-Zehnder phase  phi_i = k_eff * a_z(z_i, t) * T^2   (quasi-static)
        -> differential phase  dPhi = phi_2 - phi_1   (laser noise common-mode rejected)
        -> + atom shot noise

Gravity is done analytically by point-mass / voxel summation, NOT FEM.
Free-space Newtonian gravity is a linear convolution with an exact kernel; a
voxelised sphere or a closed-form prism beats a meshed Poisson solve on both
accuracy and speed, and has no boundary/discretisation headaches.
"""

from __future__ import annotations
from dataclasses import dataclass
from typing import Callable
import numpy as np

G = 6.67430e-11  # m^3 kg^-1 s^-2


# ----------------------------------------------------------------------------
# Gravity
# ----------------------------------------------------------------------------
def gravity_gradient_tensor(field_pts, src_pts, src_mass):
    """
    Full 3x3 gravity gradient tensor T_ij = d a_i / d x_j at each field point,
    summed over a cloud of source point masses (a voxelised extended body).

    field_pts : (F,3)   atom-cloud positions
    src_pts   : (S,3)   source voxel/point-mass positions
    src_mass  : (S,)    voxel masses

    returns   : (F,3,3) tensor, and (F,3) acceleration a_i
    """
    field_pts = np.atleast_2d(field_pts)
    src_pts = np.atleast_2d(src_pts)
    src_mass = np.atleast_1d(src_mass)

    d = field_pts[:, None, :] - src_pts[None, :, :]      # (F,S,3) source->field
    r2 = np.sum(d * d, axis=-1)                          # (F,S)
    r = np.sqrt(r2)
    inv_r3 = r ** -3
    inv_r5 = r ** -5

    # a_i = -G * sum_s m_s d_i / r^3
    acc = -G * np.einsum("s,fs,fsi->fi", src_mass, inv_r3, d)

    # T_ij = -G sum_s m_s [ delta_ij / r^3 - 3 d_i d_j / r^5 ]
    eye = np.eye(3)
    term_iso = np.einsum("s,fs,ij->fij", src_mass, inv_r3, eye)
    term_quad = 3.0 * np.einsum("s,fs,fsi,fsj->fij", src_mass, inv_r5, d, d)
    T = -G * (term_iso - term_quad)
    return T, acc


# ----------------------------------------------------------------------------
# Source bodies (parametric family — this IS your prior, choose it carefully)
# ----------------------------------------------------------------------------
def uniform_sphere(mass, radius, n_shell=6):
    """Voxelised uniform sphere centred at origin. Returns (pts, masses)."""
    if radius == 0 or n_shell <= 1:
        return np.zeros((1, 3)), np.array([mass])
    lin = np.linspace(-radius, radius, n_shell)
    gx, gy, gz = np.meshgrid(lin, lin, lin, indexing="ij")
    pts = np.stack([gx.ravel(), gy.ravel(), gz.ravel()], axis=1)
    inside = np.sum(pts * pts, axis=1) <= radius ** 2
    pts = pts[inside]
    masses = np.full(len(pts), mass / len(pts))
    return pts, masses


@dataclass
class Trajectory:
    """Rigid body on a constant-acceleration path: r(t) = r0 + v t + 0.5 a t^2."""
    r0: np.ndarray
    v: np.ndarray
    a: np.ndarray = None

    def position(self, t):
        a = self.a if self.a is not None else np.zeros(3)
        return self.r0 + self.v * t + 0.5 * a * t * t


# ----------------------------------------------------------------------------
# The instrument
# ----------------------------------------------------------------------------
@dataclass
class AtomGradiometer:
    baseline: float = 10.0          # m, vertical separation of the two clouds
    k_eff: float = 1.6e7            # m^-1 effective wavevector
    lmt_factor: int = 1             # large-momentum-transfer multiplier
    T: float = 1.0                  # s, pulse separation (interrogation time)
    cycle_time: float = 2.0         # s, time between shots
    n_atoms: float = 1e6
    contrast: float = 0.5
    axis: int = 2                   # measurement axis (z)

    @property
    def cloud_positions(self):
        z = self.baseline / 2.0
        c = np.zeros((2, 3))
        c[0, self.axis] = -z
        c[1, self.axis] = +z
        return c

    @property
    def shot_noise_rad(self):
        # quantum projection noise on the differential phase
        single = 1.0 / (self.contrast * np.sqrt(self.n_atoms))
        return single * np.sqrt(2)  # two independent interferometers

    def differential_phase(self, body_pts, body_mass, traj: Callable, t,
                           rng=None, noiseless=False):
        """
        Differential MZ phase time series.
        body_pts/body_mass : source shape in its own frame (origin = body centre)
        traj               : Trajectory giving body-centre position at time t
        t                  : (N,) sample times
        """
        clouds = self.cloud_positions
        keff = self.k_eff * self.lmt_factor
        out = np.empty(len(t))
        for i, ti in enumerate(t):
            src = body_pts + traj.position(ti)
            _, acc = gravity_gradient_tensor(clouds, src, body_mass)
            az = acc[:, self.axis]
            dphi = keff * (az[1] - az[0]) * self.T ** 2
            out[i] = dphi
        if not noiseless:
            rng = rng or np.random.default_rng()
            out = out + rng.normal(0.0, self.shot_noise_rad, size=out.shape)
        return out
