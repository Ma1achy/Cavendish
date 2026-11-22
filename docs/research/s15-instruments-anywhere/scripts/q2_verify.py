"""Computed here: engine-form source phase for a uniform-gradient 'source' potential versus the published closed form.
Engine form: phi = (m/hbar) * int_0^2T [V(r_arm2) - V(r_arm1)] dt, V = -1/2 r^T Gam r (force = +Gam r, Gam_ij = -d2U/dxidxj as Roura A2).
Closed form (Roura Eq.4 + Hogan Table II term 4 coefficient): -> phi = -(k T^2) khat^T Gam (r0 + v0 T + 7/12 g T^2), arm2 - arm1 sign as here."""
import numpy as np
rng=np.random.default_rng(1)
hbar=1.054571817e-34; m=1.4432e-25; n=1000; lam=698e-9
k=n*2*np.pi/lam*2/2  # engine: v_kick = n hbar k/m ; k_eff*hbar/m = v_kick -> use k_eff = m*v_kick/hbar
vk=n*hbar*(2*np.pi/lam)/m; keff=m*vk/hbar
T=0.73
A=rng.normal(size=(3,3)); Gam=(A+A.T)*1e-6
g=np.array([0,0,-9.81])
for khat in (np.array([0,0,1.]),np.array([1.,0,0]),np.array([0.6,0.0,0.8])):
    r0=rng.normal(size=3)*3; v0=rng.normal(size=3)*3
    # Gauss-Legendre in each half (integrand piecewise polynomial)
    x,w=np.polynomial.legendre.leggauss(8)
    tot=0.0
    for a,b in ((0,T),(T,2*T)):
        t=0.5*(b-a)*x+0.5*(b+a); ww=0.5*(b-a)*w
        s=np.where(t<=T,vk*t,vk*(2*T-t))  # arm separation magnitude along khat (triangle)
        rc=r0[None,:]+v0[None,:]*t[:,None]+0.5*g[None,:]*t[:,None]**2
        r1=rc-0.5*s[:,None]*khat; r2=rc+0.5*s[:,None]*khat
        V=lambda r:-0.5*np.einsum('ti,ij,tj->t',r,Gam,r)
        tot+=np.sum(ww*(V(r2)-V(r1)))
    phi=m/hbar*tot
    closed=-keff*T**2*khat@Gam@(r0+v0*T+7/12*g*T**2)
    print(khat,phi,closed,phi/closed-1)
