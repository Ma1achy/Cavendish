import sys; sys.path.insert(0,'.')
from vox import *
OUT='out/'
LO='#2f2c28'
def save(name,body,vb,bg): open(OUT+name+'.svg','w',newline='\n').write(wrap(body,vb,bg))
# ---------- isometric ----------
def isoC(name,bg=CH,light=True,grad=None,tower=None,depth=1,octree=False,gradfill=False):
    s=10; n=5; pat=C5
    MX,MZ=3.4,2.0  # tower / mass in the mouth (voxel coords, centre)
    items=[];pts=[];defs=''
    T=(BONE,'#c8c1b2','#9c958a') if light else ('#4a463f',CH,'#2e2b27')
    def tone(cx,cz):
        r=math.dist((cx,cz),(MX,MZ+0.5)); g=min(1,(1.5/max(r,.01))**grad)
        base=mix(LO,BONE,g); return base,mix(base,'#000000',.2),mix(base,'#000000',.38)
    def add(x,y,z,sz):
        cx,cz=x+sz/2,z+sz/2
        t=tone(cx,cz) if grad else T
        if gradfill: t=('url(#gt)','url(#gl)','url(#gr)')
        items.append(((x+y+sz/2, z),cube(x,y,z,s,*t,size=sz)))
        for a in (0,sz):
            for b in (0,sz):
                for c in (0,sz): pts.append(iso(x+a,y+b,z+c,s))
    for (x,z) in cells(pat):
        for y in range(depth):
            if octree:
                d=math.dist((x+.5,z+.5),(MX,MZ+.5))
                k=4 if d<1.3 else 2 if d<2.3 else 1
                q=1/k
                for i in range(k):
                    for j in range(k):
                        for l in range(k): add(x+i*q+q*0.08,y+j*q+q*0.08,z+l*q+q*0.08,q*0.84)
            else: add(x,y,z,1)
    if gradfill:
        defs=('<defs>'
         f'<linearGradient id="gt" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="{LO}"/><stop offset="1" stop-color="{BONE}"/></linearGradient>'
         f'<linearGradient id="gl" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#22201d"/><stop offset="1" stop-color="#c8c1b2"/></linearGradient>'
         f'<linearGradient id="gr" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#bdb6a8"/><stop offset="1" stop-color="#4a463f"/></linearGradient></defs>')
    if tower:
        w=0.30
        if tower in ('side','sidegrad'): MXT=6.3; z0=-0.0; H=5.0
        else: MXT=MX; z0=1.0; H=3.0
        N=max(1,int(H/w))
        for k in range(N):
            z=z0+k*w; t=k/N
            tt=T if light else ('#4a463f',CH,'#2e2b27')
            if tower in ('grad','sidegrad'): c=mix('#55514a',BONE,t); tt=(c,mix(c,'#000000',.2),mix(c,'#000000',.38))
            items.append(((MXT+0.5+0.01,z),cube(MXT-w/2,0.5-w/2,z,s,*tt,size=w)))
            pts.append(iso(MXT,0.5,z,s))
        for z in (z0, z0+H-0.62):
            sz=0.62; items.append(((MXT+0.6,z),cube(MXT-sz/2,0.5-sz/2,z,s,*(('#ffffff','#e3ddcf','#c4bdae') if light else (CH,'#000','#000')),size=sz)))
            pts.append(iso(MXT,0.5,z+sz,s))
    items.sort(key=lambda t:t[0])
    save(name,defs+''.join(i[1] for i in items),square_vb(bounds(pts),s*1.1),bg)
# ---------- flat orthographic (front) ----------
def flat(name,mode,bg=CH,fg=BONE,tower=True,pat=C5):
    U=10; g=1.2; n=len(pat); body=''
    MX,MZ=3.6,2.0
    def Y(z): return (n-1-z)*U
    def sq(x,y,w,c): return f'<rect x="{x:.2f}" y="{y:.2f}" width="{w:.2f}" height="{w:.2f}" fill="{c}"/>'
    for (x,z) in cells(pat):
        if mode=='grid': body+=sq(x*U+g/2,Y(z)+g/2,U-g,fg)
        elif mode=='grad':
            r=math.dist((x+.5,z+.5),(MX,MZ+.5)); t=min(1,(1.5/r)**1.4)
            body+=sq(x*U+g/2,Y(z)+g/2,U-g,mix(LO if bg==CH else '#d8d1c2',fg,t))
        elif mode=='octree':
            d=math.dist((x+.5,z+.5),(MX,MZ+.5)); k=4 if d<1.3 else 2 if d<2.3 else 1
            q=U/k
            for i in range(k):
                for j in range(k):
                    gg=g if k==1 else 0.8
                    body+=sq(x*U+i*q+gg/2,Y(z)+j*q+gg/2,q-gg,fg)
        elif mode=='solid': body+=sq(x*U,Y(z),U,'url(#vg)')
    defs=''
    if mode=='solid':
        defs=f'<defs><linearGradient id="vg" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="{n*U}" y2="0"><stop offset="0" stop-color="{mix(fg,bg,0.72)}"/><stop offset="1" stop-color="{fg}"/></linearGradient></defs>'
    if tower=='side':
        tx=6.2*U; body+=f'<rect x="{tx-0.9:.2f}" y="3" width="1.8" height="{n*U-6:.2f}" fill="{fg}"/>'
        for ty in (0.6, n*U-7.4): body+=f'<rect x="{tx-3.4:.2f}" y="{ty:.2f}" width="6.8" height="6.8" fill="{fg}"/>'
    elif tower:
        tx=MX*U; body+=f'<rect x="{tx-0.9:.2f}" y="{U+2:.2f}" width="1.8" height="{3*U-4:.2f}" fill="{fg}"/>'
        for ty in (U+1.6, 4*U-8.4): body+=f'<rect x="{tx-3.4:.2f}" y="{ty:.2f}" width="6.8" height="6.8" fill="{fg}"/>'
    vb=(-U*0.9,-U*0.9,n*U+U*(3.2 if tower=='side' else 1.8),n*U+U*1.8)
    x0,y0,w,h=vb; sd=max(w,h); vb=(x0-(sd-w)/2,y0-(sd-h)/2,sd,sd)
    save(name,defs+body,vb,bg)
isoC('iso-01-plain')
isoC('iso-02-tower-mouth',tower=True)
isoC('iso-03-tower-side',tower='side')
isoC('iso-04-gradient-voxels',grad=1.4)
isoC('iso-05-gradient-voxels-tower',grad=1.4,tower=True)
isoC('iso-06-octree',octree=True)
isoC('iso-07-octree-tower',octree=True,tower=True)
isoC('iso-08-gradient-fill-side',gradfill=True,tower='sidegrad')
isoC('iso-09-light-tower',bg=BONE,light=False,tower=True)
isoC('iso-10-light-octree-side',bg=BONE,light=False,octree=True,tower='side')
isoC('iso-11-octree-gradient',octree=True,grad=1.4,tower=True)
flat('flat-01-grid-tower','grid')
flat('flat-02-gradient-tower','grad')
flat('flat-03-octree-tower','octree')
flat('flat-04-solid-gradient-side','solid',tower='side')
flat('flat-05-octree','octree',tower=False)
flat('flat-06-gradient','grad',tower=False)
flat('flat-07-light-octree-tower','octree',bg=BONE,fg=CH)
flat('flat-08-light-gradient-side','grad',bg=BONE,fg=CH,tower='side')
