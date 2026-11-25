"""B5: box sliding on a plane, Coulomb friction. Closed form: stop time v0/(mu g), distance v0^2/(2 mu g)."""
g=9.80665; mu=0.4; v0=2.0
ts=v0/(mu*g); xs=v0*v0/(2*mu*g)
def run(h,scheme):
    x=0.0; v=v0; n=0
    while v>0:
        dv=min(v,mu*g*h)           # impulse clamp: friction cannot reverse the motion
        vn=v-dv
        x+= (vn*h if scheme=="semi-implicit" else 0.5*(v+vn)*h) ; v=vn; n+=1
    return n*h,x
for sch in ("semi-implicit","trapezoid (mean velocity)"):
    for h in (1/60,1/240,1/960):
        t,x=run(h,sch); print(f"{sch:<26} h=1/{round(1/h):<4} stop time err {t-ts:+.2e} s  distance err {x-xs:+.2e} m (closed form {xs:.6f} m); final v exactly 0: True")
