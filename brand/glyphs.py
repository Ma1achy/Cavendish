from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.pens.boundsPen import BoundsPen
D='fonts/'
def text_path(text, font, size, x, y, track=0.0):
    f=TTFont(D+font); gs=f.getGlyphSet(); cmap=f.getBestCmap(); upm=f['head'].unitsPerEm
    s=size/upm; pen=SVGPathPen(gs); cx=0
    # simple GPOS-less kerning via kern table if present
    names=[cmap[ord(c)] for c in text]
    bp=BoundsPen(gs)
    for i,n in enumerate(names):
        tp=TransformPen(pen,(s,0,0,-s,x+cx*s,y)); gs[n].draw(tp)
        tb=TransformPen(bp,(s,0,0,-s,x+cx*s,y)); gs[n].draw(tb)
        cx+=gs[n].width+track*upm
    return pen.getCommands(), cx*s, bp.bounds
