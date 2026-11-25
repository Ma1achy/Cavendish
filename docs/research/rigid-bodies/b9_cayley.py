"""B9: the free-rotation splitting of b2 with Dullweber, Leimkuhler & McLachlan's rational rotations (their Appendix A,
J. Chem. Phys. 107, 5848): cos phi -> (1 - phi^2/4)/(1 + phi^2/4), sin phi -> phi/(1 + phi^2/4), i.e. the Cayley
transform of each planar rotation.  Only + - * / : no sin/cos, so it is bit-identical on every IEEE target (B6's concern)
and exactly orthogonal in exact arithmetic.  Same case and same RK4 reference as b2_rotation.py.
"""
import numpy as np
exec(open("b2_rotation.py").read().split("R0=np.eye(3)")[0])   # reuse I, rot, expm_skew, energy, rk4_ref, splitting
def rot_cayley(axis, a):
    d = 1 + a*a/4; c = (1 - a*a/4)/d; s = a/d
    if axis == 0: return np.array([[1, 0, 0], [0, c, -s], [0, s, c]])
    if axis == 1: return np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]])
    return np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]])
def splitting_cayley(R, Lb, h):
    for k, f in ((0, .5), (1, .5), (2, 1.0), (1, .5), (0, .5)):
        a = Lb[k]/I[k]*h*f; Rk = rot_cayley(k, a); R, Lb = R@Rk, Rk.T@Lb
    return R, Lb
R0 = np.eye(3); Lb0 = np.array([0.05, 2.0, 0.04]); T = 20.0
ref_h = 1/8192
Rr, Lr = rk4_ref(R0, Lb0, ref_h, int(T/ref_h))
def err(R):
    M = R.T@Rr; sv = np.array([M[2,1]-M[1,2], M[0,2]-M[2,0], M[1,0]-M[0,1]])/2
    return np.arctan2(np.linalg.norm(sv), (np.trace(M)-1)/2)
print("rotation per sub-step is cayley angle 2*atan(phi/2) = phi - phi^3/12, a relative frequency error phi^2/12")
for name, fn in (("trig splitting (b2)", splitting), ("cayley splitting", splitting_cayley)):
    for h in (1/60, 1/240, 1/960):
        R, Lb = R0.copy(), Lb0.copy(); Emax = 0
        for _ in range(int(round(T/h))):
            R, Lb = fn(R, Lb, h); Emax = max(Emax, abs(energy(Lb)-energy(Lb0))/energy(Lb0))
        Lw = R@Lb
        print(f"{name:<20} h=1/{round(1/h):<4} max|dE/E|={Emax:.2e} |dL|/|L|={np.linalg.norm(Lw-Lb0)/np.linalg.norm(Lb0) if False else np.linalg.norm(Lw-R0@Lb0)/np.linalg.norm(Lb0):.2e}"
              f" attitude err {err(R):.2e} rad  orth {np.linalg.norm(R.T@R-np.eye(3)):.1e}")
w1 = 1/(2-2**(1/3)); w0 = -2**(1/3)*w1
for name, fn in (("yoshida4(trig)", splitting), ("yoshida4(cayley)", splitting_cayley)):
    for h in (1/60, 1/240):
        R, Lb = R0.copy(), Lb0.copy(); Emax = 0
        for _ in range(int(round(T/h))):
            for w in (w1, w0, w1): R, Lb = fn(R, Lb, w*h)
            Emax = max(Emax, abs(energy(Lb)-energy(Lb0))/energy(Lb0))
        print(f"{name:<20} h=1/{round(1/h):<4} max|dE/E|={Emax:.2e} attitude err {err(R):.2e} rad")
