import numpy as np, pickle, matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.patches import Rectangle
from scipy.ndimage import gaussian_filter, zoom
from PIL import Image, ImageFilter
rng=np.random.default_rng(3)
W,H=2656,998
pot=np.load('work/pot.npy'); vox=pickle.load(open('work/vox.pkl','rb'))
h,w=pot.shape
xs=np.linspace(0,W,w); ys=np.linspace(0,H,h)
INK='#24221e'
fig=plt.figure(figsize=(W/100,H/100),dpi=100); ax=fig.add_axes([0,0,1,1]); ax.set_xlim(0,W); ax.set_ylim(H,0); ax.axis('off')
fig.patch.set_alpha(0)
levels=np.arange(-40,1040,9.0)
cs=ax.contour(xs,ys,pot,levels=levels,colors=INK,linewidths=0.9,alpha=0.85)
# every 5th line heavier
ax.contour(xs,ys,pot,levels=levels[::5],colors=INK,linewidths=1.8,alpha=0.9)
# voxels: paper-filled cells with ink edges, hiding lines behind (solid matter)
for x0,y0,x1,y1,d in vox:
    ax.add_patch(Rectangle((x0,y0),x1-x0,y1-y0,facecolor='#e9e2d3' if d<1.2 else '#d7cfbe',edgecolor=INK,linewidth=1.1,zorder=3))
# ground
ax.fill_between([0,W],[760,760],[H,H],color='#e9e2d3',zorder=2)
ax.plot([0,W],[760,760],color=INK,lw=2.2,zorder=4)
# ground hatching
for x in np.arange(-300,W,22): ax.plot([x,x+240],[H,760],color=INK,lw=0.6,alpha=0.35,zorder=2.5)
# AION-style tower: vertical baseline with atom clouds
TX=700
ax.add_patch(Rectangle((TX-9,110),18,650,facecolor='#efe9dc',edgecolor=INK,lw=1.4,zorder=5))
for i,y in enumerate(np.linspace(150,720,9)):
    if i in (0,8): ax.add_patch(Rectangle((TX-14,y-14),28,28,facecolor=INK,edgecolor=INK,zorder=6))
    else: ax.add_patch(Rectangle((TX-6,y-6),12,12,facecolor='#efe9dc',edgecolor=INK,lw=1.4,zorder=6))
# laser line
ax.plot([TX,TX],[60,110],color=INK,lw=1.0,zorder=5)
fig.savefig('work/ink.png',dpi=100,transparent=True); plt.close(fig)
ink=np.asarray(Image.open('work/ink.png').convert('RGBA')).astype(float)/255
# paper
paper=np.ones((H,W,3))*np.array([239,233,220])/255
grain=gaussian_filter(rng.normal(0,1,(H,W)),0.7)*0.025 + gaussian_filter(rng.normal(0,1,(H,W)),18)*0.06
paper*= (1+grain)[...,None]
# pencil: modulate ink alpha by tooth noise
tooth=np.clip(0.75+gaussian_filter(rng.normal(0,1,(H,W)),0.6)*0.6,0.2,1)
a=ink[...,3]*tooth
out=paper*(1-a[...,None])+ink[...,:3]*a[...,None]
# vignette
yy,xx=np.mgrid[0:H,0:W]; v=1-0.10*(((xx-W/2)/(W/2))**2+((yy-H/2)/(H/2))**2)
out*=v[...,None]
Image.fromarray((np.clip(out,0,1)*255).astype(np.uint8)).save('out/hero.jpg',quality=88)
