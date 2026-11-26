import math, numpy as np
C30=math.cos(math.pi/6); S30=0.5
CH='#1d1b19'; BONE='#efe9dc'
def mix(a,b,t):
    a=[int(a[i:i+2],16) for i in (1,3,5)]; b=[int(b[i:i+2],16) for i in (1,3,5)]
    return '#'+''.join(f'{round(a[i]+(b[i]-a[i])*t):02x}' for i in range(3))
def iso(x,y,z,s): return ((x-y)*C30*s, (x+y)*S30*s - z*s)
def cube(x,y,z,s,top,left,right,size=1.0,stroke='self',sw=0.45):
    # cube occupying [x,x+size]x[y,y+size]x[z,z+size]
    P=lambda a,b,c: iso(a,b,c,s)
    X0,X1,Y0,Y1,Z0,Z1=x,x+size,y,y+size,z,z+size
    def poly(pts,f):
        d='M'+' L'.join(f'{a:.2f} {b:.2f}' for a,b in pts)+'Z'
        sc=f if stroke=='self' else stroke   # 'self': seal the face with its own colour so no seams show
        st=f' stroke="{sc}" stroke-width="{sw}" stroke-linejoin="round"' if (stroke and not str(sc).startswith('url(')) else ''
        return f'<path d="{d}" fill="{f}"{st}/>'
    top_=[P(X0,Y0,Z1),P(X1,Y0,Z1),P(X1,Y1,Z1),P(X0,Y1,Z1)]
    left_=[P(X0,Y1,Z0),P(X1,Y1,Z0),P(X1,Y1,Z1),P(X0,Y1,Z1)]   # +y face (front-left)
    right_=[P(X1,Y0,Z0),P(X1,Y1,Z0),P(X1,Y1,Z1),P(X1,Y0,Z1)]  # +x face (front-right)
    return poly(top_,top)+poly(left_,left)+poly(right_,right)
C5=[".####","#....","#....","#....",".####"]   # rows top->bottom
C5b=["#####","#....","#....","#....","#####"]
C7=[".#####.","##...##","#......","#......","#......","##...##",".#####."]
def cells(pat):
    n=len(pat); out=[]
    for r,row in enumerate(pat):
        for c,ch in enumerate(row):
            if ch=='#': out.append((c,n-1-r))
    return out
def order(cs):  # painter: back (small x+y) to front, low z first
    return sorted(cs,key=lambda t:(t[0]+t[1],t[2]))
def wrap(body,vb,bg=None):
    x0,y0,w,h=vb
    t=f'<rect x="{x0}" y="{y0}" width="{w}" height="{h}" fill="{bg}"/>' if bg else ''
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{x0:.1f} {y0:.1f} {w:.1f} {h:.1f}" width="512" height="512">{t}{body}</svg>\n'
def bounds(pts):
    xs=[p[0] for p in pts]; ys=[p[1] for p in pts]; return min(xs),min(ys),max(xs),max(ys)
def square_vb(b,pad):
    x0,y0,x1,y1=b; cx,cy=(x0+x1)/2,(y0+y1)/2; s=max(x1-x0,y1-y0)/2+pad; return (cx-s,cy-s,2*s,2*s)
