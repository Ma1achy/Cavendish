import sys, json, re; sys.path.insert(0,'.')
exec(open('board.py').read().split("iso=sorted(")[0])
R='/home/claude/cav/room/out6/'
names={'ember-ice-dark':('Ember / ice','A red-hot to deep-navy field, cyan smoke, a green mosaic floor.'),
 'principia-light':('Principia, light','Brick, cyan and sage on bone.'),
 'magma-dark':('Magma','A gold-to-plum field, blue smoke, a mauve floor.'),
 'blueprint':('Blueprint','One blue family, with white voxels.'),
 'clay-sage-light':('Clay / slate','A terracotta field, umber smoke, a slate floor, with slate voxels.'),
 'riso-light':('Riso','Fluorescent red, riso blue and yellow on paper.'),
 'mono-dark':('Mono, dark','Charcoal and bone.'),
 'mono-light':('Mono, light','Bone and charcoal.'),
 'bw-dark':('Black and white, dark','Neutral greys.'),
 'bw-light':('Black and white, light','Neutral greys.')}
for k,(t,d) in names.items(): CAP['c-'+k]=(t,'C in the centre. '+d)
CAP['c-ember-ice-dark-noedges']=('Ember / ice, no edges','Colour alone defines the room.')
CAP['c-principia-light-swapped']=('Principia, swapped','The field on the left wall and the smoke on the right.')
for k in ['ember-ice-dark','principia-light','magma-dark','clay-sage-light']: CAP['s-'+k]=(names[k][0]+', as first sketched','The front-left C and the tower, with the new walls.')
PICKS.update({'c-ember-ice-dark','c-principia-light','c-magma-dark','c-riso-light','c-mono-dark','c-bw-light'})
order=['c-ember-ice-dark','c-principia-light','c-magma-dark','c-riso-light','c-blueprint','c-clay-sage-light',
       'c-mono-dark','c-mono-light','c-bw-dark','c-bw-light','c-ember-ice-dark-noedges','c-principia-light-swapped',
       's-ember-ice-dark','s-principia-light','s-magma-dark','s-clay-sage-light']
files=[R+k+'.svg' for k in order]
cols=6; w=64*2+cols*300+(cols-1)*32
tiles=''.join(tile(f,i) for i,f in enumerate(files))
sz=''
for j,k in enumerate(['c-ember-ice-dark','c-principia-light','c-magma-dark','c-mono-dark','c-bw-light']):
    f=R+k+'.svg'
    sz+=f'<div style="display:flex;align-items:flex-end;gap:16px">{inl(f,96,"z96"+str(j))}{inl(f,48,"z48"+str(j))}{inl(f,32,"z32"+str(j))}{inl(f,16,"z16"+str(j))}</div>'
h=64+300+40+3*(300+12+80)+2*32+40+40+24+96+64
body=f'''<div style="width:{w}px;height:{h}px;box-sizing:border-box;padding:64px;background:#121110;color:#efe9dc;font-family:Inter,'Helvetica Neue',Helvetica,Arial,sans-serif;display:flex;flex-direction:column;gap:40px">
<div style="display:flex;gap:40px;align-items:flex-start">
<img src="/_blob/1888b1c4802c996db0ac3c07971c7179" alt="Malachy’s second sketch: a room corner with a flat C facing the camera in the centre" style="width:231px;height:300px;object-fit:cover;display:block">
<div style="display:flex;flex-direction:column;gap:12px;max-width:900px">
<h1 style="margin:0;font-size:40px;line-height:1.05;font-weight:700;letter-spacing:-0.01em">The C alone in the room</h1>
<p style="margin:0;font-size:16px;line-height:1.5;color:#a8a194">A voxel C hangs at the centre of the room, facing the camera, with no tower. Its arms are 3 voxels long. The walls are now abstract: a smooth two-colour field (high near the C, low at the edges), a simulated plume of rising smoke, and a quadtree mosaic floor that refines under the C. The same walls are shown in ten palettes, with the first-sketch layout at the end.</p>
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
s=HEAD.format(title='The C alone in the room')+body+TAIL.format(w=w,h=h)
open(P+'Centre.dc.html','w').write(s)
cj=json.load(open('/home/claude/cav/canvas/project/canvas.json'))
cj['boards']['Centre.dc.html']=dict(x=10352,y=0,w=w,h=h,title='The C alone in the room')
if 'Centre.dc.html' not in cj['order']: cj['order'].append('Centre.dc.html')
cj['notes']['t']['maxW']=10352+w
json.dump(cj,open('/home/claude/cav/canvas/project/canvas.json','w'),indent=1)
print(w,h,len(s))
