import numpy as np, matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from scipy.ndimage import zoom, gaussian_filter
from PIL import Image
rng=np.random.default_rng(2)
W,H=2656,664
d=np.load('work/plume.npz'); D=d['dye']; GY=int(d['GY']); h,w=D.shape
# take the band around the tower region, full width, rows 40..GY+10
band=D[40:GY+8,:]
F=zoom(gaussian_filter(band,0.8),(H/band.shape[0],W/band.shape[1]),order=3).clip(0)
F/=np.percentile(F,99.5); F=F.clip(0,1)
G=np.hypot(*np.gradient(gaussian_filter(F,2)))
cells=[]
def split(x,y,s,depth):
    blk=G[int(y):int(y+s),int(x):int(x+s)]; f=F[int(y):int(y+s),int(x):int(x+s)]
    if blk.size==0: return
    if depth<7 and s>6 and (blk.max()*s>0.9 or (f.max()-f.min())>0.25):
        h2=s/2
        for dx in (0,h2):
            for dy in (0,h2): split(x+dx,y+dy,h2,depth+1)
    else: cells.append((x,y,s,float(f.mean())))
S=H/1  # root cells of size H? use 332 roots
R=332
for x in np.arange(0,W,R):
    for y in np.arange(0,H,R): split(x,y,R,0)
INK='#1d1b19'
fig=plt.figure(figsize=(W/100,H/100),dpi=100); ax=fig.add_axes([0,0,1,1]); ax.set_xlim(0,W); ax.set_ylim(H,0); ax.axis('off')
fig.patch.set_facecolor('#efe9dc')
from matplotlib.patches import Rectangle
from matplotlib.collections import PatchCollection
rects=[Rectangle((x,y),s,s) for x,y,s,m in cells]
cols=[(0.114,0.106,0.098, 0.82*m**1.2) for x,y,s,m in cells]
ax.add_collection(PatchCollection(rects,facecolors=cols,edgecolors='none'))
ax.add_collection(PatchCollection([Rectangle((x,y),s,s) for x,y,s,m in cells],facecolors='none',edgecolors=INK,linewidths=[0.35 if s<20 else 0.6 if s<80 else 1.0 for x,y,s,m in cells],alpha=0.75))
fig.savefig('work/oct.png',dpi=100,facecolor='#efe9dc'); plt.close(fig)
img=np.asarray(Image.open('work/oct.png').convert('RGB')).astype(float)/255
grain=gaussian_filter(rng.normal(0,1,(H,W)),0.8)*0.035+gaussian_filter(rng.normal(0,1,(H,W)),25)*0.04
img*=1+grain[...,None]
yy,xx=np.mgrid[0:H,0:W]; img*=(1-0.10*(((xx-W/2)/(W/2))**2+((yy-H/2)/(H/2))**2))[...,None]
Image.fromarray((img.clip(0,1)*255).astype(np.uint8)).save('out/banner-octree.jpg',quality=88)
print(len(cells))
