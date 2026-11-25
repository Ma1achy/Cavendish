import sys; sys.path.insert(0,'.')
from glyphs import text_path
OUT='out/'
B='texgyreheros-bold.otf'
_,_,b=text_path("C",B,100,0,0)
x0,y0,x1,y1=b; gw,gh=x1-x0,y1-y0
def make(fg,bg,name,dot):
    S=46/gh  # glyph height 46 of 64
    size=100*S
    gx=(64-gw*S)/2 - x0*S + dot.get('shift',0); gy=32+gh*S/2 - (y1*S)
    p,_,bb=text_path("C",B,size,gx,gy)
    tile=f'<rect width="64" height="64" fill="{bg}"/>' if bg else ''
    d=''
    if dot.get('r'):
        cx=dot['cx'](bb); d=f'<circle cx="{cx:.2f}" cy="32" r="{dot["r"]}" fill="{fg}"/>'
    open(OUT+name+'.svg','w',newline='\n').write(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="512" height="512" role="img" aria-label="Cavendish">{tile}<path fill="{fg}" d="{p}"/>{d}</svg>\n')
    return bb
V={
 'counter': dict(r=5.2, cx=lambda bb:(bb[0]+bb[2])/2+1.5),        # mass in the counter
 'mouth':   dict(r=4.6, cx=lambda bb:bb[2]-2.0, shift=-3),           # mass sits in the opening
 'plain':   dict(),
}
for k,dv in V.items():
    make('#efe9dc','#1d1b19',f'hc-{k}-dark',dv); make('#1d1b19','#efe9dc',f'hc-{k}-light',dv); make('#1d1b19',None,f'hc-{k}-ink',dv)
