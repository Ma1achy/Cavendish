import sys, json, re; sys.path.insert(0,'.')
exec(open('board.py').read().split("iso=sorted(")[0])
R='/home/claude/cav/room/out6/'
names={'ember-ice-dark':('Ember / ice','red-hot to deep-navy field, cyan smoke, green mosaic'),
 'principia-light':('Principia, light','brick, cyan and sage on bone'),
 'magma-dark':('Magma RGB, emerald','gold-to-plum field, blue smoke, emerald mosaic: red, blue and green across the three faces'),
 'blueprint':('Blueprint','one blue family, white voxels'),
 'clay-sage-light':('Clay / slate','terracotta field, umber smoke, slate mosaic'),
 'riso-light':('Riso','fluorescent red, riso blue, yellow'),
 'mono-dark':('Mono, dark','charcoal and bone'),
 'mono-light':('Mono, light','bone and charcoal'),
 'bw-dark':('Black and white, dark','neutral greys'),
 'bw-light':('Black and white, light','neutral greys')}
ORDER=['ember-ice-dark','principia-light','magma-dark','riso-light','blueprint','clay-sage-light','mono-dark','mono-light','bw-dark','bw-light']
def board(fname,prefix,title,lede,img,imgw,imgh,alt,extra=()):
    keys=[prefix+k for k in ORDER]+list(extra)
    for k in ORDER: CAP[prefix+k]=(names[k][0],names[k][1][0].upper()+names[k][1][1:]+'.')
    for k,(t,d) in dict(extra).items() if isinstance(extra,dict) else []: pass
    files=[R+k+'.svg' for k in keys]
    cols=6; w=64*2+cols*300+(cols-1)*32
    rows=(len(files)+cols-1)//cols
    tiles=''.join(tile(f,i) for i,f in enumerate(files))
    sz=''
    for j,k in enumerate([prefix+'ember-ice-dark',prefix+'principia-light',prefix+'magma-dark',prefix+'mono-dark',prefix+'bw-light']):
        f=R+k+'.svg'; sz+=f'<div style="display:flex;align-items:flex-end;gap:16px">{inl(f,96,"z96"+str(j))}{inl(f,48,"z48"+str(j))}{inl(f,32,"z32"+str(j))}{inl(f,16,"z16"+str(j))}</div>'
    h=64+300+40+rows*(300+12+80)+(rows-1)*32+40+40+24+96+64
    body=f'''<div style="width:{w}px;height:{h}px;box-sizing:border-box;padding:64px;background:#121110;color:#efe9dc;font-family:Inter,'Helvetica Neue',Helvetica,Arial,sans-serif;display:flex;flex-direction:column;gap:40px">
<div style="display:flex;gap:40px;align-items:flex-start">
<img src="{img}" alt="{alt}" style="width:{imgw}px;height:{imgh}px;object-fit:cover;display:block">
<div style="display:flex;flex-direction:column;gap:12px;max-width:900px">
<h1 style="margin:0;font-size:40px;line-height:1.05;font-weight:700;letter-spacing:-0.01em">{title}</h1>
<p style="margin:0;font-size:16px;line-height:1.5;color:#a8a194">{lede}</p>
</div>
</div>
<div style="display:grid;grid-template-columns:repeat({cols}, minmax(0, 1fr));gap:32px">
{tiles}
</div>
<div style="display:flex;flex-direction:column;gap:16px">
<div style="font-size:15px;font-weight:700">At size: 96, 48, 32, 16</div>
<div style="display:flex;flex-wrap:wrap;gap:40px">{sz}</div>
</div>
</div>
'''
    s=HEAD.format(title=title)+body+TAIL.format(w=w,h=h)
    open(P+fname,'w').write(s); return w,h,len(s)
WALLS='A sun casts hard shadows from the voxels onto the walls and floor (on the outside cube, onto the top face and an implied ground). Each face is lit differently, and the scene has ambient occlusion: soft darkening where faces meet, and contact shadows under and behind the voxels. The walls: a smooth two-colour field (high near the C, low at the edges), turbulent smoke from the convection sim posterised into seven bands, and a quadtree mosaic with no borders, where only the colour changes with depth.'
CAP['c-ember-ice-dark-noedges']=('Ember / ice, no edges','Colour alone defines the room.')
CAP['c-principia-light-swapped']=('Principia, swapped','The field on the left wall, the smoke on the right.')
CAP['t-ember-ice-dark-swapped']=('Ember / ice, swapped','The field on the left face, the smoke on the right.')
CAP['t-principia-light-swapped']=('Principia, swapped','The field on the left face, the smoke on the right.')
for p in ('c-','s-','t-'):
    CAP[p+'magma-viridian']=('Magma RGB, viridian','The same triad with a cooler, bluer green floor.')
    CAP[p+'magma-lime']=('Magma RGB, lime','The same triad with a yellow-leaning green floor.')
PICKS.update({'c-magma-dark','c-magma-viridian','s-magma-dark','t-magma-dark'})
PICKS.update({'c-ember-ice-dark','c-principia-light','s-ember-ice-dark','t-ember-ice-dark','t-principia-light','t-magma-dark'})
dims={}
dims['Centre.dc.html']=board('Centre.dc.html','c-','The C alone in the room','A voxel C hangs at the centre of the room, facing the camera. It is now seven voxels tall: a spine of 5, with arms of 3. '+WALLS,'/_blob/1888b1c4802c996db0ac3c07971c7179',231,300,'Malachy’s second sketch',extra=['c-magma-viridian','c-magma-lime','c-ember-ice-dark-noedges','c-principia-light-swapped'])
dims['Sketch.dc.html']=board('Sketch.dc.html','s-','The room, as sketched','The C stands floor to ceiling at the front-left of the room, opening inwards, and the tower stands on the back-right wall. '+WALLS,'/_blob/231da94894e7e183a78de520bb48d9ba',300,292,'Malachy’s first sketch')
dims['Top.dc.html']=board('Top.dc.html','t-','C on the top face','Back to the outside of the cube, as in the third sketch. The voxel C lies on the top face, and the quadtree mosaic there refines under every voxel. The two side faces carry the fields: turbulent smoke on one, and on the other the C’s field, brightest at the top edge where the C sits.','/_blob/4dc4059d275e1263e810e57a468654ba',291,300,'Malachy’s third sketch: a cube with the C on its top face',extra=['t-magma-viridian','t-magma-lime','t-ember-ice-dark-swapped','t-principia-light-swapped'])
cj=json.load(open('/home/claude/cav/canvas/project/canvas.json'))
for k in ('Centre.dc.html','Sketch.dc.html'): cj['boards'][k]['w'],cj['boards'][k]['h']=dims[k][0],dims[k][1]
# Sketch board was 1756 wide; now 2088 -> shift Centre right to avoid overlap
cj['boards']['Sketch.dc.html']['x']=8516
cj['boards']['Centre.dc.html']['x']=8516+dims['Sketch.dc.html'][0]+80
tx=cj['boards']['Centre.dc.html']['x']+dims['Centre.dc.html'][0]+80
cj['boards']['Top.dc.html']=dict(x=tx,y=0,w=dims['Top.dc.html'][0],h=dims['Top.dc.html'][1],title='C on the top face')
if 'Top.dc.html' not in cj['order']: cj['order'].append('Top.dc.html')
cj['notes']['t']['maxW']=tx+dims['Top.dc.html'][0]
json.dump(cj,open('/home/claude/cav/canvas/project/canvas.json','w'),indent=1)
print(dims)
