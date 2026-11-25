import numpy as np
from scipy.ndimage import map_coordinates, gaussian_filter
n=192; h=w=n
u=np.zeros((h,w)); v=np.zeros((h,w)); T=np.zeros((h,w)); D=np.zeros((h,w))
yy,xx=np.mgrid[0:h,0:w].astype(float)
kx=np.fft.fftfreq(w)[None,:]*2*np.pi; ky=np.fft.fftfreq(h)[:,None]*2*np.pi; k2=kx**2+ky**2; k2[0,0]=1
src=np.exp(-((xx-w*0.5)/10)**2-((yy-(h-14))/4)**2)
rng=np.random.default_rng(4)
for it in range(320):
    jit=1+0.6*rng.normal(0,1,src.shape); T+=0.11*src*jit; D+=0.10*src; u+=0.25*src*rng.normal(0,1)
    v-=0.045*T
    wz=np.gradient(v,axis=1)-np.gradient(u,axis=0); aw=np.abs(wz); gx=np.gradient(aw,axis=1); gy=np.gradient(aw,axis=0); nn=np.hypot(gx,gy)+1e-6
    u+=0.42*(gy/nn)*wz; v+=0.42*(-gx/nn)*wz
    if it%25==0: u+=0.06*gaussian_filter(rng.normal(0,1,(h,w)),4); v+=0.04*gaussian_filter(rng.normal(0,1,(h,w)),4)
    adv=lambda f: map_coordinates(f,[yy-v,xx-u],order=1,mode='nearest')
    u=adv(u); v=adv(v); T=adv(T)*0.998; D=adv(D)
    U=np.fft.fft2(u); V=np.fft.fft2(v); dv=(kx*U+ky*V)/k2; u=np.real(np.fft.ifft2(U-kx*dv)); v=np.real(np.fft.ifft2(V-ky*dv))
    u[-6:]=0; v[-6:]=0
np.save('work/plume2.npy',D)
from PIL import Image
a=(D/np.percentile(D,99.5)).clip(0,1); Image.fromarray((255*(1-a)).astype(np.uint8)).resize((384,384)).save('work/plume2.png')
