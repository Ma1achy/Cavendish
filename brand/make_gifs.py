import subprocess, glob, os
from PIL import Image
N=48; DUR=80
def save_gif(frames,path):
    q=[f.convert('RGB').quantize(colors=128,method=Image.Quantize.MEDIANCUT,dither=Image.Dither.NONE) for f in frames]
    q[0].save(path,save_all=True,append_images=q[1:],duration=DUR,loop=0,optimize=True,disposal=1)
for mode,p,wm in [('dark','bw-dark','work/wm-dark.png'),('light','bw-light','work/wm-light.png')]:
    icons=[Image.open(f'png/{p}-{i:02d}.png').convert('RGB') for i in range(N)]
    save_gif([im.resize((256,256),Image.LANCZOS) for im in icons],f'cavendish-icon-anim-{mode}.gif')
    save_gif(icons,f'cavendish-icon-anim-{mode}-512.gif')
    base=Image.open(wm).convert('RGB'); frames=[]
    for i in range(N):
        subprocess.run(['rsvg-convert','-w','214','-h','214',f'svg/{p}-{i:02d}.svg','-o','work/ic.png'])
        f=base.copy(); f.paste(Image.open('work/ic.png').convert('RGB'),(0,0)); frames.append(f)
    save_gif(frames,f'title-anim-{mode}.gif')
for f in sorted(glob.glob('*.gif')): print(f, os.path.getsize(f)//1024,'KB')
