import sys, json, re; sys.path.insert(0,'.')
exec(open('board.py').read().split("iso=sorted(")[0])
R='/home/claude/cav/room/out4/'
CAP.update({
'd01-colour-dark':('Colour, dark','The C stands floor to ceiling at the front-left, its spine on the wall’s front edge and opening into the room. The tower is on the back-right wall. Fluid behind the C, the C’s field on the tower’s wall, quadtree floor.'),
'd02-colour-light':('Colour, light','On bone, with charcoal voxels.'),
'd03-mono-dark':('Mono, dark','Charcoal and bone.'),
'd04-mono-light':('Mono, light','The quietest version.'),
'd05-bw-dark':('Pure black and white, dark','Neutral greys only.'),
'd06-bw-light':('Pure black and white, light','Neutral greys only.'),
'd07-colour-dark-swapped':('Walls swapped','The gradient behind the C and the fluid behind the tower.'),
'd08-colour-light-swapped':('Walls swapped, light','The same swap on bone.'),
'd09-colour-dark-no-edges':('No corner edges','Colour alone defines the room.'),
'd10-colour-dark-arms2':('Arms of 2','For comparison.'),
})
PICKS.update({'d01-colour-dark','d02-colour-light','d03-mono-dark','d06-bw-light'})
order=['d01-colour-dark','d02-colour-light','d07-colour-dark-swapped','d08-colour-light-swapped','d09-colour-dark-no-edges',
       'd03-mono-dark','d04-mono-light','d05-bw-dark','d06-bw-light','d10-colour-dark-arms2']
files=[R+k+'.svg' for k in order]
cols=5; w=64*2+cols*300+(cols-1)*32
tiles=''.join(tile(f,i) for i,f in enumerate(files))
sz=''
for j,k in enumerate(['d01-colour-dark','d02-colour-light','d03-mono-dark','d06-bw-light']):
    f=R+k+'.svg'
    sz+=f'<div style="display:flex;align-items:flex-end;gap:18px">{inl(f,96,"z96"+str(j))}{inl(f,48,"z48"+str(j))}{inl(f,32,"z32"+str(j))}{inl(f,16,"z16"+str(j))}</div>'
h=64+300+40+2*(300+12+100)+32+40+40+24+96+64
body=f'''<div style="width:{w}px;height:{h}px;box-sizing:border-box;padding:64px;background:#121110;color:#efe9dc;font-family:Inter,'Helvetica Neue',Helvetica,Arial,sans-serif;display:flex;flex-direction:column;gap:40px">
<div style="display:flex;gap:40px;align-items:flex-start">
<img src="/_blob/231da94894e7e183a78de520bb48d9ba" alt="Malachy’s sketch: a room corner with a voxel C at the front-left and the tower on the right wall" style="width:300px;height:292px;object-fit:cover;display:block">
<div style="display:flex;flex-direction:column;gap:12px;max-width:820px">
<h1 style="margin:0;font-size:40px;line-height:1.05;font-weight:700;letter-spacing:-0.01em">The room, as sketched</h1>
<p style="margin:0;font-size:16px;line-height:1.5;color:#a8a194">Looking into a room corner. The C stands floor to ceiling at the front-left, its spine along the left wall’s front edge and its arms reaching into the room, three voxels long. The tower stands on the back-right wall, past the ends of the arms. The walls and floor are the background: the fluid field, the C’s gravity gradient, and the quadtree floor, refining at the foot of the C and the base of the tower.</p>
</div>
</div>
<div style="display:grid;grid-template-columns:repeat({cols}, minmax(0, 1fr));gap:32px">
{tiles}
</div>
<div style="display:flex;flex-direction:column;gap:16px">
<div style="font-size:15px;font-weight:700">At size: 96, 48, 32, 16</div>
<div style="display:flex;flex-wrap:wrap;gap:48px">{sz}</div>
</div>
</div>
'''
s=HEAD.format(title='The room, as sketched')+body+TAIL.format(w=w,h=h)
s=re.sub(r'(-?\d+\.\d)\d+',r'\1',s)
open(P+'Sketch.dc.html','w').write(s)
cj=json.load(open('/home/claude/cav/canvas/project/canvas.json'))
cj['boards']['Sketch.dc.html']=dict(x=8516,y=0,w=w,h=h,title='The room, as sketched')
if 'Sketch.dc.html' not in cj['order']: cj['order'].append('Sketch.dc.html')
cj['notes']['t']['maxW']=8516+w
json.dump(cj,open('/home/claude/cav/canvas/project/canvas.json','w'),indent=1)
print(w,h,len(s))
