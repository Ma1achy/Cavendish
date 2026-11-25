import json, datetime
names=sorted(__import__('os').listdir('/home/claude/cav/room/out6'))
ids="""50c5919ef0f81cba16147ff15056ffea aafd18ef1eb38f9a7d73c09d49486e04 c5a4630758507b6194663b511b60c0e1 7b063d989f18fe96cb4870f929662153 e7cd659c3c01abeab02c7060e4a6b064 34e603cb6017fc54c2194a208f73c4f7 992155b8903a995b1c9539f988e10b00 3e2521663b0ac317b36e8b13ba93442c 02ddbb9865669da94da3190aa0a4f4f3 0d235fcd05fe4bac6bb4e05698610c94
aa96dfc0e7c9b61487319a9ab2a5ef64 866f82e08fbdbef0451b0b520f63a283 fa252a69212b4616009efceddabd6249 2332cc98c1c4c8a8faa4501212a08557 4e32caeefd54a010fc2f34ac47cee658 d4b6d7b02a5d87bbff493bc15de76257 826eeab74c32e5388d113e5fc06c8bac 21fc863458f8ff8ef2601c2e10eb2b61 223e4e7dac401b4cd8f71db8520de3dd 8395672b4f793cb34bd0576a013e9a01
0337b159debb6bf16aa4bab680cc4c06 a7330b6f5bc128e8ac564b557afa6c26 dbd861c93bbf9e0d679b050497027061 277372588cad7a4c36ce336c59816950 e84abb6ec5ec99ceb4646cd5861b2df8 009b825c42ef9c91cf703c2ce9c4e896 d992da4eeaf2dd9e680aab11e5318231 441f50dfaad990b362d62e045b277750 11b2412bbbede4e97e7cbbca080b1d5f abb5e67f6c9649eb3f851e12aeaa3b63
5a030b00b78d717ceb71287361901e68 de68190d1e6e0562ad91305937978b98 2081a58781c3bfb9ff9f8de7cacd4434 4d2dd4df066201896e2d88b6df9198f0 f9520badc93d5a8d28a79cdbbb9e4f98 714aa9b12454ac754f31a2fc439e361c ded2fc86bea9445a6ad9106cdb6cff30 dddca896b249bc0663f1da836892ea8c e6fe3795e01e1c2f24e6ebe3e0a3546b 93ee72203e130bdb3d3e1771cedcd343""".split()
assert len(names)==40==len(ids)
URL={n[:-4]:'/_blob/'+i for n,i in zip(names,ids)}
URL.update(json.load(open('/home/claude/cav/urls.json')))
SK={'s':'/_blob/b37f47adb292748b4d100e050ee7f28e','c':'/_blob/498128e8da511e3e2c658913c4484feb','t':'/_blob/2239146631426eac8be37b78cdc3998e'}
PAL={'magma-dark':('Magma RGB, emerald','Gold-to-plum field, blue smoke, emerald floor: red, blue and green across the three faces.'),
 'magma-viridian':('Magma RGB, viridian','The same triad with a cooler, bluer green.'),
 'magma-lime':('Magma RGB, lime','The same triad with a yellow-leaning green.'),
 'ember-ice-dark':('Ember / ice','Red-hot to deep-navy field, cyan smoke, green mosaic.'),
 'principia-light':('Principia, light','Brick, cyan and sage on bone.'),
 'riso-light':('Riso','Fluorescent red, riso blue and yellow on paper.'),
 'blueprint':('Blueprint','One blue family, white voxels.'),
 'clay-sage-light':('Clay / slate','Terracotta field, umber smoke, slate mosaic.'),
 'mono-dark':('Mono, dark','Charcoal and bone.'),
 'mono-light':('Mono, light','Bone and charcoal.'),
 'bw-dark':('Black and white, dark','Neutral greys.'),
 'bw-light':('Black and white, light','Neutral greys.')}
EXTRA={'c-ember-ice-dark-noedges':('Ember / ice, no edges','Colour alone defines the room.'),
 'c-principia-light-swapped':('Principia, swapped','The field on the left wall, the smoke on the right.'),
 't-ember-ice-dark-swapped':('Ember / ice, swapped','The field on the left face, the smoke on the right.'),
 't-principia-light-swapped':('Principia, swapped','The field on the left face, the smoke on the right.')}
PICK={'c-magma-dark','s-magma-dark','t-magma-dark'}
HEAD='''<!doctype html>
<html lang="en-GB">
<head>
<meta charset="utf-8">
<title>{title}</title>
<script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;700&amp;display=swap">
<style>
body{{margin:0}}
a{{color:#c8c1b2}}a:hover{{color:#efe9dc}}
</style>
</helmet>
'''
TAIL='''</x-dc>
<script type="text/x-dc" data-dc-script data-props='{{"$preview":{{"width":{w},"height":{h}}}}}'>
class Component extends DCLogic {{
renderVals() {{ return {{}}; }}
}}
</script>
</body>
</html>
'''
def tile(k):
    t,d=(EXTRA.get(k) or PAL[k[2:]])
    pk=' · pick' if k in PICK else ''
    return f'''<div style="display:flex;flex-direction:column;gap:12px">
<img src="{URL[k]}" alt="{t}" style="width:300px;height:300px;display:block;outline:1px solid #2c2a26">
<div style="display:flex;flex-direction:column;gap:4px;width:300px">
<div style="font-size:15px;font-weight:700;color:#efe9dc">{t}<span style="font-weight:400;color:#a8a194">{pk}</span></div>
<div style="font-size:13px;line-height:1.45;color:#a8a194">{d}</div>
</div>
</div>'''
COMMON='The gravity-gradient wall is the C\u2019s own field: a soft glow in the shape of the letter. A sun casts hard shadows from the voxels onto the walls and floor (or, on the outside cube, onto the top face and an implied ground). Each face is lit differently, and the scene has ambient occlusion. The walls: the C-shaped field, turbulent smoke from the convection sim posterised into seven bands, and a quadtree mosaic where only the colour changes with depth.'
ORDER=list(PAL)
def board(fname,p,title,lede,sk,skw,skh,extra=()):
    keys=[p+k for k in ORDER]+list(extra)
    cols=6; rows=(len(keys)+cols-1)//cols
    w=64*2+cols*300+(cols-1)*32
    h=64+300+40+rows*(300+12+84)+(rows-1)*32+40+24+16+96+64
    sizes=''.join(f'<div style="display:flex;align-items:flex-end;gap:16px">'+''.join(f'<img src="{URL[p+k]}" alt="" style="width:{s}px;height:{s}px;display:block">' for s in (96,48,32,16))+'</div>' for k in ['magma-dark','ember-ice-dark','principia-light','mono-dark','bw-light'])
    body=f'''<div style="width:{w}px;height:{h}px;box-sizing:border-box;padding:64px;background:#121110;color:#efe9dc;font-family:Inter,'Helvetica Neue',Helvetica,Arial,sans-serif;display:flex;flex-direction:column;gap:40px">
<div style="display:flex;gap:40px;align-items:flex-start">
<img src="{sk}" alt="Malachy’s sketch" style="width:{skw}px;height:{skh}px;object-fit:cover;display:block">
<div style="display:flex;flex-direction:column;gap:12px;max-width:900px">
<h1 style="margin:0;font-size:40px;line-height:1.05;font-weight:700;letter-spacing:-0.01em">{title}</h1>
<p style="margin:0;font-size:16px;line-height:1.5;color:#a8a194">{lede}</p>
</div>
</div>
<div style="display:grid;grid-template-columns:repeat({cols}, minmax(0, 1fr));gap:32px">
{''.join(tile(k) for k in keys)}
</div>
<div style="display:flex;flex-direction:column;gap:16px">
<div style="font-size:15px;font-weight:700">At size: 96, 48, 32, 16</div>
<div style="display:flex;flex-wrap:wrap;gap:40px">{sizes}</div>
</div>
</div>
'''
    open('/home/claude/cav/canvas2/project/'+fname,'w').write(HEAD.format(title=title)+body+TAIL.format(w=w,h=h)); return w,h
d={}
d['Main.dc.html']=board('Main.dc.html','c-','The C alone in the room','A voxel C, seven voxels tall, hangs at the centre of the room, facing the camera, and throws its shadow across the field wall. The floor quadtree refines in a band under its footprint. '+COMMON,SK['c'],231,300,['c-ember-ice-dark-noedges','c-principia-light-swapped'])
d['Sketch.dc.html']=board('Sketch.dc.html','s-','The room, as sketched','The C stands floor to ceiling at the front-left, opening into the room, and the tower stands on the back-right wall. '+COMMON,SK['s'],300,292)
d['Top.dc.html']=board('Top.dc.html','t-','C on the top face','The voxel C lies on the top face of an outside cube, and the quadtree there refines under every voxel. The side faces carry the smoke and the C’s field, brightest at the top edge where the C sits. '+COMMON,SK['t'],291,300,['t-ember-ice-dark-swapped','t-principia-light-swapped'])
x=0; boards={}
for k,t in [('Main.dc.html','The C alone in the room'),('Top.dc.html','C on the top face'),('Sketch.dc.html','The room, as sketched')]:
    w,h=d[k]; boards[k]=dict(x=x,y=0,w=w,h=h,title=t); x+=w+80
cj={"v":3,"createdOnFiles":{"v":1,"at":datetime.datetime.now(datetime.UTC).strftime('%Y-%m-%dT%H:%M:%SZ')},"title":"Cavendish mark — rooms and cubes","launch":{"view":"canvas"},"pages":[],"boards":boards,"order":list(boards),"notes":{"t":{"x":0,"y":-300,"text":"Cavendish: the voxel C, its field, its smoke and its octree","kind":"title1","maxW":x-80}},"designSystems":[]}
json.dump(cj,open('/home/claude/cav/canvas2/project/canvas.json','w'),indent=1)
import os; print({f:os.path.getsize('/home/claude/cav/canvas2/project/'+f) for f in os.listdir('/home/claude/cav/canvas2/project')})
