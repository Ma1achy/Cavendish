import numpy as np, time
from scipy.ndimage import map_coordinates, gaussian_filter
rng=np.random.default_rng(9)
n=200; h=w=n
u=np.zeros((h,w)); v=np.zeros((h,w)); T=np.zeros((h,w)); D=np.zeros((h,w))
yy,xx=np.mgrid[0:h,0:w].astype(float)
kx=np.fft.fftfreq(w)[None,:]*2*np.pi; ky=np.fft.fftfreq(h)[:,None]*2*np.pi; k2=kx**2+ky**2; k2[0,0]=1
srcs=sum(np.exp(-((xx-cx)/7)**2-((yy-(h-12))/4)**2) for cx in (w*0.25,w*0.55,w*0.82))
frames=[]
t0=time.time()
for it in range(420+48*3):
    jit=1+0.6*rng.normal(0,1,(h,w)); T+=0.10*srcs*jit; D+=0.08*srcs; u+=0.2*srcs*rng.normal(0,1)
    v-=0.045*T
    wz=np.gradient(v,axis=1)-np.gradient(u,axis=0); aw=np.abs(wz); gx=np.gradient(aw,axis=1); gy=np.gradient(aw,axis=0); nn=np.hypot(gx,gy)+1e-6
    u+=0.4*(gy/nn)*wz; v+=0.4*(-gx/nn)*wz
    adv=lambda f: map_coordinates(f,[yy-v,xx-u],order=1,mode='wrap')
    u=adv(u); v=adv(v); T=adv(T)*0.995; D=adv(D)*0.994
    U=np.fft.fft2(u); V=np.fft.fft2(v); dv=(kx*U+ky*V)/k2; u=np.real(np.fft.ifft2(U-kx*dv)); v=np.real(np.fft.ifft2(V-ky*dv))
    u[-5:]=0; v[-5:]=0
    if it>=420 and (it-420)%3==0: frames.append(D.copy())
np.save('work/frames.npy',np.array(frames)); print(len(frames), time.time()-t0)
