import numpy as np
G=6.67430e-11; hbar=1.054571817e-34; m=1.4432e-25; lam=698e-9; N=1000
k=N*2*np.pi/lam; v=N*hbar*(2*np.pi/lam)/m; g=9.81; Om=7.292115e-5
print("k_eff=%.4e /m, v_kick=%.4f m/s"%(k,v))
gl_x,gl_w=np.polynomial.legendre.leggauss(8)
def nodes(T,panels=40):
    xs=[];ws=[]
    edges=np.linspace(0,2*T,2*panels+1)
    for a,b in zip(edges[:-1],edges[1:]):
        xs+=list(a+0.5*(b-a)*(gl_x+1)); ws+=list(0.5*(b-a)*gl_w)
    return np.array(xs),np.array(ws)
def arm_s(t,T,vv,up):
    # displacement along k_hat: symmetric launch, up=True is the +v/2 arm
    sg=1 if up else -1
    return np.where(t<=T, sg*vv*t/2, sg*(vv*T/2-vv*(t-T)/2))
def phase_ifo(M,src,o,T,vv,u0,gg,bdir,kdir):
    t,w=nodes(T)
    z=u0*t-0.5*gg*t*t
    def pos(up):
        s=arm_s(t,T,vv,up)
        return o[None,:]+s[:,None]*kdir[None,:]+z[:,None]*np.array([0,0,1.0])[None,:]
    def V(p): return -G*M/np.linalg.norm(p-src[None,:],axis=1)
    return (m/hbar)*np.sum(w*(V(pos(True))-V(pos(False))))
def grad_phase(M,src,L,T,vv=v,gg=g,sagz=True):
    bdir=np.array([1.0,0,0]); kdir=bdir
    u0=gg*T if sagz else 0.0
    o1=np.zeros(3); o2=L*bdir
    p1=phase_ifo(M,src,o1,T,vv,u0,gg if sagz else 0.0,bdir,kdir)
    p2=phase_ifo(M,src,o2,T,vv,u0,gg if sagz else 0.0,bdir,kdir)
    return p1-p2   # lower(IFO1)-upper(IFO2): engine's sign
def hess(M,r):  # Hessian of V=-GM/r at displacement r from source -> V_ij = GM(d_ij - 3 n n)/r^3 ... with r = point - source
    d=np.linalg.norm(r); n=r/d
    return G*M*(np.eye(3)-3*np.outer(n,n))/d**3
def tidal_exact_arms(M,src,L,T,vv=v,gg=g):
    # integral of s(t) k.H(c(t)).L, c = baseline midpoint moving with fall; engine sign: phi1-phi2 = -(...)
    t,w=nodes(T); z=gg*T*t-0.5*gg*t*t
    s=arm_s(t,T,vv,True)-arm_s(t,T,vv,False)  # separation (>=0)
    tot=0
    for ti,wi,si,zi in zip(t,w,s,z):
        c=np.array([L/2,0,zi]); H=hess(M,c-src)
        tot+=wi*si*H[0,0]*L
    return -(m/hbar)*tot
def tidal_simple(M,src,L,T):
    # k T^2 k.H(mid at apex).L with engine sign
    c=np.array([L/2,0,g*T*T/2]); H=hess(M,c-src)
    return -k*T**2*H[0,0]*L*(v*T**2*m/hbar/(k*T**2))
print("check (m/hbar)*v*T^2 == k*T^2:",m/hbar*v, k)
T=0.73
print("\n== AION-like horizontal: T=0.73, L=10 m, k along baseline (x) ==")
for name,M,src in [("10 kg at 10 m perpendicular (y), mid-height",10,np.array([5,10,1.3])),
                   ("1 t at 10 m perpendicular",1000,np.array([5,10,1.3])),
                   ("10 kg at 10 m along baseline (x=L/2+... beyond end)",10,np.array([20,0,1.3])),
                   ("10 kg 10 m perpendicular, no sag (g=0 for atoms)",10,np.array([5,10,0.0]))]:
    ex=grad_phase(M,src,10.0,T,sagz=("no sag" not in name))
    ta=tidal_exact_arms(M,src,10.0,T) if "no sag" not in name else float('nan')
    ts=tidal_simple(M,src,10.0,T)
    print(f"{name}: exact={ex:.4e} rad, tidal(moving centre)={ta:.4e}, tidal(apex)={ts:.4e}")
print("\n== far-field convergence (1 kg, baseline L=10, T=0.73, mass at distance R along y) ==")
for R in [10,30,100,300,1000]:
    src=np.array([5,R,1.3]); ex=grad_phase(1.0,src,10.0,T); ta=tidal_exact_arms(1.0,src,10.0,T)
    print(f"R={R:5d} m exact={ex:.5e} tidal_moving={ta:.5e} ratio-1={ta/ex-1:.2e}")
print("\n== same, mass along baseline axis ==")
for R in [10,30,100,300,1000]:
    src=np.array([5+R,0,1.3]); ex=grad_phase(1.0,src,10.0,T); ta=tidal_exact_arms(1.0,src,10.0,T)
    print(f"R={R:5d} m exact={ex:.5e} tidal_moving={ta:.5e} ratio-1={ta/ex-1:.2e}")
print("\n== vertical-axis AION-like same mass for scale: Gamma_zz kT^2 dr, 10 kg at 10 m horizontal, dr=5 (point approx at mid)")
Gam=G*10/10**3; print("GM/r^3=%.3e s^-2 ; kT^2 GM/r^3 *L(10m) = %.3e rad (order, 1 irreducible factor ~1-3)"%(Gam,k*T*T*Gam*10))
print("\n== MIGA-like: Rb-87 780nm, k=4pi/lam (2-photon Bragg, no LMT), L=150, T assumed 0.1 and 0.25 s (UNVERIFIED T)")
kr=4*np.pi/780e-9
for Tm in [0.1,0.25]:
    print(f"T={Tm}: k T^2 = {kr*Tm*Tm:.3e}; 10 kg at 10 m: {kr*Tm*Tm*G*10/1e3*150:.3e} rad (upper bound order, point-tidal not valid at L>r)")
print("\n== Earth tensor ==")
R=6.371e6; GME=3.986004418e14
Gxx=GME/R**3; print("GM/R^3=%.4e s^-2 (Gamma_xx=Gamma_yy=+, Gamma_zz=-2x)"%Gxx)
for L in [5,10,150]:
    print(f"L={L}: k T^2 Gamma_xx L = {k*T*T*Gxx*L:.3e} rad (AION k,T); vertical Gzz dr=5: {k*T*T*2*Gxx*5:.3e}")
print("centrifugal Om^2 = %.3e s^-2"%(Om**2), "-> k T^2 Om^2 L(10) = %.3e rad"%(k*T*T*Om**2*10))
print("\n== Rotation (Coriolis) ==")
lat=np.deg2rad(51.75) # Oxford approx? just illustrative
for latd in [0,45,90]:
    l=np.deg2rad(latd)
    print(f"lat {latd}: Omega=(E,N,U)=(0,{Om*np.cos(l):.4e},{Om*np.sin(l):.4e})")
vz=g*T  # speed scale
print("2 k Om cos(lat) v T^2 per m/s of v_z at lat 45: %.3e rad/(m/s)"%(2*k*Om*np.cos(np.deg2rad(45))*T*T))
print("-> single IFO, v_z at pi pulse |dz/dt| ~ 0 for apex launch; with u0=3.86 etc")
dv=1e-6/(2*k*Om*np.cos(np.deg2rad(45))*T*T); print("Delta v_z between the two IFOs for 1 urad: %.3e m/s"%dv)
dOm=1e-6/(2*k*g*T*T*T); print("single-IFO rate stability for 1 urad if v_z=gT: dOmega=%.3e rad/s"%dOm)
# Sagnac area check
t,w=nodes(T,200); vz_mean=2.0
# loop area in x-z plane: arms A,B with common z(t) drift: A_vec_y = (1/2)\oint (z dx - x dz) ; evaluate numerically for linear drift z=vz t
tt=np.linspace(0,2*T,200001); z=vz_mean*tt
xB=arm_s(tt,T,v,True); xA=arm_s(tt,T,v,False)
# area between arms integrated along z path: int (xB-xA) dz
area=np.trapezoid(xB-xA, z)
print("enclosed area numeric %.6f m^2 vs v*T^2*vz = %.6f"%(area, v*T*T*vz_mean))
print("Sagnac 2 m Om A/hbar (Om_perp=1 rad/s) =%.4e vs 2 k vz T^2 = %.4e"%(2*m/hbar*area, 2*k*vz_mean*T*T))
print("\n== tilt ==")
print("k g theta T^2 per urad: %.3e rad ; theta needed for 1 urad per IFO: %.3e rad"%(k*g*1e-6*T*T, 1e-6/(k*g*T*T)))
print("tilt of 1 urad common-mode: gradiometer difference involves g difference dg=Gamma_zz*dz or none for horizontal baseline: g tilt both IFOs same if parallel -> cancels; differential tilt dtheta needed:",1e-6/(k*g*T*T))
print("\n== gravity sag over 2T if dropped from rest: %.3f m; apex rise gT^2/2: %.3f m; for beam waist w: T_max=sqrt(2w/g): w=1cm %.3f s, 5cm %.3f s, 2 cm %.3f s"%(0.5*g*(2*T)**2,0.5*g*T*T,np.sqrt(2*0.01/g),np.sqrt(2*0.05/g),np.sqrt(2*0.02/g)))
print("\n== bit-identity experiment ==")
rng=np.random.default_rng(1)
x=rng.normal(size=10**6)*10**rng.uniform(-8,8,10**6)
print("x+0.0 bit-equal:", np.array_equal(x.view(np.uint64),(x+0.0).view(np.uint64)))
print("x*1.0 bit-equal:", np.array_equal(x.view(np.uint64),(x*1.0).view(np.uint64)))
print("0.0*x == 0 (sign aside):", np.all(0.0*x==0.0))
# engine-form distance vs 3D form with zero arm x,y
ax=rng.normal(size=10**6); ay=rng.normal(size=10**6); dz=rng.normal(size=10**6)
old=(ax*ax+ay*ay)+dz*dz
zx=0.0; new=((ax-zx)*(ax-zx)+(ay-zx)*(ay-zx))+dz*dz
print("distance^2 3D form with exact-zero arm xy bit-equal:",np.array_equal(old.view(np.uint64),new.view(np.uint64)))
# rotation by 90deg degrees
th=np.pi/2; print("cos(pi/2)=%.3e sin=%.17g ; rotate (0,0,1) by 0:"%(np.cos(th),np.sin(th)))
Rm=lambda a: np.array([[1,0,0],[0,np.cos(a),-np.sin(a)],[0,np.sin(a),np.cos(a)]])
vecs=rng.normal(size=(10**5,3)); r0=(vecs@Rm(0.0).T)
print("rotation by exactly 0 bit-equal:",np.array_equal(vecs.view(np.uint64),r0.view(np.uint64)))
r90=vecs@Rm(np.pi/2).T; r90b=np.stack([vecs[:,0],-vecs[:,2],vecs[:,1]],1)
print("rotation by pi/2 via matrix equals exact axis permutation bitwise:",np.array_equal(r90.view(np.uint64),r90b.view(np.uint64)),"max abs diff",np.abs(r90-r90b).max())
