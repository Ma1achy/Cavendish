"""B6: for the small angles of a substep (|phi| = |omega| h), is a fixed Taylor series (only + - * /, so bit-identical
wherever IEEE doubles are) as good as the platform libm?  Compare with exact (mpmath not needed: use fractions-free high precision via decimal)."""
import math, random
from decimal import Decimal, getcontext
getcontext().prec=60
def dsin(x):
    x=Decimal(x); t=x; s=x; n=1
    while abs(t)>Decimal(10)**-55: t=-t*x*x/((2*n)*(2*n+1)); s+=t; n+=1
    return s
def dcos(x):
    x=Decimal(x); t=Decimal(1); s=t; n=1
    while abs(t)>Decimal(10)**-55: t=-t*x*x/((2*n-1)*(2*n)); s+=t; n+=1
    return s
def tsin(x,N):
    x2=x*x; r=0.0
    for k in range(N,-1,-1): r=1.0 if False else r
    # Horner
    c=[(-1)**k/math.factorial(2*k+1) for k in range(N+1)]
    r=c[N]
    for k in range(N-1,-1,-1): r=r*x2+c[k]
    return x*r
def tcos(x,N):
    x2=x*x; c=[(-1)**k/math.factorial(2*k) for k in range(N+1)]
    r=c[N]
    for k in range(N-1,-1,-1): r=r*x2+c[k]
    return r
random.seed(1)
for lim in (0.05,0.25,0.5):
    xs=[random.uniform(-lim,lim) for _ in range(2000)]
    for N in (6,8,10):
        es=max(abs(float(Decimal(tsin(x,N))-dsin(x)))/math.ulp(float(dsin(x))) for x in xs)
        ec=max(abs(float(Decimal(tcos(x,N))-dcos(x)))/math.ulp(float(dcos(x))) for x in xs)
        print(f"|x|<={lim:<4} terms={N:>2}: max error sin {es:5.2f} ulp, cos {ec:5.2f} ulp")
    el=max(abs(float(Decimal(math.sin(x))-dsin(x)))/math.ulp(float(dsin(x))) for x in xs)
    print(f"             this machine's libm sin: {el:5.2f} ulp")
