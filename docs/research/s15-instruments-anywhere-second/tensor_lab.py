import numpy as np
GM=3.986004418e14; Re=6378137.0; J2=1.08262668e-3; W=7.292115e-5
G=6.6743e-11; hbar=1.054571817e-34; m=1.44315587e-25
k=2*np.pi/698e-9; n=1000
# normal gravity (Somigliana, WGS84 constants as remembered)
ge,kk,e2=9.7803253359,0.00193185265241,0.00669437999014
def som(lat,h=0.0):
    s2=np.sin(np.radians(lat))**2
    g0=ge*(1+kk*s2)/np.sqrt(1-e2*s2)
    a,f,m_=6378137.0,1/298.257223563,0.00344978650684
    return g0*(1-2/a*(1+f+m_-2*f*s2)*h+3/a**2*h*h)
for lat,name in [(0,'equator'),(51.76,'Oxford'),(41.83,'Fermilab 41.83N'),(90,'pole')]:
    print(name,lat,'g=%.6f'%som(lat),'g(h=100m)-g=%.5f'%(som(lat,100)-som(lat)))
# J2 potential tensor by central differences in ECEF; V = GM/r (1 - J2 (Re/r)^2 (3z^2/r^2-1)/2)
def V(p):
    r=np.linalg.norm(p); z=p[2]
    return GM/r*(1-J2*(Re/r)**2*(3*z*z/r**2-1)/2)
def grad(p,h=1.0):
    return np.array([(V(p+h*e)-V(p-h*e))/(2*h) for e in np.eye(3)])
def tens(p,h=50.0):
    T=np.zeros((3,3))
    for i,e in enumerate(np.eye(3)):
        T[:,i]=(grad(p+h*e,h)-grad(p-h*e,h))/(2*h)
    return T
def enu_frame(lat):
    la=np.radians(lat); return np.array([[0,1,0],[-np.sin(la),0,np.cos(la)],[np.cos(la),0,np.sin(la)]]) # rows: E?, let rows = e(east),n,u in ECEF at lon0
def local(lat,h):
    la=np.radians(lat); N=Re/np.sqrt(1-e2*np.sin(la)**2)
    p=np.array([(N+h)*np.cos(la),0,(N*(1-e2)+h)*np.sin(la)])
    up=np.array([np.cos(la),0,np.sin(la)]); east=np.array([0,1,0]); north=np.cross(up,east)
    R=np.array([east,north,up]); return p,R
print('J2 tensor in ENU (s^-2, E=1e-9) = d(grad V): note sign: V positive; gravity acc = +gradV')
for lat in (0,45,51.76,90):
    p,R=local(lat,0.0); T=R@tens(p)@R.T
    print(lat,'diag EE NN UU =',np.round(np.diag(T)*1e9,1),' EN,EU,NU =',np.round([T[0,1]*1e9,T[0,2]*1e9,T[1,2]*1e9],2),'trace',round(np.trace(T)*1e9,2))
# also with centrifugal (rotating Earth) term added: +W^2 diag-in-plane
print('point-mass 2g/R =',2*9.81/6371e3*1e9,'E ; -g/R =',-9.81/6371e3*1e9)
# phases: n k Gamma L T^2
def ph(Gam,L,T): return n*k*Gam*L*T**2
print('AION vertical Earth gradient phase (Gzz=3.086e-6,L=5,T=.73): %.3e rad'%ph(3.086e-6,5,.73))
print('AION horizontal Earth gradient (Gxx=-1.54e-6, L=5 m horizontal): %.3e rad'%ph(1.54e-6,5,.73))
print('check quasi-static vs engine: 1kg point d=5,z=5 engine -8.06e-3; gradient tensor Gzz of point mass at tower mid:')
# point mass 1 kg at (5,0,5), towers at origin; IFOs at z=2.5? use midpoint z=5 dz=+-2.5 ; Gzz and Gxx at tower mid
def tp(M,r): # tensor of -GM/r potential gradient of acceleration: d_i d_j (G M / r) at r
    rr=np.linalg.norm(r); return G*M*(3*np.outer(r,r)/rr**5-np.eye(3)/rr**3)
r=np.array([0,0,5.0])-np.array([5.0,0,5.0]); T=tp(1,r)
print(' point-mass tensor at tower middle (1 kg, 5 m east): Gzz=%.3e Gxx=%.3e s^-2'%(T[2,2],T[0,0]))
print(' quasi-static vertical phase n k Gzz L T^2 = %.3e'%ph(T[2,2],5,.73),' (engine -8.06e-3 incl. finite arms, signs)')
print(' horizontal gradiometer same mass: along x, L=5 m: %.3e'%ph(T[0,0],5,.73))
# Coriolis
Om=W
for T_,v in [(.73,9.81*.73),(.73,0.01)]:
    print('Coriolis common-mode, k horizontal, v vertical %.3f m/s: %.3e rad ; per 1 m/s of velocity mismatch: %.3e rad ; 1urad needs dv=%.2e m/s'%(v,n*k*2*Om*v*T_**2,n*k*2*Om*T_**2,1e-6/(n*k*2*Om*T_**2)))
# horizontal: sag
for T_ in (.73,2.5):
    print('free-fall drop over 2T=%.2f s: %.2f m ; apex of launch to carry across: '%(2*T_,.5*9.81*(2*T_)**2))
# tilt: horizontal beam tilt theta -> g component g*sin(theta): phase n k g sin(th) T^2 common mode; differential needs tilt difference or gradient
for th in (1e-6,1e-9):
    print('tilt %.0e rad: common-mode phase %.3e rad; differential via gradient 3.086e-6*dz(5m)*... ' % (th,n*k*9.81*th*.73**2))
# orbit
for h in (255e3,373e3,500e3):
    r=Re+h; w=np.sqrt(GM/r**3); Pper=2*np.pi/w
    print('orbit h=%d km: omega=%.4e rad/s period %.0f s; Gzz=2w^2=%.3e (%.0f E) ; Gxx=Gyy=-w^2=%.3e; M(rotating frame)=w^2 diag(0,-1,3)->Mzz=%.3e'%(h/1e3,w,Pper,2*w*w,2*w*w*1e9,-w*w,3*w*w))
r=Re+255e3; w=np.sqrt(GM/r**3)
for L,T_ in [(0.5,2.0),(0.5,5.0),(1.0,10.0)]:
    kk2=2*k  # single-photon-pair? use k_eff for two-photon Rb: 1.6e7; n=1
    keff=2*2*np.pi/780e-9
    print('orbit gradiometer L=%.1f T=%.1f: phase keff*Gzz*L*T^2 = %.3e rad ; 1 mE/sqrt(Hz)=1e-12 s^-2 -> %.3e rad'%(L,T_,keff*2*w*w*L*T_**2,keff*1e-12*L*T_**2))
keff=2*2*np.pi/780e-9
print('keff',keff,'rotation w=%.3e: Coriolis phase 2 keff w dv T^2 for dv=1 um/s, T=5: %.3e rad'%(w,2*keff*w*1e-6*25))
print('rotation angle over T=5 s: %.3e rad ; centrifugal Omega^2 L: %.3e m/s^2'%(w*5,w*w*0.5))
# lunar tide gradient at Earth: 2GM_moon/d^3
GMm=4.9028e12; d=384400e3; print('Moon tidal gradient 2GM/d^3 = %.3e s^-2 -> AION vertical phase %.3e rad'%(2*GMm/d**3,ph(2*GMm/d**3,5,.73)))
# Moon/Mars own gradient
for nm,g,R in [('Moon',1.62,1737.4e3),('Mars',3.71,3389.5e3),('Earth',9.81,6371e3)]:
    print(nm,'2g/R= %.3e s^-2 (point-mass) ; AION-like phase %.3e'%(2*g/R,ph(2*g/R,5,.73)))
