import sys; sys.path.insert(0,'.')
from room import *
def room2(name,pal,arms=3,swap=False,outline=True,tower=True,show=('grad','fluid','quad')):
    """Inside corner of a cube: left wall x=0 (spans y,z), right wall y=0 (spans x,z), floor z=0.
       The C protrudes from the left wall; the tower stands on the right wall."""
    defs=[];g=[]
    L=[P(0,0,0),P(0,N,0),P(0,N,N),P(0,0,N)]       # left wall
    R=[P(0,0,0),P(N,0,0),P(N,0,N),P(0,0,N)]       # right wall
    Fl=[P(0,0,0),P(N,0,0),P(N,N,0),P(0,N,0)]      # floor
    walls={'left':'fluid','right':'grad','floor':'quad'}
    if swap: walls={'left':'grad','right':'fluid','floor':'quad'}
    uid=name.replace('-','')[:12]
    # mass centre: the C's centre on the left wall
    _,cw=C_cells(arms); K=N/5.0
    MC=(cw*K/2, N-K/2, N/2)
    def draw_face(face,kind):
        pts={'left':L,'right':R,'floor':Fl}[face]
        base=pal[kind].get('base',pal[kind].get('fill',['#000'])[0])
        g.append(poly(pts,base))
        # mapping (u,v) in [0,1]^2 to face coordinates (u horizontal on screen-ish, v up)
        if face=='left':  mp=lambda u,v: P(0, N*(1-u), N*v)
        elif face=='right': mp=lambda u,v: P(N*u, 0, N*v)
        else: mp=lambda u,v: P(N*u, N*v, 0)
        if kind=='grad':
            # field of the mass at MC evaluated on this face, as a wash + equipotential contours
            a,b=(mp(0,0.5),mp(1,0.5)) if face!='floor' else (mp(0,0),mp(1,1))
            gid='gw'+uid+face
            st=''.join(f'<stop offset="{o}" stop-color="{c}"></stop>' for o,c in pal['grad']['stops'])
            # radial-ish: compute 1/r^2 field on a grid and contour it
            n=80; U,V=np.meshgrid(np.linspace(0,1,n),np.linspace(0,1,n))
            if face=='left': X,Y,Z=0*U+0.0, N*(1-U), N*V
            elif face=='right': X,Y,Z=N*U, 0*U, N*V
            else: X,Y,Z=N*U, N*V, 0*U
            r2=(X-MC[0])**2+(Y-MC[1])**2+(Z-MC[2])**2+0.3
            fld=np.log(1/r2); fld=(fld-fld.min())/(fld.max()-fld.min())
            lv=np.linspace(0,1.0001,9)
            fig=plt.figure(); cs=plt.contourf(U[0],V[:,0],fld,levels=lv); plt.close(fig)
            cols=pal['grad']['ramp']
            for i,segs in enumerate(cs.allsegs):
                c=mix(cols[0],cols[1],i/(len(lv)-2)) if len(cols)==2 else mix(cols[0],cols[1],min(1,2*i/(len(lv)-2))) if i<(len(lv)-1)/2 else mix(cols[1],cols[2],(2*i-(len(lv)-2))/(len(lv)-2))
                d=''.join('M'+' L'.join(f'{mp(u,v)[0]:.2f} {mp(u,v)[1]:.2f}' for u,v in seg)+'Z' for seg in segs if len(seg)>2)
                if d: g.append(f'<path d="{d}" fill="{c}" fill-rule="evenodd"></path>')
        elif kind=='fluid':
            cols=pal['fluid']['ramp']
            for i,segs in enumerate(FSEG):
                if i==0: continue
                t=i/(len(FSEG)-1); c=mix(cols[0],cols[1],t) if len(cols)==2 else (mix(cols[0],cols[1],2*t) if t<.5 else mix(cols[1],cols[2],2*t-1))
                d=''.join('M'+' L'.join(f'{mp(u,1-v)[0]:.2f} {mp(u,1-v)[1]:.2f}' for u,v in seg)+'Z' for seg in segs if len(seg)>2)
                if d: g.append(f'<path d="{d}" fill="{c}" fill-rule="evenodd"></path>')
        elif kind=='quad':
            focus=[(MC[0],N-0.3),(N-0.75,0.2),(0.3,N-0.3)]
            cells=quad(focus)
            lc=pal['quad']['line']; fills=pal['quad']['fill']
            for (x,y,s,dpt) in cells:
                pts=[P(x,y,0),P(x+s,y,0),P(x+s,y+s,0),P(x,y+s,0)]
                f=fills[min(dpt,len(fills)-1)]
                g.append(poly(pts,f,f' stroke="{lc}" stroke-width="0.45" stroke-opacity="{pal["quad"].get("op",0.7)}"'))
    for face in ('right','left','floor'): draw_face(face,walls[face])
    # corner edges
    if outline:
        ec=pal['edge']
        for a,b in [((0,0,0),(0,0,N)),((0,0,0),(N,0,0)),((0,0,0),(0,N,0))]:
            pa,pb=P(*a),P(*b); g.append(f'<path d="M{pa[0]:.2f} {pa[1]:.2f} L{pb[0]:.2f} {pb[1]:.2f}" stroke="{ec}" stroke-width="0.6" stroke-opacity="0.8" fill="none"></path>')
    # voxels: C on left wall (protruding +x), tower on right wall (protruding +y)
    items=[]
    cc,cw=C_cells(arms)
    vt=shade(pal['vox'])
    for (c,z) in cc:
        x=c*K; zz=z*K; y=N-K
        items.append((x+y+zz+1.5*K, cube(x,y,zz,S,*vt,size=K)))
    if tower:
        tx=N-0.75; w=0.34; zb=0.6; zt=N-0.7; n=int((zt-zb)/w)
        tt=shade(pal['tower'])
        for k in range(n):
            z=zb+k*(zt-zb)/n; items.append((tx+z+0.001, cube(tx-w/2,0,z,S,*tt,size=w)))
        cs_=0.62; cl=shade(pal['cloud'])
        for z in (zb-cs_*0.3, zt-cs_*0.7): items.append((tx+z+1, cube(tx-cs_/2,0,z,S,*cl,size=cs_)))
    items.sort(key=lambda t:t[0])
    g+= [i[1] for i in items]
    allp=L+R+Fl
    vb=square_vb(bounds(allp),S*0.55)
    open('out4/'+name+'.svg','w',newline='\n').write(wrap(''.join(g),vb,pal['bg']))

room2('d01-colour-dark',PAL['principia-dark'])
room2('d02-colour-light',PAL['principia-light'])
room2('d03-mono-dark',PAL['mono-dark'])
room2('d04-mono-light',PAL['mono-light'])
room2('d05-bw-dark',PAL['pure-bw-dark'])
room2('d06-bw-light',PAL['pure-bw-light'])
room2('d07-colour-dark-swapped',PAL['principia-dark'],swap=True)
room2('d08-colour-light-swapped',PAL['principia-light'],swap=True)
room2('d09-colour-dark-no-edges',PAL['principia-dark'],outline=False)
room2('d10-colour-dark-arms2',PAL['principia-dark'],arms=2)
