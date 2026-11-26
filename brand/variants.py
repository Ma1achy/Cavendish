import sys; sys.path.insert(0,'.')
from vox import *
OUT='out/'; import os; os.makedirs(OUT,exist_ok=True)
def tones(fg_dark):
    # three face tones for an object on a tile; fg_dark=True => charcoal object on bone
    if fg_dark: return ('#4a463f',CH,'#2e2b27')
    return (BONE,'#c8c1b2','#9c958a')
def iso_C(pat=C5,s=10,depth=1,tower=False,grad=None,bg=CH,light=True,name='x',mass=None,thin=False):
    T=tones(not light)
    cs=[(x,y,z) for (x,z) in cells(pat) for y in range(depth)]
    n=len(pat)
    mx,mz=(n-1)*0.72,(n-1)/2  # mass point in the mouth
    body=[]; pts=[]
    items=[]
    for (x,y,z) in cs:
        if grad:
            r=math.dist((x+.5,z+.5),(mx+.5,mz+.5)); g=min(1,(1.6/r)**grad['p'])
            top=mix(grad['lo'],grad['hi'],g); left=mix(mix(grad['lo'],grad['hi'],g),'#000000',0.18); right=mix(mix(grad['lo'],grad['hi'],g),'#000000',0.36)
        else: top,left,right=T
        items.append(((x+y,z),cube(x,y,z,s,top,left,right)))
        for a in (0,1):
            for b in (0,1):
                for c in (0,1): pts.append(iso(x+a,y+b,z+c,s))
    if tower:
        tx,ty=mx+0.3,0.3
        H=n+2.0; w=0.4
        for k in range(int(H/w)):
            z=-1+k*w
            hi=(k==0 or k==int(H/w)-1)
            tt=(BONE,'#c8c1b2','#9c958a') if light else ('#4a463f',CH,'#2e2b27')
            if tower=='grad':
                t=k/(H/w); tt=(mix('#6f6a60',BONE,1-t),mix('#5a554d','#c8c1b2',1-t),mix('#403d37','#9c958a',1-t))
            if hi: tt=('#ffffff','#e6e0d2','#cfc8b8') if light else (CH,CH,CH)
            sz=w*1.6 if hi else w
            off=(w-sz)/2
            items.append(((tx+ty+0.01,z),cube(tx+off,ty+off,z,s,*tt,size=sz)))
            for c in (0,1): pts.append(iso(tx,ty,z+c*w,s))
    if mass:
        mxx,mzz=mass
        items.append(((mxx+0.2,mzz),cube(mxx,0.2,mzz,s,*(('#ffffff','#e6e0d2','#cfc8b8') if light else T),size=0.9)))
    items.sort(key=lambda t:t[0])
    body=''.join(i[1] for i in items)
    vb=square_vb(bounds(pts),s*1.2)
    open(OUT+name+'.svg','w',newline='\n').write(wrap(body,vb,bg))
# 1 plain iso voxel C
iso_C(name='01-iso-c')
iso_C(C7,s=8,name='02-iso-c7')
iso_C(depth=2,name='03-iso-c-deep')
# with tower
iso_C(tower=True,name='04-iso-tower')
iso_C(tower='grad',name='05-iso-tower-grad')
# gradient voxels (tone ~ 1/r^p from mass in mouth)
iso_C(grad=dict(lo='#3a3731',hi=BONE,p=1.5),name='06-iso-grad')
iso_C(C7,s=8,grad=dict(lo='#3a3731',hi=BONE,p=1.5),tower=True,name='07-iso-c7-grad-tower')
iso_C(grad=dict(lo='#3a3731',hi=BONE,p=1.5),mass=(3.0,1.8),name='08-iso-grad-mass')
iso_C(bg=BONE,light=False,tower=True,name='09-iso-tower-light')
