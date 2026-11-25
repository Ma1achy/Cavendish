import numpy as np
OUT='out/'
def arc(cx,cy,r,a0,a1,n=64):
    t=np.radians(np.linspace(a0,a1,n)); return [(cx+r*np.cos(x),cy-r*np.sin(x)) for x in t]
def ring_path(cx,cy,ro,ri,gap):
    # annular sector from +gap to 360-gap (opening facing right), square ends
    o=arc(cx,cy,ro,gap,360-gap); i=arc(cx,cy,ri,360-gap,gap)
    pts=o+i; return 'M'+' L'.join(f'{x:.3f} {y:.3f}' for x,y in pts)+'Z'
def svg(body,bg,name):
    tile=f'<rect width="64" height="64" fill="{bg}"/>' if bg else ''
    open(OUT+name+'.svg','w',newline='\n').write(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="512" height="512" role="img" aria-label="Cavendish">{tile}{body}</svg>\n')
def variants(fg):
    v={}
    # A: shell — one open equipotential shell, mass at centre
    v['shell']=f'<path fill="{fg}" d="{ring_path(32,32,24,16,42)}"/><circle cx="32" cy="32" r="6" fill="{fg}"/>'
    # B: voxel — squared C, square mass
    v['voxel']=f'<path fill-rule="evenodd" fill="{fg}" d="M8 8H56V22H48V16H16V48H48V42H56V56H8Z"/><rect x="26" y="26" width="12" height="12" fill="{fg}"/>'
    # C: shells — 1/r spaced open shells, gap widening outward (field weakening)
    rs=[(27,3.0,34),(19.5,3.6,40),(13,4.2,48)]
    v['shells']=''.join(f'<path fill="{fg}" d="{ring_path(32,32,r+t/2,r-t/2,g)}"/>' for r,t,g in rs)+f'<circle cx="32" cy="32" r="5" fill="{fg}"/>'
    # D: offset — C whose hole is pulled toward an off-centre mass (gradient: thick far side, thin near mass)
    o=arc(30,32,25,40,320); 
    i=arc(35,32,15,320,40)
    d='M'+' L'.join(f'{x:.3f} {y:.3f}' for x,y in o+i)+'Z'
    v['pull']=f'<path fill="{fg}" d="{d}"/><circle cx="40" cy="32" r="5" fill="{fg}"/>'
    return v
for k,b in variants('#efe9dc').items(): svg(b,'#1d1b19','c-'+k+'-dark')
for k,b in variants('#1d1b19').items(): svg(b,'#efe9dc','c-'+k+'-light'); svg(b,None,'c-'+k+'-ink')
def oct_pts(cx,cy,R,c):
    # chamfered square half-size R, chamfer c; return clockwise points starting right-upper
    return [(cx+R,cy-R+c),(cx+R-c,cy-R),(cx-R+c,cy-R),(cx-R,cy-R+c),(cx-R,cy+R-c),(cx-R+c,cy+R),(cx+R-c,cy+R),(cx+R,cy+R-c)]
def pull_oct(fg):
    # outer chamfered square centred (30,32) half 25; inner centred (36,32) half 13; opening on right
    gy=9  # half-height of the mouth
    ox,oy,oR,oc=30,32,25,10; ix,iy,iR,ic=36,32,13,5
    # outer path from mouth top going anticlockwise round to mouth bottom
    outer=[(ox+oR-((oR-oc)-(oR-gy)) if False else ox+oR, oy-gy)]
    outer=[(ox+oR,oy-gy),(ox+oR,oy-oR+oc),(ox+oR-oc,oy-oR),(ox-oR+oc,oy-oR),(ox-oR,oy-oR+oc),(ox-oR,oy+oR-oc),(ox-oR+oc,oy+oR),(ox+oR-oc,oy+oR),(ox+oR,oy+oR-oc),(ox+oR,oy+gy)]
    inner=[(ix+iR,iy+gy if gy<iR-ic else iy+iR-ic),(ix+iR-ic,iy+iR),(ix-iR+ic,iy+iR),(ix-iR,iy+iR-ic),(ix-iR,iy-iR+ic),(ix-iR+ic,iy-iR),(ix+iR-ic,iy-iR),(ix+iR,iy-iR+ic)]
    # mouth: connect outer end (right, y=oy+gy) to inner bottom-right, and inner top-right back to start
    pts=outer+[(ix+iR,iy+gy)]+inner[1:-1]+[(ix+iR,iy-gy)]
    d='M'+' L'.join(f'{x:g} {y:g}' for x,y in pts)+'Z'
    return f'<path fill="{fg}" d="{d}"/><rect x="38" y="28" width="8" height="8" fill="{fg}"/>'
svg(pull_oct('#efe9dc'),'#1d1b19','c-pulloct-dark'); svg(pull_oct('#1d1b19'),'#efe9dc','c-pulloct-light'); svg(pull_oct('#1d1b19'),None,'c-pulloct-ink')
