"""B1: free flight under constant g. Which one-step update is exact against x0+v0 t+g t^2/2?"""
import math
g=-9.80665; x0=100.0; v0=3.0
def run(h,T,scheme,fused=False):
    n=round(T/h); x=x0; v=v0
    for _ in range(n):
        if scheme=="symplectic_euler":      # PBD/PGS predict: v+=g h ; x+=h v
            v=v+g*h; x=x+h*v
        elif scheme=="velocity_verlet":     # x+=h v+h^2 g/2 ; v+=h g  (exact for constant a)
            if fused: x=math.fma(0.5*h*h,g,math.fma(h,v,x))
            else: x=x+h*v+0.5*h*h*g
            v=v+g*h
        elif scheme=="leapfrog_kdk":
            v1=v+0.5*h*g; x=x+h*v1; v=v1+0.5*h*g
    t=n*h
    return x-(x0+v0*t+0.5*g*t*t), v-(v0+g*t)
for h in (1/60,1/240,1/960):
    for T in (1.0,10.0):
        for s in ("symplectic_euler","leapfrog_kdk","velocity_verlet"):
            ex,ev=run(h,T,s); print(f"h=1/{round(1/h):<4} T={T:>4} {s:<17} dx={ex:+.3e} dv={ev:+.3e}")
# fused vs unfused disagreement after 10 s at 1/240
a=run(1/240,10,"velocity_verlet")[0]; b=run(1/240,10,"velocity_verlet",True)[0]
print("verlet fused-unfused position diff after 10 s:", abs((a)-(b)))
print("--- dyadic step 1/256 and closed-form-from-epoch, long flights")
for T in (10.0,100.0,1000.0):
    ex,ev=run(1/256,T,"velocity_verlet")
    # closed form from epoch: x = x0+v0 t+g t^2/2 with t = n*h (exact multiple), error is only the final rounding
    n=round(T*256); t=n/256
    xc=x0+v0*t+0.5*g*t*t
    print(f"T={T:>6} verlet h=1/256 dx={ex:+.2e} dv={ev:+.2e}; closed form from epoch: dx={xc-(x0+v0*t+0.5*g*t*t):+.1e} (identical expression; rounding only ~{math.ulp(xc):.1e})")
