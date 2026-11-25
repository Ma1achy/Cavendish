import sys; sys.path.insert(0,'.')
from glyphs import text_path
B='texgyreheros-bold.otf'; R='texgyreheros-regular.otf'
def title(fg, sub, path):
    big,w,b=text_path("Cavendish",B,198,-6.7,150,track=-0.022)
    l1="Atom-Interferometer"; l2="Gravity Gradiometry"
    _,w1,b1=text_path(l1,R,32,0,0); _,w2,b2=text_path(l2,R,32,0,0)
    x=1598-max(b1[2],b2[2])
    p1,_,_=text_path(l1,R,32,x,31); p2,_,_=text_path(l2,R,32,x,71)
    svg=f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1600 214" width="1600" height="214" role="img" aria-label="Cavendish. Atom-Interferometer Gravity Gradiometry.">
<path fill="{fg}" d="{big}"/>
<path fill="{sub}" d="{p1}"/>
<path fill="{sub}" d="{p2}"/>
</svg>
'''
    open(path,'w',newline='\n').write(svg); print(path,b)
title('#1d1b19','#3d3a34','out/title-light.svg')
title('#efe9dc','#c8c1b2','out/title-dark.svg')
