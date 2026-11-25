import numpy as np, matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.patches import Rectangle
from matplotlib.collections import PatchCollection, LineCollection
from scipy.ndimage import zoom, gaussian_filter
from PIL import Image
rng=np.random.default_rng(5)
W,H=2656,998
d=np.load('work/plume.npz'); D=d['dye']; GY=int(d['GY']); h,w=D.shape
sy,sx=H/h,W/w
D=zoom(gaussian_filter(D,0.6),(sy,sx),order=3)[:H,:W].clip(0)
L=np.log1p(D/np.percentile(D,60)+1e-6); L/=np.percentile(L,99.7); L=L.clip(0,1)
gy=int(GY*sy)
PAPER=np.array([22,21,19])/255; CHALK=np.array([239,233,220])/255; INK='#efe9dc'
# ---- octree over the air, refined on plume structure ----
G=np.hypot(*np.gradient(gaussian_filter(L,2)))
cells=[]
def split(x,y,s,dep):
    y1=min(y+s,gy); 
    if y>=gy: return
    blk=G[int(y):int(y1),int(x):int(x+s)]; f=L[int(y):int(y1),int(x):int(x+s)]
    if blk.size==0: return
    if dep<7 and s>7 and (blk.max()*s>3.0 or (f.max()-f.min())>0.38): 
        hs=s/2
        for dx in (0,hs):
            for dy in (0,hs): split(x+dx,y+dy,hs,dep+1)
    else: cells.append((x,y,s,min(s,gy-y),float(f.mean())))
R=gy/2
for x in np.arange(0,W,R):
    for y in (0,R): split(x,y,R,0)
# voxelised field: cell-mean tone, blended in with a ramp so the right side is 'seen by the engine'
Q=np.zeros_like(L)
for x,y,s,hh,m in cells: Q[int(y):int(y+hh),int(x):int(x+s)]=m
xx=np.arange(W)[None,:]
ramp=np.clip((xx-0.30*W)/(0.45*W),0,1)**1.3
F=L*(1-0.85*ramp)+Q*0.85*ramp
tone=F**1.2
img=PAPER+(CHALK-PAPER)*tone[...,None]*0.82
fig=plt.figure(figsize=(W/100,H/100),dpi=100); ax=fig.add_axes([0,0,1,1]); ax.set_xlim(0,W); ax.set_ylim(H,0); ax.axis('off')
# fine isolines of the smooth field, fading out as the grid takes over
xs=np.arange(W); ys=np.arange(H)
Ls=gaussian_filter(L,1.2)
cs=ax.contour(xs,ys,Ls,levels=np.linspace(0.06,0.98,30),colors=INK,linewidths=0.5,alpha=0.5)
# octree edges: alpha rises with the ramp and with depth fineness
segs=[];alphas=[];lws=[]
for x,y,s,hh,m in cells:
    r=float(np.clip((x+s/2-0.30*W)/(0.45*W),0,1))**1.0
    a=0.13+0.42*r
    segs+= [[(x,y),(x+s,y)],[(x,y),(x,y+hh)]]
    alphas+=[a,a]; lws+=[0.35 if s<20 else 0.6 if s<80 else 0.9]*2
lc=LineCollection(segs,colors=[(0.937,0.914,0.863,a) for a in alphas],linewidths=lws,zorder=3)
ax.add_collection(lc)
# ---- voxel terrain: subsurface cross-section on an adaptive grid with density shading ----
V=16
depth=H-gy
soil=[]
def sub(x,y,s,dep):
    # refine near the surface and near density anomalies
    near=(y-gy)<s*1.2
    anom=any(abs(x+s/2-c)<s+w0 and (y-gy)<d0 for c,w0,d0 in [(0.18*W,140,200),(0.62*W,220,160),(0.9*W,120,240)])
    if dep<4 and s>V and ((near and dep<2) or anom):
        hs=s/2
        for dx in (0,hs):
            for dy in (0,hs): sub(x+dx,y+dy,hs,dep+1)
    else: soil.append((x,y,s))
S0=V*16
for x in np.arange(0,W,S0):
    for y in np.arange(gy,H+S0,S0): sub(x,y,S0,0)
def dens(x,y):
    v=0.05+0.03*np.sin(x/90)+0.02*np.cos(y/40)
    for c,wd,dd,amp in [(0.18*W,170,230,0.42),(0.62*W,240,170,-0.05),(0.9*W,150,260,0.36)]:
        v+=amp*np.exp(-((x-c)/wd)**2-((y-gy)/dd)**2)
    return float(np.clip(v,0,0.6))
pc=[Rectangle((x,y),s,s) for x,y,s in soil]
ax.add_collection(PatchCollection(pc,facecolors=[(0.937,0.914,0.863,dens(x+s/2,y+s/2)) for x,y,s in soil],edgecolors=(0.937,0.914,0.863,0.30),linewidths=[0.4 if s<=V else 0.7 for x,y,s in soil],zorder=2))
# ground surface: voxel heightmap (small mounds) on top of gy
hm=(np.round(1.6*np.sin(np.arange(0,W,V)/170+0.5)+1.1*np.sin(np.arange(0,W,V)/61+1)).clip(0)).astype(int)
for i,x in enumerate(np.arange(0,W,V)):
    for j in range(hm[i]):
        ax.add_patch(Rectangle((x,gy-(j+1)*V),V,V,facecolor='#22201d',edgecolor=INK,lw=0.6,zorder=5))
ax.plot([0,W],[gy,gy],color=INK,lw=1.4,zorder=6)
def stack(x,hs,fill='#1b1a18'):
    base=gy-hm[min(len(hm)-1,int(x//V))]*V
    for i,hh in enumerate(hs):
        for j in range(hh):
            ax.add_patch(Rectangle((x+i*V,base-(j+1)*V),V,V,facecolor=fill,edgecolor=INK,lw=0.8,zorder=7))
srcs=[0.06,0.30,0.335,0.52,0.66,0.83,0.95]
for fx in srcs: stack(fx*W-V/2,[3],'#3a3631')
stack(0.12*W,[5,5,7,7,7,4]); stack(0.58*W,[4,4,4]); stack(0.70*W,[9,9,10,10,9]); stack(0.88*W,[3,3,6,6,6,3,3])
# tower: from above ground down a shaft into the voxel soil
TX=0.43*W; top=90; bot=gy+170
ax.add_patch(Rectangle((TX-7,top),14,bot-top,facecolor='#161513',edgecolor=INK,lw=1.2,zorder=8))
for i,y in enumerate(np.linspace(top+35,bot-35,11)):
    s=11 if i in (0,10) else 5
    ax.add_patch(Rectangle((TX-s,y-s),2*s,2*s,facecolor=INK if i in (0,10) else '#161513',edgecolor=INK,lw=1.1,zorder=9))
ax.plot([TX,TX],[40,top],color=INK,lw=0.8,zorder=8)
fig.savefig('work/h2ink.png',dpi=100,transparent=True); plt.close(fig)
ink=np.asarray(Image.open('work/h2ink.png').convert('RGBA')).astype(float)/255
tooth=np.clip(0.7+gaussian_filter(rng.normal(0,1,(H,W)),0.6)*0.7,0.15,1)
a=ink[...,3]*(0.55+0.45*tooth)
# dark ground body under the ink
img[gy:]=PAPER*0.85
img=img*(1-a[...,None])+ink[...,:3]*a[...,None]
grain=gaussian_filter(rng.normal(0,1,(H,W)),0.8)*0.05+gaussian_filter(rng.normal(0,1,(H,W)),25)*0.05
img=img*(1+grain[...,None])
yy,xx2=np.mgrid[0:H,0:W]; img*=(1-0.18*(((xx2-W/2)/(W/2))**2+((yy-H/2)/(H/2))**2))[...,None]
im=Image.fromarray((np.clip(img,0,1)*255).astype(np.uint8))
top=30; bot=min(H,gy+200); ch=bot-top; cw=int(round(ch*W/H)); cx=int(0.47*W); x0=max(0,min(W-cw,cx-cw//2))
im=im.crop((x0,top,x0+cw,bot)).resize((W,H),Image.LANCZOS)
im.save('out/hero.jpg',quality=88)
print(len(cells),len(soil))
