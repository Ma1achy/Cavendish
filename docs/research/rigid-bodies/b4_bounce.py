"""B4: ball dropped from 1 m, restitution e=0.5, g=9.80665. Closed form: impact k at
t_k = t0*(1+2e(1-e^(k-1))/(1-e))  (flight k lasts 2 e^k v0/g), apex after impact k: e^(2k) h0.
Compare a fixed-step solver that detects penetration at step end, with one that solves the time of impact (TOI) inside the step."""
import math
g=9.80665;h0=1.0;e=0.5
t0=math.sqrt(2*h0/g); v0=g*t0
def impact_time(k): return t0+sum(2*e**j*v0/g for j in range(1,k))
def naive(dt,nimp=6,project=True):
    y=h0; v=0.0; t=0.0; imps=[]; apex=[]; peak=0; n_after=0
    while len(imps)<nimp:
        ynew=y+v*dt-0.5*g*dt*dt; vnew=v-g*dt; t+=dt
        if ynew<0 and vnew<0:
            imps.append(t); vnew=-e*vnew; ynew=0.0 if project else ynew; 
            if len(imps)>1: apex.append(peak)
            peak=0
        y,v=ynew,vnew; peak=max(peak,y)
    return imps,apex
def toi(dt,nimp=6):
    y=h0; v=0.0; t=0.0; imps=[]; apex=[]; peak=0; left=0
    while len(imps)<nimp:
        rem=dt
        while rem>0:
            # time to y=0 under y+v s -g s^2/2: s = (v+sqrt(v^2+2 g y))/g
            s=(v+math.sqrt(v*v+2*g*y))/g if (y>0 or v<0) else 0.0
            if s<=rem and (y>0 or v<0):
                y=0.0; v=-(v-g*s); v=e*v; t+=s; rem-=s; imps.append(t)
                if len(imps)>1: apex.append(peak)
                peak=0
                if len(imps)>=nimp: break
            else:
                y=y+v*rem-0.5*g*rem*rem; v-=g*rem; t+=rem; rem=0
            peak=max(peak,y)
    return imps,apex
for name,fn in (("fixed step, projection",lambda dt:naive(dt)),("fixed step + TOI",lambda dt:toi(dt))):
    for dt in (1/60,1/240,1/960):
        imps,apex=fn(dt)
        et=max(abs(imps[k]-impact_time(k+1)) for k in range(len(imps)))
        ea=max(abs(apex[k]-h0*e**(2*(k+1))) for k in range(len(apex)))
        print(f"{name:<24} dt=1/{round(1/dt):<4} max impact-time err={et:.2e} s  max apex err={ea:.2e} m (apex_1={apex[0]:.6f} vs {h0*e*e:.6f})")
