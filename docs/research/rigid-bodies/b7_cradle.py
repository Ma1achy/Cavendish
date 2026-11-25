"""B7: Newton's cradle, 3 equal masses touching (1 kg each), ball 1 hits at 1 m/s, e=1.
Reference (sequential binary collisions, exact): v = (0,0,1). Energy 0.5 J, momentum 1.
Simultaneous velocity-level solve with restitution targets (what a Gauss-Seidel contact solver does in one step)."""
import itertools
def pgs(iters, e=1.0):
    v=[1.0,0.0,0.0]; pre=[v[0]-v[1], v[1]-v[2]]   # approach speeds of pair (0,1),(1,2): pair0 approaching at 1, pair1 at 0
    target=[e*max(p,0) for p in pre]                # wanted separation speeds
    lam=[0.0,0.0]
    for _ in range(iters):
        for k in (0,1):
            rel=v[k+1]-v[k]
            d=(target[k]-rel)/2.0                   # effective mass 1/(1/1+1/1)=1/2 -> d = m_eff*(target-rel)
            d=d/1.0
            new=max(lam[k]+d,0.0); d=new-lam[k]; lam[k]=new
            v[k]-=d; v[k+1]+=d
    return v
for it in (1,2,10,100,10000):
    v=pgs(it); print(f"iters={it:>5}: v={[round(x,6) for x in v]}  p={sum(v):.6f}  E={0.5*sum(x*x for x in v):.6f}")
