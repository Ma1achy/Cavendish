import numpy as np
from math import *
G=6.67430e-11; hbar=6.62607015e-34/2/pi; m=86.9088775*1.66053906892e-27; lam=698e-9
k=2*pi/lam; n=1000; vk=n*hbar*k/m
print("k",k,"vkick",vk)
# 1 WGS84 from defining parameters (a,1/f,GM,omega as in NGA.STND.0036; from memory, checked by derivation)
a=6378137.0; f=1/298.257223563; GM=3.986004418e14; w=7.292115e-5
b=a*(1-f); e2=f*(2-f); ep=sqrt(e2/(1-e2)); E=sqrt(a*a-b*b)
mm=w*w*a*a*b/GM
q0=0.5*((1+3/ep**2)*atan(ep)-3/ep); q0p=3*(1+1/ep**2)*(1-atan(ep)/ep)-1
ge=GM/(a*b)*(1-mm-mm*ep*q0p/(6*q0)); gp=GM/(a*a)*(1+mm*ep*q0p/(3*q0))
kk=(b*gp)/(a*ge)-1
J2=e2*(1-2*mm*ep/(15*q0))/3
print("derived ge %.10f gp %.10f k %.14f e2 %.14f m %.9f J2 %.9e"%(ge,gp,kk,e2,mm,J2))
def som(phi,ge=9.7803253359,k_=0.00193185265241,e2_=0.00669437999014):
    s=sin(radians(phi))**2; return ge*(1+k_*s)/sqrt(1-e2_*s)
def som_old(phi): return som(phi,9.7803267714,0.00193185138639)
def fa(phi,h):
    s=sin(radians(phi))**2; g0=som(phi)
    return g0*(1-2/a*(1+f+mm-2*f*s)*h+3*h*h/a**2)
print("lat  h   gamma0   gamma(h)  [old-const gamma0]")
for name,phi in [("eq",0),("45",45),("Boulby 54.55",54.55),("Hannover 52.4",52.4),("Oxford 51.75",51.75),("LSBB 43.9",43.9),("CERN 46.2",46.2),("pole",90)]:
    print("%-14s %.10f %.10f %.10f diff_h1000_mGal %.4f; old %.10f"%(name,som(phi),fa(phi,0),fa(phi,1000),(fa(phi,1000)-fa(phi,0))*1e5,som_old(phi)))
# 2 Hessian of W in local frame (NWU/up) at lat phi, height h: W=GM/r(1-J2 (a/r)^2 P2(sin phic)) + w^2/2 (x^2+y^2)
def W(p):
    x,y,z=p; r=np.sqrt(x*x+y*y+z*z); s=z/r
    return GM/r*(1-J2*(a/r)**2*(3*s*s-1)/2)+0.5*w*w*(x*x+y*y)
def geodetic_to_xyz(phi,h):
    p=radians(phi); N=a/sqrt(1-e2*sin(p)**2)
    return np.array([(N+h)*cos(p),0,(N*(1-e2)+h)*sin(p)])
def tensor(phi,h,d=200.0):
    p=radians(phi); up=np.array([cos(p),0,sin(p)]); north=np.array([-sin(p),0,cos(p)]); west=np.array([0,-1.0,0])
    # (use ellipsoidal normal = geodetic up)
    B=np.array([north,west,up]); x0=geodetic_to_xyz(phi,h)
    T=np.zeros((3,3))
    for i in range(3):
        for j in range(3):
            def Wd(si,sj): return W(x0+si*d*B[i]+sj*d*B[j])
            T[i,j]=(Wd(1,1)-Wd(1,-1)-Wd(-1,1)+Wd(-1,-1))/(4*d*d)
    return T
print("tensor (E=1e-9 s^-2): north,west,up at (phi,h)")
for phi,h in [(0,0),(45,0),(51.75,0),(90,0),(45,9000)]:
    T=tensor(phi,h)
    ser=-som(phi)*0  # placeholder
    dg=(fa(phi,h+1)-fa(phi,h-1))/2
    print(phi,h,"diag E:",np.round(np.diag(T)*1e9,1),"trace(Laplace=2w^2 in E) %.2f"%(np.trace(T)*1e9),"offdiag NU E %.1f"%(T[0,2]*1e9),"| series dgamma/dh E %.1f"%(dg*1e9))
print("2w^2 = %.2f E"%(2*w*w*1e9))
# 3 phases
for nm,dr,T in [("AION-10",5,0.73),("AICE",70,2.5)]:
    for lab,keff in [("n k",n*k),("2 n k",2*n*k)]:
        for gam in (3.0860e-6,):
            print(nm,lab,"k_eff*Gamma*dr*T^2 = %.3e rad ; k_eff*g*T^2 = %.3e rad"%(keff*gam*dr*T*T,keff*9.81*T*T))
G0=3086e-9
for nm,dr,T in [("AION-10",5,0.73),("AICE",70,2.5)]:
    keff=n*k
    print(nm,"per 1 E: %.2f rad ; per 1 mGal dg: %.3e rad ; per 1 uGal %.3f rad; du0 per 0.052 m/s2: %.4f m/s; closure miss 2T^2 dg = %.3f m"%(keff*1e-9*dr*T*T,keff*1e-5*T*T,keff*1e-8*T*T,0.052*T,2*T*T*0.052))
# 4 variation across Earth: gradient equator vs pole (ellipsoid, h=0), and with height 9km
for phi in (0,45,90):
    print("Tuu at phi",phi,"h0: %.1f E, h9km: %.1f E"%(tensor(phi,0)[2,2]*1e9,tensor(phi,9000)[2,2]*1e9))
# 5 bodies
bodies={"Earth(eq,WGS84)":(GM,a,w,J2),"Moon":(4902.80011526323e9,1738.0e3,2*pi/(27.321661*86400),2.03e-4),"Mars":(4.282837285418775e13,3396.0e3,2*pi/(24.622962*3600),1.96e-3)}
print("body g=GM/R^2, 2GM/R^3 (E), w, w^2 (E)")
for nme,(gm,R,om,j) in bodies.items():
    print(nme,"g %.4f  2g/R %.1f E  w %.4e  w^2 %.1f E"%(gm/R**2,2*gm/R**3*1e9,om,om*om*1e9))
# 6 engine port
def arm(first,kick,half,g,floor=True):
    fall=lambda v,t:v*t-0.5*g*t*t
    lf=None
    if floor:
        back=2*first/g if first>0 else 0.0
        lf=back if back<half else None
    if lf is not None: rh,bk=0.0,0.0
    else: rh,bk=fall(first,half),first-g*half
    sec=bk+kick; ls=None
    if floor:
        back=0.0 if (rh<=0 and sec<=0) else (sec+sqrt(sec*sec+2*g*rh))/g
        ls=back if back<half else None
    def rise(tau):
        if tau<=half:
            return 0.0 if (lf is not None and tau>=lf) else fall(first,tau)
        s=tau-half
        if ls is not None and s>=ls: return 0.0
        if ls is None and lf is not None and s>=half: return 0.0
        return rh+fall(sec,s)
    return rise
def phase(g,T,u0,v,dr=5.0,mass=1.0,src=(5,0,1.0),floor=True,nn=40):
    lo=arm(u0,v,T,g,floor); up=arm(u0+v,-v,T,g,floor)
    xs,ws=np.polynomial.legendre.leggauss(6); tot=np.zeros(2)
    cuts=np.linspace(0,2*T,2*nn+1)
    for a_,b_ in zip(cuts[:-1],cuts[1:]):
        for x,wt in zip(xs,ws):
            t=0.5*(a_+b_)+0.5*(b_-a_)*x; wt*=0.5*(b_-a_)
            for i,h in enumerate((0.0,dr)):
                A=np.sqrt(src[0]**2+(h+up(t)-src[2])**2); B=np.sqrt(src[0]**2+(h+lo(t)-src[2])**2)
                tot[i]+=wt*(-mass)*(1/A-1/B)
    return G*m/hbar*(tot[0]-tot[1])
g=9.81;T=0.73
print("Earth AION-10 u0=3.86 : %.6e rad (1 kg at 5 m, z=1)"%phase(g,T,3.86,vk))
print("  symmetric u0=%.4f: %.6e"%(g*T-vk/2,phase(g,T,g*T-vk/2,vk)))
print("  g=9.78 same u0: %.6e ; g=9.83 same u0: %.6e ; g=9.78, u0=gT-v/2: %.6e"%(phase(9.78,T,3.86,vk),phase(9.83,T,3.86,vk),phase(9.78,T,9.78*T-vk/2,vk)))
print("AION-10 apex of lower arm u0/g = %.3f s; flight of lower arm 2u0/g=%.3f s; upper arm max rise %.3f m"%(3.86/9.81,2*3.86/9.81,(3.86+vk)*T-0.5*9.81*T*T))
for nme,gg in [("Moon",1.6230),("Mars",3.7130)]:
    # T with u0=0 (symmetric floor): v=2gT, rise = 1.5 g T^2 = 5 m, n scaled
    Tm=sqrt(5/(1.5*gg)); vm=2*gg*Tm; nm=n*vm/vk
    print(nme,"n=1000 symmetric needs T>=%.3f s (v/2g), rise there %.2f m; with 5 m rise, u0=0: T=%.3f s, v=%.3f m/s (n=%.0f)"%(vk/2/gg,0.5*gg*(vk/2/gg)**2+vk/2*(vk/2/gg),Tm,vm,nm))
    print("   phase 1kg: n=1000,T=%.3f(u0 clamped 0): %.3e ; u0=0,T=%.3f,v=%.2f: %.3e ; 10 m rise T=%.3f u0=gT-v/2 n=1000: %.3e"%(1.178 if nme=="Moon" else 0.9,phase(gg,1.178 if nme=="Moon" else 0.9,0.0,vk),Tm,vm,phase(gg,Tm,0.0,vm),vk/2/gg,phase(gg,vk/2/gg,0.0,vk,dr=10.0)))
    Tf=0.73
    print("   same T=0.73 u0=3.86 n=1000 (as Earth config) %.3e (arms in air %.2f s?)"%(phase(gg,Tf,3.86,vk),2*3.86/gg))
    for Tt in (Tf,Tm):
        print("   Earth-gradient phase, dr=5: Tm=%.3f -> %.3e rad (k_eff=nk, Gamma=2g/R)"%(Tt,n*k*2*gg/bodies[nme][1]*5*Tt*Tt))
# 7 formulas
print("Bouguer 2piG rho per m: rho=2670 %.4f mGal/m ; per g/cc %.4f"%(2*pi*G*2670*1e5,2*pi*G*1000*1e5))
Rr,D,d_=5.0,20.0,-2670.0; M=4/3*pi*Rr**3*d_
print("cavity R5 D20 centre: g %.3f mGal, Tzz(on axis) %.1f E"%(G*abs(M)/D**2*1e5,2*G*abs(M)/D**3*1e9))
lam_=-2670*pi*1.0**2 # tunnel r=1 m
print("tunnel r=1m D=10 m: g %.4f mGal; Tzz axis |2G lam/D^2| %.2f E"%(2*G*abs(lam_)/10*1e5,2*G*abs(lam_)/100*1e9))
print("lunar tide grad 2GM/d^3 %.2e s^-2 =%.2e E ; phase k dr T^2: %.2e"%(2*4.9028e12/3.844e8**3,2*4.9028e12/3.844e8**3*1e9,n*k*2*4.9028e12/3.844e8**3*5*T*T))
print("chirp alpha/2pi = keff g/2pi = %.3e Hz/s"%(n*k*9.81/2/pi))

# ---- 8 (added after reading primaries) checks against numbers printed in sources
print("\n== 8 checks")
# 8a Dickerson et al arXiv:1305.1700 Table I: keff=2k, 780 nm Rb87, T=1.15 s, Tzz=3075 E
kD=2*2*pi/780e-9; mRb=86.909180527*1.66053906892e-27
print("Dickerson term1 keff g T^2 = %.3e (table 2.1e8); term4 hbar keff^2 Tzz T^3/2m = %.3f (table 0.44)"%(kD*9.8*1.15**2, hbar*kD**2*3075e-9*1.15**3/(2*mRb)))
# 8b free-air second-order term: |grad W| at (phi,h) minus at h=0, vs series (differences only, J2 model)
def gmag(phi,h,d=1.0):
    x0=geodetic_to_xyz(phi,h); g=np.zeros(3)
    for i in range(3):
        e=np.zeros(3); e[i]=d; g[i]=(W(x0+e)-W(x0-e))/(2*d)
    return np.linalg.norm(g)
for phi in (0,45,90):
    for h in (1000.,9000.,100000.):
        num=gmag(phi,h)-gmag(phi,0.0); s=sin(radians(phi))**2
        ser=som(phi)*(-2/a*(1+f+mm-2*f*s)*h+3*h*h/a**2)
        lin=som(phi)*(-2/a*(1+f+mm-2*f*s)*h)
        print("phi %2d h %6.0f  |gradW| diff %.6f  series %.6f (diff %.2e m/s2 = %.3f mGal) linear-only %.6f"%(phi,h,num,ser,num-ser,(num-ser)*1e5,lin))
# 8c Bouguer slab, sphere, line from direct numerical integration of Newton's law
rho=2670.0
# slab thickness 1 m, radius R disc -> g at point on axis (disc formula): 2 pi G rho t (1 - z/sqrt(z^2+R^2))
t=1.0; R=1e6
print("slab 1 m, disc R=1000 km, on surface: %.5f mGal vs 2piG rho t = %.5f"%(2*pi*G*rho*t*(1-1e-3/np.hypot(1e-3,R))*1e5,2*pi*G*rho*t*1e5))
# sphere via midpoint voxel sum
Rr,D=5.0,20.0; n_=60; ax=(np.arange(n_)+0.5)/n_*2*Rr-Rr
X,Y,Z=np.meshgrid(ax,ax,ax,indexing='ij'); mask=X**2+Y**2+Z**2<=Rr**2; dv=(2*Rr/n_)**3
rx=-X[mask]; ry=-Y[mask]; rz=D-Z[mask]  # field point at z=D above sphere centre? point at (0,0,D): vector from voxel to point
r2=X[mask]**2+Y[mask]**2+(D-Z[mask])**2; r5=r2**2.5
gz=G*rho*dv*np.sum((D-Z[mask])/r2**1.5); Tzz_=G*rho*dv*np.sum((3*(D-Z[mask])**2-r2)/r5)
Ms=4/3*pi*Rr**3*rho
print("sphere voxel sum g %.5e vs GM/D^2 %.5e ; dg/dz-type Tzz %.4e vs 2GM/D^3 %.4e (rel err %.1e, %.1e)"%(gz,G*Ms/D**2,Tzz_,2*G*Ms/D**3,gz/(G*Ms/D**2)-1,Tzz_/(2*G*Ms/D**3)-1))
# horizontal cylinder: axis at depth D, line integral over length L large; field point directly above: g = 2 G lam / D ; Tzz = 2G lam /D^2 (magnitude)
lam=rho*pi*1.0**2; Dd=10.0; L=2e4; xs=np.linspace(-L,L,4000001); r2=xs**2+Dd**2
gz_c=G*lam*np.trapezoid(Dd/r2**1.5*0+Dd/ r2,xs) if False else None
# 3-D line: field of line element = G lam dx D/(x^2+D^2)^(3/2) perpendicular
gz_c=G*lam*np.trapezoid(Dd/(xs**2+Dd**2)**1.5,xs); Tz_c=G*lam*np.trapezoid((3*Dd**2-(xs**2+Dd**2))/(xs**2+Dd**2)**2.5,xs)
print("line/cylinder lam=%.1f kg/m D=10: g %.5e vs 2G lam/D %.5e ; Tzz %.4e vs 2G lam/D^2 %.4e"%(lam,gz_c,2*G*lam/Dd,Tz_c,2*G*lam/Dd**2))
# 8d Moon/Mars with NSSDCA values
for nme,gm,R,P,j in [("Moon NSSDCA",4900e9,1738.1e3,655.720*3600,202.7e-6),("Mars NSSDCA",42828e9,3396.2e3,24.6229*3600,1960.45e-6)]:
    om=2*pi/P; print(nme,"g=GM/R^2 %.4f 2GM/R^3 %.1f E omega %.4e rad/s omega^2 %.2f E ; J2 %.3e"%(gm/R**2,2*gm/R**3*1e9,om,om*om*1e9,j))
print("Mars pressure 636 Pa / Earth 101325 = %.4f"%(636/101325))
