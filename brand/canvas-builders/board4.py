import sys, json, re; sys.path.insert(0,'.')
exec(open('board.py').read().split("iso=sorted(")[0])
R='/home/claude/cav/room/out3/'
CAP.update({
'o01-colour-dark':('Colour, dark','The C stands on the fluid face and the tower on the gradient face, with the quadtree on top. Principia’s brick, cyan and green.'),
'o02-colour-light':('Colour, light','On bone, with charcoal voxels.'),
'o03-mono-dark':('Mono, dark','Charcoal and bone.'),
'o04-mono-light':('Mono, light','The quietest version.'),
'o05-bw-dark':('Pure black and white, dark','Neutral greys only.'),
'o06-bw-light':('Pure black and white, light','Neutral greys only.'),
'o07-colour-dark-swapped':('Swapped','The C on the gradient face, so its own field rings out around it, and the tower on the fluid face.'),
'o08-colour-dark-no-edges':('No edge lines','Colour alone separates the faces.'),
'o09-colour-light-swapped':('Swapped, light','The red rings behind the C are the most legible version of “the C is the mass”.'),
'o10-colour-dark-arms2':('Arms of 2','For comparison.'),
})
PICKS.update({'o01-colour-dark','o02-colour-light','o07-colour-dark-swapped','o09-colour-light-swapped','o03-mono-dark'})
order=['o01-colour-dark','o02-colour-light','o07-colour-dark-swapped','o09-colour-light-swapped','o08-colour-dark-no-edges',
       'o03-mono-dark','o04-mono-light','o05-bw-dark','o06-bw-light','o10-colour-dark-arms2']
files=[R+k+'.svg' for k in order]
cols=5; w=64*2+cols*300+(cols-1)*32
tiles=''.join(tile(f,i) for i,f in enumerate(files))
sz=''
for j,k in enumerate(['o01-colour-dark','o09-colour-light-swapped','o03-mono-dark','o06-bw-light']):
    f=R+k+'.svg'
    sz+=f'<div style="display:flex;align-items:flex-end;gap:18px">{inl(f,96,"z96"+str(j))}{inl(f,48,"z48"+str(j))}{inl(f,32,"z32"+str(j))}{inl(f,16,"z16"+str(j))}</div>'
h=64+44+16+48+40+2*(300+12+100)+32+40+40+24+96+64
body=f'''<div style="width:{w}px;height:{h}px;box-sizing:border-box;padding:64px;background:#121110;color:#efe9dc;font-family:Inter,'Helvetica Neue',Helvetica,Arial,sans-serif;display:flex;flex-direction:column;gap:40px">
<div style="display:flex;flex-direction:column;gap:12px">
<h1 style="margin:0;font-size:40px;line-height:1.05;font-weight:700;letter-spacing:-0.01em">The cube: C and tower on the outer faces</h1>
<p style="margin:0;font-size:16px;line-height:1.5;color:#a8a194;max-width:900px">The fields from the room, wrapped onto the outside of the cube. The C (arms of 3) is centred on one face, opening towards the edge it shares with the tower’s face. The quadtree on the top refines where the C and the tower meet it.</p>
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
s=HEAD.format(title='The cube')+body+TAIL.format(w=w,h=h)
s=re.sub(r'(-?\d+\.\d)\d+',r'\1',s)
open(P+'Outer.dc.html','w').write(s)
cj=json.load(open('/home/claude/cav/canvas/project/canvas.json'))
cj['boards']['Outer.dc.html']=dict(x=6680,y=0,w=w,h=h,title='The cube: outer faces')
if 'Outer.dc.html' not in cj['order']: cj['order'].append('Outer.dc.html')
cj['notes']['t']['maxW']=6680+w
json.dump(cj,open('/home/claude/cav/canvas/project/canvas.json','w'),indent=1)
print(w,h,len(s))
