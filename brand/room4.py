import sys, math, numpy as np
sys.path.insert(0,'.'); sys.path.insert(0,'.')
from vox import iso, cube, mix, wrap, bounds, square_vb
import matplotlib; matplotlib.use('Agg'); import matplotlib.pyplot as plt
from scipy.ndimage import gaussian_filter, zoom
N=6; S=10
def P(x,y,z): return iso(x,y,z,S)
def shade(c,k1=.16,k2=.32): return (c,mix(c,'#000000',k1),mix(c,'#000000',k2))
F=np.load('work/crop_a.npy'); F=gaussian_filter(F,1.2)
F=zoom(F,(120/F.shape[0],120/F.shape[1]),order=1)
LV=np.linspace(0.04,1.001,40)   # fine ramp: reads as a smooth field, not posterised bands
fig=plt.figure(); CS=plt.contourf(np.linspace(0,1,F.shape[1]),np.linspace(0,1,F.shape[0]),F,levels=LV); plt.close(fig)
FSEG=CS.allsegs
def ramp(stops,t):
    t=min(1,max(0,t)); k=len(stops)-1; i=min(k-1,int(t*k)); return mix(stops[i],stops[i+1],t*k-i)
def face_xf(face):
    if face=='left':  mp=lambda u,v: P(0,N*(1-u),N*v)
    elif face=='right': mp=lambda u,v: P(N*u,0,N*v)
    else: mp=lambda u,v: P(N*u,N*v,0)
    o=mp(0,0); a=mp(1,0); b=mp(0,1)
    return f'matrix({a[0]-o[0]:.4f} {a[1]-o[1]:.4f} {b[0]-o[0]:.4f} {b[1]-o[1]:.4f} {o[0]:.4f} {o[1]:.4f})'
def quad_cells(focus,maxd=5):
    out=[]
    def sp(x,y,s,d):
        c=(x+s/2,y+s/2); dist=min(math.dist(c,f) for f in focus)
        if d<maxd and dist<s*1.25: 
            for a in (0,s/2):
                for b in (0,s/2): sp(x+a,y+b,s/2,d+1)
        else: out.append((x,y,s,d))
    sp(0,0,1,0); return out
def C_pattern(arms,spine=3):
    pat=['.'+'#'*arms]+['#'+'.'*arms]*spine+['.'+'#'*arms]
    H=len(pat)
    return [(c,H-1-r) for r,row in enumerate(pat) for c,ch in enumerate(row) if ch=='#'], arms+1


def voxel_boxes(mode,arms=3):
    out=[]
    if mode=='centre':
        cc,cw=C_pattern(arms,spine=5); K=0.68; c0=(cw-1)/2
        for (c,z) in cc:
            t=(c-c0)*K; out.append((N/2-K/2+t,N/2-K/2-t,N/2-3.5*K+z*K,K))
    elif mode=='sketch':
        cc,cw=C_pattern(arms); K=N/5
        for (c,z) in cc: out.append((c*K,N-K,z*K,K))
    return out
def field_bands(boxes,xyz,stops,n=150,levels=40):
    """Gravity-gradient magnitude (sum of 1/r^3 over sub-sampled voxel volume) on a face, as smooth contour bands in unit coords."""
    U,V=np.meshgrid(np.linspace(0,1,n),np.linspace(0,1,n)); X,Y,Z=xyz(U,V)
    F=np.zeros_like(U)
    for (x,y,z,k) in boxes:
        for a in (0.25,0.75):
            for b in (0.25,0.75):
                for c in (0.25,0.75):
                    r=np.sqrt((X-(x+a*k))**2+(Y-(y+b*k))**2+(Z-(z+c*k))**2)+0.05
                    F+=1.0/r**3
    F=np.log(F); lo,hi=np.percentile(F,1),np.percentile(F,99.8); F=((F-lo)/(hi-lo)).clip(0,1)
    lv=np.linspace(0,1.0001,levels+1)
    fig=plt.figure(); cs=plt.contourf(U[0],V[:,0],F,levels=lv); plt.close(fig)
    body=f'<rect x="0" y="0" width="1" height="1" fill="{stops[-1]}"></rect>'
    rs=list(reversed(stops))  # low -> high
    for i,segs in enumerate(cs.allsegs):
        t=(i+0.5)/levels; c=ramp(rs,t)
        d=''.join('M'+' L'.join(f'{u:.4f} {v:.4f}' for u,v in seg)+'Z' for seg in segs if len(seg)>2)
        if d: body+=f'<path d="{d}" fill="{c}" fill-rule="evenodd" stroke="{c}" stroke-width="0.002"></path>'
    return body

_FL=[0]
def field_letter(cells,spine_h,center,height,stops,xf='',poly=None,d=0,n=0,levels=0):
    """Smooth C-shaped field glow, blurred in screen space and clipped to the face. Vector only."""
    _FL[0]+=1; uid=f'fl{_FL[0]}'
    s=height/spine_h; cu,cv=center
    cols=max(c for c,z in cells)+1
    rs=list(reversed(stops))
    def rects(grow):
        out=''
        for (c,z) in cells:
            x=cu+(c-cols/2)*s; y=cv+(z-spine_h/2)*s
            out+=f'<rect x="{x-grow:.4f}" y="{y-grow:.4f}" width="{s+2*grow:.4f}" height="{s+2*grow:.4f}"></rect>'
        return out
    pd='M'+' L'.join(f'{a:.2f} {b:.2f}' for a,b in poly)+'Z'
    W=60.0  # face size in screen units
    layers=[(0.30,0.11,0.16),(0.50,0.075,0.10),(0.68,0.045,0.055),(0.84,0.035,0.022),(0.97,0.020,0.0)]
    body=f'<defs><clipPath id="{uid}c"><path d="{pd}"></path></clipPath>'
    for i,(t,sd,gr) in enumerate(layers):
        body+=f'<filter id="{uid}f{i}" filterUnits="userSpaceOnUse" x="-200" y="-200" width="400" height="400"><feGaussianBlur stdDeviation="{sd*W:.2f}"></feGaussianBlur></filter>'
    body+='</defs>'
    body+=f'<path d="{pd}" fill="{rs[0]}"></path><g clip-path="url(#{uid}c)">'
    for i,(t,sd,gr) in enumerate(layers):
        body+=f'<g filter="url(#{uid}f{i})" opacity="0.95"><g transform="{xf}" fill="{ramp(rs,t)}">{rects(gr)}</g></g>'
    body+='</g>'
    return body
def lin_ao(gid,x1,y1,x2,y2,op,dist=0.22):
    # black fading to clear from (x1,y1) towards (x2,y2) over 'dist' in unit coords
    return (f'<linearGradient id="{gid}" gradientUnits="userSpaceOnUse" x1="{x1}" y1="{y1}" x2="{x1+(x2-x1)*dist}" y2="{y1+(y2-y1)*dist}">'
            f'<stop offset="0" stop-color="#000" stop-opacity="{op}"></stop><stop offset="0.45" stop-color="#000" stop-opacity="{op*0.35:.3f}"></stop><stop offset="1" stop-color="#000" stop-opacity="0"></stop></linearGradient>')
def blob(gid,cx,cy,r,op,sx=1.0,sy=1.0):
    return (f'<radialGradient id="{gid}" gradientUnits="userSpaceOnUse" cx="{cx:.4f}" cy="{cy:.4f}" r="{r:.4f}" gradientTransform="translate({cx:.4f} {cy:.4f}) scale({sx} {sy}) translate({-cx:.4f} {-cy:.4f})">'
            f'<stop offset="0" stop-color="#000" stop-opacity="{op}"></stop><stop offset="0.5" stop-color="#000" stop-opacity="{op*0.45:.3f}"></stop><stop offset="1" stop-color="#000" stop-opacity="0"></stop></radialGradient>')
def light_overlay(gid,dark,top_dark):
    # flat face darkening plus a gentle falloff towards the top (v=1 is up in face coords)
    return (f'<linearGradient id="{gid}" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="0" y2="1">'
            f'<stop offset="0" stop-color="#000" stop-opacity="{dark:.3f}"></stop><stop offset="1" stop-color="#000" stop-opacity="{min(0.9,dark+top_dark):.3f}"></stop></linearGradient>')

from scipy.spatial import ConvexHull
def _hull_path(pts2):
    pts2=np.array(pts2)
    try: h=ConvexHull(pts2); poly=pts2[h.vertices]
    except Exception: return ''
    return 'M'+' L'.join(f'{a:.2f} {b:.2f}' for a,b in poly)+'Z'
def cast(boxes,L,plane,val):
    """Project each box along sun direction L onto plane (axis index, value); return list of 3D polygons (as projected screen paths)."""
    paths=[]
    ax=plane
    for (x,y,z,sz) in boxes:
        pts=[]
        for a in (0,sz):
            for b in (0,sz):
                for c in (0,sz):
                    p=np.array([x+a,y+b,z+c],float)
                    if abs(L[ax])<1e-9: continue
                    t=(val-p[ax])/L[ax]
                    if t<0: continue
                    q=p+t*np.array(L)
                    pts.append(P(*q))
        if len(pts)>=3: paths.append(_hull_path(pts))
    return paths
def shadow_layer(uid,name,face_poly,paths,op):
    cid=f'{uid}clip{name}'
    clip=f'<clipPath id="{cid}"><path d="M'+' L'.join(f'{a:.2f} {b:.2f}' for a,b in face_poly)+'Z"></path></clipPath>'
    body=''.join(f'<path d="{d}" fill="#000"></path>' for d in paths if d)
    return clip, f'<g clip-path="url(#{cid})" opacity="{op}">{body}</g>'
def scene(name,pal,mode='centre',arms=3,walls=('fluid','grad','quad'),edges=False,uid=None):
    uid=uid or name.replace('-','')[:10]
    defs=[]; g=[]
    left,right,floor=walls
    # where the mass is (for gradient centring and quadtree focus), in each face's unit coords
    if mode=='centre':
        gc={'left':(0.5,0.5),'right':(0.5,0.5),'floor':(0.5,0.5)}
        _cc,_cw=C_pattern(arms,spine=5); _K=0.68; _c0=(_cw-1)/2
        qf=sorted({(round((N/2+(c-_c0)*_K)/N,4), round((N/2-(c-_c0)*_K)/N,4)) for (c,z) in _cc})
    else: gc={'left':(0.25,0.5),'right':(0.4,0.5),'floor':(0.4,0.9)}; qf=[(0.4,0.92),(0.88,0.03)]
    for face,kind in (('right',right),('left',left),('floor',floor)):
        xf=face_xf(face); gid=f'{uid}{face}'
        if kind=='grad':
            if face=='left': xyz=lambda U,V:(0*U,N*(1-U),N*V)
            elif face=='right': xyz=lambda U,V:(N*U,0*U,N*V)
            else: xyz=lambda U,V:(N*U,N*V,0*U)
            if mode=='centre': _cells,_cw=C_pattern(arms,spine=5); _H=7; _cen=(0.47,0.5); _ht=0.62
            else: _cells,_cw=C_pattern(arms); _H=5; _cen=((0.32,0.5) if face=='right' else (0.62,0.5)); _ht=0.62
            _poly={'left':[P(0,0,0),P(0,N,0),P(0,N,N),P(0,0,N)],'right':[P(0,0,0),P(N,0,0),P(N,0,N),P(0,0,N)]}[face]
            g.append(field_letter(_cells,_H,_cen,_ht,pal['grad'],xf=xf,poly=_poly))
        elif kind=='lingrad':
            defs.append(f'<linearGradient id="{gid}" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="0" y2="1">'+
                ''.join(f'<stop offset="{i/(len(pal["grad"])-1):.3f}" stop-color="{c}"></stop>' for i,c in enumerate(pal['grad']))+'</linearGradient>')
            g.append(f'<g transform="{xf}"><rect x="0" y="0" width="1" height="1" fill="url(#{gid})"></rect></g>')
        elif kind=='fluid':
            st=pal['fluid']; body=f'<rect x="0" y="0" width="1" height="1" fill="{st[0]}"></rect>'
            for i,segs in enumerate(FSEG):
                c=ramp(st,(i+1)/len(FSEG))
                d=''.join('M'+' L'.join(f'{u:.3f} {1-v:.3f}' for u,v in seg[::2])+'Z' for seg in segs if len(seg)>6)
                if d: body+=f'<path d="{d}" fill="{c}" fill-rule="evenodd" stroke="{c}" stroke-width="0.004" stroke-linejoin="round"></path>'
            g.append(f'<g transform="{xf}">{body}</g>')
        elif kind=='quad':
            st=pal['quad']; body=f'<rect x="0" y="0" width="1" height="1" fill="{pal["quad_bg"]}"></rect>'
            for (x,y,s,d) in quad_cells(qf):
                body+=f'<rect x="{x:.4f}" y="{y:.4f}" width="{s+0.003:.4f}" height="{s+0.003:.4f}" fill="{ramp(st,d/5)}"></rect>'
            g.append(f'<g transform="{xf}">{body}</g>')
    # lighting: key light from front-left-above -> floor brightest, right wall mid, left wall darkest
    LIGHT=pal.get('light',{'left':0.30,'right':0.12,'floor':0.0})
    AO=pal.get('ao',0.55)
    for face in ('right','left','floor'):
        xf=face_xf(face); lid=f'{uid}L{face}'
        ov=''
        if face=='floor':
            defs.append(f'<linearGradient id="{lid}" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#000" stop-opacity="{LIGHT[face]+0.10:.3f}"></stop><stop offset="1" stop-color="#000" stop-opacity="{LIGHT[face]:.3f}"></stop></linearGradient>')
        else:
            # face coords: v=0 floor ... v=1 top; local y = v here (we draw rect 0..1 in face coords where y is v)
            defs.append(light_overlay(lid,LIGHT[face],0.10).replace('y1="0" x2="0" y2="1"','y1="0" x2="0" y2="1"'))
        ov+=f'<rect x="0" y="0" width="1" height="1" fill="url(#{lid})"></rect>'
        # ambient occlusion along the interior edges this face shares
        if face=='left':   # x=0 wall: edges at v=0 (floor) and u=1 (back corner)
            defs.append(lin_ao(lid+'a',0,0,0,1,AO)); defs.append(lin_ao(lid+'b',1,0,0,0,AO))
        elif face=='right': # y=0 wall: edges at v=0 (floor) and u=0 (back corner)
            defs.append(lin_ao(lid+'a',0,0,0,1,AO)); defs.append(lin_ao(lid+'b',0,0,1,0,AO))
        else:               # floor: edges at u=0 (x=0 wall) and v=0 (y=0 wall)
            defs.append(lin_ao(lid+'a',0,0,1,0,AO)); defs.append(lin_ao(lid+'b',0,0,0,1,AO))
        ov+=f'<rect x="0" y="0" width="1" height="1" fill="url(#{lid}a)"></rect><rect x="0" y="0" width="1" height="1" fill="url(#{lid}b)"></rect>'
        g.append(f'<g transform="{xf}">{ov}</g>')
    # contact shadows from the voxels
    sh_floor=''; sh_left=''; sh_right=''; bi=0
    if mode=='centre':
        _cc,_cw=C_pattern(arms,spine=5); _K=0.68; _c0=(_cw-1)/2
        for (c,z) in _cc:
            t=(c-_c0)*_K; fx=(N/2+t)/N; fy=(N/2-t)/N
            gid=f'{uid}sf{bi}'; bi+=1; defs.append(blob(gid,fx,fy,0.12,0.22))
            sh_floor+=f'<rect x="0" y="0" width="1" height="1" fill="url(#{gid})"></rect>'
    else:
        Kc=N/5
        for (c,z) in C_pattern(arms)[0]:
            if z==0:   # touching the floor
                gid=f'{uid}sf{bi}'; bi+=1; defs.append(blob(gid,(c+0.5)*Kc/N,(N-Kc/2)/N,0.16,0.5,1.0,0.8))
                sh_floor+=f'<rect x="0" y="0" width="1" height="1" fill="url(#{gid})"></rect>'
            if c==0:   # spine against the left wall
                gid=f'{uid}sl{bi}'; bi+=1; defs.append(blob(gid,1-(N-Kc/2)/N,(z+0.5)*Kc/N,0.14,0.45))
                sh_left+=f'<rect x="0" y="0" width="1" height="1" fill="url(#{gid})"></rect>'
        tx=(N-0.75)/N
        gid=f'{uid}st'; defs.append(f'<linearGradient id="{gid}" gradientUnits="userSpaceOnUse" x1="{tx-0.07:.3f}" y1="0" x2="{tx+0.07:.3f}" y2="0"><stop offset="0" stop-color="#000" stop-opacity="0"></stop><stop offset="0.5" stop-color="#000" stop-opacity="0.45"></stop><stop offset="1" stop-color="#000" stop-opacity="0"></stop></linearGradient>')
        sh_right+=f'<rect x="{tx-0.07:.3f}" y="0.08" width="0.14" height="0.84" fill="url(#{gid})"></rect>'
        gid=f'{uid}stf'; defs.append(blob(gid,tx,0.03,0.08,0.5)); sh_floor+=f'<rect x="0" y="0" width="1" height="1" fill="url(#{gid})"></rect>'
    if sh_floor: g.append(f'<g transform="{face_xf("floor")}">{sh_floor}</g>')
    if sh_left: g.append(f'<g transform="{face_xf("left")}">{sh_left}</g>')
    if sh_right: g.append(f'<g transform="{face_xf("right")}">{sh_right}</g>')
    if edges:
        for a,b in [((0,0,0),(0,0,N)),((0,0,0),(N,0,0)),((0,0,0),(0,N,0))]:
            pa,pb=P(*a),P(*b); g.append(f'<path d="M{pa[0]:.1f} {pa[1]:.1f} L{pb[0]:.1f} {pb[1]:.1f}" stroke="{pal["edge"]}" stroke-width="0.5" stroke-opacity="0.7" fill="none"></path>')
    items=[]; vt=shade(pal['vox']); boxes=[]
    cc,cw=C_pattern(arms)
    if mode=='centre':
        # voxel C in the diagonal plane facing the camera: screen-right = (+1,-1,0), screen-up = +z
        cc,cw=C_pattern(arms,spine=5)
        K=0.68; c0=(cw-1)/2
        for (c,z) in cc:
            t=(c-c0)*K; x=N/2-K/2+t; y=N/2-K/2-t; zz=N/2-3.5*K+z*K
            items.append((x+y+zz, cube(x,y,zz,S,*vt,size=K))); boxes.append((x,y,zz,K))
    else:
        K=N/5
        for (c,z) in cc:
            x=c*K; zz=z*K; y=N-K; items.append((x+y+zz+1.5*K, cube(x,y,zz,S,*vt,size=K))); boxes.append((x,y,zz,K))
        w=0.34; zb=0.6; zt=N-0.7; n=int((zt-zb)/w); tx=N-0.75; tt=shade(pal['tower'])
        for k in range(n):
            z=zb+k*(zt-zb)/n; items.append((tx+z, cube(tx-w/2,0,z,S,*tt,size=w))); boxes.append((tx-w/2,0,z,w))
        cs_=0.62; cl=shade(pal['cloud'])
        for z in (zb-cs_*0.3, zt-cs_*0.7): items.append((tx+z+1, cube(tx-cs_/2,0,z,S,*cl,size=cs_)))
    SUN=pal.get('sun_room',(-0.32,-0.62,-1.0)); SOP=pal.get('shadow',0.32)
    faces={'floor':(2,0.0,[P(0,0,0),P(N,0,0),P(N,N,0),P(0,N,0)]),
           'left':(0,0.0,[P(0,0,0),P(0,N,0),P(0,N,N),P(0,0,N)]),
           'right':(1,0.0,[P(0,0,0),P(N,0,0),P(N,0,N),P(0,0,N)])}
    for fname,(ax,val,poly_) in faces.items():
        bx=[b for b in boxes if not (fname=='right' and b[1]==0 and b[3]<0.5 and False)]
        paths=cast(bx,SUN,ax,val)
        if paths:
            cl,lay=shadow_layer(uid,fname,poly_,paths,SOP); defs.append(cl); g.append(lay)
    items.sort(key=lambda t:t[0]); g+=[i[1] for i in items]
    pts=[P(0,0,0),P(0,N,0),P(0,N,N),P(0,0,N),P(N,0,0),P(N,0,N),P(N,N,0)]
    svg=wrap('<defs>'+''.join(defs)+'</defs>'+''.join(g),square_vb(bounds(pts),S*0.55),pal['bg'])
    open('out6/'+name+'.svg','w',newline='\n').write(svg)
PAL={
 'ember-ice-dark':dict(bg='#121110',edge='#efe9dc',vox='#efe9dc',tower='#efe9dc',cloud='#ffffff',grad=['#f0643c','#7a2a3a','#14223a'],grad_r=0.85,
     fluid=['#0c1c22','#155a66','#4fb3c0','#e8fbfd'],quad=['#1b2418','#3d5a2c','#8fbf5a','#e3f5b8'],quad_bg='#121610'),
 'principia-light':dict(bg='#f4efe4',edge='#1d1b19',vox='#2a2724',tower='#2a2724',cloud='#1d1b19',grad=['#a8321f','#e3907a','#f7e6dd'],grad_r=0.85,
     fluid=['#eaf4f5','#9fd2dc','#3d93a6','#0f4a58'],quad=['#eef1e3','#c9dba7','#86a95c','#3f5a24'],quad_bg='#f4f3ea'),
 'magma-dark':dict(bg='#0d0b10',edge='#f3e9d7',vox='#f3e9d7',tower='#f3e9d7',cloud='#ffffff',grad=['#ffd36b','#e2533c','#4a1442','#140b1e'],grad_r=0.85,
     fluid=['#0b1020','#1f3b73','#5a8fd6','#e6f0ff'],quad=['#0e1a12','#1d4a2b','#3c8f4c','#a6e38a'],quad_bg='#0b140e'),
 'magma-viridian':dict(bg='#0d0b10',edge='#f3e9d7',vox='#f3e9d7',tower='#f3e9d7',cloud='#ffffff',grad=['#ffd36b','#e2533c','#4a1442','#140b1e'],grad_r=0.85,
     fluid=['#0b1020','#1f3b73','#5a8fd6','#e6f0ff'],quad=['#0c1816','#164238','#2a8a6a','#8fe0b8'],quad_bg='#0a1412'),
 'magma-lime':dict(bg='#0d0b10',edge='#f3e9d7',vox='#f3e9d7',tower='#f3e9d7',cloud='#ffffff',grad=['#ffd36b','#e2533c','#4a1442','#140b1e'],grad_r=0.85,
     fluid=['#0b1020','#1f3b73','#5a8fd6','#e6f0ff'],quad=['#12180c','#34481a','#6f9a2a','#d4f06a'],quad_bg='#0f140a'),
 'blueprint':dict(bg='#0d2033',edge='#d7e8f7',vox='#f4f8fc',tower='#f4f8fc',cloud='#ffffff',grad=['#bfe0ff','#3a7ec2','#0d2033'],grad_r=0.85,
     fluid=['#0d2033','#1d4f7c','#6ea9dd','#eaf5ff'],quad=['#10263b','#1d4466','#3f79ab','#a9d0f2'],quad_bg='#0b1c2d'),
 'clay-sage-light':dict(bg='#f2ede3',edge='#2a2724',vox='#263544',tower='#263544',cloud='#1a2530',grad=['#c25a37','#e8b49c','#e7efe5'],grad_r=0.85,
     fluid=['#f4ece0','#dcc08f','#a8743a','#4e3216'],quad=['#e4e9ee','#b5c3d1','#6d86a0','#2f4a66'],quad_bg='#edf0f2'),
 'riso-light':dict(bg='#f6f1e7',edge='#1d1b19',vox='#1d1b19',tower='#1d1b19',cloud='#1d1b19',grad=['#ff5a36','#ffb39e','#f6f1e7'],grad_r=0.85,
     fluid=['#f6f1e7','#a9c4ef','#3a6fd0','#13307a'],quad=['#f6f1e7','#f8e3a3','#f2c14e','#c48a12'],quad_bg='#f6f1e7'),
 'mono-dark':dict(bg='#121110',edge='#efe9dc',vox='#efe9dc',tower='#efe9dc',cloud='#ffffff',grad=['#bdb5a5','#4a463f','#1a1917'],grad_r=0.85,
     fluid=['#151413','#3a3732','#8a8478','#e6dfd0'],quad=['#181715','#2a2825','#4a463f','#8a8478'],quad_bg='#141312'),
 'mono-light':dict(bg='#f4efe4',edge='#1d1b19',vox='#2a2724',tower='#2a2724',cloud='#1d1b19',grad=['#4a463f','#b5ae9f','#efe9dc'],grad_r=0.85,
     fluid=['#f1ebdf','#cfc7b6','#857e70','#2f2c28'],quad=['#f1ece2','#ddd6c7','#b3ab9a','#6f695e'],quad_bg='#f4efe4'),
 'bw-dark':dict(bg='#000000',edge='#ffffff',vox='#ffffff',tower='#ffffff',cloud='#ffffff',grad=['#e6e6e6','#3a3a3a','#050505'],grad_r=0.85,
     fluid=['#050505','#2e2e2e','#8c8c8c','#ffffff'],quad=['#080808','#1e1e1e','#4a4a4a','#9a9a9a'],quad_bg='#000000'),
 'bw-light':dict(bg='#ffffff',edge='#000000',vox='#141414',tower='#141414',cloud='#000000',grad=['#1e1e1e','#9a9a9a','#f2f2f2'],grad_r=0.85,
     fluid=['#f4f4f4','#c8c8c8','#6a6a6a','#111111'],quad=['#f6f6f6','#dedede','#a8a8a8','#555555'],quad_bg='#fbfbfb'),
}
def topcube(name,pal,arms=3,sides=('fluid','grad'),edges=False):
    uid=name.replace('-','')[:10]; defs=[]; g=[]
    # outer faces: left = +y face (x,z), right = +x face (y,z), top = z=N
    maps={'left':(lambda u,v: P(N*u,N,N*v)),'right':(lambda u,v: P(N,N*(1-u),N*v)),'top':(lambda u,v: P(N*u,N*v,N))}
    def xf(face):
        mp=maps[face]; o=mp(0,0); a=mp(1,0); b=mp(0,1)
        return f'matrix({a[0]-o[0]:.4f} {a[1]-o[1]:.4f} {b[0]-o[0]:.4f} {b[1]-o[1]:.4f} {o[0]:.4f} {o[1]:.4f})'
    gcen={'left':(0.5,1.0),'right':(0.5,1.0)}
    _SUN=pal.get('sun_cube',(0.42,-0.72,-1.9))
    gpath=cast([(0,0,0,N)],_SUN,2,0.0)
    gid=uid+'gsh'
    defs.append(f'<filter id="{gid}" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="2.2"></feGaussianBlur></filter>')
    g.append(f'<g opacity="{pal.get("ground_shadow",0.45)}" filter="url(#{gid})">'+''.join(f'<path d="{d}" fill="#000"></path>' for d in gpath)+'</g>')
    for face,kind in (('left',sides[0]),('right',sides[1])):
        gid=uid+face
        if kind=='grad':
            _cc,_cw=C_pattern(arms); _K=N/6.0; _x0=(N-_cw*_K)/2; _y0=(N-5*_K)/2
            bx=[(_x0+c*_K,_y0+(4-z)*_K,N,_K) for (c,z) in _cc]
            xyzf={'left':(lambda U,V:(N*U,N+0*U,N*V)),'right':(lambda U,V:(N+0*U,N*(1-U),N*V))}[face]
            _poly={'left':[P(0,N,0),P(N,N,0),P(N,N,N),P(0,N,N)],'right':[P(N,0,0),P(N,N,0),P(N,N,N),P(N,0,N)]}[face]
            g.append(field_letter(_cc,5,(0.5,0.55),0.6,pal['grad'],xf=xf(face),poly=_poly))
        else:
            st=pal['fluid']; body=f'<rect x="0" y="0" width="1" height="1" fill="{st[0]}"></rect>'
            for i,segs in enumerate(FSEG):
                c=ramp(st,(i+1)/len(FSEG))
                d=''.join('M'+' L'.join(f'{u:.3f} {1-v:.3f}' for u,v in seg[::2])+'Z' for seg in segs if len(seg)>6)
                if d: body+=f'<path d="{d}" fill="{c}" fill-rule="evenodd" stroke="{c}" stroke-width="0.004" stroke-linejoin="round"></path>'
            g.append(f'<g transform="{xf(face)}">{body}</g>')
    cc,cw=C_pattern(arms); K=N/6.0
    x0=(N-cw*K)/2; y0=(N-5*K)/2
    # letter-right = +x, letter-up = -y; quadtree focus under each voxel
    cells=[]
    for (c,z) in cc:
        r=4-z; cells.append((x0+c*K, y0+r*K))
    qf=[((x+K/2)/N,(y+K/2)/N) for x,y in cells]
    st=pal['quad']; body=f'<rect x="0" y="0" width="1" height="1" fill="{pal["quad_bg"]}"></rect>'
    for (x,y,s_,d) in quad_cells(qf,maxd=5):
        body+=f'<rect x="{x:.4f}" y="{y:.4f}" width="{s_+0.003:.4f}" height="{s_+0.003:.4f}" fill="{ramp(st,d/5)}"></rect>'
    g.append(f'<g transform="{xf("top")}">{body}</g>')
    # lighting: top brightest, left face mid, right face darkest
    for face,dk in (('left',0.14),('right',0.36),('top',0.0)):
        lid=uid+'L'+face
        defs.append(light_overlay(lid,dk,0.0 if face=='top' else -0.08).replace(f'stop-opacity="{min(0.9,dk-0.08):.3f}"',f'stop-opacity="{max(0,dk-0.08):.3f}"'))
        g.append(f'<g transform="{xf(face)}"><rect x="0" y="0" width="1" height="1" fill="url(#{lid})"></rect></g>')
    # contact AO around each voxel on the top face
    sh=''
    for i,(x,y) in enumerate(cells):
        gid=f'{uid}tv{i}'; defs.append(blob(gid,(x+K/2)/N,(y+K/2)/N,K/N*0.95,0.42))
        sh+=f'<rect x="0" y="0" width="1" height="1" fill="url(#{gid})"></rect>'
    g.append(f'<g transform="{xf("top")}">{sh}</g>')
    if edges:
        for a,b in [((N,N,0),(N,N,N)),((N,N,N),(0,N,N)),((N,N,N),(N,0,N))]:
            pa,pb=P(*a),P(*b); g.append(f'<path d="M{pa[0]:.1f} {pa[1]:.1f} L{pb[0]:.1f} {pb[1]:.1f}" stroke="{pal["edge"]}" stroke-width="0.5" stroke-opacity="0.7" fill="none"></path>')
    SUN=pal.get('sun_cube',(0.42,-0.72,-1.9)); SOP=pal.get('shadow',0.32)
    boxes=[(x,y,N,K) for x,y in cells]
    paths=cast(boxes,SUN,2,N)
    cl,lay=shadow_layer(uid,'top',[P(0,0,N),P(N,0,N),P(N,N,N),P(0,N,N)],paths,SOP); defs.append(cl); g.append(lay)
    vt=shade(pal['vox']); items=[(x+y,cube(x,y,N,S,*vt,size=K)) for x,y in cells]
    items.sort(key=lambda t:t[0]); g+=[i[1] for i in items]
    pts=[P(0,0,N),P(N,0,N),P(N,N,N),P(0,N,N),P(0,N,0),P(N,N,0),P(N,0,0),P(N*0.5,N*0.5,N+K)]
    for gx in (0,N):
        for gy_ in (0,N): pts.append(P(gx+_SUN[0]*N/-_SUN[2], gy_+_SUN[1]*N/-_SUN[2], 0))
    open('out6/'+name+'.svg','w',newline='\n').write(wrap('<defs>'+''.join(defs)+'</defs>'+''.join(g),square_vb(bounds(pts),S*0.6),pal['bg']))

PAL2={
 'cmyk-light':dict(bg='#f5f2ea',edge='#1a1a1a',vox='#1c1c1c',tower='#1c1c1c',cloud='#000000',grad=['#e6007e','#f2a3cb','#f5f2ea'],grad_r=0.85,
     fluid=['#f5f2ea','#a6def2','#00a0dc','#005c80'],quad=['#f5f2ea','#fbec9a','#ffd400','#c49a00'],quad_bg='#f5f2ea'),
 'cmyk-dark':dict(bg='#0c0c0c',edge='#f5f2ea',vox='#f5f2ea',tower='#f5f2ea',cloud='#ffffff',grad=['#ff2a9a','#8a0a4e','#160811'],grad_r=0.85,
     fluid=['#061218','#00506e','#00a8e6','#c8f2ff'],quad=['#14120a','#4a3f08','#c9a800','#ffe45a'],quad_bg='#100e08'),
 'cmy-overprint':dict(bg='#fbf9f4',edge='#2a2a2a',vox='#2a2a2a',tower='#2a2a2a',cloud='#000000',grad=['#ff3fa4','#ffc2df','#fbf9f4'],grad_r=0.9,
     fluid=['#fbf9f4','#c6ecf8','#4cc3ee','#1a7fb0'],quad=['#fbf9f4','#fff2b0','#ffe14d','#e0b800'],quad_bg='#fbf9f4'),
 'secondary-dark':dict(bg='#0e0c0b',edge='#f3e9d7',vox='#f3e9d7',tower='#f3e9d7',cloud='#ffffff',grad=['#ffa13a','#b8451a','#1c0d08'],grad_r=0.85,
     fluid=['#100a1c','#3a2470','#8b6ad8','#ece4ff'],quad=['#0e1a10','#1f4d2a','#3fa05a','#b8f0b0'],quad_bg='#0b140d'),
 'tertiary-a':dict(bg='#0d0c0e',edge='#f3e9d7',vox='#f3e9d7',tower='#f3e9d7',cloud='#ffffff',grad=['#ff6a2a','#9c2c1a','#1a0b0a'],grad_r=0.85,
     fluid=['#0c0a1e','#2f2a7a','#6a5cf0','#e4e0ff'],quad=['#12160a','#3a4a12','#9ac22a','#e8ff8a'],quad_bg='#0e1108'),
 'tertiary-b':dict(bg='#0b0d0d',edge='#f3e9d7',vox='#f3e9d7',tower='#f3e9d7',cloud='#ffffff',grad=['#ffbe3a','#a8661a','#1a1208'],grad_r=0.85,
     fluid=['#061614','#0e5a55','#22b5a6','#cff7f0'],quad=['#18091a','#4e1a52','#b23ab8','#f3b0f0'],quad_bg='#120714'),
}
PAL.update(PAL2)
PAL['tertiary-b-light']=dict(bg='#f6f1e6',edge='#1d1b19',vox='#2a2724',tower='#2a2724',cloud='#1d1b19',grad=['#e08a00','#f2c97a','#f6f1e6'],grad_r=0.85,
     fluid=['#f6f1e6','#ade4dc','#2aa69a','#0d5e57'],quad=['#f6f1e6','#efcdee','#c860cb','#7a2480'],quad_bg='#f6f1e6')
import os
ONLY2=os.environ.get('ONLY2')=='1'
if __name__=='__main__':
    for k in (PAL2 if ONLY2 else PAL): topcube('t-'+k,PAL[k])
    topcube('t-ember-ice-dark-swapped',PAL['ember-ice-dark'],sides=('grad','fluid'))
    topcube('t-principia-light-swapped',PAL['principia-light'],sides=('grad','fluid'))
    for k in (PAL2 if ONLY2 else PAL): _FL[0]=0; scene('c-'+k,PAL[k],'centre')   # ids start at fl1 in every mark, as shipped (and as render.py does)
    if not ONLY2: pass
    for k in ([] if ONLY2 else PAL): scene('s-'+k,PAL[k],'sketch')
    scene('c-ember-ice-dark-noedges',PAL['ember-ice-dark'],'centre',edges=False)
    scene('c-principia-light-swapped',PAL['principia-light'],'centre',walls=('grad','fluid','quad'))
