import numpy as np
rng=np.random.default_rng(21)
n=160; NF=48
y,x=np.mgrid[0:n,0:n]/n
def modes(count,kmax,amp_pow):
    ms=[]
    for _ in range(count):
        kx=rng.integers(-kmax,kmax+1); ky=rng.integers(1,kmax+1)
        k=np.hypot(kx,ky); ms.append((kx,ky,rng.uniform(0,2*np.pi),k**-amp_pow,rng.integers(-1,2)))
    return ms
M0=modes(45,6,2.1); W1=modes(24,4,1.6); W2=modes(24,4,1.6)
def field(ms,X,Y,t,scroll=1):
    f=np.zeros_like(X)
    for kx,ky,ph,a,m in ms:
        f+=a*np.cos(2*np.pi*(kx*X+ky*(Y+scroll*t))+ph+2*np.pi*m*t)
    return f
frames=[]
for i in range(NF):
    t=i/NF
    w1=field(W1,x,y,t); w2=field(W2,x,y,t,scroll=1)
    s=0.07/np.std(w1)
    f=field(M0,x+s*w1,y+s*w2,t)          # domain-warped, periodic in x, y and t
    f2=field(M0,x+1.6*s*w2,y+1.6*s*w1,t)  # second octave of warping for curl-like eddies
    F=0.6*f+0.4*f2
    frames.append(F)
fr=np.array(frames); fr=(fr-fr.mean())/fr.std()
# rising smoke: denser lower down, thinning upward
env=0.55+0.45*y
F=1/(1+np.exp(-(1.3*fr-0.5+2.2*(env[None]-0.6))))
np.save('work/frames_periodic.npy',F); print(F.shape, F.min(), F.max())
