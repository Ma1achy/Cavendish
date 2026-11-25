"""B3: a vertical stack of unit boxes on ground, three solver families at EQUAL constraint-solve budget per frame.
1D (heights only) so every number can be checked by hand: rest position y_i = i+0.5 (top surface at i+1).
Frame dt=1/60, g=-9.81, restitution 0. K = constraint sweeps per frame (iterations x substeps).
 A  PGS: 1 substep, K velocity iterations, Baumgarte beta=0.2 slop=5 mm, warm started (Bullet/Box2D-v2 style)
 B  soft step (Catto 2024): K substeps, 1 sweep each, soft contacts (30 Hz, zeta 10), warm start, relax sweep
 C  XPBD-rigid (Mueller 2020): K substeps, 1 position sweep each, v=(x-xprev)/h, no relax
"""
import math, sys
G=-9.81; DT=1/60
def make(masses): return list(masses),[i+0.5 for i in range(len(masses))],[0.0]*len(masses)
def gap(y,i): return (y[i]-(y[i-1] if i else 0.0))-(1.0 if i else 0.5)   # signed gap of contact i (below box i)

def warm(lam,v,m):
    for i in range(len(m)):
        v[i]+=lam[i]/m[i]
        if i: v[i-1]-=lam[i]/m[i-1]
def solver_A(m,y,v,K,frame_steps,sink):
    n=len(m); lam=[0.0]*n; h=DT; beta=0.2; slop=0.005
    for f in range(frame_steps):
        for i in range(n): v[i]+=G*h
        warm(lam,v,m)
        for it in range(K):
            for i in range(n):
                s=gap(y,i); vb=v[i]-(v[i-1] if i else 0.0)
                bias=beta/h*max(-s-slop,0.0)
                # effective mass of relative velocity: 1/(1/m_i+1/m_{i-1}) ; ground infinite
                im=1/m[i]+(1/m[i-1] if i else 0.0)
                d=-(vb-bias)/im if s<=0.0 else -(vb+s/h)/im      # speculative when apart
                new=max(lam[i]+d,0.0); d=new-lam[i]; lam[i]=new
                v[i]+=d/m[i]
                if i: v[i-1]-=d/m[i-1]
        for i in range(n): y[i]+=v[i]*h
        sink(f,y,v)
HZ_CAP=30.0
def solver_B(m,y,v,K,frame_steps,sink):
    n=len(m); lam=[0.0]*n; h=DT/K
    hz=min(HZ_CAP,0.25/h); zeta=10.0; w=2*math.pi*hz; a1=2*zeta+h*w; a2=h*w*a1; a3=1/(1+a2)
    biasRate=w/a1; massScale=a2*a3; impScale=a3; push=3.0
    def sweep(relax):
        for i in range(n):
            s=gap(y,i); vb=v[i]-(v[i-1] if i else 0.0)
            im=1/m[i]+(1/m[i-1] if i else 0.0)
            if s>0: bias=s/h; ms=1.0; isc=0.0
            elif relax: bias=0.0; ms=1.0; isc=0.0
            else: bias=max(biasRate*s,-push); ms=massScale; isc=impScale
            d=-ms/im*(vb+bias)-isc*lam[i]
            new=max(lam[i]+d,0.0); d=new-lam[i]; lam[i]=new
            v[i]+=d/m[i]
            if i: v[i-1]-=d/m[i-1]
    for f in range(frame_steps):
        for _ in range(K):
            for i in range(n): v[i]+=G*h
            warm(lam,v,m)
            sweep(False)
            for i in range(n): y[i]+=v[i]*h
            sweep(True)
        sink(f,y,v)
def solver_C(m,y,v,K,frame_steps,sink):
    n=len(m); h=DT/K
    for f in range(frame_steps):
        for _ in range(K):
            yp=list(y)
            for i in range(n): v[i]+=G*h; y[i]+=v[i]*h
            for i in range(n):   # one position sweep, inverse-mass weighted
                s=gap(y,i)
                if s<0:
                    wi=1/m[i]; wj=1/m[i-1] if i else 0.0
                    c=-s/(wi+wj); y[i]+=c*wi
                    if i: y[i-1]-=c*wj
            for i in range(n): v[i]=(y[i]-yp[i])/h
        sink(f,y,v)
def metrics(solver,masses,K,secs=6):
    m,y,v=make(masses); frames=int(secs*60); pen=[0.0]; mv=[0.0]; y_start=[None]
    def sink(f,y,v):
        pen[0]=max(pen[0],max(-gap(y,i) for i in range(len(y))))
        if f==frames-61: y_start[0]=list(y)
        if f>=frames-60: mv[0]=max(mv[0],max(abs(x) for x in v))
    solver(m,y,v,K,frames,sink)
    drift=max(abs(a-b) for a,b in zip(y,y_start[0]))
    return pen[0],mv[0],drift,y[-1]-(len(y)-0.5)
cases={"10 equal boxes":[1.0]*10,"heavy on light (1000:1, 5 high)":[1.0]*5+[1000.0],"light on heavy (1:1000)":[1000.0]*5+[1.0]}
import sys
if len(sys.argv)>1: HZ_CAP=float(sys.argv[1])
for name,ms in cases.items():
    print(f"\n{name}")
    print(f"{'K':>3} {'solver':<10} {'max pen mm':>11} {'max|v| last 1s m/s':>19} {'drift last 1s m':>16} {'top pose err mm':>16}")
    for K in (4,8,20):
        for nm,s in (("A PGS+Baum",solver_A),("B softstep",solver_B),("C XPBD",solver_C)):
            p,mvv,d,e=metrics(s,ms,K)
            print(f"{K:>3} {nm:<10} {p*1e3:>11.3f} {mvv:>19.2e} {d:>16.2e} {e*1e3:>16.3f}")
