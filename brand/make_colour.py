from PIL import Image
import os
N=48; DUR=80
def global_palette(frames,colors=255):
    sheet=Image.new('RGB',(frames[0].width,frames[0].height*len(frames[::4])))
    for i,f in enumerate(frames[::4]): sheet.paste(f,(0,i*f.height))
    return sheet.quantize(colors=colors,method=Image.Quantize.MEDIANCUT,dither=Image.Dither.NONE)
def gif(frames,path):
    pal=global_palette(frames)
    q=[f.quantize(palette=pal,dither=Image.Dither.NONE) for f in frames]
    q[0].save(path,save_all=True,append_images=q[1:],duration=DUR,loop=0,optimize=False,disposal=1)
for mode,p in [('dark','magma-dark'),('light','tertiary-b-light')]:
    fr=[Image.open(f'png/{p}-{i:02d}.png').convert('RGB') for i in range(N)]
    for s in (512,256):
        fs=fr if s==512 else [f.resize((s,s),Image.LANCZOS) for f in fr]
        gif(fs,f'colour/cavendish-icon-anim-{mode}-{s}.gif')
        fs[0].save(f'colour/cavendish-icon-anim-{mode}-{s}.webp',save_all=True,append_images=fs[1:],duration=DUR,loop=0,quality=90,method=6)
        fs[0].save(f'colour/cavendish-icon-anim-{mode}-{s}.png',save_all=True,append_images=fs[1:],duration=DUR,loop=0,format='PNG')  # APNG
    for s in (32,64):
        fs=[f.resize((s,s),Image.LANCZOS) for f in fr]
        gif(fs,f'colour/favicon-anim-{mode}-{s}.gif')
        fs[0].save(f'colour/favicon-anim-{mode}-{s}.png',save_all=True,append_images=fs[1:],duration=DUR,loop=0,format='PNG')
for f in sorted(os.listdir('colour')): print(f, os.path.getsize('colour/'+f)//1024,'KB')
