import sys, json, re; sys.path.insert(0,'.')
exec(open('board.py').read().split("iso=sorted(")[0])
R='/home/claude/cav/room/out/'
CAP.update({
'r01-colour-dark-arms3':('Colour, dark','The fluid wall is a rising plume in cyan. The gradient wall carries equipotential bands from the C in brick red. The floor is a quadtree in green, refining at the C’s foot and the tower’s base. Principia’s three washes.'),
'r03-colour-light-arms3':('Colour, light','The same room on bone, with the voxels in charcoal.'),
'r04-mono-dark-arms3':('Mono, dark','Charcoal and bone only: the bands and the plume become greys.'),
'r05-mono-light-arms3':('Mono, light','The quietest version.'),
'r06-bw-dark-arms3':('Pure black and white, dark','Neutral greys, no warmth.'),
'r07-bw-light-arms3':('Pure black and white, light','Neutral greys, no warmth.'),
'r08-colour-dark-swapped':('Walls swapped','The gradient sits behind the C and the fluid behind the tower.'),
'r09-colour-dark-no-edges':('No corner edges','The colour planes alone define the room.'),
'r02-colour-dark-arms2':('Arms of 2','Shorter still. Stubbier, but chunkier at small sizes.'),
'r10-colour-light-arms2':('Arms of 2, light','For comparison.'),
})
PICKS.update({'r01-colour-dark-arms3','r03-colour-light-arms3','r04-mono-dark-arms3','r07-bw-light-arms3'})
order=['r01-colour-dark-arms3','r03-colour-light-arms3','r04-mono-dark-arms3','r05-mono-light-arms3','r06-bw-dark-arms3',
       'r07-bw-light-arms3','r08-colour-dark-swapped','r09-colour-dark-no-edges','r02-colour-dark-arms2','r10-colour-light-arms2']
files=[R+k+'.svg' for k in order]
cols=5
w=64*2+cols*300+(cols-1)*32
tiles=''.join(tile(f,i) for i,f in enumerate(files))
# size strip
sz=''
for j,k in enumerate(['r01-colour-dark-arms3','r03-colour-light-arms3','r04-mono-dark-arms3','r07-bw-light-arms3']):
    f=R+k+'.svg'
    sz+=f'<div style="display:flex;align-items:flex-end;gap:18px">{inl(f,96,"z96"+str(j))}{inl(f,48,"z48"+str(j))}{inl(f,32,"z32"+str(j))}{inl(f,16,"z16"+str(j))}</div>'
h=64+44+16+72+40+2*(300+12+100)+32+40+40+24+96+64
body=f'''<div style="width:{w}px;height:{h}px;box-sizing:border-box;padding:64px;background:#121110;color:#efe9dc;font-family:Inter,'Helvetica Neue',Helvetica,Arial,sans-serif;display:flex;flex-direction:column;gap:40px">
<div style="display:flex;flex-direction:column;gap:12px">
<h1 style="margin:0;font-size:40px;line-height:1.05;font-weight:700;letter-spacing:-0.01em">The room: an inside corner</h1>
<p style="margin:0;font-size:16px;line-height:1.5;color:#a8a194;max-width:900px">Looking into the corner of a cube, not at the outside of one. The walls and floor are the background, and each carries one part of the engine: the gravity gradient (equipotential bands from the C), the fluid field (a posterised plume from the convection sim) and the voxel floor (a quadtree refining near the masses). The shorter C, with arms of 3, is centred on its wall, and the tower stands on the other.</p>
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
open(P+'Room.dc.html','w').write(HEAD.format(title='The room')+body+TAIL.format(w=w,h=h))
cj=json.load(open('/home/claude/cav/canvas/project/canvas.json'))
cj['boards']['Room.dc.html']=dict(x=4844,y=0,w=w,h=h,title='The room: inside corner')
if 'Room.dc.html' not in cj['order']: cj['order'].append('Room.dc.html')
cj['notes']['t']['maxW']=4844+w
json.dump(cj,open('/home/claude/cav/canvas/project/canvas.json','w'),indent=1)
print(w,h)
