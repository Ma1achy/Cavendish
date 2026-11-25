"""B2: torque-free asymmetric body, I=(1,2,3), spin about near-intermediate axis (the hard case).
Compare per-step integrators against a reference (RK4, h=1/65536... ) and monitor |L| and E drift."""
import numpy as np
I=np.array([1.0,2.0,3.0])
def rotz(a): c,s=np.cos(a),np.sin(a); return np.array([[c,-s,0],[s,c,0],[0,0,1]])
def rot(axis,a):
    c,s=np.cos(a),np.sin(a)
    if axis==0: return np.array([[1,0,0],[0,c,-s],[0,s,c]])
    if axis==1: return np.array([[c,0,s],[0,1,0],[-s,0,c]])
    return np.array([[c,-s,0],[s,c,0],[0,0,1]])
def skew(w): return np.array([[0,-w[2],w[1]],[w[2],0,-w[0]],[-w[1],w[0],0]])
def expm_skew(w):  # Rodrigues
    th=np.linalg.norm(w)
    if th<1e-300: return np.eye(3)
    K=skew(w/th); return np.eye(3)+np.sin(th)*K+(1-np.cos(th))*K@K
# state: R (body->world), body angular momentum Lb. omega_b = Lb/I
def energy(Lb): return 0.5*np.sum(Lb*Lb/I)
def euler_game(R,Lb,h):   # explicit Euler on Euler's equations then rotate by omega h (typical game)
    w=Lb/I; Lb2=Lb+h*np.cross(Lb,w)   # dL/dt = L x w  (body frame)
    return R@expm_skew(w*h),Lb2
def implicit_gyro(R,w_,h): pass
def splitting(R,Lb,h):    # Dullweber-Leimkuhler-McLachlan symmetric splitting, momentum in body frame
    # free rotation about body axis k: Lb rotates exactly about axis k at rate L_k/I_k ; R <- R rot_k
    def step(R,Lb,k,dt):
        a=Lb[k]/I[k]*dt
        # rotate Lb about axis k by -a (body-frame momentum components of the others rotate)
        Rk=rot(k,a); return R@Rk, Rk.T@Lb
    for k,f in ((0,.5),(1,.5),(2,1.0),(1,.5),(0,.5)):
        R,Lb=step(R,Lb,k,h*f)
    return R,Lb
def rk4_ref(R,Lb,h,n):
    def f(Lb): return np.cross(Lb,Lb/I)
    for _ in range(n):
        k1=f(Lb);k2=f(Lb+.5*h*k1);k3=f(Lb+.5*h*k2);k4=f(Lb+h*k3)
        Lb2=Lb+h/6*(k1+2*k2+2*k3+k4)
        # R update with midpoint-ish: use exponential of mean omega (4th-order enough at tiny h)
        wm=((Lb/I)+(Lb2/I))*0.5
        R=R@expm_skew(wm*h); Lb=Lb2
    return R,Lb
R0=np.eye(3); Lb0=np.array([0.05,2.0,0.04]); T=20.0
ref_h=1/8192
Rr,Lr=rk4_ref(R0,Lb0,ref_h,int(T/ref_h))
print("reference |L|,E:",np.linalg.norm(Lr),energy(Lr),"initial",np.linalg.norm(Lb0),energy(Lb0))
for name,fn in (("explicit_euler_game",euler_game),("dullweber_splitting",splitting)):
  for h in (1/60,1/240,1/960):
    R,Lb=R0.copy(),Lb0.copy(); n=int(round(T/h)); Emax=0;Lmax=0
    for i in range(n):
        R,Lb=fn(R,Lb,h)
        if name=="explicit_euler_game":  # game code renormalises nothing; leave raw
            pass
        Emax=max(Emax,abs(energy(Lb)-energy(Lb0))/energy(Lb0))
    # momentum in world frame: R Lb should be constant
    Lw=R@Lb; Lw0=R0@Lb0
    # attitude error vs reference
    M=R.T@Rr; sv=np.array([M[2,1]-M[1,2],M[0,2]-M[2,0],M[1,0]-M[0,1]])/2; ang=np.arctan2(np.linalg.norm(sv),(np.trace(M)-1)/2)
    print(f"{name:<20} h=1/{round(1/h):<4} max|dE/E|={Emax:.2e}  |Lw-Lw0|(rel)={np.linalg.norm(Lw-Lw0)/np.linalg.norm(Lw0):.2e}  attitude err vs ref at 20 s={ang:.2e} rad  orth={np.linalg.norm(R.T@R-np.eye(3)):.1e}")
# Yoshida 4th-order triple jump of the Strang splitting
w1=1/(2-2**(1/3)); w0=-2**(1/3)*w1
def yoshida(R,Lb,h):
    for w in (w1,w0,w1): R,Lb=splitting(R,Lb,w*h)
    return R,Lb
for h in (1/60,1/240):
    R,Lb=R0.copy(),Lb0.copy(); n=int(round(T/h)); Emax=0
    for i in range(n):
        R,Lb=yoshida(R,Lb,h); Emax=max(Emax,abs(energy(Lb)-energy(Lb0))/energy(Lb0))
    M=R.T@Rr; sv=np.array([M[2,1]-M[1,2],M[0,2]-M[2,0],M[1,0]-M[0,1]])/2; ang=np.arctan2(np.linalg.norm(sv),(np.trace(M)-1)/2)
    print(f"yoshida4(splitting)  h=1/{round(1/h):<4} max|dE/E|={Emax:.2e}  attitude err vs ref at 20 s={ang:.2e} rad  (3x the sub-steps per step)")
