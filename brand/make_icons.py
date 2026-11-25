"""Static icon set from the rendered marks: tightened viewBox, transparent versions, theme-switching favicon, PNGs, .ico."""
import re, os, subprocess, sys
SRC=sys.argv[1] if len(sys.argv)>1 else 'out10'; OUT=sys.argv[2] if len(sys.argv)>2 else 'icon'
os.makedirs(OUT+'/png',exist_ok=True)
marks={'dark':'c-magma-dark','light':'c-tertiary-b-light','bw-dark':'c-bw-dark','bw-light':'c-bw-light'}
for k,f in marks.items():
    s=open(f'{SRC}/{f}.svg').read()
    m=re.search(r'viewBox="([-\d.]+) ([-\d.]+) ([-\d.]+) ([-\d.]+)"',s); x,y,w,h=map(float,m.groups()); pad=w*0.035
    s=s.replace(m.group(0),f'viewBox="{x+pad:.2f} {y+pad:.2f} {w-2*pad:.2f} {h-2*pad:.2f}"')
    s=s.replace('width="512" height="512"','width="512" height="512" role="img" aria-label="Cavendish"',1)
    open(f'{OUT}/cavendish-icon-{k}.svg','w',newline='\n').write(s)
    t=re.sub(r'<rect x="[-\d.]+" y="[-\d.]+" width="[-\d.]+" height="[-\d.]+" fill="#[0-9a-f]{6}"(?:></rect>|/>)','',s,count=1)
    open(f'{OUT}/cavendish-icon-{k}-transparent.svg','w',newline='\n').write(t)
    for px in (16,32,48,64,128,180,192,256,512,1024):
        subprocess.run(['rsvg-convert','-w',str(px),'-h',str(px),f'{OUT}/cavendish-icon-{k}.svg','-o',f'{OUT}/png/cavendish-icon-{k}-{px}.png'])
def body(s,pre):
    inner=s[s.index('>',s.index('<svg'))+1:s.rindex('</svg>')]
    for i in sorted(set(re.findall(r'id="([^"]+)"',inner)),key=len,reverse=True):
        inner=inner.replace(f'id="{i}"',f'id="{pre}{i}"').replace(f'url(#{i})',f'url(#{pre}{i})')
    return inner
d=open(f'{OUT}/cavendish-icon-dark.svg').read(); l=open(f'{OUT}/cavendish-icon-light.svg').read()
vb=re.search(r'viewBox="([^"]+)"',d).group(1)
open(f'{OUT}/favicon.svg','w',newline='\n').write(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vb}" role="img" aria-label="Cavendish"><style>.d{{display:none}}@media (prefers-color-scheme: dark){{.l{{display:none}}.d{{display:inline}}}}</style><g class="l">{body(l,"L")}</g><g class="d">{body(d,"D")}</g></svg>\n')
from PIL import Image
Image.open(f'{OUT}/png/cavendish-icon-dark-256.png').save(f'{OUT}/favicon.ico',sizes=[(16,16),(32,32),(48,48),(64,64),(256,256)])
import shutil; shutil.copy(f'{OUT}/png/cavendish-icon-dark-180.png',f'{OUT}/apple-touch-icon.png')
print('icons ok')
