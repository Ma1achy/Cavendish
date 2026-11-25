import sys, math; sys.path.insert(0,'.')
from vox import *
OUT='out2/'
LO='#2f2c28'; WHITE=('#ffffff','#e3ddcf','#c4bdae')
def shade(c): return (c,mix(c,'#000000',.2),mix(c,'#000000',.38))
PL=shade(BONE)
def render(name,vox,tower=None,ghost=None,bg=CH,dark_on_light=False,s=10,pad=1.1,extra_pts=()):
    items=[];pts=list(extra_pts)
    for (x,y,z,sz,col) in vox:
        items.append((x+y+z+1.5*sz, cube(x,y,z,s,*col,size=sz)))
        for a in (0,sz):
            for b in (0,sz):
                for c in (0,sz): pts.append(iso(x+a,y+b,z+c,s))
    if tower:
        tx,ty,z0,H,w,col,cloud=tower['x'],tower['y'],tower['z0'],tower['h'],tower.get('w',0.3),tower.get('col',PL),tower.get('cloud',WHITE)
        N=max(1,round(H/w))
        for k in range(N):
            z=z0+k*(H/N); c=col(k/(N-1)) if callable(col) else col
            items.append((tx+ty+z+w*1.5+tower.get('bias',0), cube(tx-w/2,ty-w/2,z,s,*c,size=H/N if False else w)))
            pts.append(iso(tx,ty,z,s))
        cs=0.64
        for z in (z0-cs*0.2, z0+H-cs*0.8):
            items.append((tx+ty+z+cs*1.5+tower.get('bias',0)+0.01, cube(tx-cs/2,ty-cs/2,z,s,*cloud,size=cs)))
            pts.append(iso(tx,ty,z+cs,s)); pts.append(iso(tx,ty,z,s))
    items.sort(key=lambda t:t[0])
    g=''
    if ghost:
        (X0,Y0,Z0),(X1,Y1,Z1)=ghost
        P=lambda a,b,c: iso(a,b,c,s)
        E=[((X0,Y1,Z1),(X1,Y1,Z1)),((X1,Y0,Z1),(X1,Y1,Z1)),((X1,Y1,Z0),(X1,Y1,Z1)),((X0,Y0,Z1),(X1,Y0,Z1)),((X0,Y0,Z1),(X0,Y1,Z1)),
           ((X0,Y1,Z0),(X1,Y1,Z0)),((X1,Y0,Z0),(X1,Y1,Z0)),((X0,Y1,Z0),(X0,Y1,Z1)),((X1,Y0,Z0),(X1,Y0,Z1))]
        st=('#efe9dc' if not dark_on_light else CH)
        g='<g fill="none" stroke="'+st+'" stroke-opacity="0.28" stroke-width="0.6" stroke-dasharray="1.6 1.6">'+''.join(f'<path d="M{P(*a)[0]:.2f} {P(*a)[1]:.2f} L{P(*b)[0]:.2f} {P(*b)[1]:.2f}"></path>' for a,b in E)+'</g>'
        for a,b in E: pts+= [P(*a),P(*b)]
    open(OUT+name+'.svg','w',newline='\n').write(wrap(g+''.join(i[1] for i in items),square_vb(bounds(pts),s*pad),bg))
C=C5
def ground_C(mode='plain',col=PL):
    v=[]
    MX,MY=3.6,2.5
    for r,row in enumerate(C):
        for c,ch in enumerate(row):
            if ch!='#': continue
            x,y=c,r
            if mode=='grad':
                d=math.dist((x+.5,y+.5),(MX,MY)); t=min(1,(1.5/d)**1.4); v.append((x,y,0,1,shade(mix(LO,BONE,t))))
            elif mode=='octree':
                d=math.dist((x+.5,y+.5),(MX,MY)); k=4 if d<1.3 else 2 if d<2.3 else 1; q=1/k
                for i in range(k):
                    for j in range(k):
                        for l in range(k): v.append((x+i*q+q*.08,y+j*q+q*.08,l*q+q*.08,q*.84,col))
            else: v.append((x,y,0,1,col))
    return v,(MX,MY)
grad_tower=lambda t: shade(mix('#5a554d',BONE,t))
# 1-3 ground C, tower rising
v,(mx,my)=ground_C(); render('p01-ground-tower',v,dict(x=mx,y=my,z0=0.0,h=5.5))
v,(mx,my)=ground_C('grad'); render('p02-ground-gradient-tower',v,dict(x=mx,y=my,z0=0.0,h=5.5))
v,(mx,my)=ground_C('octree'); render('p03-ground-octree-tower',v,dict(x=mx,y=my,z0=0.0,h=5.5))
v,(mx,my)=ground_C(); render('p04-ground-graded-tower',v,dict(x=mx,y=my,z0=0.0,h=5.5,col=grad_tower))
# 5 shaft going down from the surface
v,(mx,my)=ground_C(); render('p05-ground-shaft',v,dict(x=mx,y=my,z0=-5.0,h=6.0,bias=-20))
v,(mx,my)=ground_C('octree'); render('p06-ground-octree-shaft',v,dict(x=mx,y=my,z0=-5.0,h=6.0,bias=-20))
# 7-9 C on left (+y) face of a 5-cube, tower on right (+x) face
def wall_C(mode='plain',col=PL):
    v=[]; MX,MZ=4.6,2.5
    for r,row in enumerate(C):
        for c,ch in enumerate(row):
            if ch!='#': continue
            x,z=c,4-r
            if mode=='grad':
                d=math.dist((x+.5,z+.5),(MX+0.5,MZ)); t=min(1,(1.7/d)**1.4); v.append((x,4,z,1,shade(mix(LO,BONE,t))))
            elif mode=='octree':
                d=math.dist((x+.5,z+.5),(MX+.4,MZ)); k=4 if d<1.6 else 2 if d<2.6 else 1; q=1/k
                for i in range(k):
                    for l in range(k): v.append((x+i*q+q*.08,4+(1-q)+q*.08,z+l*q+q*.08,q*.84,col))
            else: v.append((x,4,z,1,col))
    return v
cube5=((0,0,0),(5,5,5))
render('p07-faces-ghost',wall_C(),dict(x=5.25,y=2.5,z0=0.3,h=4.4,bias=5),ghost=cube5)
render('p08-faces-gradient-ghost',wall_C('grad'),dict(x=5.25,y=2.5,z0=0.3,h=4.4,bias=5,col=grad_tower),ghost=cube5)
render('p09-faces-octree-ghost',wall_C('octree'),dict(x=5.25,y=2.5,z0=0.3,h=4.4,bias=5),ghost=cube5)
render('p10-faces-octree',wall_C('octree'),dict(x=5.25,y=2.5,z0=0.3,h=4.4,bias=5))
# 11 C on top face of the cube, tower down the right face (surface + shaft on the cube)
v,(mx,my)=ground_C('octree'); v=[(x,y,z+5-1 if True else z,sz,c) for (x,y,z,sz,c) in v]
render('p11-top-octree-right-shaft',v,dict(x=5.25,y=2.5,z0=0.3,h=4.4,bias=5),ghost=cube5)
# 12 light versions of the best two
DK=shade('#3a3731')
v,(mx,my)=ground_C('octree',DK); render('p12-light-ground-octree-tower',v,dict(x=mx,y=my,z0=0.0,h=5.5,col=DK,cloud=shade(CH)),bg=BONE,dark_on_light=True)
render('p13-light-faces-octree-ghost',wall_C('octree',DK),dict(x=5.25,y=2.5,z0=0.3,h=4.4,bias=5,col=DK,cloud=shade(CH)),ghost=cube5,bg=BONE,dark_on_light=True)
def render_panel(name,vox,tower,axis='h',ghost=True,bg=CH,ink='#efe9dc',op=0.42,pad=1.1,s=10):
    # draw the +x face of the 5-cube as a gradient panel, then the rest
    P=lambda a,b,c: iso(a,b,c,s)
    face=[P(5,0,0),P(5,5,0),P(5,5,5),P(5,0,5)]
    if axis=='h': a,b=P(5,5,2.5),P(5,0,2.5)     # bright at shared edge, fading away from the C
    else: a,b=P(5,2.5,0),P(5,2.5,5)              # vertical
    gid='pg'+name[-3:].replace('-','')
    panel=(f'<defs><linearGradient id="{gid}" gradientUnits="userSpaceOnUse" x1="{a[0]:.2f}" y1="{a[1]:.2f}" x2="{b[0]:.2f}" y2="{b[1]:.2f}">'
           f'<stop offset="0" stop-color="{ink}" stop-opacity="{op}"></stop><stop offset="1" stop-color="{ink}" stop-opacity="0"></stop></linearGradient></defs>'
           '<path d="M'+' L'.join(f'{x:.2f} {y:.2f}' for x,y in face)+f'Z" fill="url(#{gid})"></path>')
    render(name,vox,tower,ghost=cube5 if ghost else None,bg=bg,dark_on_light=(bg==BONE),pad=pad,s=s)
    f=OUT+name+'.svg'; t=open(f).read()
    i=t.index('>',t.index('<svg'))+1
    # insert after background rect if present
    if t[i:].startswith('<rect'): i=t.index('>',i)+1
    open(f,'w',newline='\n').write(t[:i]+panel+t[i:])
render_panel('p14-faces-panel-h',wall_C(),dict(x=5.25,y=2.5,z0=0.3,h=4.4,bias=5))
render_panel('p15-faces-octree-panel-h',wall_C('octree'),dict(x=5.25,y=2.5,z0=0.3,h=4.4,bias=5))
render_panel('p16-faces-octree-panel-h-noghost',wall_C('octree'),dict(x=5.25,y=2.5,z0=0.3,h=4.4,bias=5),ghost=False)
render_panel('p17-faces-gradient-panel-v',wall_C('grad'),dict(x=5.25,y=2.5,z0=0.3,h=4.4,bias=5),axis='v')
render_panel('p18-light-faces-octree-panel',wall_C('octree',DK),dict(x=5.25,y=2.5,z0=0.3,h=4.4,bias=5,col=DK,cloud=shade(CH)),bg=BONE,ink=CH,op=0.30)
