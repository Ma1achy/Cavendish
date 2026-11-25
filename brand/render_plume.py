import numpy as np, matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.patches import Rectangle
from scipy.ndimage import zoom, gaussian_filter
from PIL import Image
import sys
rng=np.random.default_rng(5)
W,H=2656,998
d=np.load('work/plume.npz'); D=d['dye']; GY=int(d['GY']); h,w=D.shape
sy,sx=H/h,W/w
D=zoom(gaussian_filter(D,0.6),(sy,sx),order=3)[:H,:W].clip(0)
L=np.log1p(D/np.percentile(D,60)+1e-6); L/=np.percentile(L,99.7); L=L.clip(0,1)
gy=int(GY*sy)
MODE=sys.argv[1] if len(sys.argv)>1 else 'dark'
if MODE=='dark':
    PAPER=np.array([22,21,19])/255; CHALK=np.array([239,233,220])/255; INK='#efe9dc'; GROUND='#121110'; FILL='#1b1a18'; TW='#161513'
else:
    PAPER=np.array([239,233,220])/255; CHALK=np.array([29,27,25])/255; INK='#1d1b19'; GROUND='#e4ddcd'; FILL='#efe9dc'; TW='#efe9dc'
# base shading
tone=L**1.25
img=PAPER+(CHALK-PAPER)*tone[...,None]*0.82
# fine isolines (filaments)
fig=plt.figure(figsize=(W/100,H/100),dpi=100); ax=fig.add_axes([0,0,1,1]); ax.set_xlim(0,W); ax.set_ylim(H,0); ax.axis('off')
xs=np.arange(W); ys=np.arange(H)
Ls=gaussian_filter(L,1.2)
ax.contour(xs,ys,Ls,levels=np.linspace(0.06,0.98,34),colors=INK,linewidths=0.55,alpha=0.55)
# ground + voxel bodies at the heat sources, tower
ax.add_patch(Rectangle((0,gy),W,H-gy,facecolor=GROUND,edgecolor='none',zorder=2))
for x in np.arange(-400,W,18): ax.plot([x,x+300],[H,gy],color=INK,lw=0.5,alpha=0.18,zorder=2.5)
ax.plot([0,W],[gy,gy],color=INK,lw=1.6,zorder=4)
V=16
def stack(x,hs,fill=None):
    fill=fill or FILL
    for i,hh in enumerate(hs):
        for j in range(hh):
            ax.add_patch(Rectangle((x+i*V,gy-(j+1)*V),V,V,facecolor=fill,edgecolor=INK,lw=0.8,zorder=5))
srcs=[0.06,0.30,0.335,0.52,0.66,0.83,0.95]
for fx in srcs:
    cx=fx*W; stack(cx-V/2,[3])            # warm bodies
stack(0.12*W,[5,5,7,7,7,4]); stack(0.58*W,[4,4,4]); stack(0.70*W,[9,9,10,10,9]); stack(0.88*W,[3,3,6,6,6,3,3])
TX=0.43*W
ax.add_patch(Rectangle((TX-7,90),14,gy-90,facecolor=TW,edgecolor=INK,lw=1.2,zorder=6))
for i,y in enumerate(np.linspace(125,gy-35,10)):
    s=11 if i in (0,9) else 5
    ax.add_patch(Rectangle((TX-s,y-s),2*s,2*s,facecolor=INK if i in (0,9) else TW,edgecolor=INK,lw=1.1,zorder=7))
ax.plot([TX,TX],[40,90],color=INK,lw=0.8,zorder=6)
fig.savefig('work/plink.png',dpi=100,transparent=True); plt.close(fig)
ink=np.asarray(Image.open('work/plink.png').convert('RGBA')).astype(float)/255
tooth=np.clip(0.7+gaussian_filter(rng.normal(0,1,(H,W)),0.6)*0.7,0.15,1)
a=ink[...,3]*(tooth if True else 1)[...]  # pencil tooth on pale strokes only
img=img*(1-a[...,None])+ink[...,:3]*a[...,None]
# paper grain + pencil texture over tone
grain=gaussian_filter(rng.normal(0,1,(H,W)),0.8)*0.05+gaussian_filter(rng.normal(0,1,(H,W)),25)*0.05
img=img*(1+grain[...,None])
yy,xx=np.mgrid[0:H,0:W]; v=1-0.18*(((xx-W/2)/(W/2))**2+((yy-H/2)/(H/2))**2)
img*=v[...,None]
im=Image.fromarray((np.clip(img,0,1)*255).astype(np.uint8))
top=30; bot=gy+120; ch=bot-top; cw=int(round(ch*W/H)); cx=int(0.47*W); x0=max(0,min(W-cw,cx-cw//2))
im=im.crop((x0,top,x0+cw,bot)).resize((W,H),Image.LANCZOS)
im.save(sys.argv[2],quality=88)
