import numpy as np, math, subprocess, os, sys
from scipy.ndimage import gaussian_filter, zoom
import matplotlib; matplotlib.use('Agg'); import matplotlib.pyplot as plt
src=open('room4.py').read().split("import os\nONLY2=")[0]
src=src.replace("for (x,y,s,d) in quad_cells(qf):","for (x,y,s,d) in quad_cells(qf,maxd=QMAXD):")
src=src.replace("open('out6/'","open('svg/'")
src=src.replace('fill-rule="evenodd"></path>','fill-rule="evenodd" stroke="{c}" stroke-width="0.004" stroke-linejoin="round"></path>')
src=src.replace('stdDeviation="{sd*W:.2f}"','stdDeviation="{sd*W*BLUR:.2f}"')
src=src.replace('opacity="0.95"','opacity="{min(1,0.95*GLOW):.3f}"')
ns={'__name__':'roomlib','QMAXD':5,'GLOW':1.0,'BLUR':1.0}

exec(compile(src,'room4','exec'),ns)

# match GitHub's page backgrounds outside the room
ns['PAL']['bw-dark']=dict(ns['PAL']['bw-dark'],bg='#0d1117')
ns['PAL']['bw-light']=dict(ns['PAL']['bw-light'],bg='#ffffff')
ns['PAL'].setdefault('tertiary-b-light',dict(bg='#f6f1e6',edge='#1d1b19',vox='#2a2724',tower='#2a2724',cloud='#1d1b19',grad=['#e08a00','#f2c97a','#f6f1e6'],grad_r=0.85,
     fluid=['#f6f1e6','#ade4dc','#2aa69a','#0d5e57'],quad=['#f6f1e6','#efcdee','#c860cb','#7a2480'],quad_bg='#f6f1e6'))
out=list(np.load('work/frames_periodic.npy'))   # exactly periodic: frame N would equal frame 0
N=len(out)
LV=np.linspace(0.04,1.001,40)
def fseg(F):
    F=gaussian_filter(F,0.8); F=zoom(F,(120/F.shape[0],120/F.shape[1]),order=1)
    fig=plt.figure(); cs=plt.contourf(np.linspace(0,1,F.shape[1]),np.linspace(0,1,F.shape[0]),F,levels=LV); plt.close(fig)
    return cs.allsegs
os.makedirs('svg',exist_ok=True); os.makedirs('png',exist_ok=True)
pals=sys.argv[1:] or ['bw-dark','bw-light']
for i,F in enumerate(out):
    t=i/N
    ns['FSEG']=fseg(F)
    ns['QMAXD']=int(round(2+3*(0.5-0.5*math.cos(2*math.pi*t))))   # refine 2 -> 5 -> 2
    r=0.5-0.5*math.cos(2*math.pi*t)                                 # 0 -> 1 -> 0 over the loop
    ns['GLOW']=0.80+0.25*r                                          # field brightens as it resolves
    ns['BLUR']=1.75-1.45*r                                          # and pulls into focus with the refinement
    for p in pals:
        ns['_FL'][0]=0
        name=f'{p}-{i:02d}'
        ns['scene'](name,ns['PAL'][p],'centre')
        subprocess.run(['rsvg-convert','-w','512','-h','512',f'svg/{name}.svg','-o',f'png/{name}.png'])
print('done')
