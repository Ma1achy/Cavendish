import numpy as np
GM=3.986004418e14; Re=6378137.0; J2=1.08262668e-3
def V(p):
    r=np.linalg.norm(p); z=p[2]
    return GM/r*(1-J2*(Re/r)**2*(3*z*z/r**2-1)/2)
def grad(p,h=1.0): return np.array([(V(p+h*e)-V(p-h*e))/(2*h) for e in np.eye(3)])
def tens(p,h=20.0):
    T=np.zeros((3,3))
    for i,e in enumerate(np.eye(3)): T[:,i]=(grad(p+h*e,h)-grad(p-h*e,h))/(2*h)
    return T
def pm(p): r=np.linalg.norm(p); return GM*(3*np.outer(p,p)/r**5-np.eye(3)/r**3)
r=Re+255e3; w=np.sqrt(GM/r**3); inc=np.radians(96.7)
mx0=0;rows=[]
for u in np.linspace(0,2*np.pi,361)[:-1]:
    pos=r*np.array([np.cos(u),np.sin(u)*np.cos(inc),np.sin(u)*np.sin(inc)])
    vel=np.array([-np.sin(u),np.cos(u)*np.cos(inc),np.cos(u)*np.sin(inc)])
    z=pos/r; y=np.cross(z,vel); y/=np.linalg.norm(y); x=np.cross(y,z)   # x along track, y = orbit normal, z radial
    R=np.array([x,y,z]); T=R@tens(pos)@R.T; T0=R@pm(pos)@R.T
    rows.append((u,np.diag(T),np.diag(T0),np.trace(T),T[0,2]))
d=np.array([row[1]-row[2] for row in rows])
print('point-mass LORF diag (E):',np.round(rows[0][2]*1e9,1),' expected w^2 diag(-1,-1,2)=',np.round(w*w*np.array([-1,-1,2])*1e9,1))
print('J2 minus point mass, min/max over orbit (E): xx %.1f..%.1f  yy %.1f..%.1f  zz %.1f..%.1f'%tuple(np.ravel([[d[:,i].min()*1e9,d[:,i].max()*1e9] for i in range(3)])))
print('max |trace| over orbit (E):',max(abs(r_[3]) for r_ in rows)*1e9,'  max |xz| J2 (E):',max(abs(r_[4]) for r_ in rows)*1e9)
keff=2*2*np.pi/780e-9
print('J2 zz swing phase (L=0.5,T=5): %.3e rad'%(keff*(d[:,2].max()-d[:,2].min())*0.5*25))
print('1 E phase (L=0.5,T=5): %.3f rad'%(keff*1e-9*.5*25))
