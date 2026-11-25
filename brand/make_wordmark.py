"""Wordmark strips (no icon) on GitHub's page colours, for compositing the animated title GIFs."""
import sys, subprocess
sys.path.insert(0,'.')
from glyphs import text_path
B='texgyreheros-bold.otf'; R='texgyreheros-regular.otf'
def wordmark(fg,sub,bg,path):
    H=214; gap=40
    big,_,_=text_path("Cavendish",B,198,H+gap-6.7,150,track=-0.022)
    l1="Atom-Interferometer"; l2="Gravity Gradiometry"
    _,_,b1=text_path(l1,R,32,0,0); _,_,b2=text_path(l2,R,32,0,0)
    x=1598-max(b1[2],b2[2]); p1,_,_=text_path(l1,R,32,x,31); p2,_,_=text_path(l2,R,32,x,71)
    open('work/wm.svg','w',newline='\n').write(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1600 214" width="1600" height="214"><rect width="1600" height="214" fill="{bg}"/><path fill="{fg}" d="{big}"/><path fill="{sub}" d="{p1}"/><path fill="{sub}" d="{p2}"/></svg>')
    subprocess.run(['rsvg-convert','-w','1600','work/wm.svg','-o',path],check=True)
wordmark('#f0f0f0','#a8a8a8','#0d1117','work/wm-dark.png')   # GitHub dark
wordmark('#111111','#555555','#ffffff','work/wm-light.png')  # GitHub light
print('wordmarks ok')
