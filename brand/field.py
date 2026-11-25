import numpy as np, matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from scipy.signal import fftconvolve
from scipy.ndimage import gaussian_filter
from PIL import Image
rng=np.random.default_rng(7)
W,H=2656,998; S=2   # compute scale
w,h=W//S,H//S
rho=np.zeros((h,w))
GROUND=760//S
vox=[]  # (x0,y0,x1,y1) in full-res px for drawing
def block(x0,y0,x1,y1,d=1.0):
    rho[y0//S:y1//S, x0//S:x1//S]+=d; vox.append((x0,y0,x1,y1,d))
V=24  # voxel size px
# buildings: stepped voxel stacks
def stack(x,heights,d=1.0):
    for i,hh in enumerate(heights):
        for j in range(hh):
            block(x+i*V, 760-(j+1)*V, x+(i+1)*V, 760-j*V, d)
stack(1080,[6,6,9,9,9,7,7,4,4])
stack(1560,[3,3,3,3])
stack(1880,[13,13,14,14,13,13])
stack(2330,[5,5,8,8,8,5,5,5,5,5,5,5,5])
stack(120,[4,4,4,2,2,2])
# people / bodies
for x in [880,940,1420,1760,2230]:
    stack(x,[3],1.4)
# terrain undulation below ground (density anomaly bumps)
xs=np.arange(w)
rho[GROUND:,:]+=0.0
for cx,amp,wd in [(500,40,260),(1400,-30,300),(2200,35,220)]:
    prof=(amp*np.exp(-((xs*S-cx)/wd)**2))/S
    for i in range(w):
        a=int(abs(prof[i]))
        if prof[i]>0: rho[GROUND:GROUND+a,i]+=0.8
        else: rho[GROUND:GROUND+a,i]-=0.8
# thermal plumes: warm air rising = small negative density anomaly
yy,xx=np.mgrid[0:h,0:w]*S
for px in [960,1500,2230]:
    plume=np.exp(-((xx-px- 40*np.sin(yy/90))/(30+0.12*(760-yy).clip(0)))**2)*((yy<760)&(yy>80))*np.exp(-(760-yy).clip(0)/700)
    rho-=0.25*plume
# kernel 1/r
k=np.mgrid[-h:h+1,-w:w+1].astype(float); r=np.hypot(k[0],k[1])+0.8
phi=-fftconvolve(rho,1/r,mode='same')
pot=phi*0.0185 + (np.arange(h)[:,None]*S)*1.0   # uniform field + anomaly (exaggerated)
pot=gaussian_filter(pot,0.6)
np.save('work/pot.npy',pot)
import pickle; pickle.dump(vox,open('work/vox.pkl','wb'))
print(pot.min(),pot.max())
