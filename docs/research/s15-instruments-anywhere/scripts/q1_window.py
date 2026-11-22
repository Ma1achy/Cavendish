"""Joint spectral window of two towers with different cycles (AION-10: 2 s, AICE: 5 s), 20 000 s each.
|W(f)| = |sum_n exp(-2 pi i f t_n)| / N over the UNION, equal-weight and noise-weighted. Computed here."""
import numpy as np
Tobs=20000.0
def win(t,w,f): return np.abs((w[None,:]*np.exp(-2j*np.pi*f[:,None]*t[None,:])).sum(1))/w.sum()
def peaks(ta,tb,sa,sb,label):
    t=np.concatenate([ta,tb]); w=np.concatenate([np.full(len(ta),1/sa**2),np.full(len(tb),1/sb**2)])
    out=[]
    for f in [0.1,0.2,0.25,0.3,0.4,0.5,0.6,0.75,0.9,1.0,1.1,1.5,2.0,2.5]:
        out.append((f,win(t,w,np.array([f]))[0]))
    print(label,' '.join(f"{f}:{v:.3f}" for f,v in out))
ta=np.arange(0,Tobs,2.0); tb=np.arange(0,Tobs,5.0)
for sa,sb,lab in [(1,1,'equal weights'),(1,1/164,'AICE 164x more precise (noise sigma ratio)')]:
    peaks(ta,tb,sa,sb,'integer offsets, '+lab+':')
peaks(ta,tb+0.37,1,1,'AICE offset 0.37 s, equal weights:')
rng=np.random.default_rng(1); peaks(ta,tb+rng.uniform(-.3,.3,len(tb)),1,1,'AICE jitter U(-0.3,0.3), equal weights:')
# single towers
for n,t in [('AION alone',ta),('AICE alone',tb)]:
    print(n,' '.join(f"{f}:{win(t,np.ones(len(t)),np.array([f]))[0]:.3f}" for f in [0.1,0.2,0.25,0.3,0.4,0.5,1.0]))
