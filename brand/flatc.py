import sys; sys.path.insert(0,'.')
from room import *
def flatroom(name,pal,arms=3,gaps=True,cells=False,swap=False,outline=True,cscale=1.6,cfill=None):
    g=[]
    L=[P(0,0,0),P(0,N,0),P(0,N,N),P(0,0,N)]; R=[P(0,0,0),P(N,0,0),P(N,0,N),P(0,0,N)]; Fl=[P(0,0,0),P(N,0,0),P(N,N,0),P(0,N,0)]
    walls={'left':'fluid','right':'grad','floor':'quad'}
    if swap: walls={'left':'grad','right':'fluid','floor':'quad'}
    MC=(N/2,N/2,N/2)   # the C floats at the room's centre
    def face(face,kind):
        pts={'left':L,'right':R,'floor':Fl}[face]
        g.append(poly(pts,pal[kind].get('base',pal[kind].get('fill',['#000'])[0])))
        if face=='left': mp=lambda u,v: P(0,N*(1-u),N*v); xyz=lambda U,V:(0*U,N*(1-U),N*V)
        elif face=='right': mp=lambda u,v: P(N*u,0,N*v); xyz=lambda U,V:(N*U,0*U,N*V)
        else: mp=lambda u,v: P(N*u,N*v,0); xyz=lambda U,V:(N*U,N*V,0*U)
        if kind=='grad':
            n=80; U,V=np.meshgrid(np.linspace(0,1,n),np.linspace(0,1,n)); X,Y,Z=xyz(U,V)
            fld=np.log(1/((X-MC[0])**2+(Y-MC[1])**2+(Z-MC[2])**2+0.3)); fld=(fld-fld.min())/(fld.max()-fld.min())
            lv=np.linspace(0,1.0001,9); fig=plt.figure(); cs=plt.contourf(U[0],V[:,0],fld,levels=lv); plt.close(fig)
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
            lc=pal['quad']['line']; fills=pal['quad']['fill']
            for (x,y,s,dp) in quad([(N/2,N/2)]):
                g.append(poly([P(x,y,0),P(x+s,y,0),P(x+s,y+s,0),P(x,y+s,0)],fills[min(dp,len(fills)-1)],f' stroke="{lc}" stroke-width="0.45" stroke-opacity="{pal["quad"].get("op",0.7)}"'))
    for f_ in ('right','left','floor'): face(f_,walls[f_])
    if outline:
        for a,b in [((0,0,0),(0,0,N)),((0,0,0),(N,0,0)),((0,0,0),(0,N,0))]:
            pa,pb=P(*a),P(*b); g.append(f'<path d="M{pa[0]:.1f} {pa[1]:.1f} L{pb[0]:.1f} {pb[1]:.1f}" stroke="{pal["edge"]}" stroke-width="0.6" stroke-opacity="0.8" fill="none"></path>')
    # flat C facing the camera: screen-aligned bars, centred on the room's centre point
    cx,cy=P(N/2,N/2,N/2)
    total_h=N*S*cscale*0.62; u=total_h/5
    wC=(arms+1)*u; x0=cx-wC/2; y0=cy-total_h/2
    gp=u*0.16 if gaps else 0
    col=cfill or pal['vox']
    bars=[(x0+u+gp, y0, arms*u-gp, u),            # top arm
          (x0, y0+u+gp, u, 3*u-2*gp),               # spine
          (x0+u+gp, y0+4*u, arms*u-gp, u)]          # bottom arm
    for (x,y,w,h) in bars:
        g.append(f'<rect x="{x:.2f}" y="{y:.2f}" width="{w:.2f}" height="{h:.2f}" fill="{col}"></rect>')
        if cells:
            n=round(max(w,h)/u)
            for k in range(1,n):
                if w>h: xx=x+k*w/n; g.append(f'<path d="M{xx:.2f} {y:.2f} L{xx:.2f} {y+h:.2f}" stroke="{pal["bg"]}" stroke-width="{u*0.1:.2f}"></path>')
                else: yy=y+k*h/n; g.append(f'<path d="M{x:.2f} {yy:.2f} L{x+w:.2f} {yy:.2f}" stroke="{pal["bg"]}" stroke-width="{u*0.1:.2f}"></path>')
    open('out5/'+name+'.svg','w',newline='\n').write(wrap(''.join(g),square_vb(bounds(L+R+Fl),S*0.55),pal['bg']))
flatroom('f01-colour-dark',PAL['principia-dark'])
flatroom('f02-colour-light',PAL['principia-light'])
flatroom('f03-colour-dark-cells',PAL['principia-dark'],cells=True)
flatroom('f04-colour-dark-solid',PAL['principia-dark'],gaps=False)
flatroom('f05-colour-dark-swapped',PAL['principia-dark'],swap=True)
flatroom('f06-mono-dark',PAL['mono-dark'])
flatroom('f07-mono-light',PAL['mono-light'])
flatroom('f08-bw-dark',PAL['pure-bw-dark'])
flatroom('f09-bw-light',PAL['pure-bw-light'])
flatroom('f10-colour-dark-arms2',PAL['principia-dark'],arms=2)
