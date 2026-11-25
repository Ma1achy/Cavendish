import numpy as np, time
from scipy.ndimage import map_coordinates, gaussian_filter
rng=np.random.default_rng(11)
w,h=832,312  # sim grid; render 3.2x
u=np.zeros((h,w)); v=np.zeros((h,w)); T=np.zeros((h,w)); dye=np.zeros((h,w))
GY=int(h*0.80)  # ground row (y down)
yy,xx=np.mgrid[0:h,0:w].astype(float)
kx=np.fft.fftfreq(w)[None,:]*2*np.pi; ky=np.fft.fftfreq(h)[:,None]*2*np.pi
k2=kx**2+ky**2; k2[0,0]=1
def adv(f,dt):
    return map_coordinates(f,[yy-dt*v,xx-dt*u],order=1,mode='wrap')
def project(u,v):
    U=np.fft.fft2(u); V=np.fft.fft2(v)
    div=(kx*U+ky*V)/k2
    U-=kx*div; V-=ky*div
    return np.real(np.fft.ifft2(U)),np.real(np.fft.ifft2(V))
sources=[(0.06,1.0),(0.30,1.4),(0.335,1.0),(0.52,1.6),(0.66,1.1),(0.83,1.5),(0.95,0.9)]
src=np.zeros((h,w))
for fx,s in sources:
    cx=fx*w; src+=s*np.exp(-((xx-cx)/5.5)**2-((yy-(GY-3))/3)**2)
solid=yy>=GY
dt=0.8
t0=time.time()
for it in range(700):
    jitter=1+0.35*np.sin(it*0.05+np.arange(len(sources)))  # unsteady sources
    T+=dt*0.12*src*(1+0.3*rng.normal(0,1,src.shape))
    dye+=dt*0.12*src
    v-=dt*0.05*T  # buoyancy (y down => negative v rises)
    # vorticity confinement
    wz=np.gradient(v,axis=1)-np.gradient(u,axis=0)
    aw=np.abs(wz); gx=np.gradient(aw,axis=1); gy=np.gradient(aw,axis=0); n=np.hypot(gx,gy)+1e-6
    eps=0.35
    u+=dt*eps*(gy/n)*wz; v+=dt*eps*(-gx/n)*wz
    u=adv(u,dt); v=adv(v,dt); T=adv(T,dt)*0.996; dye=adv(dye,dt)*0.998
    u[solid]=0; v[solid]=0
    # top: cool and fade
    T[:6]*=0.8; dye[:4]*=0.9
    u,v=project(u,v)
    u[solid]=0; v[solid]=0
    if it%100==0: print(it, round(time.time()-t0,1), float(T.max()), float(np.abs(v).max()),flush=True)
np.savez('work/plume.npz',T=T,dye=dye,u=u,v=v,GY=GY)
