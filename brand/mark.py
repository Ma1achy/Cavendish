import numpy as np, matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
OUT='out/'
# 64-unit grid. Tower (baseline) at x=20, clouds at y=12 and y=52; source mass square right of lower cloud.
def potential(N=640):
    xs=np.linspace(0,64,N); X,Y=np.meshgrid(xs,xs)
    # uniform field (y down in svg => potential increases downward) + cube mass via sub-voxel point sum
    phi=0.9*Y
    cx,cy,h=42,38,10  # mass centre, half-size
    pts=np.linspace(-h,h,9)
    for a in pts:
        for b in pts:
            r=np.sqrt((X-cx-a)**2+(Y-cy-b)**2)+0.6
            phi-= (95/81)/r
    return xs,phi
def contour_paths(levels):
    xs,phi=potential()
    fig=plt.figure(); cs=plt.contour(xs,xs,phi,levels=levels); plt.close(fig)
    out=[]
    for segs in cs.allsegs:
        for s in segs:
            if len(s)<4: continue
            s=s[::3] if len(s)>60 else s
            d='M'+' L'.join(f'{x:.2f} {y:.2f}' for x,y in s)
            out.append(d)
    return out
def mark(fg,bg,lines=True,tile=True,name='mark'):
    g=[]
    if tile: g.append(f'<rect width="64" height="64" fill="{bg}"/>')
    if lines:
        levels=[float(v) for v in range(-6,72,3)]
        clip='<clipPath id="c"><rect x="4" y="4" width="56" height="56"/></clipPath>'
        g.append(clip)
        g.append(f'<g clip-path="url(#c)" fill="none" stroke="{fg}" stroke-width="0.45" stroke-opacity="0.6">'+''.join(f'<path d="{d}"/>' for d in contour_paths(levels))+'</g>')
    halo=len(g)
    # baseline
    g.append(f'<rect x="16" y="16" width="4" height="32" fill="{fg}"/>')
    # atom clouds: upper hollow, lower solid (differential)
    g.append(f'<path fill-rule="evenodd" fill="{fg}" d="M12 8h12v12H12zM16 12v4h4v-4z"/>' if tile or lines else f'<path fill-rule="evenodd" fill="{fg}" d="M12 8h12v12H12zM16 12v4h4v-4z"/>')
    g.append(f'<rect x="12" y="44" width="12" height="12" fill="{fg}"/>')
    # source mass
    g.append(f'<rect x="32" y="28" width="20" height="20" fill="{fg}"/>')
    if lines:
        glyph=''.join(g[halo:]); g=g[:halo]+[f'<g stroke="{bg}" stroke-width="3" stroke-linejoin="miter" fill="{bg}">'+glyph.replace('fill="'+fg+'"','')+'</g>', glyph]
    svg='<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="512" height="512" role="img" aria-label="Cavendish">'+''.join(g)+'</svg>\n'
    open(OUT+name+'.svg','w',newline='\n').write(svg)
mark('#efe9dc','#1d1b19',lines=True,name='logo-dark')
mark('#1d1b19','#efe9dc',lines=True,name='logo-light')
mark('#efe9dc','#1d1b19',lines=False,name='favicon')
mark('#1d1b19','#efe9dc',lines=False,tile=False,name='mark-ink')
