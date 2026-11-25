"""README title lockups: the transparent icon beside the outlined wordmark and descriptor."""
import re, sys
sys.path.insert(0,'.')
from glyphs import text_path
ICON=sys.argv[1] if len(sys.argv)>1 else 'icon'; OUT=sys.argv[2] if len(sys.argv)>2 else 'out'
B='texgyreheros-bold.otf'; R='texgyreheros-regular.otf'
def inner(f,pre):
    s=open(f).read(); vb=re.search(r'viewBox="([^"]+)"',s).group(1)
    b=s[s.index('>',s.index('<svg'))+1:s.rindex('</svg>')]
    for i in sorted(set(re.findall(r'id="([^"]+)"',b)),key=len,reverse=True):
        b=b.replace(f'id="{i}"',f'id="{pre}{i}"').replace(f'url(#{i})',f'url(#{pre}{i})')
    return vb,b
def lockup(icon,fg,sub,path,pre):
    vb,b=inner(icon,pre); H=214; gap=40
    big,_,_=text_path("Cavendish",B,198,H+gap-6.7,150,track=-0.022)
    l1="Atom-Interferometer"; l2="Gravity Gradiometry"
    _,_,b1=text_path(l1,R,32,0,0); _,_,b2=text_path(l2,R,32,0,0)
    x=1598-max(b1[2],b2[2]); p1,_,_=text_path(l1,R,32,x,31); p2,_,_=text_path(l2,R,32,x,71)
    open(path,'w',newline='\n').write(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1600 {H}" width="1600" height="{H}" role="img" aria-label="Cavendish. Atom-Interferometer Gravity Gradiometry."><svg x="0" y="0" width="{H}" height="{H}" viewBox="{vb}">{b}</svg><path fill="{fg}" d="{big}"/><path fill="{sub}" d="{p1}"/><path fill="{sub}" d="{p2}"/></svg>\n')
lockup(f'{ICON}/cavendish-icon-light-transparent.svg','#1d1b19','#3d3a34',f'{OUT}/title-light.svg','L')
lockup(f'{ICON}/cavendish-icon-dark-transparent.svg','#efe9dc','#c8c1b2',f'{OUT}/title-dark.svg','D')
# the black-and-white lockups, in make_wordmark.py's inks (these two calls were not in the bundle: they reproduce the shipped files byte for byte)
lockup(f'{ICON}/cavendish-icon-bw-light-transparent.svg','#111111','#555555',f'{OUT}/title-bw-light.svg','L')
lockup(f'{ICON}/cavendish-icon-bw-dark-transparent.svg','#f0f0f0','#a8a8a8',f'{OUT}/title-bw-dark.svg','D')
print('lockups ok')
