import numpy as np
G=6.6743e-11; hbar=1.054571817e-34; m=1.44315587e-25; lam=698e-9; nk=1000
vk=nk*hbar*(2*np.pi/lam)/m
def arm(p0,v0,kick,T,g):
    """position(t) of one arm; g is a vector; kick a vector"""
    def pos(t):
        t=np.asarray(t); out=[]
        for tt in np.atleast_1d(t):
            if tt<=T: out.append(p0+v0*tt+0.5*g*tt*tt)
            else:
                pT=p0+v0*T+0.5*g*T*T; vT=v0+g*T+kick; s=tt-T
                out.append(pT+vT*s+0.5*g*s*s)
        return np.array(out)
    return pos
gl=np.polynomial.legendre.leggauss(12)
def phase(srcpos,M,T,u0,n,g,ifo_offsets,lower_minus_upper=True,order=12):
    """general axis n (unit), gravity vector g, IFO launch points ifo_offsets (list of 3-vectors); static source."""
    x,w=np.polynomial.legendre.leggauss(order); res=[]
    for off in ifo_offsets:
        low=arm(off,u0*n,vk*n,T,g); up=arm(off,(u0+vk)*n,-vk*n,T,g)
        s=0.0
        for a,b in [(0,T),(T,2*T)]:
            for panel in range(8):
                l=a+(b-a)*panel/8; r_=a+(b-a)*(panel+1)/8
                t=0.5*(r_-l)*x+0.5*(r_+l); wt=0.5*(r_-l)*w
                pl=low(t); pu=up(t)
                Vu=-G*M/np.linalg.norm(pu-srcpos,axis=1); Vl=-G*M/np.linalg.norm(pl-srcpos,axis=1)
                s+=np.sum(wt*(Vu-Vl))
        res.append(m/hbar*s)
    return res
z=np.array([0,0,1.0]); x=np.array([1.0,0,0]); T=.73
g=9.81*-z
def vert(d,zz):
    r=phase(np.array([d,0,zz]),1.0,T,3.86,z,g,[np.zeros(3),5*z]); return r[0]-r[1]   # lower minus upper
for d,zz in [(5,5),(10,5),(50,5),(200,5),(3,12)]: print('vertical py d=%g z=%g %.12e'%(d,zz,vert(d,zz)))
print('--- horizontal ---')
def tens(M,r):
    rr=np.linalg.norm(r); return G*M*(3*np.outer(r,r)/rr**5-np.eye(3)/rr**3)
keff_n=nk*2*np.pi/lam
def horiz(src,u0=0.0,L=5.0,g_on=True,T=.73,ax=x):
    gg=9.81*-z if g_on else np.zeros(3)
    r=phase(src,1.0,T,u0,ax,gg,[np.zeros(3),L*ax]); return r[0]-r[1]
# far mass: along-beam (x) at 1000 m ahead, and broadside (y) at 1000 m. Source at height of the mean path.
Tm=.73; zmean=-0.5*9.81*(Tm**2*(1+1/6.))  # time-weighted triangle mean of -g t^2/2 about T
for name,src in [('broadside y=1000 m',np.array([2.5,1000.,zmean])),('end-on x=1000 m',np.array([1000.,0,zmean]))]:
    r_mid=np.array([2.5,0,zmean])-src   # tower mid to source -> use tensor at path mean; tensor symmetric in sign of r
    Gxx=tens(1.0,-r_mid)[0,0]
    pred=keff_n*Gxx*5.0*Tm**2
    # sign: ΔΦ=low-up; phase_ifo = (m/ħ)∫[V(upper arm)-V(lower arm)]; arms sep s=vk t along +x -> δφ≈ (m/ħ)∫ s dV/dx ; ΔΦ = δφ1-δφ2 = -(...)L d2V/dx2  with V=-GM/r, d2V/dx2 = -Gxx_acc?? 
    got=horiz(src)
    print(name,'py %.6e  |nkΓxxLT²| %.6e  ratio %.5f'%(got,abs(pred),abs(got)/abs(pred)),' Gxx=%.3e'%Gxx)
# vertical far check with same recipe
src=np.array([1000.,0,5.0]); Gzz=tens(1.0,np.array([0,0,0])-src)[2,2]
print('vertical far d=1000: py %.6e ; n k Gzz L T^2 %.6e ratio %.5f'%(vert(1000,5.0),keff_n*Gzz*5*T**2,abs(vert(1000,5.0))/abs(keff_n*Gzz*5*T**2)))
# near source: effect of sag. source 5 m broadside, at height 0 (beam height), with and without fall
src=np.array([2.5,5.0,0.0])
print('near mass 5 m beside beam centre, drop from rest: with fall %.6e ; without fall(g=0) %.6e'%(horiz(src),horiz(src,g_on=False)))
# launched up so the atoms are at the beam height at the pulses: u0 along z impossible with kick along x? launch vector separate
