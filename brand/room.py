import sys, math, numpy as np, json
sys.path.insert(0,'.')
from vox import iso, cube, mix, wrap, bounds, square_vb, CH, BONE
import matplotlib; matplotlib.use('Agg'); import matplotlib.pyplot as plt
from scipy.ndimage import gaussian_filter, zoom
OUT='out/'
N=6; S=10
def P(x,y,z): return iso(x,y,z,S)
def poly(pts,fill,extra=''):
    return '<path d="M'+' L'.join(f'{a:.2f} {b:.2f}' for a,b in pts)+f'Z" fill="{fill}"{extra}></path>'
def shade(c,k1=.16,k2=.32): return (c,mix(c,'#000000',k1),mix(c,'#000000',k2))
# ---------- fluid field data (from the plume sim), posterised ----------
dd=np.load('work/plume.npz')['dye']
crop=dd[30:252, 150:372]
F=gaussian_filter(crop,5.5); F=np.log1p(F/np.percentile(F,55)); F/=np.percentile(F,99.5); F=F.clip(0,1)
F=zoom(F,(120/F.shape[0],120/F.shape[1]),order=1)
def fluid_paths(levels):
    fig=plt.figure(); cs=plt.contourf(np.linspace(0,1,F.shape[1]),np.linspace(0,1,F.shape[0]),F,levels=levels); plt.close(fig)
    return cs.allsegs  # per level: list of polygons (u,v)
FL=[0.0,0.22,0.45,0.68,1.01]
FSEG=fluid_paths(FL)
# ---------- floor quadtree ----------
def quad(focus):
    cells=[]
    def sp(x,y,s,d):
        cx,cy=x+s/2,y+s/2
        dist=min(math.dist((cx,cy),f) for f in focus)
        if d<5 and s>0.19 and dist< s*1.6: 
            h=s/2
            for a in (0,h):
                for b in (0,h): sp(x+a,y+b,h,d+1)
        else: cells.append((x,y,s,d))
    sp(0,0,N,0); return cells
def C_cells(arms):
    w=arms+1
    pat=['.'+'#'*arms]+['#'+'.'*arms]*3+['.'+'#'*arms]
    out=[]
    for r,row in enumerate(pat):
        for c,ch in enumerate(row):
            if ch=='#': out.append((c,4-r))
    return out,w
def room(name,pal,arms=3,swap=False,outline=True,tower=True,show=('grad','fluid','quad')):
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
    _,cw=C_cells(arms); y0=(N-cw)/2; z0=0.5
    MC=(0.5, y0+cw/2, z0+2.5)
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
            focus=[(MC[0],MC[1]),(N/2,0.2)]
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
        y=y0+(cw-1-c)
        items.append((0+y+z0+z, cube(0,y,z0+z,S,*vt,size=1)))
    if tower:
        tx=N/2+0.4; w=0.34; zb=0.9; zt=N-0.9; n=int((zt-zb)/w)
        tt=shade(pal['tower'])
        for k in range(n):
            z=zb+k*(zt-zb)/n; items.append((tx+z+0.001, cube(tx-w/2,0,z,S,*tt,size=w)))
        cs_=0.62; cl=shade(pal['cloud'])
        for z in (zb-cs_*0.3, zt-cs_*0.7): items.append((tx+z+1, cube(tx-cs_/2,0,z,S,*cl,size=cs_)))
    items.sort(key=lambda t:t[0])
    g+= [i[1] for i in items]
    allp=L+R+Fl
    vb=square_vb(bounds(allp),S*0.55)
    open(OUT+name+'.svg','w',newline='\n').write(wrap(''.join(g),vb,pal['bg']))
PAL={
 'principia-dark':dict(bg='#141312',edge='#efe9dc',vox='#efe9dc',tower='#efe9dc',cloud='#ffffff',
    grad=dict(base='#2a1712',ramp=['#2a1712','#a3402a','#f2b49b'],stops=[]),
    fluid=dict(base='#13262b',ramp=['#1b4651','#3b9bb0','#cdebf0']),
    quad=dict(line='#b5d38f',fill=['#1a1d16','#1e2419','#232b1c','#2a3420','#323e25','#3a492a'],op=0.95)),
 'principia-light':dict(bg='#f4efe4',edge='#1d1b19',vox='#2a2724',tower='#2a2724',cloud='#1d1b19',
    grad=dict(base='#f6e3da',ramp=['#f6e3da','#d9785d','#9b3420'],stops=[]),
    fluid=dict(base='#e3f0f1',ramp=['#c4e2e7','#5fb0c1','#1d6676']),
    quad=dict(line='#5e7d3f',fill=['#eef0e2','#e6ebd6','#dde5ca','#d3debd','#c9d6b0','#bfcfa3'],op=0.7)),
 'mono-dark':dict(bg='#121110',edge='#efe9dc',vox='#efe9dc',tower='#efe9dc',cloud='#ffffff',
    grad=dict(base='#1d1b19',ramp=['#1d1b19','#5b564d','#c8c1b2'],stops=[]),
    fluid=dict(base='#1a1917',ramp=['#2b2925','#6f6a60','#d9d2c3']),
    quad=dict(line='#efe9dc',fill=['#161513','#191816','#1c1b19','#201e1b','#24221f','#282623'],op=0.45)),
 'mono-light':dict(bg='#f6f2ea',edge='#1d1b19',vox='#2a2724',tower='#2a2724',cloud='#1d1b19',
    grad=dict(base='#efe9dc',ramp=['#efe9dc','#b5ae9f','#4a463f'],stops=[]),
    fluid=dict(base='#ebe5d8',ramp=['#ddd6c7','#a39c8e','#3d3a34'],),
    quad=dict(line='#1d1b19',fill=['#f3eee4','#efe9dd','#ebe5d7','#e6e0d0','#e1dac9','#dcd5c2'],op=0.4)),
 'pure-bw-dark':dict(bg='#000000',edge='#ffffff',vox='#ffffff',tower='#ffffff',cloud='#ffffff',
    grad=dict(base='#000000',ramp=['#000000','#555555','#e8e8e8'],stops=[]),
    fluid=dict(base='#000000',ramp=['#1a1a1a','#777777','#ffffff']),
    quad=dict(line='#ffffff',fill=['#000000','#050505','#0a0a0a','#101010','#161616','#1c1c1c'],op=0.5)),
 'pure-bw-light':dict(bg='#ffffff',edge='#000000',vox='#1a1a1a',tower='#1a1a1a',cloud='#000000',
    grad=dict(base='#f4f4f4',ramp=['#f4f4f4','#b0b0b0','#2a2a2a'],stops=[]),
    fluid=dict(base='#efefef',ramp=['#dedede','#9a9a9a','#1a1a1a']),
    quad=dict(line='#000000',fill=['#ffffff','#fafafa','#f4f4f4','#eeeeee','#e8e8e8','#e2e2e2'],op=0.45)),
}
if __name__=='__main__':
    room('r01-colour-dark-arms3',PAL['principia-dark'],3)
    room('r02-colour-dark-arms2',PAL['principia-dark'],2)
    room('r03-colour-light-arms3',PAL['principia-light'],3)
    room('r04-mono-dark-arms3',PAL['mono-dark'],3)
    room('r05-mono-light-arms3',PAL['mono-light'],3)
    room('r06-bw-dark-arms3',PAL['pure-bw-dark'],3)
    room('r07-bw-light-arms3',PAL['pure-bw-light'],3)
    room('r08-colour-dark-swapped',PAL['principia-dark'],3,swap=True)
    room('r09-colour-dark-no-edges',PAL['principia-dark'],3,outline=False)
    room('r10-colour-light-arms2',PAL['principia-light'],2)
