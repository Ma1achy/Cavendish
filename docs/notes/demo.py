"""First signal: a tungsten mass passing a 10 m vertical gradiometer baseline."""
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from forward import AtomGradiometer, uniform_sphere, Trajectory, G

rng = np.random.default_rng(7)

inst = AtomGradiometer(baseline=10.0, k_eff=1.6e7, lmt_factor=20,
                       T=1.0, cycle_time=2.0, n_atoms=1e6, contrast=0.5)

# 1000 kg tungsten sphere (~0.23 m radius), passing horizontally in x,
# offset b in y, level with baseline midpoint (z=0).
M = 1000.0
pts, masses = uniform_sphere(M, radius=0.23, n_shell=6)

t = np.arange(-40, 40, inst.cycle_time)  # seconds, t=0 = closest approach

# --- single clean pass, with and without noise ---
b = 1.5
traj = Trajectory(r0=np.array([0.0, b, 0.0]),
                  v=np.array([0.2, 0.0, 0.0]))   # 0.2 m/s along x
clean = inst.differential_phase(pts, masses, traj, t, noiseless=True)
noisy = inst.differential_phase(pts, masses, traj, t, rng=rng)

# --- family of passes at different closest-approach distances ---
fig, ax = plt.subplots(1, 2, figsize=(12, 4.6))

ax[0].plot(t, clean * 1e3, color="k", lw=2, label="noiseless")
ax[0].scatter(t, noisy * 1e3, s=14, color="#c0392b", alpha=0.8,
              label=f"+ shot noise (σ={inst.shot_noise_rad*1e3:.2f} mrad)")
ax[0].set_xlabel("time (s)")
ax[0].set_ylabel("differential phase ΔΦ (mrad)")
ax[0].set_title("First signal: 1000 kg passing at b = 1.5 m, v = 0.2 m/s")
ax[0].legend(frameon=False, fontsize=9)
ax[0].grid(alpha=0.25)

for b in [0.8, 1.5, 2.5, 4.0]:
    tr = Trajectory(r0=np.array([0.0, b, 0.0]), v=np.array([0.2, 0.0, 0.0]))
    sig = inst.differential_phase(pts, masses, tr, t, noiseless=True)
    ax[1].plot(t, sig * 1e3, lw=2, label=f"b = {b:.1f} m")
ax[1].set_xlabel("time (s)")
ax[1].set_ylabel("ΔΦ (mrad)")
ax[1].set_title("Distinguishability: closest-approach sweep")
ax[1].legend(frameon=False, fontsize=9)
ax[1].grid(alpha=0.25)

plt.tight_layout()
plt.savefig("/home/claude/gradiometer/first_signal.png", dpi=130)

# --- quick sanity numbers ---
az_peak = np.abs(clean).max()
print(f"shot noise / shot      : {inst.shot_noise_rad*1e3:.3f} mrad")
print(f"peak |dPhi| (b=1.5)    : {az_peak*1e3:.3f} mrad")
print(f"peak SNR per shot      : {az_peak/inst.shot_noise_rad:.1f}")
# equivalent gravity gradient in Eotvos at closest approach
Tzz = 2*G*M*(inst.baseline/2)/((1.5**2+(inst.baseline/2)**2)**1.5) / inst.baseline
print(f"order-of-mag T_zz      : {Tzz/1e-9:.2f} E")
