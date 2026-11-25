import sys; sys.path.insert(0,'.')
from room import *
OUT3='out3/'
def outer(name,pal,arms=3,swap=False,outline=True,tower=True):
    g=[]
    TOP=[P(0,0,N),P(N,0,N),P(N,N,N),P(0,N,N)]
    LF=[P(0,N,0),P(N,N,0),P(N,N,N),P(0,N,N)]     # +y face (front-left)
    RF=[P(N,0,0),P(N,N,0),P(N,N,N),P(N,0,N)]     # +x face (front-right)
    walls={'left':'fluid','right':'grad','top':'quad'}
    if swap: walls={'left':'grad','right':'fluid','top':'quad'}
    uid=name.replace('-','')[:12]
    cc,cw=C_cells(arms); x0=(N-cw)/2; z0=0.5
    MC=(x0+cw/2, N+0.5, z0+2.5)
    TX=(N+0.2, N/2)
    def face_map(face):
        if face=='left':  return (lambda u,v: P(N*u, N, N*v)), (lambda U,V:(N*U, N+0*U, N*V))
        if face=='right': return (lambda u,v: P(N, N*(1-u), N*v)), (lambda U,V:(N+0*U, N*(1-U), N*V))
        return (lambda u,v: P(N*u, N*v, N)), (lambda U,V:(N*U, N*V, N+0*U))
    def draw(face,kind):
        pts={'left':LF,'right':RF,'top':TOP}[face]
        base=pal[kind].get('base',pal[kind].get('fill',['#000'])[0])
        g.append(poly(pts,base))
        mp,xyz=face_map(face)
        if kind=='grad':
            n=80; U,V=np.meshgrid(np.linspace(0,1,n),np.linspace(0,1,n)); X,Y,Z=xyz(U,V)
            r2=(X-MC[0])**2+(Y-MC[1])**2+(Z-MC[2])**2+0.3
            fld=np.log(1/r2); fld=(fld-fld.min())/(fld.max()-fld.min())
            lv=np.linspace(0,1.0001,9)
            fig=plt.figure(); cs=plt.contourf(U[0],V[:,0],fld,levels=lv); plt.close(fig)
            cols=pal['grad']['ramp']
            for i,segs in enumerate(cs.allsegs):
                t=i/(len(lv)-2); c=mix(cols[0],cols[1],2*t) if t<.5 else mix(cols[1],cols[2],2*t-1)
                d=''.join('M'+' L'.join(f'{mp(u,v)[0]:.1f} {mp(u,v)[1]:.1f}' for u,v in seg)+'Z' for seg in segs if len(seg)>2)
                if d: g.append(f'<path d="{d}" fill="{c}" fill-rule="evenodd"></path>')
        elif kind=='fluid':
            cols=pal['fluid']['ramp']
            for i,segs in enumerate(FSEG):
                if i==0: continue
                t=i/(len(FSEG)-1); c=mix(cols[0],cols[1],2*t) if t<.5 else mix(cols[1],cols[2],2*t-1)
                d=''.join('M'+' L'.join(f'{mp(u,1-v)[0]:.1f} {mp(u,1-v)[1]:.1f}' for u,v in seg)+'Z' for seg in segs if len(seg)>2)
                if d: g.append(f'<path d="{d}" fill="{c}" fill-rule="evenodd"></path>')
        else:
            focus=[(MC[0],N-0.2),(N-0.2,N/2)]
            lc=pal['quad']['line']; fills=pal['quad']['fill']
            for (x,y,s,dpt) in quad(focus):
                q=[P(x,y,N),P(x+s,y,N),P(x+s,y+s,N),P(x,y+s,N)]
                g.append(poly(q,fills[min(dpt,len(fills)-1)],f' stroke="{lc}" stroke-width="0.45" stroke-opacity="{pal["quad"].get("op",0.7)}"'))
    for face in ('top','left','right'): draw(face,walls[face])
    if outline:
        ec=pal['edge']
        for a,b in [((N,N,0),(N,N,N)),((N,N,N),(0,N,N)),((N,N,N),(N,0,N))]:
            pa,pb=P(*a),P(*b); g.append(f'<path d="M{pa[0]:.1f} {pa[1]:.1f} L{pb[0]:.1f} {pb[1]:.1f}" stroke="{ec}" stroke-width="0.6" stroke-opacity="0.8" fill="none"></path>')
    items=[]; vt=shade(pal['vox'])
    for (c,z) in cc:
        x=x0+c; items.append((x+N+z0+z, cube(x,N,z0+z,S,*vt,size=1)))
    if tower:
        w=0.34; zb=0.9; zt=N-0.9; n=int((zt-zb)/w); tt=shade(pal['tower'])
        for k in range(n):
            z=zb+k*(zt-zb)/n; items.append((N+N/2+z, cube(N,N/2-w/2,z,S,*tt,size=w)))
        cs_=0.62; cl=shade(pal['cloud'])
        for z in (zb-cs_*0.3, zt-cs_*0.7): items.append((N+N/2+z+1, cube(N,N/2-cs_/2,z,S,*cl,size=cs_)))
    items.sort(key=lambda t:t[0])
    g+=[i[1] for i in items]
    allp=TOP+LF+RF+[P(N+0.7,N/2,N/2),P(N/2,N+1,N/2)]
    open(OUT3+name+'.svg','w',newline='\n').write(wrap(''.join(g),square_vb(bounds(allp),S*0.6),pal['bg']))
outer('o01-colour-dark',PAL['principia-dark'])
outer('o02-colour-light',PAL['principia-light'])
outer('o03-mono-dark',PAL['mono-dark'])
outer('o04-mono-light',PAL['mono-light'])
outer('o05-bw-dark',PAL['pure-bw-dark'])
outer('o06-bw-light',PAL['pure-bw-light'])
outer('o07-colour-dark-swapped',PAL['principia-dark'],swap=True)
outer('o08-colour-dark-no-edges',PAL['principia-dark'],outline=False)
outer('o09-colour-light-swapped',PAL['principia-light'],swap=True)
outer('o10-colour-dark-arms2',PAL['principia-dark'],arms=2)
